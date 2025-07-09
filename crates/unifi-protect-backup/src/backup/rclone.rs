use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::info;
use unifi_protect_client::events::ProtectEvent;

use crate::{Result, backup, backup::Backup};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct Config {}

pub struct RcloneBackup {
    pub backup_config: backup::Config,
    pub remote_config: Config,
}

impl RcloneBackup {
    pub fn new(backup_config: backup::Config, remote_config: Config) -> Self {
        Self {
            backup_config,
            remote_config,
        }
    }
}

#[async_trait]
impl Backup for RcloneBackup {
    async fn backup(&self, event: &ProtectEvent, _video_data: &[u8]) -> Result<String> {
        let filename = event.format_filename(&self.backup_config.file_structure_format);
        info!("Backing up event {} as {}", event.id, filename);

        // todo(steve.sampson): Implement

        info!(
            filename = filename,
            "Backed up motion event to local storage"
        );
        Ok(filename)
    }

    async fn prune(&self) -> Result<()> {
        todo!("Not implemented")
    }
}
