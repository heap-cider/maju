use nostr::EventId;
use tauri::State;

use crate::{app_state::AppState, events, relay::submit_event};

#[tauri::command]
pub async fn delete_message(
    channel_id: String,
    event_id: String,
    moderator_delete: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let channel_uuid = uuid::Uuid::parse_str(&channel_id)
        .map_err(|_| format!("invalid channel UUID: {channel_id}"))?;
    let target_eid =
        EventId::from_hex(&event_id).map_err(|error| format!("invalid event ID: {error}"))?;
    let builder = if moderator_delete.unwrap_or(false) {
        events::build_moderation_delete(channel_uuid, target_eid)?
    } else {
        events::build_delete_compat(channel_uuid, target_eid)?
    };
    submit_event(builder, &state).await?;
    Ok(())
}
