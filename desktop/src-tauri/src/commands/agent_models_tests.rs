use super::*;

#[test]
fn model_normalization_keeps_full_typed_config_options() {
    let raw = serde_json::json!({
        "agent": { "name": "generic-acp", "version": "1.0" },
        "stable": {
            "configOptions": [
                {
                    "id": "model",
                    "name": "Model",
                    "category": "model",
                    "type": "select",
                    "currentValue": "base",
                    "options": [{ "value": "base", "name": "Base" }]
                },
                {
                    "id": "fast-mode",
                    "name": "Fast mode",
                    "type": "boolean",
                    "currentValue": false
                }
            ]
        }
    });

    let response = normalize_agent_models(&raw, Some("base".to_string()));
    assert_eq!(response.models.len(), 1);
    assert_eq!(response.models[0].id, "base");
    assert_eq!(response.config_options.len(), 2);
    assert_eq!(
        response.config_options[1].current_value,
        Some(serde_json::json!(false))
    );
}

#[test]
fn access_policy_change_requires_runtime_refresh_for_effective_gate_changes() {
    use crate::managed_agents::RespondTo;

    let allowlist_a = vec!["a".repeat(64)];
    let allowlist_b = vec!["b".repeat(64)];

    assert!(managed_agent_access_policy_changed(
        RespondTo::Anyone,
        &[],
        RespondTo::OwnerOnly,
        &[],
        false,
    ));
    assert!(managed_agent_access_policy_changed(
        RespondTo::Allowlist,
        &allowlist_a,
        RespondTo::Allowlist,
        &allowlist_b,
        false,
    ));
    assert!(!managed_agent_access_policy_changed(
        RespondTo::OwnerOnly,
        &allowlist_a,
        RespondTo::OwnerOnly,
        &allowlist_b,
        false,
    ));
    assert!(!managed_agent_access_policy_changed(
        RespondTo::Anyone,
        &[],
        RespondTo::OwnerOnly,
        &[],
        true,
    ));
    assert!(!managed_agent_access_policy_changed(
        RespondTo::Allowlist,
        &allowlist_a,
        RespondTo::Allowlist,
        &allowlist_b,
        true,
    ));
}

#[test]
fn openai_model_normalization_keeps_agent_text_models() {
    let models = normalize_openai_compatible_models(
        OpenAiModelListResponse {
            data: vec![
                OpenAiModelListItem {
                    id: "text-embedding-3-large".to_string(),
                    created: Some(4),
                },
                OpenAiModelListItem {
                    id: "gpt-image-2".to_string(),
                    created: Some(5),
                },
                OpenAiModelListItem {
                    id: "chatgpt-5.5-pro-2026-04-23".to_string(),
                    created: Some(7),
                },
                OpenAiModelListItem {
                    id: "chatgpt-5.5-pro".to_string(),
                    created: Some(6),
                },
                OpenAiModelListItem {
                    id: "gpt-5.4-mini".to_string(),
                    created: Some(2),
                },
                OpenAiModelListItem {
                    id: "o4-mini".to_string(),
                    created: Some(3),
                },
                OpenAiModelListItem {
                    id: "gpt-5.4-mini".to_string(),
                    created: Some(1),
                },
            ],
        },
        Some("openai"),
    );

    let ids_and_names = models
        .into_iter()
        .map(|model| (model.id, model.name))
        .collect::<Vec<_>>();
    assert_eq!(
        ids_and_names,
        vec![
            (
                "chatgpt-5.5-pro".to_string(),
                Some("ChatGPT 5.5 Pro".to_string()),
            ),
            ("o4-mini".to_string(), Some("o4-mini".to_string())),
            ("gpt-5.4-mini".to_string(), Some("GPT-5.4 mini".to_string()),),
        ]
    );
}

#[test]
fn openai_compat_model_normalization_preserves_provider_specific_ids() {
    let models = normalize_openai_compatible_models(
        OpenAiModelListResponse {
            data: vec![
                OpenAiModelListItem {
                    id: "meta-llama/Llama-3.3-70B-Instruct".to_string(),
                    created: Some(5),
                },
                OpenAiModelListItem {
                    id: "mistral-large-latest".to_string(),
                    created: Some(4),
                },
                OpenAiModelListItem {
                    id: "anthropic/claude-sonnet-4-6".to_string(),
                    created: Some(3),
                },
                OpenAiModelListItem {
                    id: "text-embedding-compatible".to_string(),
                    created: Some(2),
                },
                OpenAiModelListItem {
                    id: "meta-llama/Llama-3.3-70B-Instruct".to_string(),
                    created: Some(1),
                },
            ],
        },
        Some("openai-compat"),
    );

    let ids = models.into_iter().map(|model| model.id).collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            "meta-llama/Llama-3.3-70B-Instruct".to_string(),
            "mistral-large-latest".to_string(),
            "anthropic/claude-sonnet-4-6".to_string(),
            "text-embedding-compatible".to_string(),
        ]
    );
}

#[test]
fn openai_models_url_uses_openai_default_base_url() {
    assert_eq!(
        openai_compatible_models_url(&BTreeMap::new()),
        "https://api.openai.com/v1/models"
    );
}

#[test]
fn anthropic_models_url_uses_anthropic_default_base_url() {
    assert_eq!(
        anthropic_models_url(&BTreeMap::new()),
        "https://api.anthropic.com/v1/models"
    );
}

#[test]
fn anthropic_models_url_accepts_versioned_base_url() {
    let env = BTreeMap::from([(
        "ANTHROPIC_BASE_URL".to_string(),
        "https://proxy.example/v1/".to_string(),
    )]);

    assert_eq!(
        anthropic_models_url(&env),
        "https://proxy.example/v1/models"
    );
}

#[test]
fn anthropic_model_normalization_uses_display_names() {
    let models = normalize_anthropic_models(AnthropicModelListResponse {
        data: vec![
            AnthropicModelListItem {
                id: "claude-opus-4-6".to_string(),
                display_name: Some("Claude Opus 4.6".to_string()),
            },
            AnthropicModelListItem {
                id: "claude-opus-4-6".to_string(),
                display_name: Some("Duplicate".to_string()),
            },
        ],
        has_more: false,
        last_id: None,
    });

    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "claude-opus-4-6");
    assert_eq!(models[0].name.as_deref(), Some("Claude Opus 4.6"));
}

#[test]
fn redaction_env_records_value_used_for_request() {
    let env = BTreeMap::from([("OPENAI_COMPAT_API_KEY".to_string(), "   ".to_string())]);

    let redaction_env =
        redaction_env_with_value(&env, "OPENAI_COMPAT_API_KEY", "inherited-process-key");

    assert_eq!(
        redaction_env
            .get("OPENAI_COMPAT_API_KEY")
            .map(String::as_str),
        Some("inherited-process-key")
    );
}

#[test]
fn saved_agent_model_discovery_uses_record_snapshot_for_definition_less_agent() {
    let record: crate::managed_agents::ManagedAgentRecord = serde_json::from_str(
        r#"{
            "pubkey": "abcd1234",
            "name": "test-agent",
            "private_key_nsec": "nsec1fake",
            "relay_url": "wss://localhost:3000",
            "acp_command": "maju-acp",
            "agent_command": "goose",
            "agent_command_override": "goose",
            "agent_args": [],
            "mcp_command": "",
            "turn_timeout_seconds": 320,
            "system_prompt": null,
            "model": "record-model",
            "provider": "databricks",
            "env_vars": {
                "OPENAI_API_KEY": "record-key",
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
    .expect("sample managed agent record");

    // agent_model_discovery_config is the single helper get_agent_models
    // consumes — verify it layers env correctly, strips reserved keys, and
    // keeps the record's own model/provider for a definition-less instance
    // (matching spawn's `resolve_definition_less` arm).
    let discovery = agent_model_discovery_config(&record, &[], &Default::default())
        .expect("discovery config should resolve for a valid record");

    assert_eq!(discovery.command.as_str(), "goose");
    assert_eq!(discovery.model.as_deref(), Some("record-model"));
    assert_eq!(discovery.provider.as_deref(), Some("databricks"));
    assert_eq!(
        discovery.env.get("GOOSE_MODEL").map(String::as_str),
        Some("record-model")
    );
    assert_eq!(
        discovery.env.get("GOOSE_PROVIDER").map(String::as_str),
        Some("databricks")
    );
    assert_eq!(
        discovery.env.get("OPENAI_API_KEY").map(String::as_str),
        Some("record-key")
    );
    // Reserved keys are stripped from the descriptor env.
    assert!(!discovery.env.contains_key("MAJU_PRIVATE_KEY"));
    // The provider env var is recovered from the runtime metadata for the
    // effective command (the old SavedAgentModelDiscoveryConfig.provider_env_var).
    assert_eq!(discovery.provider_env_var, Some("GOOSE_PROVIDER"));
}

// ---------------------------------------------------------------------------
// Provider resolution for discovery
// ---------------------------------------------------------------------------

#[test]
fn effective_discovery_provider_prefers_the_explicit_provider() {
    let env = BTreeMap::from([(
        "MAJU_AGENT_PROVIDER".to_string(),
        "databricks_v2".to_string(),
    )]);

    // A saved/selected provider is a deliberate choice and must win over the
    // build-provided default, so discovery matches what spawn will use.
    assert_eq!(
        effective_discovery_provider(Some("anthropic"), Some("MAJU_AGENT_PROVIDER"), &env)
            .as_deref(),
        Some("anthropic")
    );
}

#[test]
fn effective_discovery_provider_recovers_baked_provider_when_record_has_none() {
    let env = BTreeMap::from([(
        "MAJU_AGENT_PROVIDER".to_string(),
        "databricks_v2".to_string(),
    )]);

    // The regression this guards: records predating provider persistence carry
    // `provider: null`, so every discovery gate saw None and no live Databricks
    // catalog was ever fetched on builds that bake the provider in.
    for provider in [None, Some(""), Some("   ")] {
        assert_eq!(
            effective_discovery_provider(provider, Some("MAJU_AGENT_PROVIDER"), &env).as_deref(),
            Some("databricks_v2"),
            "provider input {provider:?} must fall back to the env value"
        );
    }
}

/// A provider env-var name no environment sets, so this test does not depend on
/// what the developer happens to have exported (e.g. `MAJU_AGENT_PROVIDER`).
const UNSET_PROVIDER_VAR: &str = "MAJU_TEST_UNSET_DISCOVERY_PROVIDER";

#[test]
fn effective_discovery_provider_is_none_without_an_explicit_or_env_provider() {
    let env = BTreeMap::new();
    assert_eq!(
        effective_discovery_provider(None, Some(UNSET_PROVIDER_VAR), &env).as_deref(),
        None
    );
    // A runtime that takes no provider env var has nothing to recover from.
    assert_eq!(
        effective_discovery_provider(
            None,
            None,
            &BTreeMap::from([(UNSET_PROVIDER_VAR.to_string(), "databricks_v2".to_string())])
        )
        .as_deref(),
        None
    );
}

/// A credential name no environment sets, so `required_env` is exercised without
/// depending on what the developer happens to have exported.
const UNSET_CREDENTIAL: &str = "MAJU_TEST_UNSET_DISCOVERY_CREDENTIAL";

#[test]
fn env_derived_provider_falls_through_when_its_credential_is_missing() {
    let env = BTreeMap::from([("GOOSE_PROVIDER".to_string(), "anthropic".to_string())]);
    let inferred = effective_discovery_provider(None, Some("GOOSE_PROVIDER"), &env);
    assert_eq!(inferred.as_deref(), Some("anthropic"));

    // `export GOOSE_PROVIDER=anthropic` is goose's documented way to pick a
    // provider, and it keeps the API key in its own config/keyring rather than in
    // Maju's env — so the provider is visible here and the credential is not.
    // Erroring would swap the working subprocess catalog for a hard
    // "config: ... required" on exactly the null-provider records this fallback
    // exists to serve; the gate has to decline instead.
    assert_eq!(inferred.required_env(&env, UNSET_CREDENTIAL), Ok(None));
}

#[test]
fn explicit_provider_still_reports_a_missing_credential() {
    // An explicit provider is an assertion about this agent, so a missing
    // credential is a real misconfiguration and stays user-visible.
    let env = BTreeMap::new();
    let explicit = effective_discovery_provider(Some("anthropic"), Some("GOOSE_PROVIDER"), &env);
    assert_eq!(
        explicit.required_env(&env, UNSET_CREDENTIAL),
        Err(format!("config: {UNSET_CREDENTIAL} required"))
    );
}

#[test]
fn required_env_returns_a_configured_credential_however_the_provider_was_resolved() {
    let env = BTreeMap::from([
        ("GOOSE_PROVIDER".to_string(), "anthropic".to_string()),
        (
            UNSET_CREDENTIAL.to_string(),
            "  sk-configured  ".to_string(),
        ),
    ]);
    for provider in [Some("anthropic"), None] {
        let resolved = effective_discovery_provider(provider, Some("GOOSE_PROVIDER"), &env);
        assert_eq!(
            resolved.required_env(&env, UNSET_CREDENTIAL),
            Ok(Some("sk-configured".to_string())),
            "provider input {provider:?} must read the configured credential"
        );
    }
}

#[test]
fn effective_discovery_provider_reads_the_runtimes_own_env_var() {
    // goose keys its provider off GOOSE_PROVIDER, so a MAJU_AGENT_PROVIDER in
    // the env must not be mistaken for this runtime's provider.
    let env = BTreeMap::from([
        ("GOOSE_PROVIDER".to_string(), "databricks".to_string()),
        (
            "MAJU_AGENT_PROVIDER".to_string(),
            "databricks_v2".to_string(),
        ),
    ]);
    assert_eq!(
        effective_discovery_provider(None, Some("GOOSE_PROVIDER"), &env).as_deref(),
        Some("databricks")
    );
}

/// Definition-authoritative: a linked agent's stale materialized
/// `record.model`/`record.provider` must never drive model discovery — the
/// linked definition's current model/provider wins, mirroring spawn's
/// `resolve_effective_model_provider`.
#[test]
fn model_discovery_ignores_stale_record_for_linked_agent() {
    let record: crate::managed_agents::ManagedAgentRecord = serde_json::from_str(
        r#"{
            "pubkey": "abcd1234",
            "name": "test-agent",
            "persona_id": "persona-1",
            "private_key_nsec": "nsec1fake",
            "relay_url": "wss://localhost:3000",
            "acp_command": "maju-acp",
            "agent_command": "goose",
            "agent_args": [],
            "mcp_command": "",
            "turn_timeout_seconds": 320,
            "system_prompt": null,
            "model": "stale-record-model",
            "provider": "stale-record-provider",
            "env_vars": {},
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "last_started_at": null,
            "last_stopped_at": null,
            "last_exit_code": null,
            "last_error": null
        }"#,
    )
    .expect("sample managed agent record");

    let persona: crate::managed_agents::AgentDefinition = serde_json::from_str(
        r#"{
            "id": "persona-1",
            "display_name": "Persona",
            "system_prompt": "You are a persona.",
            "runtime": "goose",
            "model": "persona-model",
            "provider": "anthropic",
            "is_active": true,
            "created_at": "",
            "updated_at": ""
        }"#,
    )
    .expect("sample persona");

    // agent_model_discovery_config is the single helper get_agent_models
    // consumes — the stale record bytes must lose to the persona's current
    // model/provider (the same authoritative resolver spawn uses).
    let personas = [persona];
    let global = crate::managed_agents::GlobalAgentConfig::default();
    let discovery = agent_model_discovery_config(&record, &personas, &global)
        .expect("discovery config should resolve for a linked record");
    assert_eq!(discovery.model.as_deref(), Some("persona-model"));
    assert_eq!(discovery.provider.as_deref(), Some("anthropic"));

    // And the discovery env comes from the descriptor, whose layering also
    // resolves through the definition — the derived model env var must carry
    // the persona's model, not the stale record snapshot.
    assert_eq!(
        discovery.env.get("GOOSE_MODEL").map(String::as_str),
        Some("persona-model")
    );
    assert_eq!(
        discovery.env.get("GOOSE_PROVIDER").map(String::as_str),
        Some("anthropic")
    );
}

// ---------------------------------------------------------------------------
// Databricks provider detection

#[test]
fn merged_filter_value_overrides_inherited_process_value_even_when_blank() {
    let env = BTreeMap::from([("DATABRICKS_MODEL_FILTER".to_string(), "   ".to_string())]);
    assert_eq!(
        env_value_or_process_if_absent(&env, "DATABRICKS_MODEL_FILTER"),
        Some(String::new())
    );
}

#[test]
fn absent_filter_value_uses_process_value_when_available() {
    const TEST_FILTER_ENV: &str = "MAJU_TEST_DATABRICKS_MODEL_FILTER";
    let original = std::env::var(TEST_FILTER_ENV).ok();
    std::env::set_var(TEST_FILTER_ENV, "process-*");
    let value = env_value_or_process_if_absent(&BTreeMap::new(), TEST_FILTER_ENV);
    match original {
        Some(value) => std::env::set_var(TEST_FILTER_ENV, value),
        None => std::env::remove_var(TEST_FILTER_ENV),
    }
    assert_eq!(value.as_deref(), Some("process-*"));
}

#[test]
fn databricks_filtered_empty_response_is_authoritative() {
    let filter = maju_agent_pkg::config::DatabricksModelFilter::parse(Some("allowed-*")).unwrap();
    let response = databricks_models_response(
        "databricks_v2",
        Vec::new(),
        Some("configured".into()),
        filter.as_ref(),
    )
    .expect("active filter permits an empty authoritative catalog");
    assert!(response.models.is_empty());
    assert!(!response.supports_switching);
    assert_eq!(response.selected_model.as_deref(), Some("configured"));
}

// Parse/filter/pagination tests live in crates/maju-agent/src/catalog.rs
// (they moved there with the Option C refactor).
// ---------------------------------------------------------------------------
// Dead-knob guards: mcp_command and turn_timeout_seconds
// ---------------------------------------------------------------------------

#[test]
fn update_request_mcp_command_parses_for_wire_compat() {
    // UpdateManagedAgentRequest accepts mcpCommand for backward-compatibility
    // with frontends that still send it: the deprecated field must keep
    // parsing cleanly. Nothing consumes it — the patching loop in
    // update_managed_agent has no mcp_command arm (the effective MCP command
    // is always catalog-derived at spawn). That absent-arm invariant lives in
    // the code, not in this test: it only guards the wire shape.
    let req: crate::managed_agents::UpdateManagedAgentRequest =
        serde_json::from_str(r#"{"pubkey": "abc", "mcpCommand": "user-override"}"#)
            .expect("request with deprecated mcpCommand parses");
    assert_eq!(req.mcp_command.as_deref(), Some("user-override"));
}

#[test]
fn update_request_turn_timeout_parses_for_wire_compat() {
    // UpdateManagedAgentRequest accepts turnTimeoutSeconds for
    // backward-compatibility with frontends that still send it: the deprecated
    // field must keep parsing cleanly. Nothing consumes it — the patching loop
    // in update_managed_agent has no turn_timeout_seconds arm
    // (MAJU_ACP_TURN_TIMEOUT is deprecated and ignored by the harness). That
    // absent-arm invariant lives in the code, not in this test: it only
    // guards the wire shape.
    let req: crate::managed_agents::UpdateManagedAgentRequest =
        serde_json::from_str(r#"{"pubkey": "abc", "turnTimeoutSeconds": 9999}"#)
            .expect("request with deprecated turnTimeoutSeconds parses");
    assert_eq!(req.turn_timeout_seconds, Some(9999));
}

// ---------------------------------------------------------------------------
// Linked-instance write guard (model/provider/prompt)
// ---------------------------------------------------------------------------

#[test]
fn linked_instance_ignores_model_provider_prompt_writes() {
    let mut record: crate::managed_agents::ManagedAgentRecord = serde_json::from_str(
        r#"{
            "pubkey": "linked1",
            "name": "linked-agent",
            "persona_id": "p1",
            "private_key_nsec": "nsec1fake",
            "relay_url": "wss://localhost:3000",
            "acp_command": "maju-acp",
            "agent_command": "goose",
            "agent_args": [],
            "mcp_command": "",
            "turn_timeout_seconds": 320,
            "system_prompt": null,
            "model": null,
            "provider": null,
            "env_vars": {},
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "last_started_at": null,
            "last_stopped_at": null,
            "last_exit_code": null,
            "last_error": null
        }"#,
    )
    .expect("linked agent record");

    let is_linked = record.persona_id.is_some();
    assert!(is_linked, "test setup: record must be linked");

    crate::commands::agent_models::apply_model_provider_prompt_update(
        &mut record,
        Some(Some("explicit-model".to_string())),
        Some(Some("explicit-prov".to_string())),
        Some(Some("explicit-prompt".to_string())),
    )
    .unwrap();

    assert!(
        record.model.is_none(),
        "linked record model must not be updated"
    );
    assert!(
        record.provider.is_none(),
        "linked record provider must not be updated"
    );
    assert!(
        record.system_prompt.is_none(),
        "linked record system_prompt must not be updated"
    );
}

#[test]
fn definition_less_instance_accepts_model_provider_prompt_writes() {
    let mut record: crate::managed_agents::ManagedAgentRecord = serde_json::from_str(
        r#"{
            "pubkey": "standalone1",
            "name": "standalone-agent",
            "private_key_nsec": "nsec1fake",
            "relay_url": "wss://localhost:3000",
            "acp_command": "maju-acp",
            "agent_command": "goose",
            "agent_args": [],
            "mcp_command": "",
            "turn_timeout_seconds": 320,
            "system_prompt": null,
            "model": null,
            "provider": null,
            "env_vars": {},
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "last_started_at": null,
            "last_stopped_at": null,
            "last_exit_code": null,
            "last_error": null
        }"#,
    )
    .expect("standalone agent record");

    let is_linked = record.persona_id.is_some();
    assert!(!is_linked, "test setup: record must not be linked");

    crate::commands::agent_models::apply_model_provider_prompt_update(
        &mut record,
        Some(Some("new-model".to_string())),
        Some(Some("new-prov".to_string())),
        Some(Some("new-prompt".to_string())),
    )
    .unwrap();

    assert_eq!(record.model.as_deref(), Some("new-model"));
    assert_eq!(record.provider.as_deref(), Some("new-prov"));
    assert_eq!(record.system_prompt.as_deref(), Some("new-prompt"));
}

#[test]
fn is_databricks_provider_matches_both_variants() {
    assert!(is_databricks_provider(Some("databricks")));
    assert!(is_databricks_provider(Some("databricks_v2")));
    assert!(is_databricks_provider(Some("  DATABRICKS  ")));
    assert!(!is_databricks_provider(Some("anthropic")));
    assert!(!is_databricks_provider(None));
}

#[test]
fn databricks_interactive_auth_launches_only_without_a_static_token() {
    // Phase 2: both surfaces launch the browser flow when the token is empty;
    // the surface distinction is now cooldown-only (asserted separately). A
    // configured static token still short-circuits interactive auth entirely.
    assert!(should_start_interactive_auth(""));
    assert!(!should_start_interactive_auth("static-token"));
}

#[test]
fn databricks_passive_auth_error_has_reachable_create_flow_guidance() {
    let error = databricks_sign_in_required_error();
    assert!(error.contains("save this agent, then open its model picker"));
    assert!(error.contains("maju-agent auth databricks"));
}

#[test]
fn model_discovery_error_converts_dangling_sentinel_to_sentence() {
    // get_agent_models is a user-facing surface: a dangling harness must
    // render as a sentence, never as the raw DANGLING_HARNESS_ID: sentinel.
    let raw = format!("{}doomed", crate::managed_agents::DANGLING_HARNESS_PREFIX);
    let msg = model_discovery_error("agent-pk", &raw);
    assert!(msg.contains("cannot discover models for agent-pk"));
    assert!(msg.contains("\"doomed\"") && msg.contains("deleted"));
    assert!(!msg.contains(crate::managed_agents::DANGLING_HARNESS_PREFIX));

    // Non-dangling errors pass through untouched.
    let plain = model_discovery_error("agent-pk", "plain failure");
    assert_eq!(plain, "cannot discover models for agent-pk: plain failure");
}

// ---------------------------------------------------------------------------
// OpenRouter provider
// ---------------------------------------------------------------------------

#[test]
fn is_openrouter_provider_matches() {
    assert!(is_openrouter_provider(Some("openrouter")));
    assert!(is_openrouter_provider(Some("  OpenRouter  ")));
    assert!(!is_openrouter_provider(Some("openai")));
    assert!(!is_openrouter_provider(Some("anthropic")));
    assert!(!is_openrouter_provider(None));
}

#[test]
fn openrouter_models_url_uses_default_base_url() {
    assert_eq!(
        openrouter_models_url(&BTreeMap::new()),
        "https://openrouter.ai/api/v1/models"
    );
}

#[test]
fn openrouter_models_url_respects_custom_base_url() {
    let env = BTreeMap::from([(
        "OPENROUTER_BASE_URL".to_string(),
        "https://eu.openrouter.ai/api/v1".to_string(),
    )]);
    assert_eq!(
        openrouter_models_url(&env),
        "https://eu.openrouter.ai/api/v1/models"
    );
}

#[test]
fn openrouter_models_url_strips_trailing_slash() {
    let env = BTreeMap::from([(
        "OPENROUTER_BASE_URL".to_string(),
        "https://proxy.example.com/api/v1/".to_string(),
    )]);
    assert_eq!(
        openrouter_models_url(&env),
        "https://proxy.example.com/api/v1/models"
    );
}

#[test]
fn openrouter_filter_keeps_tools_capable_models() {
    let response = OpenRouterModelListResponse {
        data: vec![
            OpenRouterModelListItem {
                id: "anthropic/claude-opus-4-7".to_string(),
                supported_parameters: vec!["tools".to_string(), "reasoning".to_string()],
            },
            OpenRouterModelListItem {
                id: "openai/gpt-5.5-pro".to_string(),
                supported_parameters: vec!["tools".to_string()],
            },
            OpenRouterModelListItem {
                id: "meta-llama/llama-no-tools".to_string(),
                supported_parameters: vec!["temperature".to_string()],
            },
        ],
    };
    let result = filter_openrouter_models(response, None).unwrap().unwrap();
    let ids: Vec<_> = result.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["anthropic/claude-opus-4-7", "openai/gpt-5.5-pro"]);
}

#[test]
fn openrouter_filter_excludes_absent_supported_parameters() {
    let response: OpenRouterModelListResponse =
        serde_json::from_str(r#"{"data": [{"id": "model-no-params"}]}"#).unwrap();
    assert!(
        response.data[0].supported_parameters.is_empty(),
        "absent supported_parameters must default to empty vec"
    );
    let result = filter_openrouter_models(response, None);
    assert!(
        result.is_err(),
        "models with no supported_parameters must be excluded"
    );
    assert!(
        result.unwrap_err().contains("no tools-capable models"),
        "error must indicate no tools-capable models"
    );
}

#[test]
fn openrouter_filter_excludes_empty_supported_parameters() {
    let response = OpenRouterModelListResponse {
        data: vec![OpenRouterModelListItem {
            id: "model-empty-params".to_string(),
            supported_parameters: Vec::new(),
        }],
    };
    let result = filter_openrouter_models(response, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("no tools-capable models"));
}

#[test]
fn openrouter_filter_empty_result_returns_error() {
    let response = OpenRouterModelListResponse { data: Vec::new() };
    let result = filter_openrouter_models(response, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("no tools-capable models"));
}

#[test]
fn openrouter_filter_preserves_selected_model() {
    let response = OpenRouterModelListResponse {
        data: vec![OpenRouterModelListItem {
            id: "openai/gpt-5.5-pro".to_string(),
            supported_parameters: vec!["tools".to_string()],
        }],
    };
    let result = filter_openrouter_models(response, Some("openai/gpt-5.5-pro".to_string()))
        .unwrap()
        .unwrap();
    assert_eq!(result.selected_model.as_deref(), Some("openai/gpt-5.5-pro"));
}

#[path = "agent_models_tests/provider_discovery.rs"]
mod provider_discovery;
