use crate::config::{
    MetricsCfg, MetricsHostCfg, MetricsHostCpuCfg, MetricsHostDiskCfg, MetricsHostNetCfg,
    MetricsHostProcessCfg,
};
use anyhow::Result;
use linux_metrics::{DiskFilter, HostCollector, HostCollectorCfg, NetFilter, ProcessFilter};
use metrics_framework::{Collector, Registry};
use opentelemetry::metrics::Meter;
use regex::RegexSet;
use std::collections::HashSet;
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

    let MetricsHostNetCfg {
        enable: net_enable,
        ifaces,
    } = if let Some(net_cfg) = cfg.net.as_ref() {
        net_cfg
    } else {
        &MetricsHostNetCfg {
            enable: false,
            ifaces: Vec::new(),
        }
    };

    let iface_set = if ifaces.is_empty() {
        None
    } else {
        Some(ifaces.iter().cloned().collect::<HashSet<_>>())
    };

    let net_filter = NetFilter::builder().maybe_ifaces(iface_set).build();

    let MetricsHostCpuCfg {
        enable: cpu_enable,
        per_core,
    } = if let Some(cpu_cfg) = cfg.cpu.as_ref() {
        cpu_cfg
    } else {
        &MetricsHostCpuCfg {
            enable: false,
            per_core: false,
        }
    };

    let MetricsHostDiskCfg {
        enable: disk_enable,
        exclude_devices,
    } = if let Some(disk_cfg) = cfg.disk.as_ref() {
        disk_cfg
    } else {
        &MetricsHostDiskCfg {
            enable: false,
            exclude_devices: Vec::new(),
        }
    };

    let exclude_devices_regex = if exclude_devices.is_empty() {
        None
    } else {
        Some(RegexSet::new(exclude_devices)?)
    };
    let disk_filter = DiskFilter::builder()
        .maybe_exclude_devices_regex(exclude_devices_regex)
        .build();

    let collector_cfg = HostCollectorCfg::builder()
        .interval(Duration::from_secs(cfg.interval_secs))
        .process(*enable)
        .process_filter(process_filter)
        .net(*net_enable)
        .net_filter(net_filter)
        .cpu(*cpu_enable)
        .per_core(*per_core)
        .disk(*disk_enable)
        .disk_filter(disk_filter)
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
