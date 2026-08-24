use super::*;
use crate::managed_agents::retention::{
    mark_synced, open_retention_db, retain_event, RetainedEvent,
};
use maju_core_pkg::kind::{KIND_MANAGED_AGENT, KIND_PERSONA};
use nostr::{EventBuilder, JsonUtil, Kind, Tag};

#[test]
fn publication_waits_for_confirmed_definition() {
    let owner = nostr::Keys::generate();
    let owner_pubkey = owner.public_key().to_hex();
    let identity = EventBuilder::new(
        Kind::Custom(KIND_MANAGED_AGENT as u16),
        serde_json::json!({
            "name": "Agent",
            "persona_id": "definition-one",
            "parallelism": 1,
            "respond_to": "owner-only"
        })
        .to_string(),
    )
    .tags(vec![Tag::parse(["d", "agent-one"]).unwrap()])
    .sign_with_keys(&owner)
    .unwrap();
    let conn = open_retention_db(std::path::Path::new(":memory:")).unwrap();

    assert!(!managed_agent_dependencies_confirmed(&conn, &identity, &owner_pubkey).unwrap());

    let definition = RetainedEvent {
        kind: KIND_PERSONA,
        pubkey: owner_pubkey.clone(),
        d_tag: "definition-one".to_string(),
        content: "{}".to_string(),
        created_at: 1,
        raw_event: identity.as_json(),
        pending_sync: true,
    };
    retain_event(&conn, &definition).unwrap();
    assert!(!managed_agent_dependencies_confirmed(&conn, &identity, &owner_pubkey).unwrap());

    mark_synced(
        &conn,
        KIND_PERSONA,
        &owner_pubkey,
        "definition-one",
        definition.created_at,
        &definition.content,
    )
    .unwrap();
    assert!(managed_agent_dependencies_confirmed(&conn, &identity, &owner_pubkey).unwrap());
}
