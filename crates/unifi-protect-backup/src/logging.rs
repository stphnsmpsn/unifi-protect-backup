use crate::{
    Result,
    config::{LoggingConfig, LokiConfig},
};
use base64::Engine;
use std::collections::HashMap;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_logging(logging_config: Option<LoggingConfig>) -> Result<()> {
    let registry = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_ansi(true))
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("info,sqlx=warn,reqwest=warn,hyper=warn,tungstenite=warn")
        }));

    if let Some(loki_config) = logging_config.and_then(|logging_config| logging_config.loki) {
        let (loki_layer, task) = create_loki_layer(loki_config)?;
        registry.with(loki_layer).init();
        tokio::spawn(task);
    } else {
        registry.init();
    }

    Ok(())
}

fn create_loki_layer(
    loki_config: LokiConfig,
) -> Result<(tracing_loki::Layer, tracing_loki::BackgroundTask)> {
    let url = loki_config
        .url
        .parse()
        .map_err(|e| crate::Error::Logging(format!("Invalid Loki URL: {e}")))?;

    let mut labels = HashMap::new();
    labels.insert("service".to_string(), env!("CARGO_PKG_NAME").to_string());

    if let Some(custom_labels) = loki_config.labels {
        labels.extend(custom_labels);
    }

    let mut extra_fields = HashMap::new();

    if let (Some(username), Some(password)) = (loki_config.username, loki_config.password) {
        let auth_header = format!(
            "Basic {}",
            base64::prelude::BASE64_STANDARD.encode(format!("{username}:{password}"))
        );
        extra_fields.insert("Authorization".to_string(), auth_header);
    }

    tracing_loki::layer(url, labels, extra_fields)
        .map_err(|e| crate::Error::Logging(format!("Failed to create Loki layer: {e}")))
}
