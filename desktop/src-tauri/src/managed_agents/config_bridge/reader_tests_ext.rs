//! Additional tests for `config_bridge/reader.rs` — split out to keep
//! `reader_tests.rs` under the 1500-line file-size ratchet.
//!
//! Included as `mod ext` inside `reader_tests.rs`, so `use super::*` gives
//! access to all helpers and types from that module.

use super::*;

// ── Numerics inheritance tests ────────────────────────────────────────────────
//
// max_output_tokens and context_limit gain persona/global tiers.

#[test]
fn numeric_context_limit_inherits_from_persona_env() {
    let record = test_record();
    let runtime = maju_agent_runtime();
    let tiers = persona_env_tiers("MAJU_AGENT_MAX_CONTEXT_TOKENS", "200000");

    let surface = read_config_surface(&record, Some(runtime), None, &tiers, None);

    let field = surface.normalized.context_limit.unwrap();
    assert_eq!(field.value.as_deref(), Some("200000"));
    assert_eq!(field.origin, ConfigOrigin::PersonaDefault);
}

#[test]
fn record_max_tokens_overrides_global_env_with_secondary() {
    let mut record = test_record();
    record.env_vars.insert(
        "MAJU_AGENT_MAX_OUTPUT_TOKENS".to_string(),
        "8192".to_string(),
    );
    let runtime = maju_agent_runtime();
    let tiers = global_env_tiers("MAJU_AGENT_MAX_OUTPUT_TOKENS", "16384");

    let surface = read_config_surface(&record, Some(runtime), None, &tiers, None);

    let field = surface.normalized.max_output_tokens.unwrap();
    assert_eq!(field.value.as_deref(), Some("8192"));
    assert_eq!(field.origin, ConfigOrigin::MajuExplicit);
    // Global value is the overridden secondary.
    assert_eq!(field.overridden_value.as_deref(), Some("16384"));
    assert_eq!(field.overridden_origin, Some(ConfigOrigin::GlobalDefault));
}

// ── Env-vs-structured collision tests (plan v3, Phase 2) ─────────────────────

/// Collision test 1: persona structured prompt + global env MAJU_ACP_SYSTEM_PROMPT
/// → global env wins (env block sits entirely above structured).
#[test]
fn global_env_prompt_wins_over_persona_structured_prompt() {
    let record = test_record();
    let runtime = test_runtime();
    let tiers = InheritedConfigTiers {
        global_env: {
            let mut m = BTreeMap::new();
            m.insert(
                "MAJU_ACP_SYSTEM_PROMPT".to_string(),
                "global-env-prompt".to_string(),
            );
            m
        },
        persona_prompt: Some("persona-structured-prompt".to_string()),
        ..Default::default()
    };

    let surface = read_config_surface(&record, Some(runtime), None, &tiers, None);

    let prompt = surface.normalized.system_prompt.unwrap();
    assert_eq!(prompt.value.as_deref(), Some("global-env-prompt"));
    assert_eq!(prompt.origin, ConfigOrigin::GlobalDefault);
}

/// Collision test 2: structured persona/record model + higher user-env value at
/// the runtime's model key → env value wins.
#[test]
fn persona_env_model_wins_over_persona_structured_model() {
    let record = test_record(); // no record.model
    let runtime = test_runtime(); // GOOSE_MODEL
    let tiers = InheritedConfigTiers {
        persona_env: {
            let mut m = BTreeMap::new();
            m.insert("GOOSE_MODEL".to_string(), "env-model".to_string());
            m
        },
        persona_model: Some("struct-persona-model".to_string()),
        ..Default::default()
    };

    let surface = read_config_surface(&record, Some(runtime), None, &tiers, None);

    let model = surface.normalized.model.unwrap();
    // persona env outranks persona struct because env candidates precede struct
    assert_eq!(model.value.as_deref(), Some("env-model"));
    assert_eq!(model.origin, ConfigOrigin::PersonaDefault);
}

/// Collision test 3: no env representation → structured persona/record/global
/// fallback and provenance remain intact.
#[test]
fn structured_fallback_intact_when_no_env_representation() {
    let record = test_record(); // no record.model, no env vars
    let runtime = test_runtime();
    let tiers = InheritedConfigTiers {
        persona_model: Some("struct-persona-model".to_string()),
        ..Default::default()
    };

    let surface = read_config_surface(&record, Some(runtime), None, &tiers, None);

    let model = surface.normalized.model.unwrap();
    assert_eq!(model.value.as_deref(), Some("struct-persona-model"));
    assert_eq!(model.origin, ConfigOrigin::PersonaDefault);
}

// ── Post-sanitization fallthrough test ───────────────────────────────────────
//
// Sanitization itself happens at the command boundary in `build_inherited_tiers`
// (a value with a NUL byte or an oversize value is dropped from the tier) and is
// pinned by the tests in `commands/agent_config_tests.rs`. The reader only ever
// sees the sanitized result, so what it must guarantee is the downstream half:
// a key stripped from one tier falls through to the next.

/// A key absent from the global env tier — the shape the reader sees after the
/// command boundary strips an invalid value — falls through to the persona tier.
#[test]
fn post_sanitization_empty_global_env_falls_through_to_persona_tier() {
    let record = test_record();
    let runtime = maju_agent_rt();
    // No global env (stripped); persona provides the valid fallback.
    let tiers = persona_env_tiers("MAJU_AGENT_THINKING_EFFORT", "medium");

    let surface = read_config_surface(&record, Some(runtime), None, &tiers, None);

    // Persona value surfaces instead of the stripped global value.
    let effort = surface.normalized.thinking_effort.unwrap();
    assert_eq!(effort.value.as_deref(), Some("medium"));
    assert_eq!(effort.origin, ConfigOrigin::PersonaDefault);
}

// ── Pass-3 prompt collision test ─────────────────────────────────────────────
//
// From Thufir's pass-3 verdict MINOR clarification (promoted to required):
// definition-less record with both structured and env prompt — env wins.

/// Pass-3 clarification: record.system_prompt = A + record env
/// MAJU_ACP_SYSTEM_PROMPT = B → B wins as MajuExplicit.
/// The env block sits above the struct block per v3 candidate-preparation
/// contract; current reader semantics (struct before env) would be wrong.
#[test]
fn record_env_prompt_wins_over_record_struct_prompt_as_maju_explicit() {
    let mut record = test_record();
    record.system_prompt = Some("struct-prompt-A".to_string());
    record.env_vars.insert(
        "MAJU_ACP_SYSTEM_PROMPT".to_string(),
        "env-prompt-B".to_string(),
    );
    let runtime = test_runtime();

    let surface = read_config_surface(&record, Some(runtime), None, &no_tiers(), None);

    let prompt = surface.normalized.system_prompt.unwrap();
    assert_eq!(prompt.value.as_deref(), Some("env-prompt-B"));
    assert_eq!(prompt.origin, ConfigOrigin::MajuExplicit);
    // Struct prompt is the secondary.
    assert_eq!(prompt.overridden_value.as_deref(), Some("struct-prompt-A"));
    assert_eq!(prompt.overridden_origin, Some(ConfigOrigin::MajuExplicit));
}

// ── Definition env tier tests (Layer 2b) ─────────────────────────────────────
//
// The harness definition's `env` block sits below global env and above
// structured values in spawn's precedence (Layer 2b). These tests exercise
// the reader's mapping of that tier to `HarnessDefault` origin.

/// Definition env wins over structured persona model when no user-env or
/// global-env candidate is present.
#[test]
fn definition_env_beats_structured_persona_model() {
    let record = test_record(); // no record.model, no record.env_vars
    let runtime = test_runtime(); // model_env_var = "GOOSE_MODEL"
    let tiers = InheritedConfigTiers {
        definition_env: {
            let mut m = BTreeMap::new();
            m.insert("GOOSE_MODEL".to_string(), "harness-model".to_string());
            m
        },
        persona_model: Some("persona-struct-model".to_string()),
        ..Default::default()
    };

    let surface = read_config_surface(&record, Some(runtime), None, &tiers, None);

    let model = surface.normalized.model.unwrap();
    assert_eq!(model.value.as_deref(), Some("harness-model"));
    assert_eq!(model.origin, ConfigOrigin::HarnessDefault);
    // Structured persona model is the overridden secondary.
    assert_eq!(
        model.overridden_value.as_deref(),
        Some("persona-struct-model")
    );
    assert_eq!(model.overridden_origin, Some(ConfigOrigin::PersonaDefault));
}

/// Global env beats definition env — user-settable tiers always win over the
/// harness author's defaults.
#[test]
fn global_env_beats_definition_env() {
    let record = test_record();
    let runtime = test_runtime(); // model_env_var = "GOOSE_MODEL"
    let tiers = InheritedConfigTiers {
        global_env: {
            let mut m = BTreeMap::new();
            m.insert("GOOSE_MODEL".to_string(), "global-model".to_string());
            m
        },
        definition_env: {
            let mut m = BTreeMap::new();
            m.insert("GOOSE_MODEL".to_string(), "harness-model".to_string());
            m
        },
        ..Default::default()
    };

    let surface = read_config_surface(&record, Some(runtime), None, &tiers, None);

    let model = surface.normalized.model.unwrap();
    assert_eq!(model.value.as_deref(), Some("global-model"));
    assert_eq!(model.origin, ConfigOrigin::GlobalDefault);
    // Harness default is the overridden secondary.
    assert_eq!(model.overridden_value.as_deref(), Some("harness-model"));
    assert_eq!(model.overridden_origin, Some(ConfigOrigin::HarnessDefault));
}

/// A reserved key in the definition env is stripped by sanitization and must
/// not reach the reader. This test exercises the reader's contract (a key
/// absent from the tier falls through) — sanitization itself is pinned in
/// the `agent_config_tests.rs` constructor tests.
#[test]
fn reserved_key_absent_from_definition_env_falls_through() {
    let record = test_record();
    let runtime = test_runtime(); // model_env_var = "GOOSE_MODEL"
                                  // definition_env contains only an unrelated key — the env map here is what
                                  // the command boundary would produce after stripping a reserved key; the
                                  // reader must fall through to the next tier (persona structured model).
    let tiers = InheritedConfigTiers {
        definition_env: BTreeMap::new(), // stripped — nothing survives
        persona_model: Some("persona-struct-model".to_string()),
        ..Default::default()
    };

    let surface = read_config_surface(&record, Some(runtime), None, &tiers, None);

    let model = surface.normalized.model.unwrap();
    // Falls through to persona structured model.
    assert_eq!(model.value.as_deref(), Some("persona-struct-model"));
    assert_eq!(model.origin, ConfigOrigin::PersonaDefault);
}

// ── CLAUDE_CONFIG_DIR path resolution (#3493) ─────────────────────────────────

#[test]
fn claude_mcp_config_path_honors_custom_claude_config_dir() {
    // #3493: mcp_config_file_path_for_runtime must use the custom dir when
    // claude_config_dir is Some, not fall back to ~/.claude.json.
    let record = test_record();
    let runtime = &KnownAcpRuntime {
        id: "claude",
        config_file_path: Some("~/.claude/settings.json"),
        ..*test_runtime()
    };
    let custom_dir = std::path::PathBuf::from("/custom/config/dir");
    let surface = read_config_surface(&record, Some(runtime), None, &no_tiers(), Some(&custom_dir));

    let mcp_path = surface
        .sources
        .mcp_config_file_path
        .expect("mcp_config_file_path must be present for claude runtime");
    assert_eq!(
        std::path::Path::new(&mcp_path),
        custom_dir.join(".claude.json"),
        "mcp config path must be <custom_dir>/.claude.json when CLAUDE_CONFIG_DIR is set"
    );
    assert!(
        surface.claude_config_dir_custom,
        "claude_config_dir_custom must be true when a custom dir was passed"
    );
}

#[test]
fn claude_config_dir_none_falls_back_to_home_claude_json() {
    // #3493: None (i.e. the caller stripped an empty string) must resolve to
    // the default ~/.claude.json path, matching Claude's `CLAUDE_CONFIG_DIR || homedir()`.
    let record = test_record();
    let runtime = &KnownAcpRuntime {
        id: "claude",
        config_file_path: Some("~/.claude/settings.json"),
        ..*test_runtime()
    };
    let surface = read_config_surface(&record, Some(runtime), None, &no_tiers(), None);
    assert!(
        !surface.claude_config_dir_custom,
        "claude_config_dir_custom must be false when dir is None (unset)"
    );
    assert!(
        surface
            .sources
            .mcp_config_file_path
            .as_deref()
            .is_some_and(|p| p.ends_with(".claude.json")),
        "mcp path must fall back to ~/.claude.json when no custom dir"
    );
}

// ── #3493: config_file_path follows a custom CLAUDE_CONFIG_DIR ─────────────────

#[test]
fn claude_custom_config_dir_reports_isolated_settings_path() {
    let record = test_record();
    let runtime = &KnownAcpRuntime {
        id: "claude",
        config_file_path: Some("~/.claude/settings.json"),
        ..*test_runtime()
    };
    let custom = std::path::Path::new("/tmp/iso-config");

    let surface = read_config_surface(&record, Some(runtime), None, &no_tiers(), Some(custom));

    // The reported settings path is rooted at the custom dir the reader used,
    // not the static ~/.claude/settings.json metadata. Compare as paths so the
    // separator is native (Windows joins with `\`, not `/`).
    assert_eq!(
        surface
            .sources
            .config_file_path
            .as_deref()
            .map(std::path::Path::new),
        Some(custom.join("settings.json").as_path()),
    );
    // And the MCP file attribution follows the same custom root.
    assert_eq!(
        surface
            .sources
            .mcp_config_file_path
            .as_deref()
            .map(std::path::Path::new),
        Some(custom.join(".claude.json").as_path()),
    );
}

#[test]
fn claude_default_config_dir_reports_static_settings_path() {
    let record = test_record();
    let runtime = &KnownAcpRuntime {
        id: "claude",
        config_file_path: Some("~/.claude/settings.json"),
        ..*test_runtime()
    };

    let surface = read_config_surface(&record, Some(runtime), None, &no_tiers(), None);

    // With no custom dir, the settings path resolves the static tilde metadata.
    // Compare the trailing components as a path so the check is separator-native.
    assert!(surface
        .sources
        .config_file_path
        .as_deref()
        .map(std::path::Path::new)
        .is_some_and(|p| p.ends_with(".claude/settings.json")));
    assert!(surface
        .sources
        .config_file_path
        .as_deref()
        .is_some_and(|p| !p.starts_with('~')));
}

// ── Goose-contract reader normalization + reader/projection parity ────────────
//
// The reader (`build_thinking_field`) and the launch projection
// (`effort_launch_projection`) must resolve one effective value AND one
// authority for every record/inherited input, or the config panel displays a
// different effort than the next spawn launches. `test_runtime()` is Goose with
// `effort_normalization = GOOSE_EFFORT_NORMALIZATION`, so these exercise the
// normalization gate, alias canonicalization, invalid-value skip/fallthrough,
// and the decisive mixed-authority case — the phase-1 behavior block, not just
// fixture metadata.

use crate::managed_agents::config_bridge::effort::effort_launch_projection;

/// Drive the projection from the SAME record + global env the reader sees, so
/// the two resolvers are compared on identical inputs. Persona/definition tiers
/// use distinct input shapes across the two layers and are covered separately;
/// record-native/legacy_alias/legacy and global are expressible identically here,
/// which is exactly where the authority-order contract is decisive.
fn projection_value(
    record: &ManagedAgentRecord,
    global_env: &BTreeMap<String, String>,
) -> Option<String> {
    effort_launch_projection(
        record,
        Some(test_runtime()),
        &[],
        None,
        global_env,
        None,
        &BTreeMap::new(),
    )
    .value
}

/// Goose invalid record-native value (`minimal` — not in the Goose contract)
/// skips as absent so a valid lower tier wins, IDENTICALLY in reader and
/// projection. This is Thufir's named regression: a raw winner in the panel
/// while the launch skips it.
#[test]
fn goose_invalid_record_native_skips_to_legacy_alias_in_reader_and_projection() {
    let mut record = test_record();
    record
        .env_vars
        .insert("GOOSE_THINKING_EFFORT".to_string(), "minimal".to_string());
    record
        .env_vars
        .insert("MAJU_AGENT_THINKING_EFFORT".into(), "high".to_string());
    let runtime = test_runtime();

    let surface = read_config_surface(&record, Some(runtime), None, &no_tiers(), None);
    let effort = surface
        .normalized
        .thinking_effort
        .expect("valid legacy_alias must win when native is invalid");
    // Reader: invalid native skipped, legacy_alias wins.
    assert_eq!(effort.value.as_deref(), Some("high"));
    assert_eq!(effort.origin, ConfigOrigin::MajuExplicit);
    // Projection agrees on value.
    assert_eq!(
        projection_value(&record, &BTreeMap::new()).as_deref(),
        Some("high")
    );
}

/// Goose alias canonicalization: `xhigh` → `max` in BOTH resolvers (record
/// native), `none` → `off` (legacy_alias).
#[test]
fn goose_aliases_canonicalize_in_reader_and_projection() {
    let mut record = test_record();
    record
        .env_vars
        .insert("GOOSE_THINKING_EFFORT".to_string(), "xhigh".to_string());
    let runtime = test_runtime();

    let surface = read_config_surface(&record, Some(runtime), None, &no_tiers(), None);
    let effort = surface.normalized.thinking_effort.unwrap();
    assert_eq!(effort.value.as_deref(), Some("max"));
    assert_eq!(effort.origin, ConfigOrigin::MajuExplicit);
    assert_eq!(
        projection_value(&record, &BTreeMap::new()).as_deref(),
        Some("max")
    );

    let mut record2 = test_record();
    record2
        .env_vars
        .insert("MAJU_AGENT_THINKING_EFFORT".into(), "none".to_string());
    let surface2 = read_config_surface(&record2, Some(runtime), None, &no_tiers(), None);
    assert_eq!(
        surface2
            .normalized
            .thinking_effort
            .unwrap()
            .value
            .as_deref(),
        Some("off")
    );
    assert_eq!(
        projection_value(&record2, &BTreeMap::new()).as_deref(),
        Some("off")
    );
}

/// The decisive mixed-authority case (Thufir/Paul acceptance pin): a valid
/// record-native value and a DIFFERENT valid legacy_alias → the native value wins in
/// reader and projection alike. The legacy_alias is the surfaced override baseline.
#[test]
fn goose_record_native_outranks_legacy_alias_in_reader_and_projection() {
    let mut record = test_record();
    record
        .env_vars
        .insert("GOOSE_THINKING_EFFORT".to_string(), "high".to_string());
    record
        .env_vars
        .insert("MAJU_AGENT_THINKING_EFFORT".into(), "low".to_string());
    let runtime = test_runtime();

    let surface = read_config_surface(&record, Some(runtime), None, &no_tiers(), None);
    let effort = surface.normalized.thinking_effort.unwrap();
    assert_eq!(effort.value.as_deref(), Some("high"));
    assert_eq!(effort.origin, ConfigOrigin::MajuExplicit);
    // Column is the overridden baseline (next distinct tier below native).
    assert_eq!(effort.overridden_value.as_deref(), Some("low"));
    // Projection resolves the same authority.
    assert_eq!(
        projection_value(&record, &BTreeMap::new()).as_deref(),
        Some("high")
    );
}

/// Invalid legacy_alias AND invalid native → both skip; a valid global tier wins in
/// the reader, and the projection (driven from the same global env) agrees.
#[test]
fn goose_invalid_record_tiers_fall_through_to_global_in_reader_and_projection() {
    let mut record = test_record();
    record
        .env_vars
        .insert("GOOSE_THINKING_EFFORT".to_string(), "bogus".to_string());
    record
        .env_vars
        .insert("MAJU_AGENT_THINKING_EFFORT".into(), "alsobad".to_string());
    let runtime = test_runtime();
    let mut global = BTreeMap::new();
    global.insert("GOOSE_THINKING_EFFORT".to_string(), "medium".to_string());
    let tiers = global_env_tiers("GOOSE_THINKING_EFFORT", "medium");

    let surface = read_config_surface(&record, Some(runtime), None, &tiers, None);
    let effort = surface
        .normalized
        .thinking_effort
        .expect("global tier must win when both record tiers are invalid");
    assert_eq!(effort.value.as_deref(), Some("medium"));
    assert_eq!(effort.origin, ConfigOrigin::GlobalDefault);
    assert_eq!(
        projection_value(&record, &global).as_deref(),
        Some("medium")
    );
}

/// Goose legacy alias (`MAJU_AGENT_THINKING_EFFORT`) is accepted for the record
/// tier below the legacy_alias, canonicalized, in reader and projection alike.
#[test]
fn goose_record_legacy_alias_below_legacy_alias_in_reader_and_projection() {
    let mut record = test_record();
    record.env_vars.insert(
        "MAJU_AGENT_THINKING_EFFORT".to_string(),
        "xhigh".to_string(),
    );
    let runtime = test_runtime();

    let surface = read_config_surface(&record, Some(runtime), None, &no_tiers(), None);
    let effort = surface
        .normalized
        .thinking_effort
        .expect("record legacy alias must surface when native and legacy_alias are absent");
    assert_eq!(effort.value.as_deref(), Some("max"));
    assert_eq!(effort.origin, ConfigOrigin::MajuExplicit);
    assert_eq!(
        projection_value(&record, &BTreeMap::new()).as_deref(),
        Some("max")
    );
}

/// B same-value collapse: no record authority, live ACP echoes the inherited
/// global value → the panel shows the inherited origin (GlobalDefault), not a
/// spurious per-session AcpConfigOption override.
#[test]
fn goose_acp_equal_to_global_collapses_to_global_origin() {
    let record = test_record();
    let runtime = test_runtime();
    let cache = SessionConfigCache {
        config_options: vec![AcpConfigOptionEntry {
            config_id: "thinking_effort".to_string(),
            category: Some("thought_level".to_string()),
            display_name: Some("Effort".to_string()),
            current_value: Some(serde_json::json!("medium".to_string())),
            options: vec![],
            description: None,
            option_type: None,
        }],
        available_modes: vec![],
        available_models: vec![],
        current_model: None,
        model_overridden: false,
        goose_native_config: None,
        captured_at: "".to_string(),
    };
    let tiers = global_env_tiers("GOOSE_THINKING_EFFORT", "medium");

    let surface = read_config_surface(&record, Some(runtime), Some(&cache), &tiers, None);
    let effort = surface.normalized.thinking_effort.unwrap();
    assert_eq!(effort.value.as_deref(), Some("medium"));
    assert_eq!(
        effort.origin,
        ConfigOrigin::GlobalDefault,
        "ACP echoing the inherited value must not masquerade as a session override"
    );
}

/// B same-value collapse does NOT fire on genuine divergence: live ACP differs
/// from the inherited baseline → ACP wins as the per-session override, global
/// is the surfaced baseline.
#[test]
fn goose_acp_diverging_from_global_wins_as_override() {
    let record = test_record();
    let runtime = test_runtime();
    let cache = SessionConfigCache {
        config_options: vec![AcpConfigOptionEntry {
            config_id: "thinking_effort".to_string(),
            category: Some("thought_level".to_string()),
            display_name: Some("Effort".to_string()),
            current_value: Some(serde_json::json!("low".to_string())),
            options: vec![],
            description: None,
            option_type: None,
        }],
        available_modes: vec![],
        available_models: vec![],
        current_model: None,
        model_overridden: false,
        goose_native_config: None,
        captured_at: "".to_string(),
    };
    let tiers = global_env_tiers("GOOSE_THINKING_EFFORT", "high");

    let surface = read_config_surface(&record, Some(runtime), Some(&cache), &tiers, None);
    let effort = surface.normalized.thinking_effort.unwrap();
    assert_eq!(effort.value.as_deref(), Some("low"));
    assert_eq!(effort.origin, ConfigOrigin::AcpConfigOption);
    assert_eq!(effort.overridden_value.as_deref(), Some("high"));
    assert_eq!(effort.overridden_origin, Some(ConfigOrigin::GlobalDefault));
}

/// Invalid live ACP value is skipped as absent; a valid record tier wins and
/// no phantom ACP override is surfaced.
#[test]
fn goose_invalid_acp_skips_and_record_wins() {
    let mut record = test_record();
    record
        .env_vars
        .insert("MAJU_AGENT_THINKING_EFFORT".into(), "high".to_string());
    let runtime = test_runtime();
    let cache = SessionConfigCache {
        config_options: vec![AcpConfigOptionEntry {
            config_id: "thinking_effort".to_string(),
            category: Some("thought_level".to_string()),
            display_name: Some("Effort".to_string()),
            current_value: Some(serde_json::json!("garbage".to_string())),
            options: vec![],
            description: None,
            option_type: None,
        }],
        available_modes: vec![],
        available_models: vec![],
        current_model: None,
        model_overridden: false,
        goose_native_config: None,
        captured_at: "".to_string(),
    };

    let surface = read_config_surface(&record, Some(runtime), Some(&cache), &no_tiers(), None);
    let effort = surface.normalized.thinking_effort.unwrap();
    assert_eq!(effort.value.as_deref(), Some("high"));
    assert_eq!(effort.origin, ConfigOrigin::MajuExplicit);
    assert_eq!(
        projection_value(&record, &BTreeMap::new()).as_deref(),
        Some("high")
    );
}

// ── Consumed-legacy Advanced suppression (F2) ────────────────────────────────
//
// When the record's native effort key is absent/invalid and the legacy key
// (`MAJU_AGENT_THINKING_EFFORT`) supplies the normalized record effort, the
// legacy key must NOT also re-appear as a generic Advanced field — one
// persisted fact must not surface through two controls. Invalid/unconsumed
// legacy values stay visible in Advanced.

/// Record has valid legacy `MAJU_AGENT_THINKING_EFFORT=high` and no native
/// `GOOSE_THINKING_EFFORT` → effort surfaces from the legacy alias AND the
/// legacy key must NOT re-appear in Advanced.
#[test]
fn record_consumed_legacy_effort_hidden_from_advanced_reader() {
    let mut record = test_record();
    record
        .env_vars
        .insert("MAJU_AGENT_THINKING_EFFORT".to_string(), "high".to_string());
    let runtime = test_runtime(); // Goose (native GOOSE_THINKING_EFFORT)

    let surface = with_goose_path_root(Some("/nonexistent"), || {
        read_config_surface(&record, Some(runtime), None, &no_tiers(), None)
    });

    let effort = surface
        .normalized
        .thinking_effort
        .expect("valid legacy value must surface as effort via record-tier alias");
    assert_eq!(effort.value.as_deref(), Some("high"));

    let advanced_keys: Vec<&str> = surface.advanced.iter().map(|f| f.key.as_str()).collect();
    assert!(
        !advanced_keys.contains(&"MAJU_AGENT_THINKING_EFFORT"),
        "consumed legacy effort key must not double-emit in advanced; got {advanced_keys:?}"
    );
}

/// A valid legacy value shadowed by the canonical legacy_alias is not consumed, so
/// it remains editable in Advanced rather than silently resurfacing later if
/// the legacy_alias is cleared.
#[test]
fn record_legacy_effort_shadowed_by_legacy_alias_stays_visible_in_advanced_reader() {
    let mut record = test_record();
    record
        .env_vars
        .insert("GOOSE_THINKING_EFFORT".into(), "high".to_string());
    record
        .env_vars
        .insert("MAJU_AGENT_THINKING_EFFORT".to_string(), "low".to_string());
    let runtime = test_runtime(); // Goose

    let surface = with_goose_path_root(Some("/nonexistent"), || {
        read_config_surface(&record, Some(runtime), None, &no_tiers(), None)
    });

    let effort = surface
        .normalized
        .thinking_effort
        .expect("canonical legacy_alias must win over legacy record effort");
    assert_eq!(effort.value.as_deref(), Some("high"));
    let advanced_keys: Vec<&str> = surface.advanced.iter().map(|f| f.key.as_str()).collect();
    assert!(
        advanced_keys.contains(&"MAJU_AGENT_THINKING_EFFORT"),
        "valid but unconsumed record legacy must remain visible in Advanced; got {advanced_keys:?}"
    );
}

/// An invalid legacy `MAJU_AGENT_THINKING_EFFORT` value is unconsumed, so it
/// stays visible in Advanced.
#[test]
fn record_invalid_legacy_effort_stays_visible_in_advanced_reader() {
    let mut record = test_record();
    record.env_vars.insert(
        "MAJU_AGENT_THINKING_EFFORT".to_string(),
        "bogus".to_string(),
    );
    let runtime = test_runtime(); // Goose

    let surface = with_goose_path_root(Some("/nonexistent"), || {
        read_config_surface(&record, Some(runtime), None, &no_tiers(), None)
    });

    assert!(
        surface.normalized.thinking_effort.is_none(),
        "invalid legacy value must not be consumed as effort"
    );
    let advanced_keys: Vec<&str> = surface.advanced.iter().map(|f| f.key.as_str()).collect();
    assert!(
        advanced_keys.contains(&"MAJU_AGENT_THINKING_EFFORT"),
        "unconsumed legacy key must stay visible in advanced; got {advanced_keys:?}"
    );
}

// ── F4: legacy `effort` category fallback in find_effort_option ──────────────
//
// `thought_level` is preferred; the legacy invented category `effort` is a
// fallback for pre-canonical adapters. An advertised-but-unset `thought_level`
// must NOT fall through to a set `effort` (that would route the write to the
// wrong config_id), but a cache that advertises only `effort` must still
// surface a thinking field and write route.

/// `thought_level` present but unset, `effort` present and set → effort must
/// NOT surface from the live cache (no fallthrough); write routing never picks
/// up the legacy `effort` config id.
#[test]
fn unset_thought_level_does_not_fall_through_to_effort_category() {
    let record = test_record();
    let runtime = test_runtime(); // Goose
    let cache = SessionConfigCache {
        config_options: vec![
            AcpConfigOptionEntry {
                config_id: "thinking_effort".to_string(),
                category: Some("thought_level".to_string()),
                display_name: Some("Thinking Effort".to_string()),
                current_value: None, // advertised but unset
                options: vec![],
                description: None,
                option_type: None,
            },
            AcpConfigOptionEntry {
                config_id: "effort".to_string(),
                category: Some("effort".to_string()),
                display_name: Some("Effort (legacy)".to_string()),
                current_value: Some(serde_json::json!("low".to_string())),
                options: vec![],
                description: None,
                option_type: None,
            },
        ],
        available_modes: vec![],
        available_models: vec![],
        current_model: None,
        model_overridden: false,
        goose_native_config: None,
        captured_at: "".to_string(),
    };

    let surface = with_goose_path_root(Some("/nonexistent"), || {
        read_config_surface(&record, Some(runtime), Some(&cache), &no_tiers(), None)
    });

    assert!(
        surface.normalized.thinking_effort.is_none(),
        "unset thought_level must not fall through to the legacy effort category"
    );
}

/// `effort` category present and set, no `thought_level` at all → legacy
/// fallback still surfaces the field and routes the write to the matched
/// `effort` config id.
#[test]
fn effort_category_fallback_used_when_thought_level_absent() {
    let record = test_record();
    let runtime = test_runtime(); // Goose
    let cache = SessionConfigCache {
        config_options: vec![AcpConfigOptionEntry {
            config_id: "effort".to_string(),
            category: Some("effort".to_string()),
            display_name: Some("Effort (legacy)".to_string()),
            current_value: Some(serde_json::json!("high".to_string())),
            options: vec![],
            description: None,
            option_type: None,
        }],
        available_modes: vec![],
        available_models: vec![],
        current_model: None,
        model_overridden: false,
        goose_native_config: None,
        captured_at: "".to_string(),
    };

    let surface = with_goose_path_root(Some("/nonexistent"), || {
        read_config_surface(&record, Some(runtime), Some(&cache), &no_tiers(), None)
    });

    let effort = surface
        .normalized
        .thinking_effort
        .expect("legacy effort category must surface when thought_level is absent");
    assert_eq!(effort.value.as_deref(), Some("high"));
    assert!(
        matches!(
            &effort.write_via,
            ConfigWriteMechanism::AcpSetConfigOption { config_id }
                if config_id == "effort"
        ),
        "write route must use the legacy effort config_id when it is the only category; got {:?}",
        effort.write_via
    );
}
