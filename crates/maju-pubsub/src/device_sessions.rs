//! Account device sessions and single-active agent runner leases.
//!
//! A device id identifies an installation. A session id identifies the current
//! login on that installation. Agent processes additionally carry a runner id.
//! Redis is the live coordination plane; durable disconnect decisions are
//! stored as owner-signed Nostr events by the relay.

use std::collections::HashMap;

use deadpool_redis::Pool;
use maju_core::TenantContext;
use serde::{Deserialize, Serialize};

use crate::error::PubSubError;
use crate::topic::MAJU_PREFIX;

/// A live device remains online across two missed 30-second WebSocket pongs.
pub const DEVICE_ONLINE_TTL_SECS: u64 = 90;
/// Active runner lease. Heartbeats renew it every 30 seconds.
pub const AGENT_RUNNER_LEASE_TTL_SECS: u64 = 75;
/// Standby advertisement lifetime. Standby harnesses retry every five seconds.
pub const AGENT_STANDBY_TTL_SECS: u64 = 90;

/// Signed device metadata carried by a NIP-42 AUTH event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceDescriptor {
    /// Stable installation id.
    pub device_id: String,
    /// Current official-client login session id.
    pub session_id: String,
    /// User-facing device name.
    pub name: String,
    /// Operating-system family.
    pub platform: String,
    /// Maju app version.
    pub app_version: String,
}

/// Live agent representative stored next to the lease token.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRunner {
    /// Stable agent identity pubkey (hex).
    pub agent_pubkey: String,
    /// Owning account pubkey (hex).
    pub owner_pubkey: String,
    /// Installation running the representative.
    pub device_id: String,
    /// Login session on that installation.
    pub session_id: String,
    /// Per-process runner id.
    pub runner_id: String,
    /// Device display name captured at authentication.
    pub device_name: String,
}

/// Result of attempting to become the representative runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerAdmission {
    /// This process owns the lease. The token fences renewal and release.
    Active {
        /// Opaque token required to renew, write through, or release the lease.
        token: String,
    },
    /// Another device currently owns the lease.
    Standby {
        /// The representative that currently owns the lease.
        active: AgentRunner,
    },
}

/// Device information returned to the account's settings UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceStatus {
    /// Stable installation id.
    pub device_id: String,
    /// Current session id last presented by that installation.
    pub session_id: String,
    /// User-facing device name.
    pub name: String,
    /// Operating-system family.
    pub platform: String,
    /// Maju version last seen from that device.
    pub app_version: String,
    /// Unix timestamp of the most recent authenticated heartbeat.
    pub last_seen: u64,
    /// Whether the current session has an unexpired heartbeat.
    pub online: bool,
    /// Agent pubkeys for which this device is representative.
    pub active_agents: Vec<String>,
    /// Agent pubkeys waiting behind a representative on another device.
    pub standby_agents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredDevice {
    descriptor: DeviceDescriptor,
    last_seen: u64,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn devices_key(ctx: &TenantContext, owner_hex: &str) -> String {
    format!("{MAJU_PREFIX}:{}:devices:{owner_hex}", ctx.community())
}

fn device_online_key(ctx: &TenantContext, owner_hex: &str, device_id: &str) -> String {
    format!(
        "{MAJU_PREFIX}:{}:device-online:{owner_hex}:{device_id}",
        ctx.community()
    )
}

fn revoked_key(ctx: &TenantContext, owner_hex: &str, device_id: &str, session_id: &str) -> String {
    format!(
        "{MAJU_PREFIX}:{}:device-revoked:{owner_hex}:{device_id}:{session_id}",
        ctx.community()
    )
}

fn owner_agents_key(ctx: &TenantContext, owner_hex: &str) -> String {
    format!(
        "{MAJU_PREFIX}:{}:device-agents:{owner_hex}",
        ctx.community()
    )
}

fn runner_lease_key(ctx: &TenantContext, agent_hex: &str) -> String {
    format!("{MAJU_PREFIX}:{}:agent-runner:{agent_hex}", ctx.community())
}

fn runner_meta_key(ctx: &TenantContext, agent_hex: &str) -> String {
    format!(
        "{MAJU_PREFIX}:{}:agent-runner-meta:{agent_hex}",
        ctx.community()
    )
}

fn standby_key(ctx: &TenantContext, agent_hex: &str, device_id: &str) -> String {
    format!(
        "{MAJU_PREFIX}:{}:agent-standby:{agent_hex}:{device_id}",
        ctx.community()
    )
}

fn runner_token(session_id: &str, runner_id: &str) -> String {
    format!("{session_id}:{runner_id}")
}

/// Register or refresh one authenticated account device.
pub async fn touch_device(
    pool: &Pool,
    ctx: &TenantContext,
    owner_hex: &str,
    descriptor: &DeviceDescriptor,
) -> Result<(), PubSubError> {
    let mut conn = pool.get().await?;
    let stored = StoredDevice {
        descriptor: descriptor.clone(),
        last_seen: now_secs(),
    };
    let json = serde_json::to_string(&stored)?;
    let _: () = redis::pipe()
        .atomic()
        .cmd("HSET")
        .arg(devices_key(ctx, owner_hex))
        .arg(&descriptor.device_id)
        .arg(json)
        .ignore()
        .cmd("SET")
        .arg(device_online_key(ctx, owner_hex, &descriptor.device_id))
        .arg(&descriptor.session_id)
        .arg("EX")
        .arg(DEVICE_ONLINE_TTL_SECS)
        .ignore()
        .query_async(&mut conn)
        .await?;
    Ok(())
}

/// Whether this exact official-client login session was disconnected.
pub async fn is_session_revoked(
    pool: &Pool,
    ctx: &TenantContext,
    owner_hex: &str,
    device_id: &str,
    session_id: &str,
) -> Result<bool, PubSubError> {
    let mut conn = pool.get().await?;
    let exists: bool = redis::cmd("EXISTS")
        .arg(revoked_key(ctx, owner_hex, device_id, session_id))
        .query_async(&mut conn)
        .await?;
    Ok(exists)
}

/// Mark a session disconnected and remove its live heartbeat.
pub async fn revoke_session(
    pool: &Pool,
    ctx: &TenantContext,
    owner_hex: &str,
    device_id: &str,
    session_id: &str,
) -> Result<(), PubSubError> {
    let mut conn = pool.get().await?;
    let _: () = redis::pipe()
        .atomic()
        .cmd("SET")
        .arg(revoked_key(ctx, owner_hex, device_id, session_id))
        .arg("1")
        .ignore()
        .cmd("DEL")
        .arg(device_online_key(ctx, owner_hex, device_id))
        .ignore()
        .query_async(&mut conn)
        .await?;
    Ok(())
}

/// Atomically acquire or renew the one representative lease for an agent.
pub async fn acquire_runner(
    pool: &Pool,
    ctx: &TenantContext,
    runner: &AgentRunner,
) -> Result<RunnerAdmission, PubSubError> {
    let mut conn = pool.get().await?;
    let lease_key = runner_lease_key(ctx, &runner.agent_pubkey);
    let meta_key = runner_meta_key(ctx, &runner.agent_pubkey);
    let standby = standby_key(ctx, &runner.agent_pubkey, &runner.device_id);
    let token = runner_token(&runner.session_id, &runner.runner_id);
    let meta = serde_json::to_string(runner)?;

    let acquired: Option<String> = redis::cmd("SET")
        .arg(&lease_key)
        .arg(&token)
        .arg("NX")
        .arg("EX")
        .arg(AGENT_RUNNER_LEASE_TTL_SECS)
        .query_async(&mut conn)
        .await?;

    if acquired.is_some() {
        let _: () = redis::pipe()
            .atomic()
            .cmd("SET")
            .arg(&meta_key)
            .arg(&meta)
            .arg("EX")
            .arg(AGENT_RUNNER_LEASE_TTL_SECS)
            .ignore()
            .cmd("DEL")
            .arg(&standby)
            .ignore()
            .cmd("SADD")
            .arg(owner_agents_key(ctx, &runner.owner_pubkey))
            .arg(&runner.agent_pubkey)
            .ignore()
            .query_async(&mut conn)
            .await?;
        return Ok(RunnerAdmission::Active { token });
    }

    let current_token: Option<String> = redis::cmd("GET")
        .arg(&lease_key)
        .query_async(&mut conn)
        .await?;
    if current_token.as_deref() == Some(token.as_str()) {
        renew_runner(pool, ctx, &runner.agent_pubkey, &token, runner).await?;
        return Ok(RunnerAdmission::Active { token });
    }

    let active_json: Option<String> = redis::cmd("GET")
        .arg(&meta_key)
        .query_async(&mut conn)
        .await?;
    let active = active_json
        .and_then(|json| serde_json::from_str::<AgentRunner>(&json).ok())
        .unwrap_or_else(|| runner.clone());

    let _: () = redis::pipe()
        .atomic()
        .cmd("SET")
        .arg(&standby)
        .arg(&meta)
        .arg("EX")
        .arg(AGENT_STANDBY_TTL_SECS)
        .ignore()
        .cmd("SADD")
        .arg(owner_agents_key(ctx, &runner.owner_pubkey))
        .arg(&runner.agent_pubkey)
        .ignore()
        .query_async(&mut conn)
        .await?;
    Ok(RunnerAdmission::Standby { active })
}

/// Renew a representative lease only when the caller still owns its token.
pub async fn renew_runner(
    pool: &Pool,
    ctx: &TenantContext,
    agent_hex: &str,
    token: &str,
    runner: &AgentRunner,
) -> Result<bool, PubSubError> {
    let mut conn = pool.get().await?;
    let meta = serde_json::to_string(runner)?;
    let script = redis::Script::new(
        r#"
        if redis.call('GET', KEYS[1]) == ARGV[1] then
          redis.call('EXPIRE', KEYS[1], ARGV[2])
          redis.call('SET', KEYS[2], ARGV[3], 'EX', ARGV[2])
          return 1
        end
        return 0
        "#,
    );
    let renewed: i64 = script
        .key(runner_lease_key(ctx, agent_hex))
        .key(runner_meta_key(ctx, agent_hex))
        .arg(token)
        .arg(AGENT_RUNNER_LEASE_TTL_SECS)
        .arg(meta)
        .invoke_async(&mut conn)
        .await?;
    Ok(renewed == 1)
}

/// Release a representative lease only when the caller still owns its token.
pub async fn release_runner(
    pool: &Pool,
    ctx: &TenantContext,
    agent_hex: &str,
    token: &str,
) -> Result<bool, PubSubError> {
    let mut conn = pool.get().await?;
    let script = redis::Script::new(
        r#"
        if redis.call('GET', KEYS[1]) == ARGV[1] then
          redis.call('DEL', KEYS[1])
          redis.call('DEL', KEYS[2])
          return 1
        end
        return 0
        "#,
    );
    let released: i64 = script
        .key(runner_lease_key(ctx, agent_hex))
        .key(runner_meta_key(ctx, agent_hex))
        .arg(token)
        .invoke_async(&mut conn)
        .await?;
    Ok(released == 1)
}

/// List all devices known for an account and merge live runner state.
pub async fn list_devices(
    pool: &Pool,
    ctx: &TenantContext,
    owner_hex: &str,
) -> Result<Vec<DeviceStatus>, PubSubError> {
    let mut conn = pool.get().await?;
    let raw: HashMap<String, String> = redis::cmd("HGETALL")
        .arg(devices_key(ctx, owner_hex))
        .query_async(&mut conn)
        .await?;
    let agents: Vec<String> = redis::cmd("SMEMBERS")
        .arg(owner_agents_key(ctx, owner_hex))
        .query_async(&mut conn)
        .await?;

    let mut devices = Vec::with_capacity(raw.len());
    for (_, json) in raw {
        let Ok(stored) = serde_json::from_str::<StoredDevice>(&json) else {
            continue;
        };
        let disconnected: bool = redis::cmd("EXISTS")
            .arg(revoked_key(
                ctx,
                owner_hex,
                &stored.descriptor.device_id,
                &stored.descriptor.session_id,
            ))
            .query_async(&mut conn)
            .await?;
        if disconnected {
            continue;
        }
        let online_session: Option<String> = redis::cmd("GET")
            .arg(device_online_key(
                ctx,
                owner_hex,
                &stored.descriptor.device_id,
            ))
            .query_async(&mut conn)
            .await?;
        let mut active_agents = Vec::new();
        let mut standby_agents = Vec::new();
        for agent in &agents {
            let active_json: Option<String> = redis::cmd("GET")
                .arg(runner_meta_key(ctx, agent))
                .query_async(&mut conn)
                .await?;
            if active_json
                .as_deref()
                .and_then(|value| serde_json::from_str::<AgentRunner>(value).ok())
                .is_some_and(|runner| runner.device_id == stored.descriptor.device_id)
            {
                active_agents.push(agent.clone());
            }
            let standby_exists: bool = redis::cmd("EXISTS")
                .arg(standby_key(ctx, agent, &stored.descriptor.device_id))
                .query_async(&mut conn)
                .await?;
            if standby_exists {
                standby_agents.push(agent.clone());
            }
        }
        active_agents.sort();
        standby_agents.sort();
        devices.push(DeviceStatus {
            device_id: stored.descriptor.device_id,
            session_id: stored.descriptor.session_id.clone(),
            name: stored.descriptor.name,
            platform: stored.descriptor.platform,
            app_version: stored.descriptor.app_version,
            last_seen: stored.last_seen,
            online: online_session.as_deref() == Some(stored.descriptor.session_id.as_str()),
            active_agents,
            standby_agents,
        });
    }
    devices.sort_by(|a, b| b.online.cmp(&a.online).then(b.last_seen.cmp(&a.last_seen)));
    Ok(devices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use maju_core::{CommunityId, TenantContext};
    use uuid::Uuid;

    fn ctx(id: u128) -> TenantContext {
        TenantContext::resolved(CommunityId::from_uuid(Uuid::from_u128(id)), "example.test")
    }

    #[test]
    fn keys_are_tenant_and_identity_scoped() {
        let a = ctx(1);
        let b = ctx(2);
        assert_ne!(devices_key(&a, "owner"), devices_key(&b, "owner"));
        assert_ne!(
            runner_lease_key(&a, "agent-a"),
            runner_lease_key(&a, "agent-b")
        );
        assert_ne!(
            revoked_key(&a, "owner", "device", "session-a"),
            revoked_key(&a, "owner", "device", "session-b")
        );
    }

    #[test]
    fn runner_token_is_bound_to_login_and_process() {
        assert_ne!(
            runner_token("session-a", "runner"),
            runner_token("session-b", "runner")
        );
        assert_ne!(
            runner_token("session", "runner-a"),
            runner_token("session", "runner-b")
        );
    }
}
