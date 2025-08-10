use crate::{
    config::Config,
    opentelemetry::{logging::loki_layer, tracing::tracer},
};
use opentelemetry::global;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use tokio::task::JoinHandle;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

pub mod logging;
pub mod tracing;

pub fn init(config: &Config) -> Option<JoinHandle<()>> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let mut layers: Vec<Box<dyn Layer<_> + Send + Sync>> = vec![
        tracing_subscriber::fmt::layer().with_ansi(true).boxed(),
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| {
                EnvFilter::new("info,sqlx=warn,reqwest=warn,hyper=warn,tungstenite=warn")
            })
            .boxed(),
    ];

    let mut loki_task = None;

    if let Some(loki_config) = config.logging.as_ref().and_then(|c| c.loki.clone()) {
        if let Ok((layer, task)) = loki_layer(loki_config) {
            layers.push(Box::new(layer));
            loki_task = Some(task);
        }
    }

    if let Some(tempo_config) = config.tracing.as_ref().and_then(|c| c.tempo.clone()) {
        if let Ok(tracer) = tracer(tempo_config) {
            layers.push(Box::new(
                tracing_opentelemetry::layer()
                    .with_tracer(tracer)
                    .with_filter(tracing_core::metadata::LevelFilter::INFO),
            ));
        }
    }

    tracing_subscriber::registry().with(layers).init();

    loki_task.map(|t| tokio::spawn(t))
}
