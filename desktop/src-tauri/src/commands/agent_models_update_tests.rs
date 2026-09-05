use super::*;
// The tests call `apply_record_field_updates(...)` and consume the return value
// via `.expect(...)`, discarding `RecordFieldsApplied`. The tests verify saved
// native options, not the token itself. The lint is suppressed here so
// callers remain readable. Production code (update_managed_agent) must never
// suppress it — the token IS the outer-seam compile-time proof.

fn provider_record(deployed: bool) -> ManagedAgentRecord {
    let mut record: ManagedAgentRecord = serde_json::from_value(serde_json::json!({
        "pubkey": "agent", "name": "Agent", "relay_url": "", "acp_command": "",
        "agent_command": "", "agent_args": [], "mcp_command": "",
        "turn_timeout_seconds": 0, "system_prompt": null, "created_at": "",
        "updated_at": "", "last_started_at": null, "last_stopped_at": null,
        "last_exit_code": null, "last_error": null
    }))
    .unwrap();
    record.backend = crate::managed_agents::BackendKind::Provider {
        id: "provider".into(),
        config: serde_json::json!({}),
    };
    record.backend_agent_id = deployed.then(|| "deployment".to_string());
    record
}

#[test]
fn deployed_provider_rejects_access_edits_that_cannot_be_revoked() {
    let error = ensure_access_policy_change_supported(&provider_record(true), true)
        .expect_err("deployed provider access edit must fail closed");
    assert!(error.contains("no explicit stop or revocation acknowledgement"));
}

#[test]
fn undeployed_provider_accepts_access_edits() {
    ensure_access_policy_change_supported(&provider_record(false), true)
        .expect("no running provider deployment can retain stale access");
}

fn local_record() -> ManagedAgentRecord {
    serde_json::from_value(serde_json::json!({
        "pubkey": "local", "name": "Local Agent", "relay_url": "", "acp_command": "",
        "agent_command": "", "agent_args": [], "mcp_command": "",
        "turn_timeout_seconds": 0, "system_prompt": null, "created_at": "",
        "updated_at": "", "last_started_at": null, "last_stopped_at": null,
        "last_exit_code": null, "last_error": null
    }))
    .unwrap()
    // BackendKind deserializes as Local when the field is absent (the json! above).
}

// Native ACP controls use one saved map, shared by model discovery and launch.
const NATIVE_OPTIONS: &str = crate::managed_agents::ACP_CONFIG_OPTIONS_ENV;

fn native_options(value: &str) -> std::collections::BTreeMap<String, String> {
    std::collections::BTreeMap::from([(NATIVE_OPTIONS.to_string(), value.to_string())])
}

#[test]
fn record_field_updates_preserve_live_native_values_for_local_and_provider_agents() {
    for mut record in [local_record(), provider_record(false)] {
        let options = native_options(r#"{"reasoning_effort":"ultra","service_tier":"fast"}"#);
        let applied = apply_record_field_updates(&mut record, Some(&options), false).unwrap();
        stamp_record_updated_at(&mut record, applied);
        assert_eq!(record.env_vars, options);
        assert!(serde_json::to_value(&record)
            .unwrap()
            .get("effort_level")
            .is_none());
    }
}

#[test]
fn record_field_updates_clear_native_overrides_explicitly() {
    let mut record = local_record();
    record.env_vars = native_options(r#"{"reasoning_effort":"high"}"#);
    let cleared = native_options("{}");
    let _applied = apply_record_field_updates(&mut record, Some(&cleared), false).unwrap();
    assert_eq!(record.env_vars, cleared);
}

#[test]
fn omitted_native_options_preserve_the_saved_selection() {
    let mut record = local_record();
    let options = native_options(r#"{"reasoning_effort":"high"}"#);
    record.env_vars = options.clone();
    let _applied = apply_record_field_updates(&mut record, None, false).unwrap();
    assert_eq!(record.env_vars, options);
}

#[test]
fn inherit_transition_clears_same_request_runtime_controls_after_env_replacement() {
    let mut record = local_record();
    let mut options = native_options(r#"{"reasoning_effort":"high","service_tier":"fast"}"#);
    options.insert("GOOSE_THINKING_EFFORT".into(), "max".into());
    options.insert("USER_SETTING".into(), "keep".into());
    let _applied = apply_record_field_updates(&mut record, Some(&options), true).unwrap();
    assert_eq!(
        record.env_vars.get(NATIVE_OPTIONS).map(String::as_str),
        Some("{}")
    );
    assert!(!record.env_vars.contains_key("GOOSE_THINKING_EFFORT"));
    assert_eq!(
        record.env_vars.get("USER_SETTING").map(String::as_str),
        Some("keep")
    );
}

#[cfg(not(target_os = "windows"))]
#[test]
fn record_field_updates_persist_native_options_to_disk() {
    use crate::app_state::build_app_state;
    use crate::managed_agents::{load_managed_agents, save_managed_agents};

    // A single crate-wide process-env lock covers PATH, HOME, XDG_DATA_HOME,
    // and all effort env keys — `lock_path_mutex` and `lock_env_mutex` both
    // delegate to the same `PROCESS_ENV_MUTEX` static.
    let _env_guard = crate::managed_agents::lock_path_mutex();
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();

    // RAII guards restore HOME and XDG_DATA_HOME on Drop (even on panic).
    // Uses OsString so a pre-existing non-Unicode value is restored exactly.
    struct EnvVarGuard {
        key: String,
        prior: Option<std::ffi::OsString>,
    }
    impl EnvVarGuard {
        fn set(key: &str, value: &std::path::Path) -> Self {
            let prior = std::env::var_os(key);
            #[allow(deprecated)]
            // Caller holds the crate-wide process-env lock.
            std::env::set_var(key, value);
            Self {
                key: key.to_string(),
                prior,
            }
        }
    }
    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            #[allow(deprecated)]
            // Caller holds the crate-wide process-env lock.
            match &self.prior {
                Some(v) => std::env::set_var(&self.key, v),
                None => std::env::remove_var(&self.key),
            }
        }
    }

    let _home_guard = EnvVarGuard::set("HOME", &home);
    let _xdg_guard = EnvVarGuard::set("XDG_DATA_HOME", &home);

    let app = tauri::test::mock_builder()
        .manage(build_app_state())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("mock app builds headless");

    // Seed a local record with no effort set.
    let seed: crate::managed_agents::ManagedAgentRecord =
        serde_json::from_value(serde_json::json!({
            "pubkey": "test-effort-agent",
            "name": "Effort Test Agent",
            "relay_url": "", "acp_command": "", "agent_command": "",
            "agent_args": [], "mcp_command": "", "turn_timeout_seconds": 0,
            "system_prompt": null, "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z", "last_started_at": null,
            "last_stopped_at": null, "last_exit_code": null, "last_error": null
        }))
        .unwrap();
    save_managed_agents(app.handle(), &[seed]).unwrap();

    // Drive the production seam: load → apply_record_field_updates →
    // stamp_record_updated_at → save. This is the exact sequence that
    // `update_managed_agent` executes inside its locked transaction.
    let mut records = load_managed_agents(app.handle()).unwrap();
    let record = records
        .iter_mut()
        .find(|r| r.pubkey == "test-effort-agent")
        .expect("seeded record must load");
    let options = native_options(r#"{"reasoning_effort":"ultra","service_tier":"fast"}"#);
    let applied = apply_record_field_updates(record, Some(&options), false)
        .expect("local record must accept effort set");
    stamp_record_updated_at(record, applied);
    save_managed_agents(app.handle(), &records).unwrap();

    // Verify effort landed on disk.
    let saved = load_managed_agents(app.handle()).unwrap();
    let saved_record = saved
        .iter()
        .find(|r| r.pubkey == "test-effort-agent")
        .expect("agent must persist after update");
    assert_eq!(
        saved_record.env_vars.get(NATIVE_OPTIONS),
        options.get(NATIVE_OPTIONS),
        "native controls must survive the production save/load sequence"
    );
    // _home_guard and _xdg_guard restore HOME and XDG_DATA_HOME via Drop.
}
