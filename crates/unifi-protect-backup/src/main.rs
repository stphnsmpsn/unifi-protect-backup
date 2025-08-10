use std::sync::Arc;

use clap::Parser;
use tracing::{debug, error, info, warn};

use unifi_protect_backup::{
    Result,
    config::{Args, Config, check_and_create_config},
    context::Context,
    opentelemetry, task,
};

#[tokio::main]
async fn main() -> Result<()> {
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

    let maybe_loki_task = opentelemetry::init(&config);

    info!(
        "Starting {} v{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    let context = Arc::new(Context::new(config.clone()).await?);
    let mut unifi_event_listener = task::UnifiEventListener::new(context.clone());
    let mut db_poller = task::BackupDbPoller::new(context.clone(), config.backup.clone());
    let mut archiver = task::Archiver::new(context.clone(), config.archive.clone());
    let mut pruner = task::Pruner::new(context.clone(), config.backup.clone());

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
        res = pruner.run() => {
            warn!("Pruner stopped: {:?}", res);
        }
        res = async {
          if let Some(loki_task) = maybe_loki_task {
              loki_task.await
          } else {
              std::future::pending().await // Never resolves
          }
      } => {
          warn!("Loki task stopped: {:?}", res);
      }
    }

    info!("Exiting...");
    Ok(())
}
