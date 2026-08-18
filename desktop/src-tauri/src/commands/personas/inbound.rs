//! Inbound relay → local store reconciliation for persona/team/managed-agent
//! projections and their NIP-09 tombstones. Extracted from the parent module to
//! keep it under the file-size cap.

use tauri::{AppHandle, Emitter, Manager};

use crate::{
    app_state::AppState,
    managed_agents::{
        agent_events::ManagedAgentEventContent, load_personas, persona_events::persona_d_tag,
        save_personas, team_events::TeamEventContent, try_regenerate_nest, AgentDefinition,
        ManagedAgentRecord, TeamRecord,
    },
    util::now_iso,
};

#[cfg(test)]
mod inbound_tests;

/// Apply an inbound kind:30175 persona event from the relay onto the local
/// store. The frontend's live subscription invokes this per event for our own
/// authored coordinate so Device B inherits Device A's edits.
///
/// Retention is a sync channel that writes INTO `personas.json`, never an
/// authoritative read source — `load_personas` is untouched, so every agent
/// keeps resolving its persona by UUID and keeps its provider keys.
///
/// MATCH KEY (single source of truth, both directions): an inbound event
/// matches the local record whose `persona_d_tag(record)` equals the event's
/// d-tag. Reusing the same derivation the outbound path uses guarantees the
/// inbound key can never drift from the outbound key — in particular, an
/// in-app persona (`source_team_persona_slug == None`) whose d-tag IS its
/// `id` matches its existing UUID row instead of minting a duplicate.
///
/// On match: patch ONLY the projected fields; preserve local `id`, `env_vars`,
/// `source_team`, and `created_at`. On no match: insert the parsed record as-is
/// — `persona_from_event` already sets `id = d_tag`, so an in-app persona reuses
/// its d-tag as the id and a re-received event stays idempotent (no duplicate).
///
/// The retention store decides whether the inbound event wins over a pending
/// local edit (`retain_inbound_event`): `personas.json` is patched for a newer
/// [`InboundOutcome::Applied`] head or an exact retained
/// [`InboundOutcome::Reapply`] echo. Reapply heals a missing projection; an
/// equal-second collision with a pending local edit remains untouched.
///
/// `arrival_relay_url` is the relay the calling subscription is bound to. The
/// retention store this event belongs to is decided by the community that
/// DELIVERED it, not by whichever community happens to be active when the
/// reconcile runs — a workspace switch in flight would otherwise file community
/// A's event into community B's scoped database. An event whose arrival relay is
/// no longer the active scope is dropped: it was already durable in its own
/// community's store when it arrived there, and that community's next boot
/// reconcile refetches it.
#[tauri::command]
pub async fn reconcile_inbound_persona_event(
    event_json: String,
    arrival_relay_url: String,
    app: AppHandle,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        reconcile_inbound_persona_event_blocking(event_json, arrival_relay_url, app)
    })
    .await
    .map_err(|e| format!("spawn_blocking failed: {e}"))?
}

fn reconcile_inbound_persona_event_blocking(
    event_json: String,
    arrival_relay_url: String,
    app: AppHandle,
) -> Result<(), String> {
    use crate::managed_agents::{
        agent_events::managed_agent_content_from_event,
        apply_team_membership_to_instances, deduplicate_definition_identities, load_managed_agents,
        load_teams,
        persona_events::persona_from_event,
        retention::{open_retention_db, retain_inbound_event, InboundOutcome, RetainedEvent},
        save_managed_agents, save_teams,
        team_events::team_content_from_event,
    };
    use maju_core_pkg::kind::{KIND_DELETION, KIND_MANAGED_AGENT, KIND_PERSONA, KIND_TEAM};
    use nostr::JsonUtil;

    let state = app.state::<AppState>();
    let event = parse_verified_inbound_event(&event_json)?;

    // The live filter subscribes to 30175/30176/30177 (upserts) plus kind:5
    // (NIP-09 deletions). d-tags are NOT unique across kinds, so every path
    // below dispatches on kind FIRST and only ever touches its own store — a
    // cross-kind d-tag collision can never link a team to a persona or agent.
    let kind = event.kind.as_u16() as u32;

    // kind:5 deletion: a tombstone removes the local record at the coordinate
    // in its `a` tag (`<target_kind>:<owner>:<d_tag>`). Handled before the
    // upsert dispatch because its coordinate and retention key differ.
    if kind == KIND_DELETION {
        return reconcile_inbound_tombstone(&event, &arrival_relay_url, &app, &state);
    }

    if !matches!(kind, KIND_PERSONA | KIND_TEAM | KIND_MANAGED_AGENT) {
        return Ok(());
    }

    // The d-tag identifies the record within its kind. Persona derives it from
    // the parsed record (`persona_d_tag`); team/agent carry it as the event's
    // d-tag directly. Definition-bearing content is parsed and validated once
    // here, before retention, then reused in the apply branch below. This keeps
    // an unsafe event out of both the retention database and the local store.
    let owner_keys = state.signing_keys()?;
    let owner_pubkey = owner_keys.public_key().to_hex();
    if !event.pubkey.to_hex().eq_ignore_ascii_case(&owner_pubkey) {
        return Err(
            "inbound agent-definition event was not authored by the active owner".to_string(),
        );
    }

    let inbound_persona = (kind == KIND_PERSONA)
        .then(|| persona_from_event(&event))
        .transpose()?;
    if let Some(persona) = &inbound_persona {
        validate_inbound_persona_definition(persona)?;
    }
    let inbound_managed_agent = (kind == KIND_MANAGED_AGENT)
        .then(|| managed_agent_content_from_event(&event))
        .transpose()?;
    if let Some(managed_agent) = &inbound_managed_agent {
        validate_inbound_managed_agent_definition(managed_agent)?;
    }
    let d_tag = match &inbound_persona {
        Some(persona) => persona_d_tag(persona),
        None => event_d_tag(&event)?,
    };
    // Validate the encrypted identity before the inbound event is allowed to
    // replace the retained head. A corrupt envelope must not overwrite the
    // last recoverable copy and only then fail during local-store apply.
    if let Some(envelope) = inbound_managed_agent
        .as_ref()
        .and_then(|content| content.identity_key_envelope.as_deref())
    {
        crate::managed_agents::agent_events::decrypt_agent_identity_key(
            &owner_keys,
            envelope,
            &d_tag,
        )?;
    }

    let _store_guard = state
        .managed_agents_store_lock
        .lock()
        .map_err(|error| error.to_string())?;

    // Resolve inbound vs. any pending local edit before touching the store, in
    // the scope the event ARRIVED on. A workspace switch since arrival leaves
    // this event to its own community's store — dropping it here is what keeps
    // community A's head out of community B's database.
    let Some(scope) = crate::managed_agents::retention::arrival_retention_scope(
        &app,
        &state,
        &arrival_relay_url,
    )?
    else {
        return Ok(());
    };
    let conn = open_retention_db(&scope.db_path)?;
    let outcome = retain_inbound_event(
        &conn,
        &RetainedEvent {
            kind,
            pubkey: event.pubkey.to_hex(),
            d_tag: d_tag.clone(),
            content: event.content.to_string(),
            created_at: event.created_at.as_secs() as i64,
            raw_event: event.as_json(),
            pending_sync: false,
        },
    )?;
    if outcome == InboundOutcome::Skipped {
        return Ok(());
    }

    match kind {
        KIND_PERSONA => {
            let mut personas = load_personas(&app)?;
            // `inbound_persona` is `Some` for KIND_PERSONA (set above).
            apply_inbound_persona(
                &mut personas,
                inbound_persona.expect("persona parsed above"),
            );
            save_personas(&app, &personas)?;
        }
        KIND_TEAM => {
            let mut teams = load_teams(&app)?;
            apply_inbound_team(&mut teams, d_tag, team_content_from_event(&event)?);
            save_teams(&app, &teams)?;
            let mut agents = load_managed_agents(&app)?;
            let previous_team_ids: Vec<_> =
                agents.iter().map(|agent| agent.team_id.clone()).collect();
            apply_team_membership_to_instances(&mut agents, &teams);
            if agents
                .iter()
                .map(|agent| &agent.team_id)
                .ne(previous_team_ids.iter())
            {
                save_managed_agents(&app, &agents)?;
            }
        }
        KIND_MANAGED_AGENT => {
            let mut agents = load_managed_agents(&app)?;
            let was_known = agents.iter().any(|record| record.pubkey == d_tag);
            let inbound = inbound_managed_agent.ok_or_else(|| {
                "managed-agent content was not parsed before retention".to_string()
            })?;
            apply_inbound_managed_agent(&mut agents, &d_tag, inbound, &owner_keys)?;
            if !was_known {
                let created_at = i64::try_from(event.created_at.as_secs())
                    .ok()
                    .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
                    .map(|timestamp| timestamp.to_rfc3339())
                    .unwrap_or_else(now_iso);
                if let Some(record) = agents.iter_mut().find(|record| record.pubkey == d_tag) {
                    record.created_at = created_at.clone();
                    record.updated_at = created_at;
                }
            }
            let teams = load_teams(&app)?;
            apply_team_membership_to_instances(&mut agents, &teams);
            let discarded = deduplicate_definition_identities(&mut agents);
            save_managed_agents(&app, &agents)?;
            for pubkey in discarded {
                super::super::agents::tombstone_managed_agent_pending(&app, &state, &pubkey);
                super::super::agents::archive_managed_agent_pending(&app, &state, &pubkey, None);
            }
        }
        _ => unreachable!("kind gated above"),
    }
    try_regenerate_nest(&app);

    // Signal the live UI to refetch agents data — inbound relay events otherwise
    // land on disk silently, leaving the Agents tab stale until restart.
    let _ = app.emit("agents-data-changed", ());

    Ok(())
}

fn validate_inbound_persona_definition(persona: &AgentDefinition) -> Result<(), String> {
    crate::managed_agents::validate_agent_definition_text(
        &persona.display_name,
        &persona.system_prompt,
    )
    .map_err(|error| format!("Inbound persona definition is unsafe: {error}"))
}

fn validate_inbound_managed_agent_definition(
    managed_agent: &ManagedAgentEventContent,
) -> Result<(), String> {
    crate::managed_agents::validate_managed_agent_definition_text(
        &managed_agent.name,
        managed_agent.persona_id.as_deref(),
        managed_agent.system_prompt.as_deref(),
    )
    .map_err(|error| format!("Inbound managed-agent definition is unsafe: {error}"))
}

/// Parse an inbound wire event and enforce the signature gate. Everything
/// downstream trusts `event.pubkey` (ownership routing, tombstone scoping,
/// behavioral-quad application), so a forged pubkey must die here — the
/// TS-side owner filter reads the same attacker-controlled field and is no
/// defense.
fn parse_verified_inbound_event(event_json: &str) -> Result<nostr::Event, String> {
    use nostr::JsonUtil;
    let event = nostr::Event::from_json(event_json)
        .map_err(|e| format!("failed to parse inbound event: {e}"))?;
    event
        .verify()
        .map_err(|e| format!("inbound event failed signature verification: {e}"))?;
    Ok(event)
}

/// Parse a NIP-09 `a`-tag coordinate `<kind>:<owner_pubkey>:<d_tag>` into its
/// target kind and d-tag. Returns `None` if the tag is absent or malformed, so
/// the caller no-ops on a tombstone it can't route.
fn parse_deletion_coordinate(event: &nostr::Event) -> Option<(u32, String)> {
    event.tags.iter().find_map(|tag| {
        let values: Vec<&str> = tag.as_slice().iter().map(|s| s.as_str()).collect();
        if values.first() != Some(&"a") {
            return None;
        }
        let coord = values.get(1)?;
        // `<kind>:<owner>:<d_tag>` — d_tag may itself contain ':' so split at
        // most twice and keep the remainder as the d_tag.
        let mut parts = coord.splitn(3, ':');
        let kind: u32 = parts.next()?.parse().ok()?;
        let owner = parts.next()?;
        // NIP-09 scoping: only the record's author may tombstone it. The
        // signature gate upstream proves `event.pubkey`; requiring the
        // coordinate owner to match closes the other half — a validly
        // signed kind:5 naming ANOTHER owner's coordinate must no-op.
        if owner != event.pubkey.to_hex() {
            return None;
        }
        let d_tag = parts.next()?;
        Some((kind, d_tag.to_string()))
    })
}

/// Apply an inbound kind:5 NIP-09 deletion: remove the local record at the
/// tombstone's target coordinate, scoped per-kind. Mirrors the upsert spine —
/// arrival-scoped retention resolution under the store lock, then a per-kind
/// store mutation — but removes rather than patches. Unknown/malformed
/// coordinates no-op, as does a tombstone whose arrival community is no longer
/// active.
fn reconcile_inbound_tombstone(
    event: &nostr::Event,
    arrival_relay_url: &str,
    app: &AppHandle,
    state: &AppState,
) -> Result<(), String> {
    use crate::managed_agents::{
        load_managed_agents, load_teams,
        retention::{
            open_retention_db, retain_inbound_event, tombstone_retention_d_tag, InboundOutcome,
            RetainedEvent,
        },
        save_managed_agents, save_teams,
    };
    use maju_core_pkg::kind::{KIND_DELETION, KIND_MANAGED_AGENT, KIND_PERSONA, KIND_TEAM};
    use nostr::JsonUtil;

    let Some((target_kind, target_d_tag)) = parse_deletion_coordinate(event) else {
        return Ok(()); // no routable coordinate — nothing to delete
    };
    if !matches!(target_kind, KIND_PERSONA | KIND_TEAM | KIND_MANAGED_AGENT) {
        return Ok(()); // deletion for a kind we don't track locally
    }

    let _store_guard = state
        .managed_agents_store_lock
        .lock()
        .map_err(|error| error.to_string())?;

    // Resolve against the retained tombstone row (keyed by the target
    // coordinate, F2c) so a re-received tombstone or one older than a pending
    // local edit is a no-op. Scoped to the arrival community, so a workspace
    // switch since arrival drops the tombstone instead of retaining it — and
    // deleting a record — in the wrong community's store.
    let Some(scope) =
        crate::managed_agents::retention::arrival_retention_scope(app, state, arrival_relay_url)?
    else {
        return Ok(());
    };
    let conn = open_retention_db(&scope.db_path)?;
    let outcome = retain_inbound_event(
        &conn,
        &RetainedEvent {
            kind: KIND_DELETION,
            pubkey: event.pubkey.to_hex(),
            d_tag: tombstone_retention_d_tag(target_kind, &target_d_tag),
            content: event.content.to_string(),
            created_at: event.created_at.as_secs() as i64,
            raw_event: event.as_json(),
            pending_sync: false,
        },
    )?;
    if outcome == InboundOutcome::Skipped {
        return Ok(());
    }

    // Remove the local record using the SAME per-kind match rule the apply fns
    // use: persona by `persona_d_tag`, team by `id`, managed-agent by `pubkey`.
    match target_kind {
        KIND_PERSONA => {
            let mut personas = load_personas(app)?;
            personas.retain(|record| persona_d_tag(record) != target_d_tag);
            save_personas(app, &personas)?;
        }
        KIND_TEAM => {
            let mut teams = load_teams(app)?;
            teams.retain(|record| record.id != target_d_tag);
            save_teams(app, &teams)?;
        }
        KIND_MANAGED_AGENT => {
            let mut agents = load_managed_agents(app)?;
            agents.retain(|record| record.pubkey != target_d_tag);
            save_managed_agents(app, &agents)?;
        }
        _ => unreachable!("target kind gated above"),
    }
    try_regenerate_nest(app);

    // Refresh the live UI on inbound deletion — a removal is as user-visible as
    // an upsert and the Agents tab must drop the tombstoned record without restart.
    let _ = app.emit("agents-data-changed", ());

    Ok(())
}

/// Extract the `d` tag value from an event, the match key for team (= team id)
/// and managed-agent (= agent pubkey) inbound reconcile.
fn event_d_tag(event: &nostr::Event) -> Result<String, String> {
    event
        .tags
        .iter()
        .find_map(|tag| {
            let values: Vec<&str> = tag.as_slice().iter().map(|s| s.as_str()).collect();
            (values.first() == Some(&"d"))
                .then(|| values.get(1).map(|s| s.to_string()))
                .flatten()
        })
        .ok_or_else(|| "inbound event missing d-tag".to_string())
}

/// Merge a parsed inbound persona into the local set: patch the matching record
/// in place, or push it when none matches.
///
/// The match key is `persona_d_tag` — the same derivation the outbound path
/// uses — so the inbound and outbound keys can never drift. On match, only the
/// projected fields are overwritten; local `id`, `env_vars`, `source_team`, and
/// `created_at` survive. On no match, the parsed record is inserted as-is; since
/// `persona_from_event` sets `id = d_tag`, an in-app persona reuses its d-tag as
/// the id and a re-received event stays idempotent (no duplicate row).
fn apply_inbound_persona(personas: &mut Vec<AgentDefinition>, inbound: AgentDefinition) {
    let d_tag = persona_d_tag(&inbound);
    match personas
        .iter_mut()
        .find(|record| persona_d_tag(record) == d_tag)
    {
        Some(local) => {
            local.display_name = inbound.display_name;
            local.avatar_url = inbound.avatar_url;
            local.system_prompt = inbound.system_prompt;
            local.runtime = inbound.runtime;
            local.model = inbound.model;
            local.provider = inbound.provider;
            local.name_pool = inbound.name_pool;
            local.respond_to = inbound.respond_to;
            local.respond_to_allowlist = inbound.respond_to_allowlist;
            local.parallelism = inbound.parallelism;
            local.shared = inbound.shared;
            local.updated_at = inbound.updated_at;
        }
        None => personas.push(inbound),
    }
}

/// Merge an inbound kind:30177 managed-agent projection into the local set.
///
/// Matches the local record whose `pubkey` equals the event's d-tag (the d-tag
/// IS the agent pubkey — see `build_agent_event`). On match, overwrite ONLY the
/// projected config fields. Runtime credentials, backend placement, harness
/// pins, and runtime state stay local. The agent key is the sole exception: a
/// valid owner-encrypted envelope may hydrate it so the logical signing
/// identity follows the owner across devices.
///
/// On a new device, a valid owner-self-encrypted identity envelope materializes
/// a stopped local runtime record with the same agent pubkey and key. Runtime
/// placement and credentials remain device-local; only the logical signing
/// identity follows the owner. An old event without an envelope still no-ops on
/// no match, because it cannot create a runnable identity safely.
fn apply_inbound_managed_agent(
    agents: &mut Vec<ManagedAgentRecord>,
    d_tag: &str,
    inbound: ManagedAgentEventContent,
    owner_keys: &nostr::Keys,
) -> Result<(), String> {
    let recovered_nsec = inbound
        .identity_key_envelope
        .as_deref()
        .map(|envelope| {
            crate::managed_agents::agent_events::decrypt_agent_identity_key(
                owner_keys, envelope, d_tag,
            )
        })
        .transpose()?;

    if let Some(local) = agents.iter_mut().find(|record| record.pubkey == d_tag) {
        local.name = inbound.name;
        // Mirror of the slimmed writer (agent_event_content): a
        // definition-linked event omits the definition quad because those
        // fields resolve through the kind:30175 definition — absent means
        // "not carried", never "clear". Definition-less events still carry
        // the quad and apply it unconditionally (including clears).
        let definition_linked = inbound.persona_id.is_some();
        local.persona_id = inbound.persona_id;
        // Old 30177 events predate team_id. Absence means "not carried", not
        // "clear the local assignment"; new events make the logical team
        // portable across devices.
        if inbound.team_id.is_some() {
            local.team_id = inbound.team_id;
        }
        if !definition_linked {
            local.system_prompt = inbound.system_prompt;
            local.model = inbound.model;
            local.provider = inbound.provider;
            local.persona_source_version = inbound.persona_source_version;
        }
        local.parallelism = inbound.parallelism;
        local.respond_to = inbound.respond_to;
        local.respond_to_allowlist = inbound.respond_to_allowlist;
        if let Some(nsec) = recovered_nsec {
            local.private_key_nsec = nsec;
            if local.auth_tag.is_none() {
                local.auth_tag = Some(agent_owner_auth_tag(owner_keys, d_tag)?);
            }
        }
        return Ok(());
    }

    let Some(private_key_nsec) = recovered_nsec else {
        return Ok(());
    };
    let now = now_iso();
    agents.push(ManagedAgentRecord {
        pubkey: d_tag.to_ascii_lowercase(),
        name: inbound.name,
        persona_id: inbound.persona_id,
        team_id: inbound.team_id,
        private_key_nsec,
        auth_tag: Some(agent_owner_auth_tag(owner_keys, d_tag)?),
        // Empty means "use this device's active community". Runtime placement
        // is deliberately local even though the signing identity is shared.
        relay_url: String::new(),
        avatar_url: None,
        acp_command: crate::managed_agents::DEFAULT_ACP_COMMAND.to_string(),
        agent_command: crate::managed_agents::default_agent_command(),
        agent_command_override: None,
        agent_args: Vec::new(),
        mcp_command: String::new(),
        turn_timeout_seconds: crate::managed_agents::DEFAULT_AGENT_TURN_TIMEOUT_SECONDS,
        idle_timeout_seconds: None,
        max_turn_duration_seconds: None,
        parallelism: inbound.parallelism,
        system_prompt: inbound.system_prompt,
        model: inbound.model,
        provider: inbound.provider,
        persona_source_version: inbound.persona_source_version,
        env_vars: Default::default(),
        start_on_app_launch: false,
        auto_restart_on_config_change: true,
        runtime_pid: None,
        backend: crate::managed_agents::BackendKind::Local,
        backend_agent_id: None,
        provider_binary_path: None,
        persona_team_dir: None,
        persona_name_in_team: None,
        created_at: now.clone(),
        updated_at: now,
        last_started_at: None,
        last_stopped_at: None,
        last_exit_code: None,
        last_error: None,
        last_error_code: None,
        respond_to: inbound.respond_to,
        respond_to_allowlist: inbound.respond_to_allowlist,
        display_name: None,
        slug: None,
        runtime: None,
        name_pool: Vec::new(),
        is_builtin: false,
        is_active: true,
        shared: false,
        source_team: None,
        source_team_persona_slug: None,
        catalog_source: None,
        definition_respond_to: None,
        definition_respond_to_allowlist: Vec::new(),
        definition_parallelism: None,
        relay_mesh: None,
    });
    Ok(())
}

fn agent_owner_auth_tag(owner_keys: &nostr::Keys, agent_pubkey: &str) -> Result<String, String> {
    let compat_owner = nostr::Keys::parse(&owner_keys.secret_key().to_secret_hex())
        .map_err(|e| format!("failed to bridge owner keys: {e}"))?;
    let compat_agent = nostr::PublicKey::from_hex(agent_pubkey)
        .map_err(|e| format!("failed to bridge agent pubkey: {e}"))?;
    maju_sdk_pkg::nip_oa::compute_auth_tag(&compat_owner, &compat_agent, "")
        .map_err(|e| format!("failed to compute NIP-OA auth tag: {e}"))
}

/// Merge an inbound kind:30176 team projection into the local set.
///
/// Matches the local record whose `id` equals the event's d-tag (the d-tag IS
/// the team id — see `build_team_event`). On match, overwrite ONLY the three
/// shared fields (`name`, `description`, `persona_ids`); install-specific local
/// fields (`source_dir`, `is_symlink`, `symlink_target`, `is_builtin`,
/// `version`, `created_at`) are preserved. On no match, insert a fresh record
/// reusing the d-tag as the id so a re-received event stays idempotent —
/// symmetric to the persona path, since a team (like a persona) is a secretless
/// definition that another device may legitimately learn about from the relay.
fn apply_inbound_team(teams: &mut Vec<TeamRecord>, d_tag: String, inbound: TeamEventContent) {
    // Team membership is a single-valued definition property. When a newer
    // team event assigns definitions here, remove those definitions from any
    // other local team before applying this record.
    if let Some(persona_ids) = inbound.persona_ids.as_ref() {
        for team in teams.iter_mut().filter(|team| team.id != d_tag) {
            team.persona_ids
                .retain(|persona_id| !persona_ids.contains(persona_id));
        }
    }
    match teams.iter_mut().find(|record| record.id == d_tag) {
        Some(local) => {
            local.name = inbound.name;
            local.description = inbound.description;
            // `None` means the event came from a client that predates
            // always-publish — its true value is unknown, so preserve
            // local. Only `Some` (including the explicit-clear variants)
            // overwrites. See `TeamEventContent` for the wire rules.
            if let Some(instructions) = inbound.instructions {
                local.instructions = instructions;
            }
            if let Some(persona_ids) = inbound.persona_ids {
                local.persona_ids = persona_ids;
            }
        }
        None => teams.push(TeamRecord {
            id: d_tag,
            name: inbound.name,
            description: inbound.description,
            // Fresh insert has no local value to preserve; `None` from a
            // pre-fix client simply means no known value.
            instructions: inbound.instructions.unwrap_or_default(),
            persona_ids: inbound.persona_ids.unwrap_or_default(),
            is_builtin: false,
            source_dir: None,
            is_symlink: false,
            symlink_target: None,
            version: None,
            created_at: now_iso(),
            updated_at: now_iso(),
        }),
    }
}
