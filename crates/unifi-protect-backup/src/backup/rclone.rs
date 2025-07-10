use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use tokio::{io::AsyncWriteExt, process::Command};
use tracing::{debug, info, trace};
use unifi_protect_client::events::ProtectEvent;

use crate::{Error, Result, backup, backup::Backup};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct Config {
    pub remote: String,
    pub base_path: String,
}

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
    async fn backup(&self, event: &ProtectEvent, video_data: &[u8]) -> Result<String> {
        let filename = event.format_filename(&self.backup_config.file_structure_format);

        let temp_file = NamedTempFile::new()
            .map_err(|e| Error::Backup(format!("Failed to create temp file: {e}")))?;
        let temp_path = temp_file.path();

        let mut file = tokio::fs::File::create(temp_path)
            .await
            .map_err(|e| Error::Backup(format!("Failed to open temp file: {e}")))?;

        file.write_all(video_data)
            .await
            .map_err(|e| Error::Backup(format!("Failed to write video data: {e}")))?;

        file.flush()
            .await
            .map_err(|e| Error::Backup(format!("Failed to flush temp file: {e}")))?;

        let dest_path = format!(
            "{}:/{}/{}",
            self.remote_config.remote,
            self.remote_config
                .base_path
                .trim_start_matches('/')
                .trim_end_matches('/'),
            filename
        );

        debug!("Uploading {} to {}", temp_path.display(), dest_path);

        // Execute rclone copyto command (copies file to specific destination name)
        let output = Command::new("rclone")
            .arg("copyto")
            .arg(temp_path)
            .arg(&dest_path)
            .arg("--progress")
            .output()
            .await
            .map_err(|e| Error::Backup(format!("Failed to execute rclone: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Backup(format!("Rclone upload failed: {stderr}")));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        trace!("Rclone output: {}", stdout);

        info!(
            filename = filename,
            remote = self.remote_config.remote,
            dest_path = dest_path,
            "Successfully backed up event to rclone remote"
        );

        Ok(filename)
    }

    async fn prune(&self) -> Result<()> {
        todo!("Not implemented")
    }
}
