//! Retention-queue helpers for managed-agent lifecycle events: pending
//! upserts, NIP-09 tombstones, and NIP-IA archive requests. Split from
//! `agents.rs` along the retention seam; every function runs inside the
//! `managed_agents_store_lock`-held body and never across an `.await`.

use tauri::AppHandle;

use crate::{app_state::AppState, managed_agents::ManagedAgentRecord};

/// Retain a freshly authored managed-agent event in the local store, flagged
/// for relay sync. This must be called inside the `managed_agents_store_lock`-
/// held body after `save_managed_agents` and never across an `.await`.
///
/// The workspace owner signs the event, and the agent pubkey is its `d_tag`.
/// Best-effort failures are logged so retention cannot block the authoritative
/// disk write.
pub(crate) fn retain_managed_agent_pending(
    app: &AppHandle,
    state: &AppState,
    record: &ManagedAgentRecord,
) {
    use crate::managed_agents::{reconcile::retain_agent_record, retention::open_retention_db};

    let result = (|| -> Result<(), String> {
        let scope = crate::managed_agents::retention::active_retention_scope(app, state)?;
        let conn = open_retention_db(&scope.db_path)?;
        retain_agent_record(&conn, &scope.owner_keys, record).map(|_| ())
    })();
    if let Err(e) = result {
        eprintln!("maju-desktop: agent-retain: {e}");
    }
}

/// Purge the pending agent head, then enqueue its NIP-09 tombstone while the
/// store lock is held. The result lets one-time workspace repair retry when
/// retention fails.
pub(crate) fn tombstone_managed_agent_pending(
    app: &AppHandle,
    state: &AppState,
    agent_pubkey: &str,
) -> bool {
    use crate::managed_agents::{
        agent_events::build_agent_delete,
        retention::{
            delete_retained_event, open_retention_db, retain_event, tombstone_retention_d_tag,
            RetainedEvent,
        },
    };
    use maju_core_pkg::kind::KIND_MANAGED_AGENT;
    use nostr::JsonUtil;

    const KIND_DELETE: u32 = 5;

    let result = (|| -> Result<(), String> {
        let scope = crate::managed_agents::retention::active_retention_scope(app, state)?;
        let owner_pubkey = scope.owner_keys.public_key().to_hex();
        let event = build_agent_delete(agent_pubkey, &owner_pubkey)?
            .sign_with_keys(&scope.owner_keys)
            .map_err(|e| format!("failed to sign managed-agent tombstone: {e}"))?;
        let conn = open_retention_db(&scope.db_path)?;
        delete_retained_event(&conn, KIND_MANAGED_AGENT, &owner_pubkey, agent_pubkey)?;
        retain_event(
            &conn,
            &RetainedEvent {
                kind: KIND_DELETE,
                pubkey: owner_pubkey,
                // Key by the target coordinate so cross-kind d-tag tombstones
                // occupy distinct rows (F2c).
                d_tag: tombstone_retention_d_tag(KIND_MANAGED_AGENT, agent_pubkey),
                content: event.content.to_string(),
                created_at: event.created_at.as_secs() as i64,
                raw_event: event.as_json(),
                pending_sync: true,
            },
        )
    })();
    match result {
        Ok(()) => true,
        Err(e) => {
            eprintln!("maju-desktop: agent-tombstone: {e}");
            false
        }
    }
}

/// Build a deleted agent's owner-authenticated NIP-IA archive request while
/// retaining its persona id.
pub(crate) fn build_agent_archive_request(
    keys: &nostr::Keys,
    agent_pubkey: &str,
    persona_id: Option<&str>,
) -> Result<nostr::Event, String> {
    let auth_tag = if keys
        .public_key()
        .to_hex()
        .eq_ignore_ascii_case(agent_pubkey)
    {
        None
    } else {
        let agent = nostr::PublicKey::from_hex(agent_pubkey)
            .map_err(|e| format!("invalid agent pubkey: {e}"))?;
        let tag_json = maju_sdk_pkg::nip_oa::compute_auth_tag(keys, &agent, "")
            .map_err(|e| format!("failed to build owner auth tag: {e}"))?;
        let parts: Vec<String> = serde_json::from_str(&tag_json)
            .map_err(|e| format!("failed to parse owner auth tag: {e}"))?;
        Some(
            <[String; 4]>::try_from(parts)
                .map_err(|_| "owner auth tag must have four elements".to_string())?,
        )
    };
    let content = persona_id
        .filter(|id| !id.trim().is_empty())
        .map(|id| serde_json::json!({ "persona_id": id }).to_string())
        .unwrap_or_default();
    crate::events::build_archive_identity_request(
        agent_pubkey,
        &content,
        Some("retired"),
        None,
        auth_tag.as_ref(),
    )?
    .sign_with_keys(keys)
    .map_err(|e| format!("failed to sign archive request: {e}"))
}

/// Enqueue a deleted agent's NIP-IA archive request next to its tombstone.
/// The result lets one-time workspace repair retry when retention fails.
pub(crate) fn archive_managed_agent_pending(
    app: &AppHandle,
    state: &AppState,
    agent_pubkey: &str,
    persona_id: Option<&str>,
) -> bool {
    use crate::managed_agents::retention::{open_retention_db, retain_event, RetainedEvent};
    use maju_core_pkg::kind::KIND_IA_ARCHIVE_REQUEST;
    use nostr::JsonUtil;

    let result = (|| -> Result<(), String> {
        let scope = crate::managed_agents::retention::active_retention_scope(app, state)?;
        let owner_pubkey = scope.owner_keys.public_key().to_hex();
        let event = build_agent_archive_request(&scope.owner_keys, agent_pubkey, persona_id)?;
        let conn = open_retention_db(&scope.db_path)?;
        retain_event(
            &conn,
            &RetainedEvent {
                kind: KIND_IA_ARCHIVE_REQUEST,
                pubkey: owner_pubkey,
                d_tag: agent_pubkey.to_string(),
                content: event.content.to_string(),
                created_at: event.created_at.as_secs() as i64,
                raw_event: event.as_json(),
                pending_sync: true,
            },
        )
    })();
    match result {
        Ok(()) => true,
        Err(e) => {
            eprintln!("maju-desktop: agent-archive: {e}");
            false
        }
    }
}
