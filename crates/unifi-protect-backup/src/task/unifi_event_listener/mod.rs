use std::sync::Arc;

use tracing::{info, warn};

use unifi_protect_client::events::{Kind, WebSocketAction, WebSocketMessage};
use unifi_protect_data::Event;

use crate::{Result, context::Context, convert, convert::protect_event_from_parts};

pub struct UnifiEventListener {
    context: Arc<Context>,
}

impl UnifiEventListener {
    pub fn new(context: Arc<Context>) -> Self {
        Self { context }
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut rx = self.context.protect_client.connect_websocket().await?;
        loop {
            let Some(ws_message) = rx.recv().await else {
                continue;
            };

            match State::from(ws_message) {
                State::NewMotionEvent(NewMotionEvent {
                    id,
                    start_time,
                    ws_message,
                }) => {
                    self.process_new_motion_event(id, start_time, ws_message)
                        .await?
                }
                State::CompletedMotionEvent(CompletedMotionEvent {
                    id,
                    end_time,
                    ws_message,
                }) => {
                    self.process_completed_motion_event(id, end_time, ws_message)
                        .await?
                }

                State::Other => continue,
            };
        }
    }

    async fn process_new_motion_event(
        &mut self,
        id: String,
        start_time: i64,
        _ws_message: WebSocketMessage,
    ) -> Result<()> {
        self.context
            .database
            .insert_event(&Event {
                id,
                event_type: "Motion".to_string(),
                camera_id: "".to_string(),
                start_time,
                end_time: None,
                backed_up: false,
            })
            .await?;
        Ok(())
    }

    async fn process_completed_motion_event(
        &mut self,
        id: String,
        _end_time: i64,
        ws_message: WebSocketMessage,
    ) -> Result<()> {
        let bootstrap = &self.context.protect_bootstrap;

        // it is a backup candidate!
        let Some(motion_detected_db_event) =
            self.context.database.get_event_by_id(id.as_str()).await?
        else {
            warn!(
                "We missed the start of this motion event and can't get the start time for it to export"
            );
            return Ok(());
        };

        let motion_event_completed_ws_message = ws_message;
        let known_camera = motion_event_completed_ws_message
            .action_frame
            .record_id
            .as_ref()
            .and_then(|c| bootstrap.cameras.get(c));

        if let Ok(event) = protect_event_from_parts(
            &motion_detected_db_event,
            &motion_event_completed_ws_message,
            known_camera,
        ) {
            info!(
                id = event.id,
                camera_name = event.camera_name,
                event_type = event.event_type.to_string(),
                "Detected event. Persisting record pending backup."
            );
            let database_event = convert::protect_event_to_database_event(&event);
            self.context.database.insert_event(&database_event).await?;
        }

        Ok(())
    }
}

struct NewMotionEvent {
    id: String,
    start_time: i64,
    ws_message: WebSocketMessage,
}
struct CompletedMotionEvent {
    id: String,
    end_time: i64,
    ws_message: WebSocketMessage,
}

enum State {
    NewMotionEvent(NewMotionEvent),
    CompletedMotionEvent(CompletedMotionEvent),
    Other,
}

impl From<WebSocketMessage> for State {
    fn from(ws_message: WebSocketMessage) -> Self {
        match (
            &ws_message.action_frame.action,
            &ws_message.action_frame.record_id,
            &ws_message.data_frame.kind,
            &ws_message.data_frame.id,
            &ws_message.data_frame.start,
            &ws_message.data_frame.end,
        ) {
            (WebSocketAction::Add, _, Some(Kind::Motion), Some(id), Some(start_time), _) => {
                Self::NewMotionEvent(NewMotionEvent {
                    id: id.clone(),
                    start_time: *start_time,
                    ws_message: ws_message.clone(),
                })
            }
            (WebSocketAction::Update, _, _, _, _, Some(end_time)) => {
                Self::CompletedMotionEvent(CompletedMotionEvent {
                    id: ws_message.action_frame.id.clone(),
                    end_time: *end_time,
                    ws_message: ws_message.clone(),
                })
            }
            _ => Self::Other,
        }
    }
}
