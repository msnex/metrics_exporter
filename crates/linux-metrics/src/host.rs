//! Per-process I/O metrics (`process_io_*`) with a single pass over `/proc`.
//!
//! Each sampling cycle enumerates the process table once and, for every
//! process, reads each `/proc/<pid>/` file at most once: `comm` and `io`
//! through the `procfs` atomic layer. All values of one process share a
//! single `SampleGroup` (attributes `hostname` + `pid` + `comm`), so the `comm` string is
//! built exactly once per process per cycle.

use crate::{NetFilter, ProcessFilter};
use metrics_framework::{Collector, ItemKind, MetricItem, Number, SampleGroup};
use opentelemetry::KeyValue;
use smallvec::smallvec;
use std::{sync::Arc, time::Duration};

#[allow(non_snake_case, non_upper_case_globals)]
mod MetricItemType {
    type Item = u16;
    pub const Uptime: Item = 0;
    pub const Process: Item = 1;
    pub const Net: Item = 2;
}

static ITEMS: &[MetricItem] = &[
    MetricItem {
        item_type: MetricItemType::Uptime,
        name: "uptime",
        kind: ItemKind::GaugeF64,
        unit: "s",
        description: "System uptime",
    },
    MetricItem {
        item_type: MetricItemType::Process,
        name: "process_io_rchar_total",
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Number of bytes the process has read (rchar)",
    },
    MetricItem {
        item_type: MetricItemType::Process,
        name: "process_io_wchar_total",
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Number of bytes the process has written (wchar)",
    },
    MetricItem {
        item_type: MetricItemType::Process,
        name: "process_io_read_bytes_total",
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Bytes of read(2) I/O for the process",
    },
    MetricItem {
        item_type: MetricItemType::Process,
        name: "process_io_write_bytes_total",
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Bytes of write(2) I/O for the process",
    },
    MetricItem {
        item_type: MetricItemType::Process,
        name: "process_io_cancelled_write_bytes_total",
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Bytes of cancelled write(2) I/O for the process",
    },
    MetricItem {
        item_type: MetricItemType::Process,
        name: "process_io_syscr_total",
        kind: ItemKind::CounterU64,
        unit: "{operations}",
        description: "Number of read(2) syscalls for the process",
    },
    MetricItem {
        item_type: MetricItemType::Process,
        name: "process_io_syscw_total",
        kind: ItemKind::CounterU64,
        unit: "{operations}",
        description: "Number of write(2) syscalls for the process",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: "iface_rx_bytes_total",
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Number of bytes received by the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: "iface_rx_packets_total",
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of packets received by the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: "iface_rx_errs_total",
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of receive errors on the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: "iface_rx_drop_total",
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of received packets dropped by the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: "iface_tx_bytes_total",
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Number of bytes transmitted by the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: "iface_tx_packets_total",
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of packets transmitted by the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: "iface_tx_errs_total",
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of transmit errors on the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: "iface_tx_drop_total",
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of transmitted packets dropped by the network interface",
    },
];

#[derive(Clone, bon::Builder)]
pub struct HostCollectorCfg {
    #[builder(default = Duration::from_secs(1))]
    interval: Duration,
    #[builder(default = false)]
    process: bool,
    #[builder(default)]
    process_filter: ProcessFilter,
    #[builder(default = false)]
    net: bool,
    #[builder(default)]
    net_filter: NetFilter,
}

pub struct HostCollector {
    cfg: HostCollectorCfg,
    hostname: KeyValue,
}

impl HostCollector {
    pub fn new(cfg: HostCollectorCfg) -> Self {
        let hostname: Arc<str> = hostname::get()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
            .into();
        Self {
            cfg,
            hostname: KeyValue::new("hostname", hostname),
        }
    }
}

impl Collector for HostCollector {
    fn name(&self) -> &'static str {
        "linux-host"
    }

    fn interval(&self) -> Duration {
        self.cfg.interval
    }

    fn items(&self) -> Vec<&MetricItem> {
        let mut items = Vec::new();

        for item in ITEMS.as_ref() {
            match item.item_type {
                MetricItemType::Uptime => items.push(item),
                MetricItemType::Process if self.cfg.process => items.push(item),
                MetricItemType::Net if self.cfg.net => items.push(item),
                _ => {}
            }
        }
        items
    }

    fn collect(&mut self, out: &mut Vec<SampleGroup>) {
        // uptime
        if let Ok(uptime) = procfs::uptime::uptime() {
            let mut group = SampleGroup::with_attrs(smallvec![self.hostname.clone()]);
            group.push("uptime", Number::F64(uptime.uptime));
            out.push(group);
        }

        if self.cfg.process {
            super::process::collect_process_metrics(&self.hostname, &self.cfg.process_filter, out);
        }

        if self.cfg.net {
            super::net::collect_net_metrics(&self.hostname, &self.cfg.net_filter, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn net_collect_contains_loopback() {
        let cfg = HostCollectorCfg::builder()
            .interval(Duration::from_secs(1))
            .net(true)
            .build();
        let mut collector = HostCollector::new(cfg);
        let mut out = Vec::new();
        collector.collect(&mut out);

        assert!(
            out.iter().any(|group| {
                group
                    .attrs
                    .iter()
                    .any(|kv| kv.key.as_str() == "device" && kv.value.as_str() == "lo")
            }),
            "loopback device missing from net sample"
        );
        assert!(
            out.iter()
                .flat_map(|group| group.values.iter())
                .any(|value| value.name == "iface_rx_bytes_total"),
            "iface_rx_bytes_total missing from net sample"
        );
    }
}
