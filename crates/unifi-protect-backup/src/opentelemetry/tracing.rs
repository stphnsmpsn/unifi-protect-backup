use crate::{Result, config::TempoConfig};
use opentelemetry::{KeyValue, global, trace::TracerProvider};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    Resource,
    propagation::TraceContextPropagator,
    trace::{RandomIdGenerator, Sampler, SdkTracer, SdkTracerProvider},
};
use std::time::Duration;

pub fn tracer(config: TempoConfig) -> Result<SdkTracer> {
    global::set_text_map_propagator(TraceContextPropagator::new());
    let service_name = env!("CARGO_PKG_NAME").to_string();

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(format!("{}:{}", config.url, config.port))
        .with_timeout(Duration::from_secs(3))
        .with_protocol(opentelemetry_otlp::Protocol::Grpc)
        .build()
        .map_err(|e| crate::Error::Tracing(format!("Failed to create OTLP exporter: {e}")))?;

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
