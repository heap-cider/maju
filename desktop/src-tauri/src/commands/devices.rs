use maju_core_pkg::kind::{KIND_DEVICE_SESSION, KIND_DEVICE_STATUS};
use nostr::{EventBuilder, Kind, Tag};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::app_state::AppState;
use crate::device_session::{self, LocalDeviceSession};
use crate::relay::{query_relay, submit_event};

#[derive(Debug, Clone, Deserialize)]
struct RelayDeviceStatus {
    device_id: String,
    session_id: String,
    name: String,
    platform: String,
    app_version: String,
    last_seen: u64,
    online: bool,
    active_agents: Vec<String>,
    standby_agents: Vec<String>,
}

/// One account device shown in Settings.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceStatusWire {
    pub device_id: String,
    pub session_id: String,
    pub name: String,
    pub platform: String,
    pub app_version: String,
    pub last_seen: u64,
    pub online: bool,
    pub current: bool,
    pub active_agents: Vec<String>,
    pub standby_agents: Vec<String>,
}

#[derive(Serialize)]
struct DeviceSessionContent<'a> {
    state: &'a str,
    session_id: &'a str,
    name: &'a str,
    platform: &'a str,
    app_version: &'a str,
}

async fn list_devices_inner(
    app: &AppHandle,
    state: &AppState,
) -> Result<Vec<DeviceStatusWire>, String> {
    let current = device_session::load_or_create(app)?;
    let owner = state.signing_keys()?.public_key().to_hex();
    let events = query_relay(
        state,
        &[serde_json::json!({
            "kinds": [KIND_DEVICE_STATUS],
            "authors": [owner],
        })],
    )
    .await?;
    let mut devices = Vec::new();
    for event in events {
        let Ok(device) = serde_json::from_str::<RelayDeviceStatus>(&event.content) else {
            continue;
        };
        devices.push(DeviceStatusWire {
            current: device.device_id == current.device_id
                && device.session_id == current.session_id,
            device_id: device.device_id,
            session_id: device.session_id,
            name: device.name,
            platform: device.platform,
            app_version: device.app_version,
            last_seen: device.last_seen,
            online: device.online,
            active_agents: device.active_agents,
            standby_agents: device.standby_agents,
        });
    }
    Ok(devices)
}

async fn publish_state(
    state: &AppState,
    session: &LocalDeviceSession,
    status: &str,
    platform: &str,
    app_version: &str,
) -> Result<(), String> {
    let content = serde_json::to_string(&DeviceSessionContent {
        state: status,
        session_id: &session.session_id,
        name: &session.name,
        platform,
        app_version,
    })
    .map_err(|error| format!("serialize device state: {error}"))?;
    let tag = Tag::parse(["d", session.device_id.as_str()])
        .map_err(|error| format!("device id tag: {error}"))?;
    let builder = EventBuilder::new(Kind::Custom(KIND_DEVICE_SESSION as u16), content).tags([tag]);
    submit_event(builder, state).await?;
    Ok(())
}

/// List logged-in devices for the current account.
#[tauri::command]
pub async fn list_logged_in_devices(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<DeviceStatusWire>, String> {
    list_devices_inner(&app, &state).await
}

/// Rename this installation and refresh its relay record.
#[tauri::command]
pub async fn rename_current_device(
    name: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let session = device_session::rename(&app, &name)?;
    publish_state(
        &state,
        &session,
        "connected",
        device_session::platform(),
        device_session::app_version(),
    )
    .await
}

/// Disconnect one other official-client session. The account key remains valid.
#[tauri::command]
pub async fn disconnect_logged_in_device(
    device_id: String,
    session_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let current = device_session::load_or_create(&app)?;
    if current.device_id == device_id && current.session_id == session_id {
        return Err("use Sign out to disconnect this device".to_string());
    }
    let target = list_devices_inner(&app, &state)
        .await?
        .into_iter()
        .find(|device| device.device_id == device_id && device.session_id == session_id)
        .ok_or_else(|| "device session is no longer available".to_string())?;
    let session = LocalDeviceSession {
        device_id: target.device_id,
        session_id: target.session_id,
        name: target.name,
    };
    publish_state(
        &state,
        &session,
        "disconnected",
        &target.platform,
        &target.app_version,
    )
    .await
}
