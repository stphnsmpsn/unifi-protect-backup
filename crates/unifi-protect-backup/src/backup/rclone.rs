use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tempfile::NamedTempFile;
use tokio::{io::AsyncWriteExt, process::Command};
use tracing::{debug, info, trace};
use unifi_protect_client::events::ProtectEvent;

use crate::{Error, Result, backup, backup::Backup, task::Prune};

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
    #[tracing::instrument(skip(self, video_data))]
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
}

#[async_trait]
impl Prune for RcloneBackup {
    #[tracing::instrument(skip(self))]
    async fn prune(&self) -> Result<()> {
        info!(
            "Pruning old backups from rclone remote (retention: {:?})",
            self.backup_config.retention_period
        );

        // Convert Duration to a format rclone understands (e.g., "30d", "720h", "43200m")
        let retention_days = self.backup_config.retention_period.as_secs() / (24 * 60 * 60);
        let min_age = if retention_days > 0 {
            format!("{retention_days}d")
        } else {
            // Fallback to hours if less than a day
            let retention_hours = self.backup_config.retention_period.as_secs() / (60 * 60);
            if retention_hours > 0 {
                format!("{retention_hours}h")
            } else {
                // Fallback to minutes
                let retention_minutes = self.backup_config.retention_period.as_secs() / 60;
                format!("{}m", retention_minutes.max(1)) // Ensure at least 1 minute
            }
        };

        let remote_path = format!(
            "{}:/{}",
            self.remote_config.remote,
            self.remote_config
                .base_path
                .trim_start_matches('/')
                .trim_end_matches('/')
        );

        debug!("Pruning files older than {} from {}", min_age, remote_path);

        // First, do a dry run to see what would be deleted
        let dry_run_output = Command::new("rclone")
            .arg("delete")
            .arg(&remote_path)
            .arg("--min-age")
            .arg(&min_age)
            .arg("--dry-run")
            .arg("--verbose")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| Error::Backup(format!("Failed to execute rclone dry-run: {e}")))?;

        if !dry_run_output.status.success() {
            let stderr = String::from_utf8_lossy(&dry_run_output.stderr);
            return Err(Error::Backup(format!("Rclone dry-run failed: {stderr}")));
        }

        // rclone prints dry run info to stderr
        let dry_run_stderr = String::from_utf8_lossy(&dry_run_output.stderr);
        let files_to_delete: Vec<&str> = dry_run_stderr
            .lines()
            .filter(|line| line.contains("Skipped delete as --dry-run is set"))
            .collect();

        if files_to_delete.is_empty() {
            info!(
                remote = self.remote_config.remote,
                min_age = min_age,
                "No files older than {} found to prune",
                min_age
            );
            return Ok(());
        }

        info!(
            "Found {} files to delete that are older than {}",
            files_to_delete.len(),
            min_age
        );

        // Execute actual rclone delete command with --min-age filter
        let output = Command::new("rclone")
            .arg("delete")
            .arg(&remote_path)
            .arg("--min-age")
            .arg(&min_age)
            .arg("--verbose")
            .arg("--b2-hard-delete")
            .arg("--stats")
            .arg("1s")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| Error::Backup(format!("Failed to execute rclone delete: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Backup(format!("Rclone prune failed: {stderr}")));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("Rclone delete output: {}", stdout);

        // Run cleanup to remove hidden versions on B2
        info!("Running cleanup to remove hidden file versions from B2");
        let cleanup_output = Command::new("rclone")
            .arg("cleanup")
            .arg(&remote_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| Error::Backup(format!("Failed to execute rclone cleanup: {e}")))?;

        if !cleanup_output.status.success() {
            let stderr = String::from_utf8_lossy(&cleanup_output.stderr);
            debug!("Rclone cleanup warning (may be normal): {}", stderr);
        }

        info!(
            remote = self.remote_config.remote,
            min_age = min_age,
            files_deleted = files_to_delete.len(),
            "Successfully pruned old backups from rclone remote and cleaned up hidden versions"
        );

        Ok(())
    }
}

impl RcloneBackup {
    #[tracing::instrument(skip(self, video_data))]
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

    #[tracing::instrument(skip(self, video_data))]
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

    #[tracing::instrument(skip(self, video_data))]
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
