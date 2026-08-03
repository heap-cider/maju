//! Persistent installation id and current account-login session metadata.

use std::path::Path;

use nostr::Tag;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const DEVICE_SESSION_FILE: &str = "device-session.json";

/// Local device/session metadata shared by the desktop socket and its ACP children.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalDeviceSession {
    /// Stable installation id.
    pub device_id: String,
    /// Current login session id. Importing an account key rotates it.
    pub session_id: String,
    /// User-facing installation name.
    pub name: String,
}

fn platform_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "desktop"
    }
}

fn default_device_name() -> String {
    ["COMPUTERNAME", "HOSTNAME"]
        .into_iter()
        .find_map(|key| {
            std::env::var(key)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| match platform_name() {
            "windows" => "Windows PC".to_string(),
            "linux" => "Linux PC".to_string(),
            "macos" => "Mac".to_string(),
            _ => "Maju device".to_string(),
        })
}

fn session_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join(DEVICE_SESSION_FILE)
}

fn write_at(data_dir: &Path, session: &LocalDeviceSession) -> Result<(), String> {
    std::fs::create_dir_all(data_dir)
        .map_err(|error| format!("create app data directory: {error}"))?;
    let bytes = serde_json::to_vec_pretty(session)
        .map_err(|error| format!("serialize device session: {error}"))?;
    std::fs::write(session_path(data_dir), bytes)
        .map_err(|error| format!("write device session: {error}"))
}

fn load_at(data_dir: &Path) -> Result<LocalDeviceSession, String> {
    let path = session_path(data_dir);
    if let Ok(bytes) = std::fs::read(&path) {
        if let Ok(session) = serde_json::from_slice::<LocalDeviceSession>(&bytes) {
            if Uuid::parse_str(&session.device_id).is_ok()
                && Uuid::parse_str(&session.session_id).is_ok()
                && !session.name.trim().is_empty()
            {
                return Ok(session);
            }
        }
    }
    let session = LocalDeviceSession {
        device_id: Uuid::new_v4().to_string(),
        session_id: Uuid::new_v4().to_string(),
        name: default_device_name(),
    };
    write_at(data_dir, &session)?;
    Ok(session)
}

/// Load or create this installation's device/session file.
pub fn load_or_create(app: &AppHandle) -> Result<LocalDeviceSession, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("app data dir: {error}"))?;
    load_at(&data_dir)
}

/// Rotate only the login session id. The installation id and name stay stable.
pub fn rotate_session(data_dir: &Path) -> Result<LocalDeviceSession, String> {
    let mut session = load_at(data_dir)?;
    session.session_id = Uuid::new_v4().to_string();
    write_at(data_dir, &session)?;
    Ok(session)
}

/// Rename this installation locally.
pub fn rename(app: &AppHandle, name: &str) -> Result<LocalDeviceSession, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > 80 {
        return Err("device name must be 1-80 bytes".to_string());
    }
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("app data dir: {error}"))?;
    let mut session = load_at(&data_dir)?;
    session.name = trimmed.to_string();
    write_at(&data_dir, &session)?;
    Ok(session)
}

/// Build the signed NIP-42 tag for the desktop socket or one ACP child.
pub fn auth_tag(session: &LocalDeviceSession, runner_id: Option<&str>) -> Result<Tag, String> {
    Tag::parse([
        "maju-device",
        "1",
        session.device_id.as_str(),
        session.session_id.as_str(),
        runner_id.unwrap_or(""),
        session.name.as_str(),
        platform_name(),
        env!("CARGO_PKG_VERSION"),
        if runner_id.is_some() {
            "agent"
        } else {
            "desktop"
        },
    ])
    .map_err(|error| format!("device auth tag: {error}"))
}

/// Platform string included in durable device status records.
pub fn platform() -> &'static str {
    platform_name()
}

/// Desktop app version included in durable device status records.
pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Add the signed installation/session coordinates to a managed agent child.
pub fn apply_agent_env(app: &AppHandle, command: &mut std::process::Command) -> Result<(), String> {
    let device = load_or_create(app)?;
    command
        .env("MAJU_DEVICE_ID", &device.device_id)
        .env("MAJU_DEVICE_SESSION_ID", &device.session_id)
        .env("MAJU_DEVICE_NAME", &device.name)
        .env("MAJU_DEVICE_PLATFORM", platform())
        .env("MAJU_DEVICE_VERSION", app_version())
        .env("MAJU_AGENT_RUNNER_ID", uuid::Uuid::new_v4().to_string());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_preserves_installation_and_changes_login() {
        let dir = tempfile::tempdir().unwrap();
        let before = load_at(dir.path()).unwrap();
        let after = rotate_session(dir.path()).unwrap();
        assert_eq!(before.device_id, after.device_id);
        assert_eq!(before.name, after.name);
        assert_ne!(before.session_id, after.session_id);
    }

    #[test]
    fn auth_tag_distinguishes_desktop_and_agent() {
        let session = LocalDeviceSession {
            device_id: Uuid::new_v4().to_string(),
            session_id: Uuid::new_v4().to_string(),
            name: "Office PC".to_string(),
        };
        let desktop = auth_tag(&session, None).unwrap();
        assert_eq!(desktop.as_slice()[8], "desktop");
        let runner = Uuid::new_v4().to_string();
        let agent = auth_tag(&session, Some(&runner)).unwrap();
        assert_eq!(agent.as_slice()[4], runner);
        assert_eq!(agent.as_slice()[8], "agent");
    }
}
