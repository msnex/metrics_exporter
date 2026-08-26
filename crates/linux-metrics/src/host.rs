//! Per-process I/O metrics (`process_io_*`) with a single pass over `/proc`.
//!
//! Each sampling cycle enumerates the process table once and, for every
//! process, reads each `/proc/<pid>/` file at most once: `comm` and `io`
//! through the `procfs` atomic layer. All values of one process share a
//! single `SampleGroup` (attributes `hostname` + `pid` + `comm`), so the `comm` string is
//! built exactly once per process per cycle.

use crate::ProcessFilter;
use metrics_framework::{Collector, ItemKind, MetricItem, Number, SampleGroup};
use opentelemetry::KeyValue;
use smallvec::smallvec;
use std::{sync::Arc, time::Duration};

#[allow(non_snake_case, non_upper_case_globals)]
mod MetricItemType {
    type Item = u16;
    pub const Uptime: Item = 0;
    pub const Process: Item = 1;
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
];

#[derive(Clone, bon::Builder)]
pub struct HostCollectorCfg {
    #[builder(default = Duration::from_secs(1))]
    interval: Duration,
    #[builder(default = false)]
    process: bool,
    #[builder(default)]
    process_filter: ProcessFilter,
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
            if self.cfg.process && item.item_type == MetricItemType::Process {
                items.push(item);
            }
            match item.item_type {
                MetricItemType::Uptime => items.push(item),
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
    }
}
