use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use unifi_protect_client::events::ProtectEvent;

use crate::Result;

pub mod borg;

#[async_trait]
pub trait Backup {
    async fn init(&self) -> Result<()>;
    async fn backup(&self, event: &ProtectEvent, video_data: &[u8]) -> Result<String>;
    async fn prune(&self) -> Result<()>;
    async fn list_archives(&self) -> Result<Vec<String>>;
    async fn check_repository(&self) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct Config {
    pub retention_days: u32,
    pub file_structure_format: String,
    pub detection_types: Vec<String>,
    pub ignore_cameras: Vec<String>,
    pub cameras: Vec<String>,
    pub max_event_length_seconds: u32,
    pub download_buffer_size: u64,
    pub parallel_uploads: u32,
    pub purge_interval_hours: u32,
    pub skip_missing: bool,
    pub remote: RemoteConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub enum RemoteConfig {
    Borg(borg::Config),
}
