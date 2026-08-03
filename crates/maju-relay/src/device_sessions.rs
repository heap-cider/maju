//! Device-session admission, durable logout checks, and agent runner fencing.

use std::sync::Arc;

use maju_core::kind::KIND_DEVICE_SESSION;
use maju_core::tenant::TenantContext;
use maju_db::EventQuery;
use maju_pubsub::device_sessions::{AgentRunner, DeviceDescriptor, RunnerAdmission};
use nostr::{Event, PublicKey};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

/// Signed AUTH tag name for an official Maju installation.
pub const DEVICE_AUTH_TAG: &str = "maju-device";
/// Current device tag wire version.
pub const DEVICE_AUTH_VERSION: &str = "1";

/// Device metadata presented inside a signed NIP-42 AUTH event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentedDevice {
    /// Installation/session metadata.
    pub descriptor: DeviceDescriptor,
    /// Per-process id for an ACP runner. Empty on the human desktop socket.
    pub runner_id: Option<String>,
}

/// Representative lease attached to an authenticated agent connection.
#[derive(Debug, Clone)]
pub struct RunnerLease {
    /// Metadata stored beside the Redis lease.
    pub runner: AgentRunner,
    /// Fencing token required for renewal, writes, and release.
    pub token: String,
}

/// Device/session identity attached to a live WebSocket connection.
#[derive(Debug, Clone)]
pub struct AuthenticatedDevice {
    /// Owning account pubkey (hex); differs from authenticated pubkey for agents.
    pub owner_pubkey: String,
    /// Installation/session metadata.
    pub descriptor: DeviceDescriptor,
    /// Present only for the active representative agent connection.
    pub runner_lease: Option<RunnerLease>,
}

/// Durable owner-signed device record body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceSessionContent {
    /// `connected` or `disconnected`.
    pub state: String,
    /// Session targeted by this record.
    pub session_id: String,
    /// Device display name.
    pub name: String,
    /// Operating-system family.
    pub platform: String,
    /// Maju app version.
    pub app_version: String,
}

fn single_tag<'a>(event: &'a Event, name: &str) -> Result<&'a str, String> {
    let mut values = event.tags.iter().filter_map(|tag| {
        let values = tag.as_slice();
        (values.first().map(String::as_str) == Some(name))
            .then(|| values.get(1).map(String::as_str))
            .flatten()
    });
    let value = values
        .next()
        .ok_or_else(|| format!("device session requires one `{name}` tag"))?;
    if values.next().is_some() {
        return Err(format!("device session requires exactly one `{name}` tag"));
    }
    Ok(value)
}

fn validate_uuid(label: &str, value: &str) -> Result<(), String> {
    uuid::Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| format!("{label} must be a UUID"))
}

fn validate_text(label: &str, value: &str, max: usize) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > max {
        return Err(format!("{label} must be 1-{max} bytes"));
    }
    Ok(())
}

/// Parse and validate at most one signed Maju device AUTH tag.
pub fn parse_auth_device(event: &Event) -> Result<Option<PresentedDevice>, String> {
    let mut tags = event
        .tags
        .iter()
        .filter(|tag| tag.as_slice().first().map(String::as_str) == Some(DEVICE_AUTH_TAG));
    let Some(tag) = tags.next() else {
        return Ok(None);
    };
    if tags.next().is_some() {
        return Err("AUTH contains more than one maju-device tag".to_string());
    }
    let values = tag.as_slice();
    if values.len() != 9 || values[1] != DEVICE_AUTH_VERSION {
        return Err("invalid maju-device tag version or field count".to_string());
    }
    let device_id = values[2].clone();
    let session_id = values[3].clone();
    let runner_id = values[4].trim();
    validate_uuid("device id", &device_id)?;
    validate_uuid("device session id", &session_id)?;
    if !runner_id.is_empty() {
        validate_uuid("agent runner id", runner_id)?;
    }
    validate_text("device name", &values[5], 80)?;
    validate_text("device platform", &values[6], 32)?;
    validate_text("app version", &values[7], 32)?;
    if values[8] != "desktop" && values[8] != "agent" {
        return Err("maju-device client type must be desktop or agent".to_string());
    }
    if values[8] == "agent" && runner_id.is_empty() {
        return Err("agent maju-device tag requires a runner id".to_string());
    }
    if values[8] == "desktop" && !runner_id.is_empty() {
        return Err("desktop maju-device tag cannot carry a runner id".to_string());
    }
    Ok(Some(PresentedDevice {
        descriptor: DeviceDescriptor {
            device_id,
            session_id,
            name: values[5].clone(),
            platform: values[6].clone(),
            app_version: values[7].clone(),
        },
        runner_id: (!runner_id.is_empty()).then(|| runner_id.to_string()),
    }))
}

/// Validate a durable kind:30360 device-session event before storage.
pub fn validate_device_session_event(event: &Event) -> Result<DeviceSessionContent, String> {
    let device_id = single_tag(event, "d")?;
    validate_uuid("device id", device_id)?;
    let content: DeviceSessionContent = serde_json::from_str(&event.content)
        .map_err(|_| "device session content must be valid JSON".to_string())?;
    if content.state != "connected" && content.state != "disconnected" {
        return Err("device session state must be connected or disconnected".to_string());
    }
    validate_uuid("device session id", &content.session_id)?;
    validate_text("device name", &content.name, 80)?;
    validate_text("device platform", &content.platform, 32)?;
    validate_text("app version", &content.app_version, 32)?;
    Ok(content)
}

async fn durable_session_disconnected(
    state: &AppState,
    tenant: &TenantContext,
    owner: &PublicKey,
    device: &DeviceDescriptor,
) -> Result<bool, String> {
    let mut query = EventQuery::for_community(tenant.community());
    query.kinds = Some(vec![KIND_DEVICE_SESSION as i32]);
    query.pubkey = Some(owner.to_bytes().to_vec());
    query.d_tag = Some(device.device_id.clone());
    query.global_only = true;
    query.limit = Some(1);
    let events = state
        .db
        .query_events(&query)
        .await
        .map_err(|error| format!("device session lookup failed: {error}"))?;
    Ok(events.first().is_some_and(|stored| {
        validate_device_session_event(&stored.event).is_ok_and(|content| {
            content.state == "disconnected" && content.session_id == device.session_id
        })
    }))
}

/// Admit one signed device session and, for an agent, acquire its representative lease.
pub async fn admit(
    state: &Arc<AppState>,
    tenant: &TenantContext,
    authenticated_pubkey: &PublicKey,
    owner_pubkey: &PublicKey,
    presented: PresentedDevice,
) -> Result<AuthenticatedDevice, String> {
    let owner_hex = owner_pubkey.to_hex();
    if durable_session_disconnected(state, tenant, owner_pubkey, &presented.descriptor).await?
        || state
            .pubsub
            .is_device_session_revoked(
                tenant,
                &owner_hex,
                &presented.descriptor.device_id,
                &presented.descriptor.session_id,
            )
            .await
            .map_err(|error| format!("device session lookup failed: {error}"))?
    {
        return Err("blocked: device session disconnected".to_string());
    }

    state
        .pubsub
        .touch_device(tenant, &owner_hex, &presented.descriptor)
        .await
        .map_err(|error| format!("device heartbeat failed: {error}"))?;

    let runner_lease = if authenticated_pubkey != owner_pubkey {
        let runner_id = presented.runner_id.ok_or_else(|| {
            "invalid: managed agent authentication requires a runner id".to_string()
        })?;
        let runner = AgentRunner {
            agent_pubkey: authenticated_pubkey.to_hex(),
            owner_pubkey: owner_hex.clone(),
            device_id: presented.descriptor.device_id.clone(),
            session_id: presented.descriptor.session_id.clone(),
            runner_id,
            device_name: presented.descriptor.name.clone(),
        };
        match state
            .pubsub
            .acquire_agent_runner(tenant, &runner)
            .await
            .map_err(|error| format!("agent runner coordination failed: {error}"))?
        {
            RunnerAdmission::Active { token } => Some(RunnerLease { runner, token }),
            RunnerAdmission::Standby { active } => {
                return Err(format!(
                    "standby: agent is active on {}",
                    active.device_name
                ));
            }
        }
    } else {
        if presented.runner_id.is_some() {
            return Err("invalid: account device cannot claim an agent runner".to_string());
        }
        None
    };

    Ok(AuthenticatedDevice {
        owner_pubkey: owner_hex,
        descriptor: presented.descriptor,
        runner_lease,
    })
}

/// Refresh device liveness and, when present, the representative lease.
/// Returns false if this connection lost its fencing lease.
pub async fn refresh(
    state: &AppState,
    tenant: &TenantContext,
    device: &AuthenticatedDevice,
) -> bool {
    match state
        .pubsub
        .is_device_session_revoked(
            tenant,
            &device.owner_pubkey,
            &device.descriptor.device_id,
            &device.descriptor.session_id,
        )
        .await
    {
        Ok(true) => return false,
        Ok(false) => {}
        Err(error) => {
            tracing::warn!(%error, "device revocation refresh failed");
            return device.runner_lease.is_none();
        }
    }
    if let Err(error) = state
        .pubsub
        .touch_device(tenant, &device.owner_pubkey, &device.descriptor)
        .await
    {
        tracing::warn!(%error, "device heartbeat refresh failed");
        return device.runner_lease.is_none();
    }
    let Some(lease) = &device.runner_lease else {
        return true;
    };
    state
        .pubsub
        .renew_agent_runner(
            tenant,
            &lease.runner.agent_pubkey,
            &lease.token,
            &lease.runner,
        )
        .await
        .unwrap_or(false)
}

/// Release a representative lease after its WebSocket closes.
pub async fn release(state: &AppState, tenant: &TenantContext, device: &AuthenticatedDevice) {
    let Some(lease) = &device.runner_lease else {
        return;
    };
    let _ = state
        .pubsub
        .release_agent_runner(tenant, &lease.runner.agent_pubkey, &lease.token)
        .await;
}

/// Apply a durable connected/disconnected record after it commits.
pub async fn apply_device_session_event(
    state: &Arc<AppState>,
    tenant: &TenantContext,
    event: &Event,
) {
    let Ok(content) = validate_device_session_event(event) else {
        return;
    };
    let Ok(device_id) = single_tag(event, "d") else {
        return;
    };
    let owner_hex = event.pubkey.to_hex();
    let descriptor = DeviceDescriptor {
        device_id: device_id.to_string(),
        session_id: content.session_id.clone(),
        name: content.name,
        platform: content.platform,
        app_version: content.app_version,
    };
    if content.state == "connected" {
        let _ = state
            .pubsub
            .touch_device(tenant, &owner_hex, &descriptor)
            .await;
        return;
    }

    let _ = state
        .pubsub
        .revoke_device_session(
            tenant,
            &owner_hex,
            &descriptor.device_id,
            &descriptor.session_id,
        )
        .await;
    let reason = "blocked: device session disconnected";
    state.conn_manager.disconnect_device(
        tenant.community(),
        &owner_hex,
        &descriptor.device_id,
        &descriptor.session_id,
        &event.id.to_hex(),
        reason,
    );
    let pubsub = Arc::clone(&state.pubsub);
    let tenant = tenant.clone();
    let command = maju_pubsub::conn_control::ConnControl::DisconnectDevice {
        owner_pubkey: owner_hex,
        device_id: descriptor.device_id,
        session_id: descriptor.session_id,
        event_id: event.id.to_hex(),
        reason: reason.to_string(),
    };
    tokio::spawn(async move {
        if let Err(error) = pubsub.publish_conn_control(&tenant, &command).await {
            tracing::warn!(%error, "device disconnect fan-out failed");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{EventBuilder, Keys, Kind, Tag};

    fn auth_event(tag: Tag) -> Event {
        EventBuilder::new(Kind::Authentication, "")
            .tags([tag])
            .sign_with_keys(&Keys::generate())
            .unwrap()
    }

    #[test]
    fn parses_desktop_and_agent_device_tags() {
        let device = uuid::Uuid::new_v4().to_string();
        let session = uuid::Uuid::new_v4().to_string();
        let desktop = auth_event(
            Tag::parse([
                DEVICE_AUTH_TAG,
                DEVICE_AUTH_VERSION,
                &device,
                &session,
                "",
                "Office PC",
                "windows",
                "0.1.2",
                "desktop",
            ])
            .unwrap(),
        );
        assert!(parse_auth_device(&desktop)
            .unwrap()
            .unwrap()
            .runner_id
            .is_none());

        let runner = uuid::Uuid::new_v4().to_string();
        let agent = auth_event(
            Tag::parse([
                DEVICE_AUTH_TAG,
                DEVICE_AUTH_VERSION,
                &device,
                &session,
                &runner,
                "Office PC",
                "windows",
                "0.1.2",
                "agent",
            ])
            .unwrap(),
        );
        assert_eq!(
            parse_auth_device(&agent)
                .unwrap()
                .unwrap()
                .runner_id
                .as_deref(),
            Some(runner.as_str())
        );
    }

    #[test]
    fn rejects_agent_tag_without_runner_id() {
        let event = auth_event(
            Tag::parse([
                DEVICE_AUTH_TAG,
                DEVICE_AUTH_VERSION,
                &uuid::Uuid::new_v4().to_string(),
                &uuid::Uuid::new_v4().to_string(),
                "",
                "PC",
                "windows",
                "0.1.2",
                "agent",
            ])
            .unwrap(),
        );
        assert!(parse_auth_device(&event).is_err());
    }
}
