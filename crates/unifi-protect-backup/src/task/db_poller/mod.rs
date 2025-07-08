use std::sync::Arc;

use crate::{Result, context::Context};

pub async fn db_poller(_context: Arc<Context>) -> Result<()> {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(7500)).await;
        // info!("Downloading Motion Event");
        // let video_data = context
        //     .protect_client
        //     .download_event_video(
        //         event.camera_id.as_str(),
        //         event.start.unwrap(),
        //         event.end.unwrap(),
        //     )
        //     .await?;
        // info!("Saving Motion Event to Disk");
        // let filename = context
        //     .backup_target
        //     .backup(&event, video_data.as_slice())
        //     .await?;
        // info!(filename = filename, "Backed up motion event");
    }
}
