//! Stable identity invariants for definition-backed agents.

use std::collections::{HashMap, HashSet};

use super::{ManagedAgentRecord, TeamRecord};

pub fn ensure_definition_identity_is_unique(
    records: &[ManagedAgentRecord],
    persona_id: Option<&str>,
) -> Result<(), String> {
    let Some(persona_id) = persona_id else {
        return Ok(());
    };
    if let Some(existing) = records
        .iter()
        .find(|record| record.persona_id.as_deref() == Some(persona_id))
    {
        return Err(format!(
            "Agent definition {persona_id} already has identity {} in this community for this account.",
            existing.pubkey
        ));
    }
    Ok(())
}

/// Keep one recoverable identity for each definition in the active
/// community+owner store.
///
/// Legacy team deployment could mint several pubkeys for one definition. The
/// oldest record is the least surprising identity to preserve because it owns
/// the earliest conversation history. Pubkey is a deterministic tie-breaker.
/// The returned pubkeys are the discarded identities, suitable for scoped
/// relay cleanup; definition-less agents are intentionally unaffected.
pub fn deduplicate_definition_identities(records: &mut Vec<ManagedAgentRecord>) -> Vec<String> {
    let mut preferred_by_persona: HashMap<String, (usize, String, String)> = HashMap::new();

    for (index, record) in records.iter().enumerate() {
        let Some(persona_id) = record
            .persona_id
            .as_deref()
            .filter(|_| !record.pubkey.is_empty())
        else {
            continue;
        };
        let candidate = (index, record.created_at.clone(), record.pubkey.clone());
        match preferred_by_persona.get(persona_id) {
            Some((_, created_at, pubkey))
                if (created_at.as_str(), pubkey.as_str())
                    <= (candidate.1.as_str(), candidate.2.as_str()) => {}
            _ => {
                preferred_by_persona.insert(persona_id.to_string(), candidate);
            }
        }
    }

    let preferred_indexes: HashSet<usize> = preferred_by_persona
        .values()
        .map(|(index, _, _)| *index)
        .collect();
    let preferred_pubkeys: HashSet<String> = preferred_by_persona
        .values()
        .map(|(_, _, pubkey)| pubkey.clone())
        .collect();
    let mut discarded = Vec::new();
    let mut index = 0;
    records.retain(|record| {
        let definition_linked = record.persona_id.is_some() && !record.pubkey.is_empty();
        let keep = !definition_linked || preferred_indexes.contains(&index);
        if !keep && !preferred_pubkeys.contains(record.pubkey.as_str()) {
            discarded.push(record.pubkey.clone());
        }
        index += 1;
        keep
    });
    discarded.sort();
    discarded.dedup();
    discarded
}

pub fn resolve_definition_team_id(
    teams: &[TeamRecord],
    persona_id: Option<&str>,
    requested_team_id: Option<&str>,
) -> Result<Option<String>, String> {
    let requested = requested_team_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let assigned = persona_id.and_then(|persona_id| {
        teams
            .iter()
            .find(|team| team.persona_ids.iter().any(|id| id == persona_id))
            .map(|team| team.id.as_str())
    });
    if requested.is_some() && requested != assigned {
        return Err(
            "Team assignment comes from the agent definition. Update the definition's team first."
                .to_string(),
        );
    }
    Ok(assigned.map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn team(id: &str, persona_ids: &[&str]) -> TeamRecord {
        TeamRecord {
            shared: false,
            catalog_source: None,
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            instructions: None,
            persona_ids: persona_ids.iter().map(|id| (*id).to_string()).collect(),
            is_builtin: false,
            source_dir: None,
            is_symlink: false,
            symlink_target: None,
            version: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn definition_team_is_authoritative() {
        let teams = vec![team("team-a", &["persona-a"])];
        assert_eq!(
            resolve_definition_team_id(&teams, Some("persona-a"), None).unwrap(),
            Some("team-a".to_string())
        );
        assert!(resolve_definition_team_id(&teams, Some("persona-a"), Some("team-b")).is_err());
    }

    fn record(pubkey: &str, persona_id: Option<&str>, created_at: &str) -> ManagedAgentRecord {
        serde_json::from_value(serde_json::json!({
            "pubkey": pubkey,
            "name": pubkey,
            "persona_id": persona_id,
            "private_key_nsec": "",
            "relay_url": "",
            "acp_command": "maju-acp",
            "agent_command": "goose",
            "agent_args": [],
            "mcp_command": "",
            "turn_timeout_seconds": 320,
            "created_at": created_at,
            "updated_at": created_at
        }))
        .expect("valid managed-agent fixture")
    }

    #[test]
    fn definition_identity_is_unique_but_generic_agents_remain_independent() {
        let records = vec![
            record("first", Some("persona-a"), "2026-01-01T00:00:00Z"),
            record("generic", None, "2026-01-01T00:00:00Z"),
        ];

        assert!(ensure_definition_identity_is_unique(&records, Some("persona-a")).is_err());
        assert!(ensure_definition_identity_is_unique(&records, None).is_ok());
    }

    #[test]
    fn legacy_identity_dedup_keeps_the_oldest_record() {
        let mut records = vec![
            record("newer", Some("persona-a"), "2026-02-01T00:00:00Z"),
            record("generic", None, "2026-01-01T00:00:00Z"),
            record("older", Some("persona-a"), "2026-01-01T00:00:00Z"),
        ];

        assert_eq!(
            deduplicate_definition_identities(&mut records),
            vec!["newer"]
        );
        assert_eq!(
            records
                .iter()
                .map(|record| record.pubkey.as_str())
                .collect::<Vec<_>>(),
            vec!["generic", "older"]
        );
    }
}
