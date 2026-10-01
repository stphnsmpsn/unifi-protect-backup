use std::collections::HashMap;

use base64::Engine;

use crate::{Result, config::LokiConfig};

/// The Loki layer and the task that ships its batches. Credentials go on the HTTP request as
/// `Authorization: Basic …`; they were once put in `extra_fields`, which tracing-loki serialises
/// as a JSON field of every log line, so the header was never sent (the ingress answered 401)
/// and the password went into the log stream instead.
pub(crate) fn loki_layer(
    loki_config: LokiConfig,
) -> Result<(tracing_loki::Layer, tracing_loki::BackgroundTask)> {
    let url: tracing_loki::url::Url = loki_config
        .url
        .parse()
        .map_err(|e| crate::Error::Logging(format!("Invalid Loki URL: {e}")))?;
    // tracing-loki joins `loki/api/v1/push` onto whatever base it is given, so hand it the origin
    // only: `Url::join("/")` resolves like a browser link and replaces the path with the root
    // (https://host/loki/api/v1/push -> https://host/). A config that names the push path, the
    // natural thing to copy from a Loki ingress, then does not get it twice.
    let base = url
        .join("/")
        .map_err(|e| crate::Error::Logging(format!("Invalid Loki URL: {e}")))?;

    let mut labels = HashMap::new();
    labels.insert("service".to_string(), env!("CARGO_PKG_NAME").to_string());
    if let Some(custom_labels) = loki_config.labels {
        labels.extend(custom_labels);
    }

    let mut builder = tracing_loki::builder();
    for (key, value) in labels {
        builder = builder
            .label(key, value)
            .map_err(|e| crate::Error::Logging(format!("Invalid Loki label: {e}")))?;
    }
    if let (Some(username), Some(password)) = (loki_config.username, loki_config.password) {
        let auth_header = format!(
            "Basic {}",
            base64::prelude::BASE64_STANDARD.encode(format!("{username}:{password}"))
        );
        builder = builder
            .http_header("Authorization", auth_header)
            .map_err(|e| crate::Error::Logging(format!("Invalid Loki credentials: {e}")))?;
    }

    builder
        .build_url(base)
        .map_err(|e| crate::Error::Logging(format!("Failed to create Loki layer: {e}")))
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    use super::*;

    // A one-request HTTP server on a loopback port: returns the request head it received.
    fn capture_one_request() -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = vec![0u8; 8192];
            let n = stream.read(&mut buf).unwrap();
            let head = String::from_utf8_lossy(&buf[..n]).to_string();
            stream
                .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            head
        });
        (format!("http://{addr}/loki/api/v1/push"), handle)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn credentials_travel_as_a_basic_auth_header_not_a_log_field() {
        use tracing_subscriber::prelude::*;

        let (url, server) = capture_one_request();
        let (layer, task) = loki_layer(LokiConfig {
            url,
            username: Some("push".into()),
            password: Some("s3cret".into()),
            labels: None,
        })
        .unwrap();
        let runner = tokio::spawn(task);
        // The layer must outlive the capture: dropping it closes the channel and the task quits.
        let subscriber = tracing_subscriber::registry().with(layer);
        let guard = tracing::subscriber::set_default(subscriber);
        tracing::info!("hello");

        let head = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::task::spawn_blocking(move || server.join().unwrap()),
        )
        .await
        .expect("the layer posted within 10 s")
        .unwrap();
        drop(guard);
        runner.abort();

        let first_line = head.lines().next().unwrap_or_default();
        assert!(
            first_line.starts_with("POST /loki/api/v1/push "),
            "push path appended exactly once: {first_line}"
        );
        assert!(
            head.to_ascii_lowercase()
                .contains("authorization: basic chvzadpzm2nyzxq="),
            "header missing from request head:\n{head}"
        );
    }
}
