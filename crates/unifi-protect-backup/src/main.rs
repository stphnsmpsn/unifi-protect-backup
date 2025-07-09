use std::sync::Arc;

use clap::Parser;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use unifi_protect_backup::{
    Result,
    config::{Args, Config, check_and_create_config},
    context::Context,
    task,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_ansi(true))
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("info,sqlx=warn,reqwest=warn,hyper=warn,tungstenite=warn")
        }))
        .init();

    debug!("Parsing config...");
    let args: Args<Config> = Args::parse();

    // Only prompt for config setup if no config file was provided via --config
    if args.config.is_none() {
        check_and_create_config()
            .await
            .inspect_err(|err| error!(err = ?err, "Error checking for (or creating) config"))?;
    }

    let config = args
        .get_config()
        .inspect_err(|err| error!(err = ?err, "Error getting config"))?;
    debug!(config = ?config, "Parsed config successfully");

    info!("Starting UniFi Protect Backup");

    let context = Arc::new(Context::new(config.clone()).await?);
    let mut unifi_event_listener = task::UnifiEventListener::new(context.clone());
    let mut db_poller = task::BackupDbPoller::new(context.clone(), config.backup.clone());
    let mut archiver = task::Archiver::new(context.clone(), config.archive.clone());

    tokio::select! {
        res = unifi_event_listener.run() => {
            warn!("Unifi Event Listener stopped: {:?}", res);
        }
        res = db_poller.run() => {
            warn!("DB Poller stopped: {:?}", res);
        }
        res = archiver.run() => {
            warn!("Archiver stopped: {:?}", res);
        }
    }

    info!("Exiting...");
    Ok(())
}
