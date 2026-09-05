use super::*;

/// In-memory core of the inbound `KIND_TEAM` reconcile: capture the matched
/// team's roster *before* applying the inbound projection, apply it, persist
/// teams authoritatively, then propagate the prior→current membership delta to
/// live instances best-effort — the same binding semantics the local
/// create/update commands use. Without this, a 30176 team edit from another
/// device lands on `teams.json` but never touches `ManagedAgentRecord.team_id`:
/// an added persona's running instances stay unbound (member in roster, not in
/// behavior) and a removed persona's instances keep drawing the old team's
/// instructions at spawn until restart.
///
/// A no-match insert has no prior roster, so its whole roster is the added
/// delta — symmetric with `commit_team_create`. Injected persistence keeps it
/// `AppHandle`-free so the prior-roster capture and delta direction are
/// unit-testable; a `persist_teams` error propagates, agent IO is best-effort
/// (mirrors the local command path: the authoritative team write already
/// landed, and boot repair is the designed retry for a stale binding).
pub(super) fn commit_inbound_team(
    teams: &mut Vec<TeamRecord>,
    d_tag: String,
    inbound: TeamEventContent,
    persist_teams: impl FnOnce(&[TeamRecord]) -> Result<(), String>,
    load_agents: impl FnOnce() -> Result<Vec<ManagedAgentRecord>, String>,
    save_agents: impl FnOnce(&[ManagedAgentRecord]) -> Result<(), String>,
) -> Result<(), String> {
    let team_id = d_tag.clone();
    let previous_persona_ids = teams
        .iter()
        .find(|record| record.id == team_id)
        .map(|record| record.persona_ids.clone())
        .unwrap_or_default();
    apply_inbound_team(teams, d_tag, inbound);
    let current_persona_ids = teams
        .iter()
        .find(|record| record.id == team_id)
        .map(|record| record.persona_ids.clone())
        .unwrap_or_default();
    persist_teams(teams)?;
    crate::commands::teams::propagate_membership_best_effort(
        &team_id,
        &previous_persona_ids,
        &current_persona_ids,
        load_agents,
        save_agents,
    );
    Ok(())
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
pub(super) fn apply_inbound_team(
    teams: &mut Vec<TeamRecord>,
    d_tag: String,
    inbound: TeamEventContent,
) {
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
            // Catalog share state is scoped and never inbound-authoritative.
            shared: false,
            // Owner-device sync, not a catalog add: the team is this owner's
            // own, so it has no foreign publication to attribute.
            catalog_source: None,
            source_dir: None,
            is_symlink: false,
            symlink_target: None,
            version: None,
            created_at: now_iso(),
            updated_at: now_iso(),
        }),
    }
}
