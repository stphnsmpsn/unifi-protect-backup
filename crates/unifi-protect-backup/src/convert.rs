use unifi_protect_client::events::ProtectEvent;

pub fn protect_event_to_database_event(protect_event: ProtectEvent) -> crate::database::Event {
    crate::database::Event {
        id: protect_event.id,
        event_type: protect_event.event_type.to_string(),
        camera_id: protect_event.camera_id,
        start_time: protect_event.start.unwrap(),
        end_time: protect_event.end.unwrap(),
        backed_up: false,
    }
}
