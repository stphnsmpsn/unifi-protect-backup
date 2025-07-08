use std::sync::Arc;

use tracing::debug;

use unifi_protect_client::{ProtectClient, models::Bootstrap};
use unifi_protect_data::Database;

use crate::{
    backup::{Backup, RemoteConfig},
    config::Config,
};

pub struct Context {
    pub protect_client: ProtectClient,
    pub protect_bootstrap: Bootstrap,
    pub backup_target: Arc<dyn Backup>, // dyn b/c we don't know the backup target until runtime (config-driven)
    pub database: Database,
}

impl Context {
    pub async fn new(config: Config) -> crate::Result<Self> {
        let mut protect_client = ProtectClient::new(config.unifi)?;

        protect_client.login().await?;
        let protect_bootstrap = protect_client.get_bootstrap().await?;
        debug!(bootstrap_data = ?protect_bootstrap, "Received Bootstrap Data from Controller");

        Ok(Self {
            protect_client,
            protect_bootstrap,
            backup_target: Arc::new(match &config.backup.remote {
                RemoteConfig::Borg(remote) => crate::backup::borg::BorgBackup {
                    config: config.backup.clone(),
                    remote: remote.clone(),
                },
            }),
            database: Database::new(config.database.path.as_path()).await?,
        })
    }
}
