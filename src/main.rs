use clap::Parser;
use tracing::{debug, info};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use unifi_protect_backup_rs::{
    Config, Result,
    config::{Args, check_and_create_config},
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_ansi(true))
        .with(EnvFilter::from_default_env())
        .init();

    debug!("Parsing config...");
    let args: Args<Config> = Args::parse();

    // Only prompt for config setup if no config file was provided via --config
    if args.config.is_none() {
        check_and_create_config().await?;
    }

    let config = args.get_config()?;
    debug!(config = ?config, "Parsed config successfully");

    info!("Starting UniFi Protect Backup");

    // todo(steve.sampson): implement main loop

    info!("UniFi Protect Backup Exiting...");
    Ok(())
}
