//! Per-process I/O metrics (`process_io_*`) with a single pass over `/proc`.
//!
//! Each sampling cycle enumerates the process table once and, for every
//! process, reads each `/proc/<pid>/` file at most once: `comm` and `io`
//! through the `procfs` atomic layer. All values of one process share a
//! single `SampleGroup` (attributes `hostname` + `pid` + `comm`), so the `comm` string is
//! built exactly once per process per cycle.

use crate::names::*;
use crate::{DiskFilter, NetFilter, ProcessFilter};
use metrics_framework::{Collector, ItemKind, MetricItem, Number, SampleGroup};
use opentelemetry::KeyValue;
use procfs::loadavg::LoadAvg;
use smallvec::smallvec;
use std::{sync::Arc, time::Duration};

#[allow(non_snake_case, non_upper_case_globals)]
mod MetricItemType {
    type Item = u16;
    pub const Uptime: Item = 0;
    pub const Loadavg: Item = 1;
    pub const Process: Item = 2;
    pub const Net: Item = 3;
    pub const Cpu: Item = 4;
    pub const Disk: Item = 5;
    pub const Mem: Item = 6;
}

static ITEMS: &[MetricItem] = &[
    MetricItem {
        item_type: MetricItemType::Uptime,
        name: NAME_UPTIME,
        kind: ItemKind::GaugeF64,
        unit: "s",
        description: "System uptime",
    },
    MetricItem {
        item_type: MetricItemType::Loadavg,
        name: NAME_LOADAVG_1M,
        kind: ItemKind::GaugeF64,
        unit: "",
        description: "1m load average",
    },
    MetricItem {
        item_type: MetricItemType::Loadavg,
        name: NAME_LOADAVG_5M,
        kind: ItemKind::GaugeF64,
        unit: "",
        description: "5m load average",
    },
    MetricItem {
        item_type: MetricItemType::Loadavg,
        name: NAME_LOADAVG_15M,
        kind: ItemKind::GaugeF64,
        unit: "",
        description: "15m load average",
    },
    MetricItem {
        item_type: MetricItemType::Loadavg,
        name: NAME_LOADAVG_RUNNING_TASKS,
        kind: ItemKind::GaugeU64,
        unit: "{tasks}",
        description: "Number of currently running tasks",
    },
    MetricItem {
        item_type: MetricItemType::Loadavg,
        name: NAME_LOADAVG_TOTAL_TASKS,
        kind: ItemKind::GaugeU64,
        unit: "{tasks}",
        description: "Total number of tasks",
    },
    MetricItem {
        item_type: MetricItemType::Cpu,
        name: NAME_CPU_SECONDS_TOTAL,
        kind: ItemKind::CounterF64,
        unit: "s",
        description: "Seconds the CPUs spent in each mode",
    },
    MetricItem {
        item_type: MetricItemType::Process,
        name: NAME_PROCESS_IO_RCHAR_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Number of bytes the process has read (rchar)",
    },
    MetricItem {
        item_type: MetricItemType::Process,
        name: NAME_PROCESS_IO_WCHAR_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Number of bytes the process has written (wchar)",
    },
    MetricItem {
        item_type: MetricItemType::Process,
        name: NAME_PROCESS_IO_READ_BYTES_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Bytes of read(2) I/O for the process",
    },
    MetricItem {
        item_type: MetricItemType::Process,
        name: NAME_PROCESS_IO_WRITE_BYTES_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Bytes of write(2) I/O for the process",
    },
    MetricItem {
        item_type: MetricItemType::Process,
        name: NAME_PROCESS_IO_CANCELLED_WRITE_BYTES_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Bytes of cancelled write(2) I/O for the process",
    },
    MetricItem {
        item_type: MetricItemType::Process,
        name: NAME_PROCESS_IO_SYSCR_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{operations}",
        description: "Number of read(2) syscalls for the process",
    },
    MetricItem {
        item_type: MetricItemType::Process,
        name: NAME_PROCESS_IO_SYSCW_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{operations}",
        description: "Number of write(2) syscalls for the process",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: NAME_IFACE_RX_BYTES_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Number of bytes received by the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: NAME_IFACE_RX_PACKETS_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of packets received by the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: NAME_IFACE_RX_ERRS_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of receive errors on the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: NAME_IFACE_RX_DROP_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of received packets dropped by the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: NAME_IFACE_TX_BYTES_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Number of bytes transmitted by the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: NAME_IFACE_TX_PACKETS_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of packets transmitted by the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: NAME_IFACE_TX_ERRS_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of transmit errors on the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Net,
        name: NAME_IFACE_TX_DROP_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of transmitted packets dropped by the network interface",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_READ_BYTES_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Number of bytes read from the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_READS_COMPLETED_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of reads completed successfully by the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_READS_MERGED_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of reads merged by the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_READ_TIME_MS_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "ms",
        description: "Time spent reading from the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_WRITE_BYTES_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Number of bytes written to the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_WRITES_COMPLETED_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of writes completed successfully by the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_WRITES_MERGED_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of writes merged by the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_WRITE_TIME_MS_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "ms",
        description: "Time spent writing to the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_IO_IN_PROGRESS,
        kind: ItemKind::GaugeU64,
        unit: "{operations}",
        description: "Number of I/Os currently in progress on the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_IO_TIME_MS_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "ms",
        description: "Time spent doing I/Os on the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_IO_TIME_WEIGHTED_MS_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "ms",
        description: "Weighted time spent doing I/Os on the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_DISCARD_BYTES_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Number of bytes discarded from the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_DISCARDS_COMPLETED_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of discards completed successfully by the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_DISCARDS_MERGED_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of discards merged by the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_DISCARD_TIME_MS_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "ms",
        description: "Time spent discarding on the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_FLUSH_REQUESTS_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "Number of flush requests completed successfully by the block device",
    },
    MetricItem {
        item_type: MetricItemType::Disk,
        name: NAME_DISK_FLUSH_TIME_MS_TOTAL,
        kind: ItemKind::CounterU64,
        unit: "ms",
        description: "Time spent flushing the block device",
    },
    MetricItem {
        item_type: MetricItemType::Mem,
        name: NAME_MEM_TOTAL_BYTES,
        kind: ItemKind::GaugeU64,
        unit: "By",
        description: "Total usable system memory",
    },
    MetricItem {
        item_type: MetricItemType::Mem,
        name: NAME_MEM_FREE_BYTES,
        kind: ItemKind::GaugeU64,
        unit: "By",
        description: "Free system memory",
    },
    MetricItem {
        item_type: MetricItemType::Mem,
        name: NAME_MEM_AVAILABLE_BYTES,
        kind: ItemKind::GaugeU64,
        unit: "By",
        description: "Estimated memory available for starting new applications",
    },
    MetricItem {
        item_type: MetricItemType::Mem,
        name: NAME_SWAP_TOTAL_BYTES,
        kind: ItemKind::GaugeU64,
        unit: "By",
        description: "Total swap space",
    },
    MetricItem {
        item_type: MetricItemType::Mem,
        name: NAME_SWAP_FREE_BYTES,
        kind: ItemKind::GaugeU64,
        unit: "By",
        description: "Free swap space",
    },
    MetricItem {
        item_type: MetricItemType::Mem,
        name: NAME_HUGE_PAGES_TOTAL_BYTES,
        kind: ItemKind::GaugeU64,
        unit: "By",
        description: "Total hugepage memory",
    },
    MetricItem {
        item_type: MetricItemType::Mem,
        name: NAME_HUGE_PAGES_FREE_BYTES,
        kind: ItemKind::GaugeU64,
        unit: "By",
        description: "Free hugepage memory",
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
    #[builder(default = false)]
    cpu: bool,
    #[builder(default = false)]
    per_core: bool,
    #[builder(default = false)]
    disk: bool,
    #[builder(default)]
    disk_filter: DiskFilter,
    #[builder(default = false)]
    mem: bool,
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

        for item in ITEMS {
            match item.item_type {
                MetricItemType::Uptime => items.push(item),
                MetricItemType::Loadavg => items.push(item),
                MetricItemType::Cpu if self.cfg.cpu => items.push(item),
                MetricItemType::Process if self.cfg.process => items.push(item),
                MetricItemType::Net if self.cfg.net => items.push(item),
                MetricItemType::Disk if self.cfg.disk => items.push(item),
                MetricItemType::Mem if self.cfg.mem => items.push(item),
                _ => {}
            }
        }
        items
    }

    fn collect(&mut self, out: &mut Vec<SampleGroup>) {
        // uptime
        if let Ok(uptime) = procfs::uptime::uptime() {
            let mut group = SampleGroup::with_attrs(smallvec![self.hostname.clone()]);
            group.push(NAME_UPTIME, Number::F64(uptime.uptime));
            out.push(group);
        }

        // loadavg
        if let Ok(loadavg) = procfs::loadavg::loadavg() {
            collect_loadavg_metrics(&self.hostname, &loadavg, out);
        }

        if self.cfg.cpu {
            super::cpu::collect_cpu_metrics(&self.hostname, self.cfg.per_core, out);
        }

        if self.cfg.process {
            super::process::collect_process_metrics(&self.hostname, &self.cfg.process_filter, out);
        }

        if self.cfg.net {
            super::net::collect_net_metrics(&self.hostname, &self.cfg.net_filter, out);
        }

        if self.cfg.disk {
            super::disk::collect_disk_metrics(&self.hostname, &self.cfg.disk_filter, out);
        }

        if self.cfg.mem {
            super::mem::collect_mem_metrics(&self.hostname, out);
        }
    }
}

/// Append one host-level sample group for a `/proc/loadavg` snapshot.
fn collect_loadavg_metrics(hostname: &KeyValue, loadavg: &LoadAvg, out: &mut Vec<SampleGroup>) {
    let mut group = SampleGroup::with_attrs(smallvec![hostname.clone()]);
    group.push(NAME_LOADAVG_1M, Number::F64(loadavg.load1));
    group.push(NAME_LOADAVG_5M, Number::F64(loadavg.load5));
    group.push(NAME_LOADAVG_15M, Number::F64(loadavg.load15));
    group.push(
        NAME_LOADAVG_RUNNING_TASKS,
        Number::U64(loadavg.running as u64),
    );
    group.push(NAME_LOADAVG_TOTAL_TASKS, Number::U64(loadavg.total as u64));
    out.push(group);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(group: &SampleGroup, name: &str) -> Number {
        group
            .values
            .iter()
            .find(|value| value.name == name)
            .map(|value| value.value)
            .expect("metric value missing")
    }

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
                    .any(|kv| kv.key.as_str() == "iface" && kv.value.as_str() == "lo")
            }),
            "loopback device missing from net sample"
        );
        assert!(
            out.iter()
                .flat_map(|group| group.values.iter())
                .any(|value| value.name == NAME_IFACE_RX_BYTES_TOTAL),
            "{} missing from net sample",
            NAME_IFACE_RX_BYTES_TOTAL
        );
    }

    #[test]
    fn collect_loadavg_metrics_emits_all_fields() {
        let hostname = KeyValue::new("hostname", Arc::from("test-host"));
        let loadavg = LoadAvg {
            load1: 0.25,
            load5: 0.5,
            load15: 0.75,
            running: 2,
            total: 100,
            last_pid: 42,
        };
        let mut out = Vec::new();
        collect_loadavg_metrics(&hostname, &loadavg, &mut out);

        assert_eq!(out.len(), 1);
        let group = &out[0];
        assert!(
            group
                .attrs
                .iter()
                .any(|kv| { kv.key.as_str() == "hostname" && kv.value.as_str() == "test-host" })
        );
        assert_eq!(value(group, NAME_LOADAVG_1M), Number::F64(0.25));
        assert_eq!(value(group, NAME_LOADAVG_5M), Number::F64(0.5));
        assert_eq!(value(group, NAME_LOADAVG_15M), Number::F64(0.75));
        assert_eq!(value(group, NAME_LOADAVG_RUNNING_TASKS), Number::U64(2));
        assert_eq!(value(group, NAME_LOADAVG_TOTAL_TASKS), Number::U64(100));
    }

    #[test]
    fn host_collect_contains_loadavg() {
        let cfg = HostCollectorCfg::builder()
            .interval(Duration::from_secs(1))
            .build();
        let mut collector = HostCollector::new(cfg);
        let mut out = Vec::new();
        collector.collect(&mut out);

        assert!(
            out.iter()
                .flat_map(|group| group.values.iter())
                .any(|value| value.name == NAME_LOADAVG_1M),
            "{} missing from host sample",
            NAME_LOADAVG_1M
        );
    }

    #[test]
    fn host_collect_contains_cpu() {
        let cfg = HostCollectorCfg::builder()
            .interval(Duration::from_secs(1))
            .cpu(true)
            .build();
        let mut collector = HostCollector::new(cfg);
        let mut out = Vec::new();
        collector.collect(&mut out);

        assert!(
            out.iter().any(|group| {
                group
                    .attrs
                    .iter()
                    .any(|kv| kv.key.as_str() == "cpu" && kv.value.as_str() == "cpu")
            }),
            "aggregate cpu group missing from host sample"
        );
        assert!(
            out.iter()
                .flat_map(|group| group.values.iter())
                .any(|value| value.name == NAME_CPU_SECONDS_TOTAL),
            "{} missing from host sample",
            NAME_CPU_SECONDS_TOTAL
        );
    }

    #[test]
    fn host_collect_contains_disk() {
        let cfg = HostCollectorCfg::builder()
            .interval(Duration::from_secs(1))
            .disk(true)
            .build();
        let mut collector = HostCollector::new(cfg);
        let mut out = Vec::new();
        collector.collect(&mut out);

        assert!(
            out.iter()
                .any(|group| { group.attrs.iter().any(|kv| kv.key.as_str() == "device") }),
            "whole-disk group missing from host sample"
        );
        assert!(
            out.iter()
                .flat_map(|group| group.values.iter())
                .any(|value| value.name == NAME_DISK_READ_BYTES_TOTAL),
            "{} missing from host sample",
            NAME_DISK_READ_BYTES_TOTAL
        );
    }

    #[test]
    fn host_collect_contains_mem() {
        let cfg = HostCollectorCfg::builder()
            .interval(Duration::from_secs(1))
            .mem(true)
            .build();
        let mut collector = HostCollector::new(cfg);
        let mut out = Vec::new();
        collector.collect(&mut out);

        assert!(
            out.iter()
                .flat_map(|group| group.values.iter())
                .any(|value| value.name == NAME_MEM_TOTAL_BYTES),
            "{} missing from host sample",
            NAME_MEM_TOTAL_BYTES
        );
    }
}
