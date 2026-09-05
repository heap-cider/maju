use super::*;

#[test]
fn persona_content_hash_changes_on_acp_config_edit() {
    let key = crate::managed_agents::env_vars::ACP_CONFIG_OPTIONS_ENV;
    let mut before = sample_persona();
    before
        .env_vars
        .insert(key.to_string(), r#"{"thought_level":"low"}"#.to_string());
    let mut after = before.clone();
    after
        .env_vars
        .insert(key.to_string(), r#"{"thought_level":"high"}"#.to_string());

    assert_ne!(
        persona_content_hash(&persona_event_content(&before)),
        persona_content_hash(&persona_event_content(&after)),
        "a synchronized ACP selection must mark running instances for restart",
    );
}

/// `description` is public display metadata, deliberately excluded from
/// `persona_content_hash`: two contents differing only in description must
/// hash identically, so a description-only edit never flips the
/// "restart required" drift badge on linked instances.
#[test]
fn description_change_does_not_change_content_hash() {
    let without = PersonaEventContent {
        acp_config_options: None,
        description: None,
        display_name: "Test".to_string(),
        avatar_url: None,
        system_prompt: Some("Hello".to_string()),
        runtime: None,
        model: None,
        provider: None,
        name_pool: vec![],
        respond_to: None,
        respond_to_allowlist: Vec::new(),
        parallelism: None,
    };
    let mut with = without.clone();
    with.description = Some("A friendly test agent.".to_string());
    assert_eq!(
        persona_content_hash(&without),
        persona_content_hash(&with),
        "description must not participate in the content hash"
    );

    let mut edited = with.clone();
    edited.description = Some("A different description.".to_string());
    assert_eq!(
        persona_content_hash(&with),
        persona_content_hash(&edited),
        "description-only edits must not change the content hash"
    );
}
