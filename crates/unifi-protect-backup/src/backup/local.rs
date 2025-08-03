use std::{path::PathBuf, time::SystemTime};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::{fs, io::AsyncWriteExt};
use tracing::{debug, info, warn};

use unifi_protect_client::events::ProtectEvent;

use crate::{Result, backup, backup::Backup, task::Prune};

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

    async fn prune_directory(
        &self,
        dir_path: &PathBuf,
        cutoff_time: SystemTime,
        total_deleted: &mut i32,
        total_size_freed: &mut u64,
    ) -> Result<()> {
        let mut dir_entries = fs::read_dir(dir_path).await?;

        while let Some(entry) = dir_entries.next_entry().await? {
            let path = entry.path();
            let metadata = entry.metadata().await?;

            if metadata.is_dir() {
                // Recursively prune subdirectories
                if let Err(e) = Box::pin(self.prune_directory(
                    &path,
                    cutoff_time,
                    total_deleted,
                    total_size_freed,
                ))
                .await
                {
                    warn!("Failed to prune directory {}: {}", path.display(), e);
                    continue;
                }

                // Try to remove empty directories
                if let Ok(mut empty_check) = fs::read_dir(&path).await {
                    if empty_check.next_entry().await?.is_none() {
                        if let Err(e) = fs::remove_dir(&path).await {
                            debug!("Failed to remove empty directory {}: {}", path.display(), e);
                        } else {
                            debug!("Removed empty directory: {}", path.display());
                        }
                    }
                }
            } else if metadata.is_file() {
                // Check if file is older than retention period
                if let Ok(modified_time) = metadata.modified() {
                    if modified_time < cutoff_time {
                        let file_size = metadata.len();

                        match fs::remove_file(&path).await {
                            Ok(()) => {
                                debug!("Pruned old file: {}", path.display());
                                *total_deleted += 1;
                                *total_size_freed += file_size;
                            }
                            Err(e) => {
                                warn!("Failed to remove file {}: {}", path.display(), e);
                            }
                        }
                    }
                } else {
                    warn!(
                        "Could not get modification time for file: {}",
                        path.display()
                    );
                }
            }
        }

        Ok(())
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
}

#[async_trait]
impl Prune for LocalBackup {
    async fn prune(&self) -> Result<()> {
        info!(
            "Pruning old backups from local storage (retention: {:?})",
            self.backup_config.retention_period
        );

        let retention_period = self.backup_config.retention_period;
        let cutoff_time = SystemTime::now()
            .checked_sub(retention_period)
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid retention period")
            })?;

        let mut total_deleted = 0;
        let mut total_size_freed = 0u64;

        match self
            .prune_directory(
                &self.remote_config.path_buf,
                cutoff_time,
                &mut total_deleted,
                &mut total_size_freed,
            )
            .await
        {
            Ok(()) => {
                info!(
                    "Successfully pruned {} files, freed {} bytes from local storage",
                    total_deleted, total_size_freed
                );
                Ok(())
            }
            Err(e) => {
                warn!("Error during pruning: {}", e);
                Err(e)
            }
        }
    }
}
