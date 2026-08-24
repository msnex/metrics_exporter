use crate::config::MetricsCfg;
use anyhow::Result;
use linux_metrics::HostCollector;
use metrics_framework::{Collector, Registry};
use opentelemetry::metrics::Meter;
use std::time::Duration;
use tracing::info;

/// Build the collection runtime: instantiate every enabled collector from the
/// configuration and register it on the meter.
///
/// Each collector is an independent metric source; the framework has no
/// metric category concept and never touches `procfs`.
pub fn register_collectors(cfg: &MetricsCfg, meter: &Meter) -> Result<Registry> {
    let mut registry = Registry::new();

    if let Some(host_cfg) = cfg.host.as_ref()
        && host_cfg.enable
    {
        let collector = HostCollector::new(Duration::from_secs(host_cfg.interval_secs));
        let collector_name = collector.name();
        registry.add(Box::new(collector), meter)?;
        info!("collector registered: {}", collector_name);
    }

    Ok(registry)
}
