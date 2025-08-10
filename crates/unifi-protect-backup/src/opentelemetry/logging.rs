use crate::{Result, config::LokiConfig};
use base64::Engine;
use std::collections::HashMap;

pub(crate) fn loki_layer(
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
