use std::sync::Arc;

use tracing::debug;

use unifi_protect_client::{ProtectClient, models::Bootstrap};
use unifi_protect_data::Database;

use crate::{
    archive::{Archive, archive_targets},
    backup::{Backup, backup_targets},
    config::Config,
    metrics::Metrics,
};

pub struct Context {
    pub protect_client: ProtectClient,
    pub protect_bootstrap: Bootstrap,
    pub backup_targets: Vec<Arc<dyn Backup>>, // dyn b/c we don't know the enabled backup targets until runtime (config-driven)
    pub archive_targets: Vec<Arc<dyn Archive>>, // dyn b/c we don't know the enabled archive targets until runtime (config-driven)
    pub database: Database,
    pub metrics: Arc<Metrics>,
}

impl Context {
    #[tracing::instrument(skip(config))]
    pub async fn new(config: Config) -> crate::Result<Self> {
        let protect_client = ProtectClient::new(config.unifi.clone())?;
        protect_client.login().await?;
        let protect_bootstrap = protect_client.get_bootstrap().await?;
        debug!(bootstrap_data = ?protect_bootstrap, "Received Bootstrap Data from Controller");

        let metrics = Arc::new(Metrics::default());

        Ok(Self {
            protect_client,
            protect_bootstrap,
            archive_targets: archive_targets(&config, &metrics),
            backup_targets: backup_targets(&config, &metrics),
            database: Database::new(config.database.path.as_path()).await?,
            metrics,
        })
    }
}
