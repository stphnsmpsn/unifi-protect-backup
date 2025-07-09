use std::sync::Arc;

use tracing::debug;

use unifi_protect_client::{ProtectClient, models::Bootstrap};
use unifi_protect_data::Database;

use crate::{
    backup,
    backup::{Backup, RemoteBackupConfig},
    config::Config,
};

pub struct Context {
    pub protect_client: ProtectClient,
    pub protect_bootstrap: Bootstrap,
    pub backup_targets: Vec<Arc<dyn Backup>>, // dyn b/c we don't know the enabled backup targets until runtime (config-driven)
    pub database: Database,
}

impl Context {
    pub async fn new(config: Config) -> crate::Result<Self> {
        let mut protect_client = ProtectClient::new(config.unifi.clone())?;

        protect_client.login().await?;
        let protect_bootstrap = protect_client.get_bootstrap().await?;
        debug!(bootstrap_data = ?protect_bootstrap, "Received Bootstrap Data from Controller");

        Ok(Self {
            protect_client,
            protect_bootstrap,
            backup_targets: backup_targets(&config),
            database: Database::new(config.database.path.as_path()).await?,
        })
    }
}

fn backup_targets(config: &Config) -> Vec<Arc<dyn Backup>> {
    let mut targets = vec![];

    for remote in &config.backup.remote {
        targets.push(match remote {
            RemoteBackupConfig::Local(remote) => Arc::new(backup::local::LocalBackup {
                backup_config: config.backup.clone(),
                remote_config: remote.clone(),
            }) as Arc<dyn Backup>,
            RemoteBackupConfig::Rclone(remote) => Arc::new(backup::rclone::RcloneBackup {
                backup_config: config.backup.clone(),
                remote_config: remote.clone(),
            }) as Arc<dyn Backup>,
        });
    }

    targets
}
