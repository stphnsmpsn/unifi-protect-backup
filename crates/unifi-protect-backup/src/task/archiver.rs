use std::sync::Arc;

use tokio::time::interval;
use tracing::{info, warn};

use crate::{Result, context::Context};

pub struct Archiver {
    context: Arc<Context>,
    config: crate::archive::Config,
}

impl Archiver {
    pub fn new(context: Arc<Context>, config: crate::archive::Config) -> Self {
        Self { context, config }
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("Starting Archiver");

        let mut interval = interval(self.config.archive_interval);

        loop {
            interval.tick().await;
            for archiver in self.context.archive_targets.as_slice() {
                let _ = archiver.archive().await.inspect_err(|err| {
                    warn!(err = ?err, "Failed to create archive");
                });
            }
        }
    }
}
