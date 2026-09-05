use nostr::Keys;
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::app_state::AppState;
use crate::managed_agents::{
    effective_repos_dir, ensure_repos_symlink, nest_dir, restore_managed_agents_on_launch,
    try_regenerate_nest, write_persisted_repos_dir,
};
use crate::relay;

const WORKSPACE_APPLY_SUPERSEDED: &str = "workspace apply superseded by a newer request";

fn next_apply_generation(generation: &std::sync::atomic::AtomicU64) -> u64 {
    generation.fetch_add(1, Ordering::AcqRel).wrapping_add(1)
}

fn assert_current_apply_generation(
    generation: &std::sync::atomic::AtomicU64,
    ticket: u64,
) -> Result<(), String> {
    if generation.load(Ordering::Acquire) == ticket {
        Ok(())
    } else {
        Err(WORKSPACE_APPLY_SUPERSEDED.to_string())
    }
}

async fn begin_workspace_apply(
    lock: std::sync::Arc<tokio::sync::Mutex<()>>,
    generation: &std::sync::atomic::AtomicU64,
) -> Result<(tokio::sync::OwnedMutexGuard<()>, u64), String> {
    // Claim the request's place in latest-wins order before waiting for the
    // transaction lock. A request that was selected later must supersede an
    // older request even when the older one reaches the lock last.
    let ticket = next_apply_generation(generation);
    let guard = lock.lock_owned().await;
    assert_current_apply_generation(generation, ticket)?;
    Ok((guard, ticket))
}

/// Adopt the pre-scoping global retention database's pending rows into `scope`.
///
/// A retention error stops workspace activation. Continuing would force the
/// JSON migration to either guess a community or permanently initialize an
/// empty scope, and neither is safe.
fn migrate_legacy_retention_into(
    app: &AppHandle,
    scope: &crate::managed_agents::retention::RetentionScope,
) -> Result<(), String> {
    let base_dir = crate::managed_agents::managed_agents_base_dir(app)?;
    match crate::managed_agents::retention::migrate_legacy_retention_db(
        &base_dir,
        &scope.db_path,
        &scope.owner_keys.public_key().to_hex(),
    ) {
        Ok(0) => Ok(()),
        Ok(copied) => {
            eprintln!(
                "maju-desktop: adopted {copied} legacy retained event(s) into this community"
            );
            Ok(())
        }
        Err(error) => Err(format!("legacy retention migration failed: {error}")),
    }
}

#[derive(Deserialize)]
struct RelayInfoIcon {
    #[serde(default)]
    icon: Option<String>,
}

/// Fetch a relay's workspace icon from its NIP-11 relay information document.
///
/// Works for any workspace (active or not) with a plain unauthenticated HTTP
/// GET — no WebSocket session needed. Returns `None` when the relay has no
/// icon set, is unreachable, or serves a malformed document: the rail falls
/// back to initials in all three cases.
#[tauri::command]
pub async fn fetch_workspace_icon(
    relay_url: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let http_url = relay::relay_http_base_url(&relay_url);
    let Ok(response) = state
        .http_client
        .get(&http_url)
        .header("Accept", "application/nostr+json")
        .send()
        .await
    else {
        return Ok(None);
    };
    if !response.status().is_success() {
        return Ok(None);
    }
    let doc = response
        .json::<RelayInfoIcon>()
        .await
        .unwrap_or(RelayInfoIcon { icon: None });
    Ok(doc.icon.filter(|icon| !icon.is_empty()))
}

#[derive(Serialize)]
pub struct ActiveWorkspaceInfo {
    relay_url: String,
    pubkey: String,
}

/// Exact workspace scope installed by [`apply_workspace`].
///
/// The frontend verifies this receipt before mounting community-scoped UI, so
/// a stale command result can never make a different relay or signer appear
/// ready.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedWorkspaceInfo {
    scope_id: String,
    relay_url: String,
    owner_pubkey: String,
    generation: u64,
}

/// Returns the current active workspace info (relay URL + pubkey).
#[tauri::command]
pub fn get_active_workspace(state: State<'_, AppState>) -> Result<ActiveWorkspaceInfo, String> {
    let keys = state.keys.lock().map_err(|e| e.to_string())?;
    let relay_url = relay::relay_ws_url_with_override(&state);
    Ok(ActiveWorkspaceInfo {
        relay_url,
        pubkey: keys.public_key().to_hex(),
    })
}

/// Validate a candidate `repos_dir` without mutating the filesystem.
///
/// The Add/Edit workspace dialogs call this on submit to block Save on a bad
/// path, so a typo never reaches `apply_workspace`. Reuses the same
/// `validate_repos_dir` the boot/apply path uses — one source of truth for
/// "what's a valid repos dir". An empty/whitespace value clears the override
/// and is valid. `Err` carries the human-readable reason for inline display.
#[tauri::command]
pub async fn validate_repos_dir(dir: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let trimmed = dir.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        let nest = nest_dir().ok_or("cannot resolve home directory for nest")?;
        crate::managed_agents::validate_repos_dir(&nest, trimmed).map(|_| ())
    })
    .await
    .map_err(|e| format!("spawn_blocking failed: {e}"))?
}

/// Apply a workspace's configuration to the backend session.
///
/// Called by the frontend on app init (after reload) to configure the
/// Tauri backend with the selected workspace's relay URL, keys, and repos
/// directory.
///
/// A bad `repos_dir` is non-fatal: relay/keys always apply (the relay is the
/// active workspace's own choice — orthogonal to the filesystem repos dir),
/// the bad value is NOT persisted (so the next boot starts clean), the
/// `REPOS` symlink is skipped (REPOS stays a real dir), a `repos-dir-error`
/// event surfaces the reason, and the command returns `Ok`. The dialogs
/// already block a bad path at Save (`validate_repos_dir`); this fallback only
/// catches a value that went bad after save (deleted dir, unmounted volume).
#[tauri::command]
pub async fn apply_workspace(
    relay_url: String,
    nsec: Option<String>,
    repos_dir: Option<String>,
    agent_managed_profiles: Option<bool>,
    thread_scoped_acp_sessions: Option<bool>,
    app: AppHandle,
) -> Result<AppliedWorkspaceInfo, String> {
    let state = app.state::<AppState>();
    // The request claims its generation before waiting for the serialized
    // transaction. `begin_workspace_apply` rejects it immediately after lock
    // acquisition when a newer selection arrived while it was queued.
    let (apply_guard, apply_generation) = begin_workspace_apply(
        state.workspace_apply_lock.clone(),
        &state.workspace_apply_generation,
    )
    .await?;

    let restore_app = app.clone();
    let apply_app = app.clone();
    // Capture the caller's relay before the blocking apply. Reading shared
    // state afterward could pick up a newer concurrent community switch.
    let profile_reconcile_relay = relay_url.clone();
    let (applied_workspace, event_sync_bootstrap) = tokio::task::spawn_blocking(move || {
        let app = apply_app;
        let state = app.state::<AppState>();

        // ── Validate before mutating ──────────────────────────────────────────
        let parsed_keys = match nsec.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(nsec_trimmed) => {
                Some(Keys::parse(nsec_trimmed).map_err(|e| format!("invalid nsec: {e}"))?)
            }
            None => None,
        };

        // Decide the effective repos_dir from the candidate. A bad path does NOT
        // reject — it is treated as if no override were set: relay/keys still
        // apply, the bad value is not persisted, and a `repos-dir-error` surfaces
        // the reason. Persisting a bad path would make every later boot read it,
        // fail to resolve the symlink, and silently skip agent restore. One
        // validate (inside `effective_repos_dir`) drives both the emit and the
        // persisted value. `nest` is resolved softly: when absent there is nothing
        // to persist or symlink, and relay/keys must still apply unconditionally.
        let nest = nest_dir();
        let effective_repos_dir = match nest.as_deref() {
            Some(nest) => match effective_repos_dir(nest, repos_dir.as_deref()) {
                Ok(value) => value,
                Err(error) => {
                    let _ = app.emit("repos-dir-error", error);
                    None
                }
            },
            None => None,
        };

        // This transaction must still own the serialized apply generation
        // before it changes the active workspace.
        assert_current_apply_generation(&state.workspace_apply_generation, apply_generation)?;
        // ── Apply workspace state and activate its Agent/Team store ───────────
        // Keep the relay+owner switch and scoped Agent/Team store activation
        // atomic with respect to every agent command.
        let store_guard = state
            .managed_agents_store_lock
            .lock()
            .map_err(|error| error.to_string())?;

        // Prepare the candidate scope before changing the active relay or
        // account. A migration failure therefore leaves the old workspace
        // fully active instead of producing a half-switched backend.
        let owner_keys = match parsed_keys.as_ref() {
            Some(keys) => keys.clone(),
            None => state.signing_keys()?,
        };
        let scope =
            crate::managed_agents::retention::retention_scope(&app, &relay_url, owner_keys)?;
        let scope_id = scope
            .db_path
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "retention scope path has no usable scope id".to_string())?
            .to_string();
        let applied_workspace = AppliedWorkspaceInfo {
            scope_id,
            relay_url: scope.relay_url.clone(),
            owner_pubkey: scope.owner_keys.public_key().to_hex(),
            generation: apply_generation,
        };
        migrate_legacy_retention_into(&app, &scope)?;
        let scope_initialization: crate::managed_agents::AgentStoreScopeInitialization =
            crate::managed_agents::initialize_agent_store_scope(&app, &scope)?;
        let scope_repair = scope_initialization.repair;

        // Acquire every fallible state lock before writing either value.
        let mut override_guard = state.relay_url_override.lock().map_err(|e| e.to_string())?;
        let mut keys_guard = match parsed_keys.as_ref() {
            Some(_) => Some(state.keys.lock().map_err(|e| e.to_string())?),
            None => None,
        };
        *override_guard = Some(relay_url);
        if let (Some(keys), Some(keys_guard)) = (parsed_keys, keys_guard.as_mut()) {
            **keys_guard = keys;
        }
        // ── Apply all state changes (nothing below can fail) ──────────────────
        // ── Apply all state changes (nothing below can fail) ──────────────────
        drop(keys_guard);
        drop(override_guard);

        // Reset the Rust-side admission gate when switching workspace/community,
        // matching `resetRateLimitGate()` on the TS side (useCommunityInit.ts:38).
        crate::relay_admission::reset_gate_for_workspace_change();

        // Keep the backend-side reconcile guard aligned with the frontend
        // experiment before launch-time restore can spawn any agents. Missing
        // means the stable behavior: desktop remains authoritative.
        state
            .managed_agent_profile_reconcile_enabled()
            .store(!agent_managed_profiles.unwrap_or(false), Ordering::Release);
        // Persisted frontend experiment state must land before launch-time
        // restore so every restored agent starts with the selected ACP policy.
        // Missing preserves the stable channel-scoped behavior.
        state.thread_scoped_acp_sessions_enabled().store(
            thread_scoped_acp_sessions.unwrap_or(false),
            Ordering::Release,
        );

        // Retire only records the scoped migration proved were either orphaned
        // in this community or later copies of the same definition identity.
        // The old flat JSON remains untouched as a recovery copy.
        let mut repair_queued = true;
        for pubkey in &scope_repair.agent_pubkeys {
            repair_queued &= super::agents::tombstone_managed_agent_pending(&app, &state, pubkey);
        }
        for team_id in &scope_repair.team_ids {
            repair_queued &= super::teams::tombstone_team_pending(&app, &state, team_id);
        }
        if repair_queued {
            crate::managed_agents::complete_active_agent_store_scope_repair(&app)?;
        }
        drop(store_guard);

        // This helper owns the same store lock, so it must run after the scope
        // switch guard above is released.
        if let Err(error) = crate::managed_agents::backfill_persona_snapshots(&app) {
            eprintln!("maju-desktop: persona-snapshot backfill failed: {error}");
        }
        try_regenerate_nest(&app);

        // ── Filesystem side-effect (non-fatal) ────────────────────────────────
        // Persist the *effective* repos_dir (None when the candidate failed
        // validation) for the backend to read at boot, then re-point REPOS to
        // match. Persisting first makes the dotfile authoritative even if the
        // symlink apply fails here (e.g. a non-empty real REPOS): the next boot
        // reads the persisted value and resolves the symlink before any agent can
        // clone into REPOS. A bad candidate persists `None`, so the next boot is
        // clean and agent restore proceeds. Failure of either must NOT fail the
        // command — relay/keys are already applied. Surface symlink errors via
        // `repos-dir-error`.
        if let Some(nest) = nest.as_deref() {
            if let Err(error) = write_persisted_repos_dir(nest, effective_repos_dir.as_deref()) {
                eprintln!("maju-desktop: persist repos dir failed: {error}");
            }
            if let Err(error) = ensure_repos_symlink(nest, effective_repos_dir.as_deref()) {
                eprintln!("maju-desktop: repos dir setup failed: {error}");
                let _ = app.emit("repos-dir-error", error);
            }
        }

        Ok::<
            (
                AppliedWorkspaceInfo,
                crate::managed_agents::EventSyncBootstrap,
            ),
            String,
        >((applied_workspace, scope_initialization.event_sync_bootstrap))
    })
    .await
    .map_err(|e| format!("spawn_blocking failed: {e}"))??;

    assert_current_apply_generation(&state.workspace_apply_generation, apply_generation)?;

    let state = restore_app.state::<AppState>();
    super::agents::provider_access::reconcile_on_workspace_apply(&restore_app, &state).await?;
    assert_current_apply_generation(&state.workspace_apply_generation, apply_generation)?;
    // The Bumble→Pollen migration may have renamed stopped agents. Reconcile
    // their relay profiles independently of runtime restore; successful writes
    // record this relay while retaining the agent for other communities, and
    // failures retry on the next workspace apply.
    crate::managed_agents::spawn_pending_profile_reconciliations(
        &restore_app,
        &profile_reconcile_relay,
    );

    // Backfill this exact relay+owner scope only after the workspace has been
    // applied. Running at process boot would target the fallback relay and
    // collapse every community into one pending-event store.
    match crate::managed_agents::retention::active_retention_scope(&restore_app, &state) {
        Ok(scope) => {
            // Adopt whatever the pre-scoping release left queued in the global
            // retention database BEFORE the scoped reconcile and flush run, so
            // stranded tombstones and archive requests publish on this boot
            // instead of being abandoned by the storage cutover. Best-effort:
            // it is not a prerequisite for the superseding head — the team leg
            // below builds the repaired roster's head fresh from disk with a
            // monotonic `created_at` regardless of what the legacy copy left.
            migrate_legacy_retention_into(&restore_app, &scope)?;
            // Await the reconcile to completion — do NOT spawn it — and
            // propagate its failure. The boot migration may have repaired team
            // membership on disk; the frontend starts inbound history replay
            // the moment `useCommunityInit` observes the applied workspace, and
            // an old relay team head could otherwise win that race and overwrite
            // the repaired `persona_ids`. The team leg is fatal (see
            // `run_event_sync`): only its success durably retains the corrected
            // head with a superseding `monotonic_created_at`, so
            // `retain_inbound_event`'s equal/older guard rejects the stale head.
            // On failure we return `Err` — the command reports failure,
            // `useCommunityInit` never exposes the community, and inbound replay
            // never starts against an un-superseded disk state.
            let store_dir = crate::managed_agents::agent_store_dir_for_relay(
                &restore_app,
                &state,
                &scope.relay_url,
            )
            .map_err(|error| format!("scoped event-sync store unavailable: {error}"))?;
            crate::event_sync::run_event_sync_blocking(
                restore_app.clone(),
                scope.owner_keys,
                scope.db_path,
                store_dir,
                event_sync_bootstrap,
            )
            .await?;
            crate::managed_agents::complete_active_event_sync_bootstrap(&restore_app)?;
            assert_current_apply_generation(&state.workspace_apply_generation, apply_generation)?;
        }
        Err(error) => {
            // Scope resolution is a prerequisite for establishing the
            // superseding head, so its failure is fatal for the same reason:
            // without a scope we cannot retain the repaired roster ahead of an
            // inbound replay. Fail the command rather than silently opening the
            // inbound lane.
            return Err(format!(
                "scoped event-sync unavailable after workspace apply: {error}"
            ));
        }
    }

    // A newer selection may have arrived during provider reconcile or event
    // sync. Never transfer the lock into launch restore for a stale request.
    assert_current_apply_generation(&state.workspace_apply_generation, apply_generation)?;
    let restore_pending = state
        .managed_agent_restore_pending
        .swap(false, Ordering::AcqRel);
    if let Err(error) =
        assert_current_apply_generation(&state.workspace_apply_generation, apply_generation)
    {
        if restore_pending {
            state
                .managed_agent_restore_pending
                .store(true, Ordering::Release);
        }
        return Err(error);
    }

    // Transfer the apply guard to launch restoration. The command can return
    // promptly, but a queued workspace cannot mutate relay/identity until the
    // restore has completed every mutable workspace read and side effect.
    #[cfg(feature = "mesh-llm")]
    {
        let restore_lock = apply_guard;
        let app = restore_app.clone();
        tauri::async_runtime::spawn(async move {
            let _restore_lock = restore_lock;
            let state = app.state::<AppState>();
            if restore_pending {
                if let Err(error) =
                    crate::commands::mesh_llm::restore_mesh_sharing(&app, &state).await
                {
                    eprintln!("maju-desktop: failed to restore Share Compute: {error}");
                }
            }
            crate::mesh_llm::publish_current_status_once(&app, "workspace apply").await;
            if restore_pending {
                if let Err(error) =
                    restore_managed_agents_on_launch(&app, &state.shutdown_started).await
                {
                    eprintln!("maju-desktop: failed to restore managed agents: {error}");
                }
            }
        });
        return Ok(applied_workspace);
    }

    #[cfg(not(feature = "mesh-llm"))]
    if restore_pending {
        let restore_lock = apply_guard;
        let app = restore_app.clone();
        tauri::async_runtime::spawn(async move {
            let _restore_lock = restore_lock;
            let state = app.state::<AppState>();
            if let Err(error) =
                restore_managed_agents_on_launch(&app, &state.shutdown_started).await
            {
                eprintln!("maju-desktop: failed to restore managed agents: {error}");
            }
        });
        return Ok(applied_workspace);
    }

    assert_current_apply_generation(&state.workspace_apply_generation, apply_generation)?;

    Ok(applied_workspace)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    };

    use super::{assert_current_apply_generation, begin_workspace_apply, next_apply_generation};

    #[test]
    fn explicit_newer_generation_supersedes_older_ticket() {
        let generation = AtomicU64::new(0);
        let older = next_apply_generation(&generation);
        let newer = next_apply_generation(&generation);

        let error = assert_current_apply_generation(&generation, older).unwrap_err();
        assert!(error.contains("superseded"), "{error}");
        assert_current_apply_generation(&generation, newer).unwrap();
    }

    #[tokio::test]
    async fn newer_request_supersedes_the_running_transaction_before_lock_handoff() {
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        let generation = Arc::new(AtomicU64::new(0));
        let (running_guard, running_ticket) = begin_workspace_apply(Arc::clone(&lock), &generation)
            .await
            .unwrap();

        let queued_lock = Arc::clone(&lock);
        let queued_generation = Arc::clone(&generation);
        let queued = tokio::spawn(async move {
            let (_guard, ticket) = begin_workspace_apply(queued_lock, &queued_generation)
                .await
                .unwrap();
            ticket
        });
        tokio::task::yield_now().await;

        // Selection order, not lock-acquisition order, is authoritative. The
        // queued request advances the generation immediately, so the running
        // request must fail its next boundary and release the lock.
        assert!(generation.load(Ordering::Acquire) > running_ticket);
        assert_current_apply_generation(&generation, running_ticket).unwrap_err();
        assert!(!queued.is_finished());

        drop(running_guard);
        let queued_ticket = queued.await.unwrap();
        assert!(queued_ticket > running_ticket);
        assert_current_apply_generation(&generation, queued_ticket).unwrap();
    }

    #[tokio::test]
    async fn stale_waiter_is_rejected_when_a_newer_waiter_was_selected() {
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        let held = Arc::clone(&lock).lock_owned().await;
        let generation = Arc::new(AtomicU64::new(0));

        let older_lock = Arc::clone(&lock);
        let older_generation = Arc::clone(&generation);
        let older =
            tokio::spawn(async move { begin_workspace_apply(older_lock, &older_generation).await });
        tokio::task::yield_now().await;

        let newer_lock = Arc::clone(&lock);
        let newer_generation = Arc::clone(&generation);
        let newer =
            tokio::spawn(async move { begin_workspace_apply(newer_lock, &newer_generation).await });
        tokio::task::yield_now().await;

        drop(held);
        let stale_error = older.await.unwrap().unwrap_err();
        assert!(stale_error.contains("superseded"), "{stale_error}");
        let (_guard, newest_ticket) = newer.await.unwrap().unwrap();
        assert_current_apply_generation(&generation, newest_ticket).unwrap();
    }
}
