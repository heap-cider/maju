use std::collections::HashSet;

use maju_core_pkg::kind::{KIND_MANAGED_AGENT, KIND_PERSONA, KIND_TEAM};
use rusqlite::{params, Connection};

use super::RetainedEvent;

/// Preserve, then remove, pending agent-domain rows whose disk projection is
/// gone during the v1→v2 scope repair.
///
/// Retention is a publication queue, not the local definition authority. A
/// pending row with no corresponding JSON record can otherwise suppress an
/// older relay-confirmed head forever while also being impossible to review or
/// edit in the UI. Quarantine keeps the exact signed bytes for recovery and
/// removes only the active coordinate so inbound history can heal projection.
pub fn quarantine_unprojected_pending_agent_events(
    conn: &mut Connection,
    owner_pubkey: &str,
    persona_d_tags: &HashSet<String>,
    team_d_tags: &HashSet<String>,
    agent_d_tags: &HashSet<String>,
) -> Result<u32, String> {
    let rows = {
        let mut stmt = conn
            .prepare(
                "SELECT kind, pubkey, d_tag, content, created_at, raw_event, pending_sync
                 FROM persona_events
                 WHERE pubkey = ?1 AND pending_sync = 1 AND kind IN (?2, ?3, ?4)",
            )
            .map_err(|e| format!("failed to prepare pending projection audit: {e}"))?;
        let rows = stmt
            .query_map(
                params![owner_pubkey, KIND_PERSONA, KIND_TEAM, KIND_MANAGED_AGENT],
                |row| {
                    Ok(RetainedEvent {
                        kind: row.get(0)?,
                        pubkey: row.get(1)?,
                        d_tag: row.get(2)?,
                        content: row.get(3)?,
                        created_at: row.get(4)?,
                        raw_event: row.get(5)?,
                        pending_sync: row.get::<_, i32>(6)? != 0,
                    })
                },
            )
            .map_err(|e| format!("failed to query pending projection audit: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("failed to read pending projection audit: {e}"))?
    };
    let orphaned: Vec<_> = rows
        .into_iter()
        .filter(|row| match row.kind {
            KIND_PERSONA => !persona_d_tags.contains(&row.d_tag),
            KIND_TEAM => !team_d_tags.contains(&row.d_tag),
            KIND_MANAGED_AGENT => !agent_d_tags.contains(&row.d_tag),
            _ => false,
        })
        .collect();
    if orphaned.is_empty() {
        return Ok(0);
    }

    let transaction = conn
        .transaction()
        .map_err(|e| format!("failed to begin pending projection quarantine: {e}"))?;
    for row in &orphaned {
        transaction
            .execute(
                "INSERT OR IGNORE INTO quarantined_persona_events
                 (kind, pubkey, d_tag, content, created_at, raw_event, pending_sync, reason)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    row.kind,
                    row.pubkey,
                    row.d_tag,
                    row.content,
                    row.created_at,
                    row.raw_event,
                    row.pending_sync as i32,
                    "v2 repair: pending row missing from scoped JSON projection",
                ],
            )
            .map_err(|e| format!("failed to preserve quarantined pending row: {e}"))?;
        transaction
            .execute(
                "DELETE FROM persona_events
                 WHERE kind = ?1 AND pubkey = ?2 AND d_tag = ?3
                   AND created_at = ?4 AND raw_event = ?5 AND pending_sync = 1",
                params![
                    row.kind,
                    row.pubkey,
                    row.d_tag,
                    row.created_at,
                    row.raw_event,
                ],
            )
            .map_err(|e| format!("failed to detach quarantined pending row: {e}"))?;
    }
    transaction
        .commit()
        .map_err(|e| format!("failed to commit pending projection quarantine: {e}"))?;
    u32::try_from(orphaned.len()).map_err(|_| "too many quarantined rows".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed_agents::retention::{get_retained_event, open_retention_db, retain_event};

    fn sample_event() -> RetainedEvent {
        RetainedEvent {
            kind: KIND_PERSONA,
            pubkey: "abc123".to_string(),
            d_tag: "test-persona".to_string(),
            content: r#"{"display_name":"Test"}"#.to_string(),
            created_at: 1000,
            raw_event: r#"{"id":"projected"}"#.to_string(),
            pending_sync: true,
        }
    }

    #[test]
    fn preserves_then_detaches_unprojected_pending_rows() {
        let mut conn = open_retention_db(std::path::Path::new(":memory:")).unwrap();
        let projected = sample_event();
        retain_event(&conn, &projected).unwrap();
        let orphan = RetainedEvent {
            kind: KIND_MANAGED_AGENT,
            d_tag: "orphan-agent".to_string(),
            raw_event: r#"{"id":"orphan"}"#.to_string(),
            ..sample_event()
        };
        retain_event(&conn, &orphan).unwrap();

        let quarantined = quarantine_unprojected_pending_agent_events(
            &mut conn,
            &projected.pubkey,
            &HashSet::from([projected.d_tag.clone()]),
            &HashSet::new(),
            &HashSet::new(),
        )
        .unwrap();
        assert_eq!(quarantined, 1);
        assert!(
            get_retained_event(&conn, KIND_MANAGED_AGENT, &orphan.pubkey, &orphan.d_tag)
                .unwrap()
                .is_none()
        );
        assert!(
            get_retained_event(&conn, KIND_PERSONA, &projected.pubkey, &projected.d_tag)
                .unwrap()
                .is_some()
        );
        let preserved: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM quarantined_persona_events WHERE raw_event = ?1",
                params![orphan.raw_event],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(preserved, 1);
    }
}
