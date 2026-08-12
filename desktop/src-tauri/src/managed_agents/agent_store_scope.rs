//! Community+owner scoping and one-time migration for Agent/Team JSON stores.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
#[cfg(not(test))]
use tauri::Manager;

#[cfg(not(test))]
use crate::app_state::AppState;

use super::{
    deduplicate_definition_identities, retention, storage::atomic_write_json,
    storage::atomic_write_json_restricted, storage::managed_agents_base_dir,
    storage::read_agent_records, ManagedAgentRecord, TeamRecord,
};

const SCOPED_STORE_MARKER: &str = ".initialized-v1";
const LEGACY_STORE_CLAIM: &str = "legacy-store-claim.json";

#[derive(Debug, Serialize)]
struct AgentStoreScopeManifest<'a> {
    relay_url: &'a str,
    owner_pubkey: &'a str,
}

#[derive(Debug, Deserialize, Serialize)]
struct LegacyStoreClaim {
    scope_id: String,
}

/// Definite legacy leaks removed from the active local projection.
///
/// The caller retires these coordinates on the active community relay. The
/// preserved flat JSON remains the recovery copy, so this cleanup is scoped
/// and reversible rather than a destructive file migration.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct AgentStoreScopeRepair {
    #[serde(default)]
    pub agent_pubkeys: Vec<String>,
    #[serde(default)]
    pub team_ids: Vec<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct AgentStoreScopeMarker {
    repair: AgentStoreScopeRepair,
    repair_queued: bool,
}

/// Directory holding mutable Agent/Team JSON for one relay+owner scope.
/// Uses the same stable scope hash as the retention database.
pub(crate) fn scoped_agent_store_dir(
    base_dir: &Path,
    retention_db_path: &Path,
) -> Result<PathBuf, String> {
    let scope_id = retention_db_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "retention scope path has no usable scope id".to_string())?;
    Ok(base_dir.join("scopes").join(scope_id))
}

/// Unit tests historically use a bare Tauri app without applying a workspace,
/// so they retain the old flat seam. Production is always relay+owner scoped.
#[cfg(test)]
pub(crate) fn active_agent_store_dir(app: &AppHandle) -> Result<PathBuf, String> {
    managed_agents_base_dir(app)
}

#[cfg(not(test))]
pub(crate) fn active_agent_store_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let state = app.state::<AppState>();
    let scope = retention::active_retention_scope(app, &state)?;
    let base_dir = managed_agents_base_dir(app)?;
    let dir = scoped_agent_store_dir(&base_dir, &scope.db_path)?;
    fs::create_dir_all(&dir)
        .map_err(|error| format!("failed to create scoped agents dir: {error}"))?;
    Ok(dir)
}

fn read_team_records(path: &Path) -> Result<Vec<TeamRecord>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)
        .map_err(|error| format!("failed to read legacy teams store: {error}"))?;
    serde_json::from_str(&content)
        .map_err(|error| format!("failed to parse legacy teams store: {error}"))
}

fn select_scoped_agent_records(
    records: Vec<ManagedAgentRecord>,
    persona_d_tags: &HashSet<String>,
    agent_d_tags: &HashSet<String>,
    claim_all: bool,
) -> (Vec<ManagedAgentRecord>, Vec<String>) {
    // A legacy install may have retained the identity event but failed to
    // retain its definition event. When both the identity and its local
    // definition still exist, preserve that pair instead of treating a
    // retention gap as proof of a cross-community leak.
    let personas_referenced_by_retained_agents: HashSet<_> = records
        .iter()
        .filter(|record| {
            !record.pubkey.is_empty() && (claim_all || agent_d_tags.contains(&record.pubkey))
        })
        .filter_map(|record| record.persona_id.clone())
        .collect();
    let mut definitions: Vec<_> = records
        .iter()
        .filter(|record| record.pubkey.is_empty())
        .filter(|record| {
            claim_all
                || record.is_builtin
                || record.slug.as_ref().is_some_and(|slug| {
                    persona_d_tags.contains(slug)
                        || personas_referenced_by_retained_agents.contains(slug)
                })
        })
        .cloned()
        .collect();
    definitions.sort_by(|left, right| left.slug.cmp(&right.slug));
    let definition_ids: HashSet<_> = definitions
        .iter()
        .filter_map(|record| record.slug.clone())
        .collect();

    let instances: Vec<_> = records
        .into_iter()
        .filter(|record| !record.pubkey.is_empty())
        .filter(|record| claim_all || agent_d_tags.contains(&record.pubkey))
        .collect();

    // A definition-linked identity without that definition is not usable in
    // this community. Quarantine it instead of recreating the old
    // `Unknown agents / configuration missing` leak.
    let (mut instances, mut quarantined): (Vec<_>, Vec<_>) =
        instances.into_iter().partition(|record| {
            record
                .persona_id
                .as_ref()
                .is_none_or(|persona_id| definition_ids.contains(persona_id))
        });
    let mut quarantined_pubkeys: Vec<_> =
        quarantined.drain(..).map(|record| record.pubkey).collect();

    // Product identity is `(community, definition, owner)`. Keep the oldest
    // recoverable legacy identity and quarantine later accidental copies.
    quarantined_pubkeys.extend(deduplicate_definition_identities(&mut instances));
    quarantined_pubkeys.sort();
    quarantined_pubkeys.dedup();

    definitions.extend(instances);
    (definitions, quarantined_pubkeys)
}

fn select_scoped_teams(
    mut teams: Vec<TeamRecord>,
    team_d_tags: &HashSet<String>,
    definition_ids: &HashSet<String>,
    claim_all: bool,
) -> (Vec<TeamRecord>, Vec<String>) {
    teams.retain(|team| claim_all || team.is_builtin || team_d_tags.contains(&team.id));
    teams.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut assigned_personas = HashSet::new();
    let mut quarantined_team_ids = Vec::new();
    teams.retain_mut(|team| {
        let had_members = !team.persona_ids.is_empty();
        team.persona_ids.retain(|persona_id| {
            definition_ids.contains(persona_id) && assigned_personas.insert(persona_id.clone())
        });
        // A leaked team whose definitions are absent has no useful local
        // meaning. Built-ins and intentionally empty teams remain.
        let keep = team.is_builtin || !had_members || !team.persona_ids.is_empty();
        if !keep {
            quarantined_team_ids.push(team.id.clone());
        }
        keep
    });
    (teams, quarantined_team_ids)
}

pub(crate) fn apply_team_membership_to_instances(
    records: &mut [ManagedAgentRecord],
    teams: &[TeamRecord],
) {
    let team_by_persona: HashMap<&str, &str> = teams
        .iter()
        .flat_map(|team| {
            team.persona_ids
                .iter()
                .map(move |persona_id| (persona_id.as_str(), team.id.as_str()))
        })
        .collect();
    for record in records
        .iter_mut()
        .filter(|record| !record.pubkey.is_empty())
    {
        record.team_id = record
            .persona_id
            .as_deref()
            .and_then(|persona_id| team_by_persona.get(persona_id).copied())
            .map(str::to_string);
    }
}

fn any_retention_scope_has_agent_heads(base_dir: &Path) -> Result<bool, String> {
    let retention_dir = base_dir.join("retention");
    if !retention_dir.exists() {
        return Ok(false);
    }
    for entry in fs::read_dir(&retention_dir)
        .map_err(|error| format!("failed to read retention scopes: {error}"))?
    {
        let path = entry
            .map_err(|error| format!("failed to read retention scope entry: {error}"))?
            .path();
        if path.extension().and_then(|value| value.to_str()) != Some("db") {
            continue;
        }
        match retention::open_retention_db(&path)
            .and_then(|conn| retention::has_any_agent_domain_heads(&conn))
        {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(error) => {
                // Fail closed: an unreadable sibling scope means the legacy
                // global JSON must not be claimed by a new empty community.
                eprintln!(
                    "maju-desktop: could not inspect retention scope {}: {error}",
                    path.display()
                );
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn legacy_store_claims_scope(base_dir: &Path, scope_id: &str) -> Result<bool, String> {
    let path = base_dir.join(LEGACY_STORE_CLAIM);
    if path.exists() {
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read legacy store claim: {error}"))?;
        let claim: LegacyStoreClaim = serde_json::from_str(&content)
            .map_err(|error| format!("failed to parse legacy store claim: {error}"))?;
        return Ok(claim.scope_id == scope_id);
    }
    let payload = serde_json::to_vec_pretty(&LegacyStoreClaim {
        scope_id: scope_id.to_string(),
    })
    .map_err(|error| format!("failed to serialize legacy store claim: {error}"))?;
    atomic_write_json(&path, &payload)?;
    Ok(true)
}

/// Initialize the active scoped JSON store without deleting or moving the
/// legacy global files.
///
/// Existing retention heads define the community boundary. A truly
/// pre-scoping installation may claim the legacy JSON exactly once; every
/// later empty community starts empty instead of inheriting that file.
pub fn initialize_agent_store_scope(
    app: &AppHandle,
    scope: &retention::RetentionScope,
) -> Result<AgentStoreScopeRepair, String> {
    use maju_core_pkg::kind::{KIND_MANAGED_AGENT, KIND_PERSONA, KIND_TEAM};

    let base_dir = managed_agents_base_dir(app)?;
    let scoped_dir = scoped_agent_store_dir(&base_dir, &scope.db_path)?;
    fs::create_dir_all(&scoped_dir)
        .map_err(|error| format!("failed to create scoped agents dir: {error}"))?;
    let marker = scoped_dir.join(SCOPED_STORE_MARKER);
    if marker.exists() {
        let content = fs::read_to_string(&marker)
            .map_err(|error| format!("failed to read scoped agent-store marker: {error}"))?;
        let marker: AgentStoreScopeMarker = serde_json::from_str(&content)
            .map_err(|error| format!("failed to parse scoped agent-store marker: {error}"))?;
        return Ok(if marker.repair_queued {
            AgentStoreScopeRepair::default()
        } else {
            marker.repair
        });
    }

    let owner_pubkey = scope.owner_keys.public_key().to_hex();
    let conn = retention::open_retention_db(&scope.db_path)?;
    let persona_d_tags = retention::get_retained_d_tags(&conn, KIND_PERSONA, &owner_pubkey)?;
    let team_d_tags = retention::get_retained_d_tags(&conn, KIND_TEAM, &owner_pubkey)?;
    let agent_d_tags = retention::get_retained_d_tags(&conn, KIND_MANAGED_AGENT, &owner_pubkey)?;
    let scope_has_heads =
        !persona_d_tags.is_empty() || !team_d_tags.is_empty() || !agent_d_tags.is_empty();
    let scope_id = scope
        .db_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "retention scope path has no usable scope id".to_string())?;
    let claim_all = !scope_has_heads
        && !any_retention_scope_has_agent_heads(&base_dir)?
        && legacy_store_claims_scope(&base_dir, scope_id)?;

    let scoped_agent_path = scoped_dir.join("managed-agents.json");
    let scoped_team_path = scoped_dir.join("teams.json");
    // The marker is the commit point. If a previous initialization stopped
    // after writing one scoped file but before writing the marker, recompute
    // from the preserved flat store so the repair list cannot be lost.
    let source_agent_path = base_dir.join("managed-agents.json");
    let (mut selected_agents, quarantined_agent_pubkeys) = select_scoped_agent_records(
        read_agent_records(&source_agent_path)?,
        &persona_d_tags,
        &agent_d_tags,
        claim_all,
    );
    let definition_ids: HashSet<_> = selected_agents
        .iter()
        .filter(|record| record.pubkey.is_empty())
        .filter_map(|record| record.slug.clone())
        .collect();
    let source_team_path = base_dir.join("teams.json");
    let (selected_teams, quarantined_team_ids) = select_scoped_teams(
        read_team_records(&source_team_path)?,
        &team_d_tags,
        &definition_ids,
        claim_all,
    );
    apply_team_membership_to_instances(&mut selected_agents, &selected_teams);

    let payload = serde_json::to_vec_pretty(&selected_agents)
        .map_err(|error| format!("failed to serialize scoped agent store: {error}"))?;
    atomic_write_json_restricted(&scoped_agent_path, &payload)?;
    let payload = serde_json::to_vec_pretty(&selected_teams)
        .map_err(|error| format!("failed to serialize scoped teams store: {error}"))?;
    atomic_write_json(&scoped_team_path, &payload)?;
    let manifest = serde_json::to_vec_pretty(&AgentStoreScopeManifest {
        relay_url: &scope.relay_url,
        owner_pubkey: &owner_pubkey,
    })
    .map_err(|error| format!("failed to serialize agent scope manifest: {error}"))?;
    atomic_write_json(&scoped_dir.join("scope.json"), &manifest)?;
    let repair = AgentStoreScopeRepair {
        agent_pubkeys: quarantined_agent_pubkeys,
        team_ids: quarantined_team_ids,
    };
    let marker_payload = serde_json::to_vec_pretty(&AgentStoreScopeMarker {
        repair: repair.clone(),
        repair_queued: false,
    })
    .map_err(|error| format!("failed to serialize scoped agent-store marker: {error}"))?;
    atomic_write_json(&marker, &marker_payload)?;
    Ok(repair)
}

/// Mark the one-time scoped relay repair as durably queued.
///
/// If the app exits after initialization but before this marker update, the
/// next workspace apply returns the same repair list and safely retries it.
pub(crate) fn complete_active_agent_store_scope_repair(app: &AppHandle) -> Result<(), String> {
    let marker_path = active_agent_store_dir(app)?.join(SCOPED_STORE_MARKER);
    if !marker_path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&marker_path)
        .map_err(|error| format!("failed to read scoped agent-store marker: {error}"))?;
    let mut marker: AgentStoreScopeMarker = serde_json::from_str(&content)
        .map_err(|error| format!("failed to parse scoped agent-store marker: {error}"))?;
    marker.repair_queued = true;
    let payload = serde_json::to_vec_pretty(&marker)
        .map_err(|error| format!("failed to serialize scoped agent-store marker: {error}"))?;
    atomic_write_json(&marker_path, &payload)
}

#[cfg(test)]
#[path = "agent_store_scope_tests.rs"]
mod tests;
