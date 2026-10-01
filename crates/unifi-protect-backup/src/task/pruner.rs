use std::sync::Arc;

use futures_util::future::join_all;
use tokio::time::interval;
use tracing::{info, warn};

use crate::{Result, context::Context};

pub struct Pruner {
    context: Arc<Context>,
    config: crate::backup::Config,
}

impl Pruner {
    pub fn new(context: Arc<Context>, config: crate::backup::Config) -> Self {
        Self { context, config }
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("Starting Backup Pruner");

        let mut interval = interval(self.config.purge_interval);

        loop {
            interval.tick().await;

            // Backup storage only. Archive targets prune themselves right after each archive
            // (see the Archiver), so their repository lock is never contended from here.
            let futs = self
                .context
                .backup_targets
                .as_slice()
                .iter()
                .map(|e| e.prune());

            let results = join_all(futs).await;

            for result in results {
                if let Err(err) = result {
                    warn!(err = ?err, "Failed to prune backup");
                }
            }
        }
    }
}
