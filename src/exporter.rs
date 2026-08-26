use crate::config::{OtlpCfg, OtlpProtocol};
use anyhow::{Result, bail};
use opentelemetry::metrics::{Meter, MeterProvider};
use opentelemetry_otlp::{MetricExporter, WithExportConfig, WithHttpConfig, WithTonicConfig};
use opentelemetry_sdk::{
    Resource,
    metrics::{Instrument, PeriodicReader, SdkMeterProvider, Stream},
};
use std::collections::HashMap;
use std::time::Duration;
use tokio::runtime::Runtime;
use tonic::metadata::{Ascii, MetadataKey, MetadataMap, MetadataValue};
use tracing::error;

pub struct MetricsExporter {
    provider: SdkMeterProvider,
    timeout: u64,
}

impl MetricsExporter {
    pub fn new(cfg: &OtlpCfg, rt: &Runtime) -> Result<Self> {
        if cfg.endpoint.is_empty() {
            bail!("otlp.endpoint must not be empty.");
        }

        let grpc_metadata = cfg.headers.as_ref().map(metadata_map).transpose()?;

        let exporter = rt.block_on(async {
            match cfg.protocol {
                OtlpProtocol::Grpc => {
                    let mut builder = MetricExporter::builder()
                        .with_tonic()
                        .with_endpoint(&cfg.endpoint)
                        .with_timeout(Duration::from_secs(cfg.timeout));
                    if let Some(metadata) = grpc_metadata.as_ref() {
                        builder = builder.with_metadata(metadata.clone());
                    }
                    builder.build()
                }
                OtlpProtocol::Http => {
                    let mut builder = MetricExporter::builder()
                        .with_http()
                        .with_endpoint(&cfg.endpoint)
                        .with_timeout(Duration::from_secs(cfg.timeout));
                    if let Some(headers) = cfg.headers.as_ref() {
                        builder = builder.with_headers(headers.clone());
                    }
                    builder.build()
                }
            }
        })?;

        let reader = PeriodicReader::builder(exporter)
            .with_interval(Duration::from_secs(cfg.interval_secs))
            .build();
        let provider = SdkMeterProvider::builder()
            .with_reader(reader)
            .with_resource(
                Resource::builder()
                    .with_service_name("metrics_exporter")
                    .build(),
            )
            .with_view(|instrument: &Instrument| {
                if instrument.name().starts_with("process_") {
                    Some(
                        Stream::builder()
                            .with_cardinality_limit(1_000_000)
                            .build()
                            .expect("valid process metric stream"),
                    )
                } else {
                    None
                }
            })
            .build();

        Ok(Self {
            provider,
            timeout: cfg.timeout,
        })
    }

    pub fn meter(&self) -> Meter {
        self.provider.meter("metrics_exporter")
    }

    pub fn shutdown(&self) {
        if let Err(err) = self
            .provider
            .shutdown_with_timeout(Duration::from_secs(self.timeout))
        {
            error!("shutdown meter provider failed: {}", err.to_string());
        }
    }
}

fn metadata_map(headers: &HashMap<String, String>) -> Result<MetadataMap> {
    let mut metadata = MetadataMap::new();
    for (key, value) in headers {
        let key: MetadataKey<Ascii> = key
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid grpc metadata key: {key}"))?;
        let value: MetadataValue<Ascii> = value
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid grpc metadata value for key: {key}"))?;
        metadata.insert(key, value);
    }
    Ok(metadata)
}
