use std::sync::Arc;

use futures_util::future::join_all;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::{Error, Result, context::Context, convert::protect_event_from_database_event};

const BATCH_SIZE: usize = 10;

pub struct BackupDbPoller {
    context: Arc<Context>,
    config: crate::backup::Config,
}

impl BackupDbPoller {
    pub fn new(context: Arc<Context>, config: crate::backup::Config) -> Self {
        Self { context, config }
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("Starting DB Poller");

        let mut interval = interval(self.config.poll_interval);

        loop {
            interval.tick().await;

            let pending_backup = self.context.database.get_events_not_backed_up().await?;

            if pending_backup.is_empty() {
                continue;
            }

            info!("Found {} events pending backup", pending_backup.len());

            // Process events in batches of BATCH_SIZE
            for batch in pending_backup.chunks(BATCH_SIZE) {
                let batch_futures = batch.iter().map(|event| {
                    let context = Arc::clone(&self.context);
                    let event = event.clone();

                    async move { process_event(context, event).await }
                });

                // Wait for all events in this batch to complete
                let results = join_all(batch_futures).await;

                // Log any errors from the batch processing
                for result in results.into_iter() {
                    if let Err(e) = result {
                        error!("Failed to process event in batch: {}", e);
                    }
                }
            }
        }
    }
}

async fn process_event(context: Arc<Context>, event: unifi_protect_data::Event) -> Result<()> {
    info!("Processing event: {}", event.id);

    let Some(end_time) = event.end_time else {
        return Err(Error::Backup(
            "Can not back up ongoing event...".to_string(),
        ));
    };

    // 1. Download video data from UniFi Protect
    debug!(event_id = event.id, "Downloading Motion Event");
    let video_data = context
        .protect_client
        .download_event_video(event.camera_id.as_str(), event.start_time, end_time)
        .await?;

    let event_id = event.id.clone();
    let protect_event = protect_event_from_database_event(event, &context.protect_bootstrap);
    // todo(steve.sampson): parallelize backups to different targets
    let mut error = false;
    for target in context.backup_targets.as_slice() {
        // 2. Run backup operations using configured backup targets
        let _ = target
            .backup(&protect_event, video_data.as_slice())
            .await
            .inspect_err(|err| {
                warn!(err= ?err, "Failed to create backup");
                error = true;
            });
    }

    if !error {
        // 3. Update database to mark event as backed up (assuming no error backing up to any targets)
        context
            .database
            .mark_event_backed_up(event_id.as_str())
            .await?;
    }

    Ok(())
}
