use clap::Parser;
use tracing::{debug, info};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use unifi_protect_backup_rs::{Config, Result, config::Args};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_ansi(true))
        .with(EnvFilter::from_default_env())
        .init();

    debug!("Parsing config...");
    let args: Args<Config> = Args::parse();
    let config = args.config;
    debug!(config = ?config, "Parsed config successfully");

    info!("Starting UniFi Protect Backup");

    // todo(steve.sampson): implement main loop

    info!("UniFi Protect Backup Exiting...");
    Ok(())
}
