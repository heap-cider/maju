use tauri::AppHandle;

use crate::managed_agents::{ManagedAgentProcess, ManagedAgentRecord};

use super::spawn_agent_child_with_context;

/// Spawn an agent process using the active community's definitions and teams.
/// The caller owns record/runtime locking and persists the returned process.
pub fn spawn_agent_child(
    app: &AppHandle,
    record: &ManagedAgentRecord,
    relay_url: &str,
    lazy: bool,
    owner_hex: Option<&str>,
) -> Result<ManagedAgentProcess, String> {
    let personas = crate::managed_agents::load_personas(app).unwrap_or_default();
    let teams = crate::managed_agents::load_teams(app).unwrap_or_default();
    spawn_agent_child_with_context(app, record, relay_url, lazy, owner_hex, &personas, &teams)
}
