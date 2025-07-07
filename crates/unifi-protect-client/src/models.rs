use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::{
    error::Error,
    events::{EventType, ProtectEvent, WebSocketMessage},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Camera {
    pub id: String,
    pub name: String,
    pub mac: String,
    pub model: String,
    pub is_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bootstrap {
    pub cameras: HashMap<String, Camera>,
    pub nvr: Nvr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nvr {
    pub id: String,
    pub name: String,
    pub version: String,
    pub timezone: String,
}

pub(crate) fn parse_camera(camera_data: &Value) -> crate::error::Result<Camera> {
    Ok(Camera {
        id: camera_data
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        name: camera_data
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        mac: camera_data
            .get("mac")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        model: camera_data
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        is_connected: camera_data
            .get("isConnected")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    })
}

pub(crate) fn parse_nvr(nvr_data: &Value) -> crate::error::Result<Nvr> {
    Ok(Nvr {
        id: nvr_data
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        name: nvr_data
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        version: nvr_data
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        timezone: nvr_data
            .get("timezone")
            .and_then(|v| v.as_str())
            .unwrap_or("UTC")
            .to_string(),
    })
}

pub(crate) fn parse_protect_event(
    original_ws_message: &WebSocketMessage,
    motion_event_completed_ws_message: &WebSocketMessage,
    known_camera: Option<&Camera>,
) -> crate::error::Result<ProtectEvent> {
    let Some(camera_id) = motion_event_completed_ws_message
        .action_frame
        .record_id
        .clone()
    else {
        return Err(Error::Api("Missing camera ID".to_string()));
    };

    let start = original_ws_message
        .data_frame
        .start
        .and_then(DateTime::from_timestamp_millis);
    let end = motion_event_completed_ws_message
        .data_frame
        .end
        .and_then(DateTime::from_timestamp_millis);

    Ok(ProtectEvent {
        id: motion_event_completed_ws_message.action_frame.id.clone(),
        camera_id,
        camera_name: known_camera.map(|c| c.name.clone()),
        start,
        end,
        event_type: EventType::Motion,
        smart_detect_types: vec![],
        thumbnail_id: None,
        heatmap_id: None,
        is_finished: motion_event_completed_ws_message.data_frame.end.is_some(),
    })
}
