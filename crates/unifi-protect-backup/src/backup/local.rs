use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tracing::info;

use unifi_protect_client::events::ProtectEvent;

use crate::{Result, backup, backup::Backup};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct Config {
    pub path_buf: PathBuf,
}

pub struct LocalBackup {
    pub backup_config: backup::Config,
    pub remote_config: Config,
}

impl LocalBackup {
    pub fn new(backup_config: backup::Config, remote_config: Config) -> Self {
        Self {
            backup_config,
            remote_config,
        }
    }
}

#[async_trait]
impl Backup for LocalBackup {
    async fn backup(&self, event: &ProtectEvent, video_data: &[u8]) -> Result<String> {
        let filename = event.format_filename(&self.backup_config.file_structure_format);
        info!("Backing up event {} as {}", event.id, filename);

        // Use configured base path
        let file_path = self.remote_config.path_buf.join(&filename);

        // Create parent directories
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(video_data).await?;
        file.flush().await?;

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
