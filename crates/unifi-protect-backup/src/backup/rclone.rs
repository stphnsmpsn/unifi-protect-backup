use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
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
    #[serde(default)]
    pub stream_upload: bool,
    #[serde(default)]
    pub chunk_stream_uploads: bool,
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

        let dest_path = format!(
            "{}:/{}/{}",
            self.remote_config.remote,
            self.remote_config
                .base_path
                .trim_start_matches('/')
                .trim_end_matches('/'),
            filename
        );

        if self.remote_config.stream_upload {
            if self.remote_config.chunk_stream_uploads {
                // Use chunked streaming upload
                self.chunked_stream_upload(video_data, &dest_path, &filename)
                    .await
            } else {
                // Use single write streaming upload
                self.single_stream_upload(video_data, &dest_path, &filename)
                    .await
            }
        } else {
            // Use traditional temp file upload
            self.temp_file_upload(video_data, &dest_path, &filename)
                .await
        }
    }

    async fn prune(&self) -> Result<()> {
        todo!("Not implemented")
    }
}

impl RcloneBackup {
    async fn single_stream_upload(
        &self,
        video_data: &[u8],
        dest_path: &str,
        filename: &str,
    ) -> Result<String> {
        debug!(
            "Single stream upload {} bytes to {}",
            video_data.len(),
            dest_path
        );

        // Execute rclone rcat command with size parameter
        let mut child = Command::new("rclone")
            .arg("rcat")
            .arg(dest_path)
            .arg("--size")
            .arg(video_data.len().to_string())
            .arg("--progress")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Backup(format!("Failed to spawn rclone rcat: {e}")))?;

        // Take stdin handle and write all data at once
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| Error::Backup("Failed to get stdin handle".to_string()))?;

            // Write all data at once
            stdin
                .write_all(video_data)
                .await
                .map_err(|e| Error::Backup(format!("Failed to write data to rclone stdin: {e}")))?;

            // Ensure all data is flushed
            stdin
                .flush()
                .await
                .map_err(|e| Error::Backup(format!("Failed to flush stdin: {e}")))?;

            // Close stdin to signal end of data (stdin is dropped automatically here)
        }

        // Wait for command to complete
        let output = child
            .wait_with_output()
            .await
            .map_err(|e| Error::Backup(format!("Failed to wait for rclone rcat: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Backup(format!(
                "Rclone single stream upload failed: {stderr}"
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        trace!("Rclone rcat output: {}", stdout);

        info!(
            filename = filename,
            remote = self.remote_config.remote,
            dest_path = dest_path,
            size_bytes = video_data.len(),
            "Successfully single streamed event to rclone remote"
        );

        Ok(filename.to_string())
    }

    async fn chunked_stream_upload(
        &self,
        video_data: &[u8],
        dest_path: &str,
        filename: &str,
    ) -> Result<String> {
        debug!(
            "Chunked stream upload {} bytes to {}",
            video_data.len(),
            dest_path
        );

        // Execute rclone rcat command with size parameter
        let mut child = Command::new("rclone")
            .arg("rcat")
            .arg(dest_path)
            .arg("--size")
            .arg(video_data.len().to_string())
            .arg("--progress")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Backup(format!("Failed to spawn rclone rcat: {e}")))?;

        // Take stdin handle (this moves it out of the child)
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| Error::Backup("Failed to get stdin handle".to_string()))?;

            // Stream data in chunks to avoid memory pressure
            const CHUNK_SIZE: usize = 100 * 1024 * 1024; // 100MiB chunks
            for chunk in video_data.chunks(CHUNK_SIZE) {
                stdin.write_all(chunk).await.map_err(|e| {
                    Error::Backup(format!("Failed to write chunk to rclone stdin: {e}"))
                })?;
            }

            // Ensure all data is flushed
            stdin
                .flush()
                .await
                .map_err(|e| Error::Backup(format!("Failed to flush stdin: {e}")))?;

            // Close stdin to signal end of data (stdin is dropped automatically here)
        }

        // Wait for command to complete
        let output = child
            .wait_with_output()
            .await
            .map_err(|e| Error::Backup(format!("Failed to wait for rclone rcat: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Backup(format!(
                "Rclone chunked stream upload failed: {stderr}"
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        trace!("Rclone rcat output: {}", stdout);

        info!(
            filename = filename,
            remote = self.remote_config.remote,
            dest_path = dest_path,
            size_bytes = video_data.len(),
            "Successfully chunked streamed event to rclone remote"
        );

        Ok(filename.to_string())
    }

    async fn temp_file_upload(
        &self,
        video_data: &[u8],
        dest_path: &str,
        filename: &str,
    ) -> Result<String> {
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

        debug!("Uploading {} to {}", temp_path.display(), dest_path);

        // Execute rclone copyto command (copies file to specific destination name)
        let output = Command::new("rclone")
            .arg("copyto")
            .arg(temp_path)
            .arg(dest_path)
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

        Ok(filename.to_string())
    }
}
