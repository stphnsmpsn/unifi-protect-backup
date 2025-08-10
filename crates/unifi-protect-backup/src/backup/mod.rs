use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use unifi_protect_client::events::ProtectEvent;

use crate::{Result, metrics::Metrics, task::Prune};

pub mod local;
pub mod rclone;

#[async_trait]
pub trait Backup: Prune + Send + Sync {
    async fn backup(&self, event: &ProtectEvent, video_data: &[u8]) -> Result<String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct Config {
    #[serde(with = "humantime_serde")]
    pub retention_period: Duration,
    #[serde(with = "humantime_serde")]
    pub poll_interval: Duration,
    #[serde(with = "humantime_serde")]
    pub max_event_length: Duration,
    #[serde(with = "humantime_serde")]
    pub purge_interval: Duration,
    pub file_structure_format: String,
    pub detection_types: Vec<String>,
    pub ignore_cameras: Vec<String>,
    pub cameras: Vec<String>,
    pub download_buffer_size: u64,
    pub parallel_uploads: u32,
    pub skip_missing: bool,
    pub remote: Vec<RemoteBackupConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub enum RemoteBackupConfig {
    Local(local::Config),
    Rclone(rclone::Config),
}

pub fn backup_targets(
    config: &crate::config::Config,
    metrics: &Arc<Metrics>,
) -> Vec<Arc<dyn Backup>> {
    let mut targets = vec![];

    for remote in &config.backup.remote {
        targets.push(match remote {
            RemoteBackupConfig::Local(remote) => Arc::new(local::LocalBackup {
                backup_config: config.backup.clone(),
                remote_config: remote.clone(),
                metrics: metrics.local_backup.clone(),
            }) as Arc<dyn Backup>,
            RemoteBackupConfig::Rclone(remote) => Arc::new(rclone::RcloneBackup {
                backup_config: config.backup.clone(),
                remote_config: remote.clone(),
                metrics: metrics.rclone_backup.clone(),
            }) as Arc<dyn Backup>,
        });
    }

    targets
}
