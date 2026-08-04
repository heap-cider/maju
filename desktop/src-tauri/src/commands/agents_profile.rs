//! Agent kind:0 profile reconciliation — split from `agents.rs` (file-size
//! guard). Owns the reconcile data carrier, the legacy-avatar backfill, and
//! the needs-sync predicate.

use tauri::AppHandle;

use crate::app_state::AppState;
use crate::managed_agents::managed_agent_avatar_url;

use super::*;

pub(crate) struct ProfileReconcileData {
    pub(crate) private_key_nsec: String,
    pub(crate) name: String,
    pub(crate) relay_url: String,
    /// Expected avatar URL for the published profile. `None` for legacy records
    /// that predate the `avatar_url` field — these will be backfilled from the
    /// relay's existing kind:0 profile on first reconciliation.
    pub(crate) avatar_url: Option<String>,
    pub(crate) auth_tag: Option<String>,
    /// The agent's pubkey (hex). Needed to update the persisted record during
    /// avatar backfill migration.
    pub(crate) pubkey: String,
    /// The agent's command (e.g. "goose"). Used as fallback when no profile
    /// exists on the relay during avatar backfill.
    pub(crate) agent_command: String,
    /// Persona ID if this agent was created from a persona. Used during avatar
    /// backfill to recover the correct avatar from the persona record when the
    /// relay profile has been corrupted.
    pub(crate) persona_id: Option<String>,
}

/// Resolve the avatar to backfill for a legacy agent record (pre-PR-921, no
/// stored `avatar_url`).
///
/// Priority: the persona's avatar wins, because the old reconciliation code
/// could have overwritten the relay's kind:0 `picture` with the command default
/// — making the relay an unreliable source for persona-backed agents. Only fall
/// back to the relay's `picture`, then the command icon, for agents with no
/// persona avatar to recover from.
pub(super) fn resolve_legacy_avatar(
    persona_avatar: Option<String>,
    relay_picture: Option<String>,
    agent_command: &str,
) -> String {
    persona_avatar
        .or(relay_picture)
        .or_else(|| managed_agent_avatar_url(agent_command))
        .unwrap_or_default()
}

/// Reconcile an agent's kind:0 profile on the relay.
///
/// Queries the relay for the agent's existing profile and re-publishes if missing
/// or stale (display_name or picture mismatch). This is fire-and-forget — errors
/// are returned to the caller for logging but never block agent startup.
///
/// For legacy records (pre-PR-921) where `avatar_url` is `None`, this function
/// backfills via `resolve_legacy_avatar` — preferring the persona record's avatar
/// over the relay's `picture`, since the old code may have corrupted the relay
/// profile — and persists the updated record. After backfill, normal
/// reconciliation proceeds.
///
/// Query and publish target the relay returned by `effective_agent_relay_url`
/// for every agent regardless of backend: an explicit per-agent `relay_url`
/// wins, and a blank one falls back to the active workspace relay. This keeps
/// reconciliation following the session's relay for never-pinned agents while
/// honoring a deliberate pin wherever it points.
pub(crate) async fn reconcile_agent_profile(
    state: &AppState,
    app: &AppHandle,
    agent_pubkey: &str,
    data: &ProfileReconcileData,
) -> Result<(), String> {
    use crate::relay::{query_agent_profile, sync_managed_agent_profile};

    // An explicit per-agent relay wins; an empty one falls back to the active
    // workspace relay. Resolved once and used for both the read and write-back.
    let relay_url = crate::relay::effective_agent_relay_url(
        &data.relay_url,
        &relay_ws_url_with_override(state),
    );

    if !state
        .managed_agent_profile_reconcile_enabled
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Ok(());
    }

    // Query the relay for the agent's existing kind:0 profile.
    let existing = query_agent_profile(state, &relay_url, agent_pubkey).await?;

    // The current definition wins over the per-device runtime snapshot. This
    // prevents another PC with an old command-default icon from publishing it
    // over a newer custom avatar. Unlinked legacy records still recover from
    // their relay profile before falling back to the command icon.
    let personas = load_personas(app).unwrap_or_default();
    let stored_or_legacy_avatar = data.avatar_url.clone().or_else(|| {
        let resolved = resolve_legacy_avatar(
            None,
            existing.as_ref().and_then(|info| info.picture.clone()),
            &data.agent_command,
        );
        (!resolved.is_empty()).then_some(resolved)
    });
    let expected_avatar = crate::managed_agents::resolve_current_agent_avatar_url(
        data.persona_id.as_deref(),
        stored_or_legacy_avatar,
        &data.agent_command,
        &personas,
    );

    // Repair the local snapshot too. Future starts then agree even if the
    // definition is temporarily unavailable during an early sync window.
    let stored_avatar = data
        .avatar_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty());
    if stored_avatar != expected_avatar.as_deref() {
        let _store_guard = state
            .managed_agents_store_lock
            .lock()
            .map_err(|e| e.to_string())?;
        let mut records = load_managed_agents(app)?;
        if let Some(record) = records.iter_mut().find(|r| r.pubkey == data.pubkey) {
            record.avatar_url = expected_avatar.clone();
            save_managed_agents(app, &records)?;
        }
    }

    if !profile_needs_sync(existing.as_ref(), &data.name, expected_avatar.as_deref()) {
        return Ok(());
    }

    let agent_keys = Keys::parse(&data.private_key_nsec)
        .map_err(|e| format!("failed to parse agent keys: {e}"))?;

    if !state
        .managed_agent_profile_reconcile_enabled
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Ok(());
    }

    sync_managed_agent_profile(
        state,
        &relay_url,
        &agent_keys,
        &data.name,
        expected_avatar.as_deref(),
        data.auth_tag.as_deref(),
    )
    .await
}

/// Decide whether a published profile is missing or stale relative to the
/// expected name and avatar. A missing profile always needs sync; a present
/// one is stale when either the display name or picture diverges.
pub(super) fn profile_needs_sync(
    existing: Option<&crate::relay::AgentProfileInfo>,
    expected_name: &str,
    expected_avatar: Option<&str>,
) -> bool {
    match existing {
        None => true,
        Some(info) => {
            let name_matches = info.display_name.as_deref() == Some(expected_name);
            let picture_matches = info.picture.as_deref() == expected_avatar;
            !name_matches || !picture_matches
        }
    }
}

// Async so the blocking body (disk reads/writes + process termination) runs off
// the main UI thread via spawn_blocking. State is re-derived from the owned
// AppHandle inside the closure (`State<'_, _>` is borrowed, MutexGuard is !Send).
