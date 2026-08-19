use rusqlite::{params, Connection};

use super::{get_retained_event, RetainedEvent};

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
                && !row.pending_sync
                && event.raw_event == row.raw_event =>
        {
            InboundOutcome::Reapply
        }
        // Equal-but-different or older: keep any pending local edit intact.
        Some(_) => InboundOutcome::Skipped,
    })
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
