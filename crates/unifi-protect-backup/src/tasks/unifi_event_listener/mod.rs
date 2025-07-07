use std::sync::Arc;

use crate::{Result, context::Context, convert};

pub async fn unifi_event_listener(context: Arc<Context>) -> Result<()> {
    let mut rx = context
        .protect_client
        .connect_websocket(context.protect_bootstrap.clone())
        .await?;
    loop {
        let Some(event) = rx.recv().await else {
            continue;
        };
        let database_event = convert::protect_event_to_database_event(event);
        context.database.insert_event(&database_event).await?;
    }
}
