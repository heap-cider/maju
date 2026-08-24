use super::*;

#[test]
fn syncs_only_the_explicit_acp_option_envelope() {
    let key = crate::managed_agents::ACP_CONFIG_OPTIONS_ENV;
    let mut local = local_in_app();
    local
        .env_vars
        .insert(key.to_string(), r#"{"thought_level":"low"}"#.to_string());
    let mut personas = vec![local];

    // A legacy event without the field must not erase the current value.
    apply_inbound_persona(&mut personas, inbound_for(UUID, "Legacy"));
    assert_eq!(
        personas[0].env_vars.get(key).map(String::as_str),
        Some(r#"{"thought_level":"low"}"#)
    );

    // A new explicit empty envelope clears selections while preserving every
    // unrelated local/secret environment entry.
    let mut cleared = inbound_for(UUID, "Cleared");
    cleared.env_vars.insert(key.to_string(), "{}".to_string());
    apply_inbound_persona(&mut personas, cleared);
    assert_eq!(
        personas[0].env_vars.get(key).map(String::as_str),
        Some("{}")
    );
    assert_eq!(
        personas[0].env_vars.get("API_KEY").map(String::as_str),
        Some("secret")
    );
}
