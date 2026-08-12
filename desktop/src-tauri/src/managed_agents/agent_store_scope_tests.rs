use std::collections::HashSet;

use super::{
    apply_team_membership_to_instances, scoped_agent_store_dir, select_scoped_agent_records,
    select_scoped_teams, ManagedAgentRecord, TeamRecord,
};

fn record(
    pubkey: &str,
    persona_id: Option<&str>,
    slug: Option<&str>,
    created_at: &str,
) -> ManagedAgentRecord {
    serde_json::from_value(serde_json::json!({
        "pubkey": pubkey,
        "name": slug.or(persona_id).unwrap_or("agent"),
        "persona_id": persona_id,
        "private_key_nsec": "",
        "relay_url": "",
        "acp_command": "maju-acp",
        "agent_command": "goose",
        "agent_args": [],
        "mcp_command": "",
        "turn_timeout_seconds": 320,
        "created_at": created_at,
        "updated_at": created_at,
        "slug": slug
    }))
    .expect("valid managed-agent fixture")
}

fn team(id: &str, personas: &[&str], created_at: &str) -> TeamRecord {
    TeamRecord {
        id: id.to_string(),
        name: id.to_string(),
        description: None,
        instructions: None,
        persona_ids: personas.iter().map(|id| (*id).to_string()).collect(),
        is_builtin: false,
        source_dir: None,
        is_symlink: false,
        symlink_target: None,
        version: None,
        created_at: created_at.to_string(),
        updated_at: created_at.to_string(),
    }
}

#[test]
fn scope_directory_reuses_retention_scope_id() {
    let dir = scoped_agent_store_dir(
        std::path::Path::new("agents"),
        std::path::Path::new("agents/retention/abc123.db"),
    )
    .expect("scope directory");

    assert_eq!(dir, std::path::Path::new("agents/scopes/abc123"));
}

#[test]
fn scoped_selection_quarantines_foreign_or_orphaned_and_duplicate_identities() {
    let records = vec![
        record("", None, Some("office"), "2026-01-01T00:00:00Z"),
        record("office-old", Some("office"), None, "2026-01-01T00:00:00Z"),
        record("office-copy", Some("office"), None, "2026-02-01T00:00:00Z"),
        record("home-agent", Some("home"), None, "2026-01-01T00:00:00Z"),
    ];
    let persona_tags = HashSet::from(["office".to_string()]);
    let agent_tags = HashSet::from([
        "office-old".to_string(),
        "office-copy".to_string(),
        "home-agent".to_string(),
    ]);

    let (selected, quarantined) =
        select_scoped_agent_records(records, &persona_tags, &agent_tags, false);
    let definitions: Vec<_> = selected
        .iter()
        .filter_map(|record| record.slug.as_deref())
        .collect();
    let identities: Vec<_> = selected
        .iter()
        .filter(|record| !record.pubkey.is_empty())
        .map(|record| record.pubkey.as_str())
        .collect();

    assert_eq!(definitions, vec!["office"]);
    assert_eq!(identities, vec!["office-old"]);
    assert_eq!(
        quarantined,
        vec!["home-agent".to_string(), "office-copy".to_string()]
    );
}

#[test]
fn scoped_selection_preserves_a_local_definition_when_its_agent_head_survived() {
    let records = vec![
        record("", None, Some("legacy"), "2026-01-01T00:00:00Z"),
        record("legacy-agent", Some("legacy"), None, "2026-01-01T00:00:00Z"),
    ];

    let (selected, quarantined) = select_scoped_agent_records(
        records,
        &HashSet::new(),
        &HashSet::from(["legacy-agent".to_string()]),
        false,
    );

    assert_eq!(selected.len(), 2);
    assert!(quarantined.is_empty());
}

#[test]
fn team_selection_and_assignment_keep_one_team_per_definition() {
    let teams = vec![
        team("first", &["office"], "2026-01-01T00:00:00Z"),
        team("copy", &["office"], "2026-02-01T00:00:00Z"),
        team("foreign", &["home"], "2026-01-01T00:00:00Z"),
    ];
    let team_tags = HashSet::from([
        "first".to_string(),
        "copy".to_string(),
        "foreign".to_string(),
    ]);
    let definition_ids = HashSet::from(["office".to_string()]);

    let (selected, quarantined) = select_scoped_teams(teams, &team_tags, &definition_ids, false);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, "first");
    assert_eq!(quarantined, vec!["foreign", "copy"]);

    let mut agents = vec![record(
        "office-agent",
        Some("office"),
        None,
        "2026-01-01T00:00:00Z",
    )];
    apply_team_membership_to_instances(&mut agents, &selected);
    assert_eq!(agents[0].team_id.as_deref(), Some("first"));
}
