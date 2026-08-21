use crate::config::MetricsCfg;
use anyhow::Result;
use linux_metrics::ProcessCollector;
use metrics_framework::Registry;
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

    if let Some(process_cfg) = cfg.process.as_ref()
        && process_cfg.enable
    {
        registry.add(
            Box::new(ProcessCollector::new(Duration::from_secs(
                process_cfg.interval_secs,
            ))),
            meter,
        )?;
        info!("collector registered: process");
    }

    Ok(registry)
}
