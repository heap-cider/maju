use super::*;

#[test]
fn openrouter_credential_redaction_env_records_key() {
    let env = BTreeMap::from([(
        "OPENROUTER_API_KEY".to_string(),
        "sk-or-v1-secret-key-12345".to_string(),
    )]);
    let redaction =
        redaction_env_with_value(&env, "OPENROUTER_API_KEY", "sk-or-v1-secret-key-12345");
    assert_eq!(
        redaction.get("OPENROUTER_API_KEY").map(String::as_str),
        Some("sk-or-v1-secret-key-12345"),
        "redaction env must record the API key for error body redaction"
    );
}

#[test]
fn openrouter_saved_agent_model_discovery_resolves_provider() {
    let record: crate::managed_agents::ManagedAgentRecord = serde_json::from_str(
        r#"{
            "pubkey": "abcd1234",
            "name": "test-agent",
            "private_key_nsec": "nsec1fake",
            "relay_url": "wss://localhost:3000",
            "acp_command": "maju-acp",
            "agent_command": "maju-agent",
            "agent_command_override": "maju-agent",
            "agent_args": [],
            "mcp_command": "",
            "turn_timeout_seconds": 320,
            "system_prompt": null,
            "model": "anthropic/claude-sonnet-4",
            "provider": "openrouter",
            "env_vars": {
                "OPENROUTER_API_KEY": "sk-or-test-key",
                "MAJU_PRIVATE_KEY": "must-not-leak"
            },
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "last_started_at": null,
            "last_stopped_at": null,
            "last_exit_code": null,
            "last_error": null
        }"#,
    )
    .expect("sample openrouter managed agent record");

    let discovery = agent_model_discovery_config(
        &record,
        &[],
        &crate::managed_agents::GlobalAgentConfig::default(),
    )
    .expect("discovery config should resolve for an openrouter record");
    assert_eq!(discovery.provider.as_deref(), Some("openrouter"));
    assert_eq!(
        discovery.model.as_deref(),
        Some("anthropic/claude-sonnet-4")
    );
    assert_eq!(
        discovery.env.get("OPENROUTER_API_KEY").map(String::as_str),
        Some("sk-or-test-key")
    );
    assert!(!discovery.env.contains_key("MAJU_PRIVATE_KEY"));
}

/// B5/T4: unsaved-agent ("draft") discovery mirrors the saved-agent path —
/// `draft_agent_model_discovery_env` must derive the provider env var from
/// form input the same way `agent_model_discovery_config` derives it from a
/// persisted record's harness descriptor, and preserve caller-supplied env
/// (including the OpenRouter API key) unmodified.
#[test]
fn openrouter_draft_agent_model_discovery_derives_provider_env() {
    let env_vars = BTreeMap::from([(
        "OPENROUTER_API_KEY".to_string(),
        "sk-or-draft-key".to_string(),
    )]);

    let merged = draft_agent_model_discovery_env(
        "maju-agent",
        Some("openrouter"),
        &BTreeMap::new(),
        &env_vars,
    );

    assert_eq!(
        merged.get("MAJU_AGENT_PROVIDER").map(String::as_str),
        Some("openrouter"),
        "provider env var must be derived from form input for a known ACP runtime"
    );
    assert_eq!(
        merged.get("OPENROUTER_API_KEY").map(String::as_str),
        Some("sk-or-draft-key"),
        "caller-supplied env vars must survive the merge"
    );
}

#[test]
fn draft_agent_model_discovery_env_omits_provider_when_absent() {
    let merged =
        draft_agent_model_discovery_env("maju-agent", None, &BTreeMap::new(), &BTreeMap::new());
    assert!(
        !merged.contains_key("MAJU_AGENT_PROVIDER"),
        "no provider must be derived when the caller supplies none"
    );
}

/// The three-tier precedence this merge exists to preserve: main's inline
/// `derived → definition_env → env_vars` layering was folded into
/// `draft_agent_model_discovery_env`, so pin the order at every collision
/// boundary rather than trusting the two single-tier tests above.
///
/// `SHARED` collides across all three tiers, so the user value proves the
/// full chain; the pairwise keys prove each adjacent boundary independently
/// (a merge that dropped only the middle tier would still satisfy `SHARED`).
/// `MAJU_PRIVATE_KEY` proves a reserved key cannot ride in on a harness
/// definition, which is the tier a user never types.
#[test]
fn draft_agent_model_discovery_env_layers_all_three_tiers_in_order() {
    // Tier 2 (middle): harness definition env — overlays the runtime-derived
    // floor, loses to user env.
    let definition_env = BTreeMap::from([
        ("SHARED".to_string(), "from-definition".to_string()),
        // Collides with tier 1: `maju-agent`'s own provider env var, which the
        // `provider` argument derives below.
        ("MAJU_AGENT_PROVIDER".to_string(), "openai".to_string()),
        ("USER_OVER_DEF".to_string(), "from-definition".to_string()),
        ("DEFINITION_ONLY".to_string(), "from-definition".to_string()),
        // Reserved: must never reach the child, even from a definition.
        ("MAJU_PRIVATE_KEY".to_string(), "must-not-leak".to_string()),
    ]);
    // Tier 3 (top): user-entered env — wins over everything.
    let env_vars = BTreeMap::from([
        ("SHARED".to_string(), "from-user".to_string()),
        ("USER_OVER_DEF".to_string(), "from-user".to_string()),
        ("USER_ONLY".to_string(), "from-user".to_string()),
    ]);

    // Tier 1 (floor): `Some("openrouter")` derives MAJU_AGENT_PROVIDER.
    let merged = draft_agent_model_discovery_env(
        "maju-agent",
        Some("openrouter"),
        &definition_env,
        &env_vars,
    );

    let expected: &[(&str, Option<&str>)] = &[
        // Collides in all three tiers — the top tier wins.
        ("SHARED", Some("from-user")),
        // Tier 2 over tier 1: the definition's value survives, proving the
        // derived provider is the floor and not layered on top.
        ("MAJU_AGENT_PROVIDER", Some("openai")),
        // Tier 3 over tier 2.
        ("USER_OVER_DEF", Some("from-user")),
        // Single-tier keys pass through untouched.
        ("DEFINITION_ONLY", Some("from-definition")),
        ("USER_ONLY", Some("from-user")),
        // Reserved keys never survive the definition tier. Doubly enforced —
        // the explicit `is_reserved_env_key` filter here and `merged_user_env`'s
        // own `retain` — so this pins the contract, not either mechanism.
        ("MAJU_PRIVATE_KEY", None),
    ];
    for (key, want) in expected {
        assert_eq!(
            merged.get(*key).map(String::as_str),
            *want,
            "env key `{key}` must resolve to {want:?} after three-tier layering"
        );
    }
}

#[test]
fn databricks_static_token_error_redacts_echoed_token() {
    let token = "secret-databricks-token";
    let redaction_env = BTreeMap::from([("DATABRICKS_TOKEN".to_string(), token.to_string())]);

    let error = databricks_static_token_error(
        &format!("Databricks rejected bearer {token}"),
        &redaction_env,
    );

    assert!(error.contains("[REDACTED]"), "got: {error}");
    assert!(!error.contains(token), "token leaked in error: {error}");
    assert!(
        error.contains("update it in agent settings"),
        "error lost its remediation: {error}"
    );
}
