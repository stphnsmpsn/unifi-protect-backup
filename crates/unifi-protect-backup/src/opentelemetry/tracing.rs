use std::{collections::HashMap, time::Duration};

use base64::Engine;
use opentelemetry::{KeyValue, global, trace::TracerProvider};
use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithHttpConfig, WithTonicConfig};
use opentelemetry_sdk::{
    Resource,
    propagation::TraceContextPropagator,
    trace::{RandomIdGenerator, Sampler, SdkTracer, SdkTracerProvider},
};
use tonic::metadata::{MetadataMap, MetadataValue};

use crate::{
    Result,
    config::{Protocol, TempoConfig},
};

/// Where the exporter sends spans. OTLP/HTTP addresses the traces path itself; gRPC addresses
/// the server and multiplexes services on it.
fn endpoint(config: &TempoConfig) -> String {
    let base = format!("{}:{}", config.url.trim_end_matches('/'), config.port);
    match config.protocol {
        Protocol::Grpc => base,
        Protocol::Http => format!("{base}/v1/traces"),
    }
}

/// The `Authorization` header value for a username and password, if both are set.
fn basic_auth(config: &TempoConfig) -> Option<String> {
    match (&config.username, &config.password) {
        (Some(username), Some(password)) => Some(format!(
            "Basic {}",
            base64::prelude::BASE64_STANDARD.encode(format!("{username}:{password}"))
        )),
        _ => None,
    }
}

fn exporter(config: &TempoConfig) -> Result<SpanExporter> {
    let endpoint = endpoint(config);
    let auth = basic_auth(config);
    let timeout = Duration::from_secs(3);
    let built = match config.protocol {
        Protocol::Grpc => {
            let mut metadata = MetadataMap::new();
            if let Some(auth) = auth {
                let value = MetadataValue::try_from(auth).map_err(|e| {
                    crate::Error::Tracing(format!("Invalid Tempo credentials: {e}"))
                })?;
                metadata.insert("authorization", value);
            }
            SpanExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .with_timeout(timeout)
                .with_protocol(opentelemetry_otlp::Protocol::Grpc)
                .with_metadata(metadata)
                .build()
        }
        Protocol::Http => {
            let mut headers = HashMap::new();
            if let Some(auth) = auth {
                headers.insert("Authorization".to_string(), auth);
            }
            SpanExporter::builder()
                .with_http()
                .with_endpoint(endpoint)
                .with_timeout(timeout)
                .with_protocol(opentelemetry_otlp::Protocol::HttpBinary)
                .with_headers(headers)
                .build()
        }
    };
    built.map_err(|e| crate::Error::Tracing(format!("Failed to create OTLP exporter: {e}")))
}

pub fn tracer(config: TempoConfig) -> Result<SdkTracer> {
    global::set_text_map_propagator(TraceContextPropagator::new());
    let service_name = env!("CARGO_PKG_NAME").to_string();

    let exporter = exporter(&config)?;

    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_sampler(Sampler::AlwaysOn)
        .with_id_generator(RandomIdGenerator::default())
        .with_max_events_per_span(64)
        .with_max_attributes_per_span(16)
        .with_max_events_per_span(16)
        .with_resource(
            Resource::builder()
                .with_attribute(KeyValue::new("service.name", service_name.clone()))
                .build(),
        )
        .build();

    let tracer = tracer_provider.tracer(service_name);

    Ok(tracer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(protocol: Protocol, url: &str, port: u16) -> TempoConfig {
        TempoConfig {
            protocol,
            url: url.to_string(),
            port,
            username: None,
            password: None,
        }
    }

    #[test]
    fn grpc_addresses_the_server_and_http_the_traces_path() {
        assert_eq!(
            endpoint(&config(Protocol::Grpc, "http://tempo.internal", 4317)),
            "http://tempo.internal:4317"
        );
        assert_eq!(
            endpoint(&config(Protocol::Http, "https://tempo.example.com/", 443)),
            "https://tempo.example.com:443/v1/traces"
        );
    }

    #[test]
    fn basic_auth_needs_both_halves() {
        let mut c = config(Protocol::Http, "https://tempo.example.com", 443);
        assert_eq!(basic_auth(&c), None);
        c.username = Some("push".into());
        assert_eq!(basic_auth(&c), None, "a username alone sends nothing");
        c.password = Some("s3cret".into());
        assert_eq!(basic_auth(&c).as_deref(), Some("Basic cHVzaDpzM2NyZXQ="));
    }

    // The gRPC exporter opens its channel lazily but still needs a reactor to register with.
    #[tokio::test]
    async fn both_exporters_build() {
        let mut c = config(Protocol::Http, "https://tempo.example.com", 443);
        c.username = Some("push".into());
        c.password = Some("s3cret".into());
        assert!(exporter(&c).is_ok());
        c.protocol = Protocol::Grpc;
        c.port = 4317;
        assert!(exporter(&c).is_ok());
    }

    #[test]
    fn protocol_defaults_to_grpc_for_configs_written_before_the_field() {
        let c: TempoConfig = toml::from_str("url = \"http://tempo\"\nport = 4317\n").unwrap();
        assert_eq!(c.protocol, Protocol::Grpc);
        let c: TempoConfig =
            toml::from_str("protocol = \"http\"\nurl = \"https://tempo\"\nport = 443\n").unwrap();
        assert_eq!(c.protocol, Protocol::Http);
    }
}
