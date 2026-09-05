use rusqlite::{params, Connection};

use super::{delete_retained_event, get_retained_event, RetainedEvent};

/// Outcome of an inbound retain — whether the local store now reflects the
/// inbound event, so the caller knows whether to patch `personas.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundOutcome {
    /// The inbound event was applied (no row, or it was strictly newer than a
    /// non-conflicting local row). The caller patches the local record store.
    Applied,
    /// The exact relay-confirmed head was already retained. The caller reapplies
    /// it to repair a missing or stale JSON projection without rewriting SQLite.
    Reapply,
    /// The inbound event was not applied because it is older or collides with a
    /// pending local edit at the same `created_at`.
    Skipped,
}

/// Decide whether an inbound event is newer than the retained coordinate without
/// mutating retention. Callers can update another durable store first, then
/// commit retention with [`retain_inbound_event`].
pub fn inbound_event_outcome(
    conn: &Connection,
    event: &RetainedEvent,
) -> Result<InboundOutcome, String> {
    let existing = get_retained_event(conn, event.kind, &event.pubkey, &event.d_tag)?;
    Ok(match existing {
        None => InboundOutcome::Applied,
        Some(row) if event.created_at > row.created_at => InboundOutcome::Applied,
        Some(row)
            if event.created_at == row.created_at
                && equal_second_inbound_wins(&event.raw_event, &row.raw_event) =>
        {
            InboundOutcome::Applied
        }
        Some(row)
            if event.created_at == row.created_at
                && !row.pending_sync
                && event.raw_event == row.raw_event =>
        {
            InboundOutcome::Reapply
        }
        // Older or an equal-second loser: keep the retained head intact.
        Some(_) => InboundOutcome::Skipped,
    })
}

// NIP-01: at the same timestamp, the lower event id wins. Undecidable ties
// must preserve a pending local write.
fn equal_second_inbound_wins(inbound_raw: &str, retained_raw: &str) -> bool {
    match (raw_event_id(inbound_raw), raw_event_id(retained_raw)) {
        (Some(inbound_id), Some(retained_id)) => inbound_id < retained_id,
        _ => false,
    }
}

fn raw_event_id(raw_event: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(raw_event)
        .ok()?
        .get("id")?
        .as_str()
        .map(str::to_owned)
}

/// Apply the local JSON projection before advancing its durable retention head.
/// A failed projection leaves the event eligible for retry on the next replay.
pub fn commit_inbound_with_store<F>(
    conn: &Connection,
    event: &RetainedEvent,
    apply_store: F,
) -> Result<InboundOutcome, String>
where
    F: FnOnce() -> Result<(), String>,
{
    if inbound_event_outcome(conn, event)? == InboundOutcome::Skipped {
        return Ok(InboundOutcome::Skipped);
    }
    apply_store()?;
    retain_inbound_event(conn, event)
}

/// Remove an inbound tombstone's JSON projection before atomically retaining
/// the tombstone and purging its covered head. A newer recreation survives.
pub fn commit_inbound_tombstone_with_store<F>(
    conn: &Connection,
    tombstone: &RetainedEvent,
    target_kind: u32,
    target_owner: &str,
    target_d_tag: &str,
    remove_json: F,
) -> Result<InboundOutcome, String>
where
    F: FnOnce() -> Result<(), String>,
{
    let covered_head = get_retained_event(conn, target_kind, target_owner, target_d_tag)?;
    if covered_head
        .as_ref()
        .is_some_and(|head| head.created_at > tombstone.created_at)
        || inbound_event_outcome(conn, tombstone)? == InboundOutcome::Skipped
    {
        return Ok(InboundOutcome::Skipped);
    }
    remove_json()?;
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| format!("failed to begin inbound tombstone transaction: {e}"))?;
    let result = (|| -> Result<(), String> {
        retain_inbound_event(conn, tombstone)?;
        delete_retained_event(conn, target_kind, target_owner, target_d_tag)
    })();
    match result {
        Ok(()) => conn
            .execute_batch("COMMIT")
            .map_err(|e| format!("failed to commit inbound tombstone transaction: {e}"))?,
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    }
    Ok(InboundOutcome::Applied)
}

/// Retain an event arriving from the relay, resolving it against any local row.
///
/// Relay events clear `pending_sync` only when no row exists, the inbound event
/// is strictly newer, or it exactly reapplies an already-confirmed head.
pub fn retain_inbound_event(
    conn: &Connection,
    event: &RetainedEvent,
) -> Result<InboundOutcome, String> {
    let outcome = inbound_event_outcome(conn, event)?;

    if outcome == InboundOutcome::Skipped {
        return Ok(InboundOutcome::Skipped);
    }

    conn.execute(
        "INSERT INTO persona_events (kind, pubkey, d_tag, content, created_at, raw_event, pending_sync)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)
         ON CONFLICT (kind, pubkey, d_tag) DO UPDATE SET
            content = excluded.content,
            created_at = excluded.created_at,
            raw_event = excluded.raw_event,
            pending_sync = 0",
        params![
            event.kind,
            event.pubkey,
            event.d_tag,
            event.content,
            event.created_at,
            event.raw_event,
        ],
    )
    .map_err(|e| format!("failed to retain inbound event: {e}"))?;

    Ok(outcome)
}
