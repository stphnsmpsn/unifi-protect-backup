use unifi_protect_client::{
    events::{EventType, ProtectEvent, WebSocketMessage},
    models::{Bootstrap, Camera},
};
use unifi_protect_data::Event;

use crate::{Error, Result};

pub fn protect_event_to_database_event(protect_event: &ProtectEvent) -> Event {
    Event {
        id: protect_event.id.clone(),
        event_type: protect_event.event_type.to_string(),
        camera_id: protect_event.camera_id.clone(),
        start_time: protect_event.start_time.unwrap(),
        end_time: protect_event.end_time,
        backed_up: false,
    }
}

pub fn protect_event_from_database_event(event: Event, bootstrap: &Bootstrap) -> ProtectEvent {
    ProtectEvent {
        id: event.id,
        camera_id: event.camera_id.clone(),
        camera_name: bootstrap
            .cameras
            .get(&event.camera_id)
            .map(|c| c.name.clone()),
        start_time: Some(event.start_time),
        end_time: event.end_time,
        event_type: EventType::Motion, // todo(steve.sampson): extract this
        smart_detect_types: vec![],    // todo(steve.sampson): extract this
        thumbnail_id: None,            // todo(steve.sampson): extract this
        heatmap_id: None,              // todo(steve.sampson): extract this
        is_finished: event.end_time.is_some(),
    }
}

pub fn protect_event_from_parts(
    motion_detected_db_event: &Event,
    motion_event_completed_ws_message: &WebSocketMessage,
    known_camera: Option<&Camera>,
) -> Result<ProtectEvent> {
    let Some(camera_id) = motion_event_completed_ws_message
        .action_frame
        .record_id
        .clone()
    else {
        return Err(Error::Api("Missing camera ID".to_string()));
    };

    Ok(ProtectEvent {
        id: motion_event_completed_ws_message.action_frame.id.clone(),
        camera_id,
        camera_name: known_camera.map(|c| c.name.clone()),
        start_time: Some(motion_detected_db_event.start_time),
        end_time: motion_event_completed_ws_message.data_frame.end,
        event_type: EventType::Motion,
        smart_detect_types: vec![],
        thumbnail_id: None,
        heatmap_id: None,
        is_finished: motion_event_completed_ws_message.data_frame.end.is_some(),
    })
}
