use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{Result, task::Prune};

pub mod borg;

#[async_trait]
pub trait Archive: Prune + Send + Sync {
    async fn archive(&self) -> Result<String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct Config {
    #[serde(with = "humantime_serde")]
    pub archive_interval: Duration,
    #[serde(with = "humantime_serde")]
    pub retention_period: Duration,
    #[serde(with = "humantime_serde")]
    pub purge_interval: Duration,
    pub remote: Vec<RemoteArchiveConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub enum RemoteArchiveConfig {
    Borg(borg::Config),
}

pub fn archive_targets(config: &crate::config::Config) -> Vec<Arc<dyn Archive>> {
    let mut targets = vec![];

    for remote in &config.archive.remote {
        targets.push(match remote {
            RemoteArchiveConfig::Borg(remote) => Arc::new(borg::BorgBackup {
                backup_config: config.archive.clone(),
                remote_config: remote.clone(),
            }) as Arc<dyn Archive>,
        });
    }

    targets
}
