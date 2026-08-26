use crate::config::{MetricsCfg, MetricsHostCfg, MetricsHostProcessCfg};
use anyhow::Result;
use linux_metrics::{HostCollector, HostCollectorCfg, ProcessFilter};
use metrics_framework::{Collector, Registry};
use opentelemetry::metrics::Meter;
use regex::RegexSet;
use std::time::Duration;
use tracing::info;

fn init_host_collector(cfg: &MetricsHostCfg) -> Result<HostCollector> {
    let MetricsHostProcessCfg {
        enable,
        comms,
        thread,
    } = if let Some(process_cfg) = cfg.process.as_ref() {
        process_cfg
    } else {
        &MetricsHostProcessCfg {
            enable: false,
            thread: false,
            comms: Vec::new(),
        }
    };

    let comms_regex_set = if comms.is_empty() {
        None
    } else {
        Some(RegexSet::new(comms)?)
    };

    let process_filter = ProcessFilter::builder()
        .thread(*thread)
        .maybe_comms_regex(comms_regex_set)
        .build();

    let collector_cfg = HostCollectorCfg::builder()
        .interval(Duration::from_secs(cfg.interval_secs))
        .process(*enable)
        .process_filter(process_filter)
        .build();

    Ok(HostCollector::new(collector_cfg))
}

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
        let collector = init_host_collector(host_cfg)?;
        let collector_name = collector.name();
        registry.add(Box::new(collector), meter)?;
        info!("collector registered: {}", collector_name);
    }

    Ok(registry)
}
