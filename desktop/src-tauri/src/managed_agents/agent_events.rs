//! Serialize `ManagedAgentRecord` ↔ kind:30177 managed-agent events and
//! publish via relay.
//!
//! Managed-agent events are NIP-33 parameterized replaceable events keyed by
//! `(pubkey, kind, d_tag)` where `d_tag` is the agent's pubkey. They mirror the
//! persona event flow (see `persona_events`): the same retention store and
//! flush loop publish them — this module only owns the kind-specific
//! projection, build, content-hash, and tombstone.
//!
//! # Security: opt-IN allowlist, never record-minus-denylist
//!
//! These events are world-readable on the relay. [`ManagedAgentEventContent`]
//! is an explicit opt-IN projection: it lists exactly the fields that are safe
//! to publish. A field added to [`super::ManagedAgentRecord`] later defaults to
//! NOT published unless it is deliberately added here. The flush loop is a
//! blind pre-signed pipe — this projection is the ONLY guard. It MUST NEVER
//! carry:
//! - `private_key_nsec` — the agent's secret key.
//! - `auth_tag` — the NIP-OA owner attestation.
//! - `env_vars` — may hold API keys / credentials.
//! - `backend` — `Provider { config }` is an opaque blob that may hold secrets.
//! - any runtime field (`runtime_pid`, `last_*`, `backend_agent_id`, …) — these
//!   mutate on every start/stop and describe transient process state.
//!
//! The one identity-bearing wire field is `identity_key_envelope`: the nsec is
//! NIP-44 encrypted to the owner identity before it enters this projection.
//! It is never plaintext and inbound code must verify author + decrypt + d-tag.

use maju_core_pkg::kind::KIND_MANAGED_AGENT;
use nostr::{nips::nip44, EventBuilder, Kind, Tag, ToBech32};
use serde::{Deserialize, Serialize};

use super::{ManagedAgentRecord, RespondTo};

/// The JSON body stored in a managed-agent event's content field.
///
/// Explicit opt-IN allowlist of the agent's public identity + behavioral
/// config. See the module docs for the exclusion contract — secrets, the
/// provider backend blob, and all runtime/install fields are deliberately
/// absent and must stay that way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAgentEventContent {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// `persona_content_hash` of the persona snapshot pinned at create time.
    /// Public drift indicator (not a secret) — lets other clients flag a stale
    /// snapshot without re-reading the source persona.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona_source_version: Option<String>,
    pub parallelism: u32,
    /// Inbound author gate mode (wire string).
    pub respond_to: RespondTo,
    /// Allowlisted author pubkeys when `respond_to == Allowlist`. These are
    /// public keys, not secrets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub respond_to_allowlist: Vec<String>,
    /// The agent nsec encrypted to the owning human identity with NIP-44.
    ///
    /// This is intentionally part of the owner-authored relay projection: a
    /// second device signed in as the same owner can recover the *same* agent
    /// identity instead of minting a device-local replacement. The plaintext
    /// key never leaves the device, and the envelope is accepted only after
    /// the event author, decryption result, and d-tag pubkey all agree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_key_envelope: Option<String>,
}

/// Project a `ManagedAgentRecord` onto the content fields published in
/// managed-agent events.
///
/// This is the single source of truth for what leaves the device. The
/// retention upsert compares the serialized projection to suppress a
/// re-publish when only excluded runtime/local fields changed, so an
/// operational start/stop produces an identical projection and never
/// republishes.
#[cfg(test)]
fn agent_event_content(record: &ManagedAgentRecord) -> ManagedAgentEventContent {
    agent_event_content_with_envelope(record, None)
}

/// Project a managed agent while carrying its owner-encrypted identity key.
///
/// Keep this separate from [`agent_event_content`] so callers that only need
/// the public/config projection (tests, diagnostics, content classification)
/// cannot accidentally invent or re-encrypt an identity envelope.
pub fn agent_event_content_with_envelope(
    record: &ManagedAgentRecord,
    identity_key_envelope: Option<String>,
) -> ManagedAgentEventContent {
    // Slimmed projection (NIP-AP "Slimming: kind:30177"): definition-linked
    // instances resolve prompt/model/provider/source_version through their
    // kind:30175 definition, so those fields are omitted from the wire.
    // Definition-less instances ARE their own definition and keep emitting
    // the quad — old readers parse a slimmed event successfully and would
    // otherwise overwrite their local snapshot with absent values, with no
    // restore path. This branch retires once every record is
    // definition-backed (B5 backfill).
    let definition_linked = record.persona_id.is_some();
    ManagedAgentEventContent {
        name: record.name.clone(),
        persona_id: record.persona_id.clone(),
        system_prompt: if definition_linked {
            None
        } else {
            record.system_prompt.clone()
        },
        model: if definition_linked {
            None
        } else {
            record.model.clone()
        },
        provider: if definition_linked {
            None
        } else {
            record.provider.clone()
        },
        persona_source_version: if definition_linked {
            None
        } else {
            record.persona_source_version.clone()
        },
        parallelism: record.parallelism,
        respond_to: record.respond_to,
        respond_to_allowlist: record.respond_to_allowlist.clone(),
        identity_key_envelope,
    }
}

/// Recover an agent nsec from an owner-self-encrypted envelope and prove that
/// it belongs to `expected_agent_pubkey`.
///
/// The pubkey check is the important boundary: a valid ciphertext from an old
/// or tampered retained row must never be attached to a different agent
/// coordinate.
pub fn decrypt_agent_identity_key(
    owner_keys: &nostr::Keys,
    envelope: &str,
    expected_agent_pubkey: &str,
) -> Result<String, String> {
    let plaintext = nip44::decrypt(owner_keys.secret_key(), &owner_keys.public_key(), envelope)
        .map_err(|e| format!("failed to decrypt agent identity key: {e}"))?;
    let agent_keys = nostr::Keys::parse(plaintext.trim())
        .map_err(|e| format!("decrypted agent identity key is invalid: {e}"))?;
    if !agent_keys
        .public_key()
        .to_hex()
        .eq_ignore_ascii_case(expected_agent_pubkey)
    {
        return Err("decrypted agent identity key does not match the agent pubkey".to_string());
    }
    agent_keys
        .secret_key()
        .to_bech32()
        .map_err(|e| format!("failed to encode decrypted agent identity key: {e}"))
}

/// Reuse the retained envelope when it is valid, otherwise create one from
/// the local key. Reuse matters because NIP-44 encryption is randomized: a
/// fresh ciphertext on every boot would make an unchanged agent look edited
/// and republish forever.
fn resolve_identity_key_envelope(
    record: &ManagedAgentRecord,
    owner_keys: &nostr::Keys,
    retained_content: Option<&str>,
) -> Result<Option<String>, String> {
    if let Some(envelope) = retained_content
        .and_then(|content| serde_json::from_str::<ManagedAgentEventContent>(content).ok())
        .and_then(|content| content.identity_key_envelope)
    {
        if decrypt_agent_identity_key(owner_keys, &envelope, &record.pubkey).is_ok() {
            return Ok(Some(envelope));
        }
    }

    if record.private_key_nsec.is_empty() {
        return Ok(None);
    }

    // Refuse to publish an envelope unless the local plaintext key actually
    // owns the record coordinate. This catches corrupt local stores before a
    // bad replacement can overwrite the recoverable relay head.
    let agent_keys = nostr::Keys::parse(record.private_key_nsec.trim())
        .map_err(|e| format!("agent identity key is invalid: {e}"))?;
    if !agent_keys
        .public_key()
        .to_hex()
        .eq_ignore_ascii_case(&record.pubkey)
    {
        return Err("agent identity key does not match the agent pubkey".to_string());
    }

    nip44::encrypt(
        owner_keys.secret_key(),
        &owner_keys.public_key(),
        record.private_key_nsec.trim(),
        nip44::Version::V2,
    )
    .map(Some)
    .map_err(|e| format!("failed to encrypt agent identity key: {e}"))
}

/// Build a kind:30177 event from a `ManagedAgentRecord`.
///
/// Returns an unsigned `EventBuilder` — the caller signs and submits. The
/// `d_tag` is the agent's pubkey.
#[cfg(test)]
fn build_agent_event(record: &ManagedAgentRecord) -> Result<EventBuilder, String> {
    let content = serde_json::to_string(&agent_event_content(record))
        .map_err(|e| format!("failed to serialize managed-agent content: {e}"))?;
    let tags =
        vec![Tag::parse(["d", record.pubkey.as_str()]).map_err(|e| format!("invalid d-tag: {e}"))?];
    Ok(EventBuilder::new(Kind::Custom(KIND_MANAGED_AGENT as u16), content).tags(tags))
}

/// Build the owner-authored wire event, reusing the retained NIP-44 envelope
/// when possible so stable identity sync does not create content churn.
pub fn build_agent_event_for_owner(
    record: &ManagedAgentRecord,
    owner_keys: &nostr::Keys,
    retained_content: Option<&str>,
) -> Result<EventBuilder, String> {
    let envelope = resolve_identity_key_envelope(record, owner_keys, retained_content)?;
    let content = serde_json::to_string(&agent_event_content_with_envelope(record, envelope))
        .map_err(|e| format!("failed to serialize managed-agent content: {e}"))?;
    let tags =
        vec![Tag::parse(["d", record.pubkey.as_str()]).map_err(|e| format!("invalid d-tag: {e}"))?];
    Ok(EventBuilder::new(Kind::Custom(KIND_MANAGED_AGENT as u16), content).tags(tags))
}

/// Parse a kind:30177 event's content into the projection — the inbound
/// counterpart of [`agent_event_content`].
///
/// # Security: the projection type is the structural guard
///
/// Returns [`ManagedAgentEventContent`], NOT a [`ManagedAgentRecord`]. The
/// return type physically cannot represent `private_key_nsec`, `auth_tag`,
/// `env_vars`, `backend`, `agent_command`/`agent_command_override`, or any
/// runtime field, so a malicious inbound event carrying those keys cannot
/// smuggle them onto the apply path — they are dropped at deserialization, not
/// filtered afterward. The caller patches only these fields onto the local
/// record (see `apply_inbound_managed_agent`), matching on the d-tag (the
/// agent's pubkey).
pub fn managed_agent_content_from_event(
    event: &nostr::Event,
) -> Result<ManagedAgentEventContent, String> {
    serde_json::from_str(event.content.as_ref())
        .map_err(|e| format!("failed to parse managed-agent event content: {e}"))
}

/// Build a NIP-09 deletion (kind:5) targeting an agent's kind:30177 event.
///
/// Carries a single `a`-tag with the NIP-33 coordinate
/// `30177:<owner>:<d_tag>` and no `e`-tag: an `e`-tag routes the relay to the
/// event-id deletion path, leaving the parameterized-replaceable coordinate
/// live. The coordinate delete removes the agent for every client and across
/// reboots.
pub fn build_agent_delete(d_tag: &str, owner_pubkey_hex: &str) -> Result<EventBuilder, String> {
    let coord = format!("{KIND_MANAGED_AGENT}:{owner_pubkey_hex}:{d_tag}");
    let tag = Tag::parse(["a", coord.as_str()]).map_err(|e| format!("invalid a-tag: {e}"))?;
    Ok(EventBuilder::new(Kind::Custom(5), "").tags(vec![tag]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample_agent() -> ManagedAgentRecord {
        ManagedAgentRecord {
            pubkey: "agentpubkeyhex".to_string(),
            name: "Test Agent".to_string(),
            persona_id: Some("persona-1".to_string()),
            private_key_nsec: "nsec1secretdonotpublish".to_string(),
            auth_tag: Some("authtagsecret".to_string()),
            relay_url: "wss://relay.example".to_string(),
            avatar_url: Some("https://example.com/a.png".to_string()),
            acp_command: "maju-acp".to_string(),
            agent_command: "goose".to_string(),
            agent_command_override: None,
            agent_args: vec!["--flag".to_string()],
            mcp_command: "maju-dev-mcp".to_string(),
            turn_timeout_seconds: 320,
            idle_timeout_seconds: None,
            max_turn_duration_seconds: None,
            parallelism: 24,
            system_prompt: Some("You are a test agent.".to_string()),
            model: Some("claude-opus-4".to_string()),
            provider: Some("anthropic".to_string()),
            persona_source_version: Some("abc123".to_string()),
            env_vars: BTreeMap::from([("OPENAI_API_KEY".to_string(), "sk-secret".to_string())]),
            start_on_app_launch: true,
            auto_restart_on_config_change: true,
            runtime_pid: Some(4242),
            backend: super::super::BackendKind::Provider {
                id: "maju-backend-x".to_string(),
                config: serde_json::json!({ "api_key": "sk-provider-secret" }),
            },
            backend_agent_id: Some("remote-id".to_string()),
            provider_binary_path: Some("/path/to/binary".to_string()),
            team_id: None,
            persona_team_dir: None,
            persona_name_in_team: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            last_started_at: Some("2025-01-02T00:00:00Z".to_string()),
            last_stopped_at: Some("2025-01-03T00:00:00Z".to_string()),
            last_exit_code: Some(0),
            last_error: Some("some runtime error".to_string()),
            last_error_code: None,
            respond_to: RespondTo::Allowlist,
            respond_to_allowlist: vec!["79be667e".to_string()],
            // Unified-model fields carry real values so the exclusion test
            // proves they are absent from the wire, not vacuously empty.
            display_name: Some("Display Name Secretish".to_string()),
            slug: Some("sample-slug".to_string()),
            runtime: Some("goose".to_string()),
            name_pool: vec!["poolname".to_string()],
            is_builtin: true,
            is_active: false,
            shared: false,
            source_team: None,
            source_team_persona_slug: None,
            catalog_source: None,
            definition_respond_to: None,
            definition_respond_to_allowlist: Vec::new(),
            definition_parallelism: None,
            relay_mesh: None,
        }
    }

    #[test]
    fn build_agent_event_produces_correct_kind() {
        let builder = build_agent_event(&sample_agent()).unwrap();
        let keys = nostr::Keys::generate();
        let event = builder.sign_with_keys(&keys).unwrap();
        assert_eq!(event.kind.as_u16() as u32, KIND_MANAGED_AGENT);
    }

    #[test]
    fn d_tag_is_agent_pubkey() {
        let builder = build_agent_event(&sample_agent()).unwrap();
        let keys = nostr::Keys::generate();
        let event = builder.sign_with_keys(&keys).unwrap();
        let d = event
            .tags
            .iter()
            .find_map(|t| {
                let v: Vec<&str> = t.as_slice().iter().map(|s| s.as_str()).collect();
                (v.first() == Some(&"d"))
                    .then(|| v.get(1).map(|s| s.to_string()))
                    .flatten()
            })
            .unwrap();
        assert_eq!(d, "agentpubkeyhex");
    }

    /// The security-critical assertion: the published projection must NEVER
    /// carry secrets, the provider backend blob, env vars, or runtime fields.
    #[test]
    fn content_excludes_secrets_and_runtime_fields() {
        let json = serde_json::to_string(&agent_event_content(&sample_agent())).unwrap();

        // Secrets — must never appear.
        assert!(
            !json.contains("nsec1secretdonotpublish"),
            "leaked private key"
        );
        assert!(!json.contains("private_key"), "leaked private key field");
        assert!(!json.contains("authtagsecret"), "leaked auth tag value");
        assert!(!json.contains("auth_tag"), "leaked auth tag field");
        assert!(!json.contains("OPENAI_API_KEY"), "leaked env var key");
        assert!(!json.contains("sk-secret"), "leaked env var value");
        assert!(!json.contains("env_vars"), "leaked env var field");
        assert!(
            !json.contains("sk-provider-secret"),
            "leaked provider config secret"
        );
        assert!(!json.contains("backend"), "leaked backend blob");

        // Runtime / install fields — must never appear.
        assert!(!json.contains("runtime_pid"), "leaked runtime pid");
        assert!(!json.contains("4242"), "leaked runtime pid value");
        assert!(!json.contains("last_started_at"));
        assert!(!json.contains("last_stopped_at"));
        assert!(!json.contains("last_exit_code"));
        assert!(!json.contains("last_error"));
        assert!(!json.contains("some runtime error"));

        // Unified-agent-model fields (Phase 1A) — deliberately NOT published
        // yet; adding them to the wire projection is a Phase 2 (relay
        // canonicalization) decision, not a record-shape side effect.
        assert!(!json.contains("display_name"), "leaked display_name");
        assert!(!json.contains("\"slug\""), "leaked slug");
        assert!(!json.contains("\"runtime\""), "leaked runtime");
        assert!(!json.contains("name_pool"), "leaked name_pool");
        assert!(!json.contains("is_builtin"), "leaked is_builtin");
        assert!(!json.contains("is_active"), "leaked is_active");
        assert!(!json.contains("backend_agent_id"));
        assert!(!json.contains("provider_binary_path"));
        assert!(!json.contains("relay_url"));

        // Identity fields — must appear.
        assert!(json.contains("\"name\""));
        assert!(json.contains("Test Agent"));
        assert!(json.contains("persona_id"));
        // Slimmed projection: a definition-linked record resolves its prompt
        // through the definition, so the wire must NOT carry it.
        assert!(
            !json.contains("system_prompt"),
            "definition-linked projection must omit system_prompt"
        );
    }

    /// Slimming (NIP-AP): definition-linked records omit the definition quad;
    /// definition-less records keep emitting it (they ARE their own
    /// definition — old readers would otherwise wipe fields with no restore
    /// path).
    #[test]
    fn projection_slims_definition_quad_only_when_linked() {
        let linked = sample_agent(); // persona_id: Some
        let json = serde_json::to_string(&agent_event_content(&linked)).unwrap();
        assert!(!json.contains("system_prompt"));
        assert!(!json.contains("\"model\""));
        assert!(!json.contains("\"provider\""));
        assert!(!json.contains("persona_source_version"));
        // Instance fields stay on the wire.
        assert!(json.contains("parallelism"));
        assert!(json.contains("respond_to"));

        let mut standalone = sample_agent();
        standalone.persona_id = None;
        let json = serde_json::to_string(&agent_event_content(&standalone)).unwrap();
        assert!(json.contains("system_prompt"), "standalone keeps prompt");
        assert!(json.contains("\"model\""), "standalone keeps model");
        assert!(json.contains("\"provider\""), "standalone keeps provider");
        assert!(
            json.contains("persona_source_version"),
            "standalone keeps source_version"
        );
    }

    #[test]
    fn projection_is_deterministic() {
        let agent = sample_agent();
        let a = serde_json::to_string(&agent_event_content(&agent)).unwrap();
        let b = serde_json::to_string(&agent_event_content(&agent)).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn owner_envelope_round_trips_same_agent_key_without_plaintext_leak() {
        use nostr::ToBech32;

        let owner = nostr::Keys::generate();
        let agent_keys = nostr::Keys::generate();
        let mut agent = sample_agent();
        agent.pubkey = agent_keys.public_key().to_hex();
        agent.private_key_nsec = agent_keys.secret_key().to_bech32().unwrap();

        let event = build_agent_event_for_owner(&agent, &owner, None)
            .unwrap()
            .sign_with_keys(&owner)
            .unwrap();
        assert!(!event.content.contains(&agent.private_key_nsec));

        let content = managed_agent_content_from_event(&event).unwrap();
        let envelope = content
            .identity_key_envelope
            .as_deref()
            .expect("new managed-agent events carry an identity envelope");
        assert_eq!(
            decrypt_agent_identity_key(&owner, envelope, &agent.pubkey).unwrap(),
            agent.private_key_nsec
        );
    }

    #[test]
    fn owner_event_reuses_retained_envelope_to_avoid_boot_churn() {
        use nostr::ToBech32;

        let owner = nostr::Keys::generate();
        let agent_keys = nostr::Keys::generate();
        let mut agent = sample_agent();
        agent.pubkey = agent_keys.public_key().to_hex();
        agent.private_key_nsec = agent_keys.secret_key().to_bech32().unwrap();

        let first = build_agent_event_for_owner(&agent, &owner, None)
            .unwrap()
            .sign_with_keys(&owner)
            .unwrap();
        let second = build_agent_event_for_owner(&agent, &owner, Some(first.content.as_ref()))
            .unwrap()
            .sign_with_keys(&owner)
            .unwrap();
        assert_eq!(first.content, second.content);
    }

    #[test]
    fn owner_envelope_rejects_a_different_agent_coordinate() {
        use nostr::ToBech32;

        let owner = nostr::Keys::generate();
        let agent_keys = nostr::Keys::generate();
        let other = nostr::Keys::generate();
        let mut agent = sample_agent();
        agent.pubkey = agent_keys.public_key().to_hex();
        agent.private_key_nsec = agent_keys.secret_key().to_bech32().unwrap();
        let event = build_agent_event_for_owner(&agent, &owner, None)
            .unwrap()
            .sign_with_keys(&owner)
            .unwrap();
        let envelope = managed_agent_content_from_event(&event)
            .unwrap()
            .identity_key_envelope
            .unwrap();

        assert!(
            decrypt_agent_identity_key(&owner, &envelope, &other.public_key().to_hex()).is_err()
        );
    }

    /// Mutating only runtime fields must NOT change the projection — the
    /// guarantee that operational start/stop never republishes.
    #[test]
    fn projection_ignores_runtime_field_churn() {
        let agent = sample_agent();
        let mut churned = agent.clone();
        churned.runtime_pid = Some(9999);
        churned.last_started_at = Some("2099-01-01T00:00:00Z".to_string());
        churned.last_exit_code = Some(1);
        churned.last_error = Some("different error".to_string());
        churned.updated_at = "2099-12-31T00:00:00Z".to_string();
        assert_eq!(
            agent_event_content(&agent),
            agent_event_content(&churned),
            "runtime field churn must not alter the published projection"
        );
    }

    #[test]
    fn projection_changes_on_meaningful_edit() {
        // Instance-level edit surfaces for every record.
        let agent = sample_agent();
        let mut edited = agent.clone();
        edited.parallelism += 1;
        assert_ne!(agent_event_content(&agent), agent_event_content(&edited));

        // Definition-level edit surfaces only for definition-less records —
        // linked records resolve the prompt through their definition.
        let mut standalone = sample_agent();
        standalone.persona_id = None;
        let mut edited = standalone.clone();
        edited.system_prompt = Some("A different prompt.".to_string());
        assert_ne!(
            agent_event_content(&standalone),
            agent_event_content(&edited)
        );
        let mut linked_edit = sample_agent();
        linked_edit.system_prompt = Some("A different prompt.".to_string());
        assert_eq!(
            agent_event_content(&sample_agent()),
            agent_event_content(&linked_edit),
            "a linked record's local prompt snapshot is not wire state"
        );
    }

    /// The inbound structural guard: a foreign event whose content JSON crams
    /// in secrets and harness fields parses successfully but the projection type
    /// silently DROPS every non-projected key — they can never reach the apply
    /// path. This is the inbound mirror of
    /// `content_excludes_secrets_and_runtime_fields`.
    #[test]
    fn from_event_drops_injected_secret_and_harness_keys() {
        use nostr::{EventBuilder, JsonUtil, Keys, Kind, Tag};
        let content = serde_json::json!({
            "name": "Agent",
            "parallelism": 1,
            "respond_to": "owner-only",
            // Injected — not part of the projection, must be dropped.
            "private_key_nsec": "nsec1leak",
            "auth_tag": "leak",
            "env_vars": { "K": "leak" },
            "agent_command": "leak",
            "agent_command_override": "leak",
            "backend": { "type": "local" },
        });
        let keys = Keys::generate();
        let event = EventBuilder::new(Kind::Custom(KIND_MANAGED_AGENT as u16), content.to_string())
            .tags(vec![Tag::parse(["d", "agentpubkeyhex"]).unwrap()])
            .sign_with_keys(&keys)
            .unwrap();
        let event = nostr::Event::from_json(event.as_json()).unwrap();

        let parsed = managed_agent_content_from_event(&event).unwrap();
        // The projected fields parse through.
        assert_eq!(parsed.name, "Agent");
        assert_eq!(parsed.parallelism, 1);
        assert_eq!(parsed.respond_to, RespondTo::OwnerOnly);
        // Re-serializing the projection contains no injected key.
        let json = serde_json::to_string(&parsed).unwrap();
        assert!(!json.contains("nsec1leak"));
        assert!(!json.contains("private_key"));
        assert!(!json.contains("auth_tag"));
        assert!(!json.contains("env_vars"));
        assert!(!json.contains("agent_command"));
        assert!(!json.contains("backend"));
    }

    #[test]
    fn build_agent_delete_has_single_a_tag_no_e_tag() {
        const OWNER: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let builder = build_agent_delete("agentpubkeyhex", OWNER).unwrap();
        let keys = nostr::Keys::generate();
        let event = builder.sign_with_keys(&keys).unwrap();

        assert_eq!(event.kind, Kind::Custom(5));

        let a_tags: Vec<&[String]> = event
            .tags
            .iter()
            .map(|t| t.as_slice())
            .filter(|v| v.first().map(String::as_str) == Some("a"))
            .collect();
        assert_eq!(a_tags.len(), 1);
        assert_eq!(
            a_tags[0][1],
            format!("{KIND_MANAGED_AGENT}:{OWNER}:agentpubkeyhex")
        );

        assert!(event
            .tags
            .iter()
            .all(|t| t.as_slice().first().map(String::as_str) != Some("e")));
    }
}
