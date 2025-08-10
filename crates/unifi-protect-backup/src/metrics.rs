use crate::{
    archive::borg::Metrics as BorgArchiveMetrics,
    backup::{local::Metrics as LocalBackupMetrics, rclone::Metrics as RcloneBackupMetrics},
};
use hyper::{Request, Response, body::Incoming, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

#[derive(Default, Serialize)]
pub struct Metrics {
    pub local_backup: Arc<LocalBackupMetrics>,
    pub rclone_backup: Arc<RcloneBackupMetrics>,
    pub borg_archive: Arc<BorgArchiveMetrics>,
}

pub async fn start_metrics_server(
    metrics: Arc<Metrics>,
    address: &str,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = format!("{address}:{port}").parse()?;
    let listener = TcpListener::bind(addr).await?;

    tracing::info!("Metrics server listening on http://{addr}");

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let metrics = metrics.clone();

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(|req| handle_request(req, metrics.clone())))
                .await
            {
                tracing::error!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn handle_request(
    req: Request<Incoming>,
    metrics: Arc<Metrics>,
) -> Result<Response<String>, hyper::Error> {
    match req.uri().path() {
        "/metrics" => {
            let prometheus_output =
                serde_prometheus::to_string(&*metrics, None, std::collections::HashMap::new())
                    .unwrap_or_else(|e| format!("Error serializing metrics: {e}"));

            Ok(Response::builder()
                .status(200)
                .header("Content-Type", "text/plain; version=0.0.4; charset=utf-8")
                .body(prometheus_output)
                .unwrap())
        }
        _ => Ok(Response::builder()
            .status(404)
            .body("Not Found".to_string())
            .unwrap()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_metrics() {
        insta::assert_snapshot!(
            serde_prometheus::to_string(
                &Metrics::default(),
                None,
                std::collections::HashMap::new()
            )
            .unwrap()
        );
    }
}
