//! Per-process I/O metrics (`process_io_*`) with a single pass over `/proc`.
//!
//! Each sampling cycle enumerates the process table once and, for every
//! process, reads each `/proc/<pid>/` file at most once: `comm` and `io`
//! through the `procfs` atomic layer. All values of one process share a
//! single `SampleGroup` (attributes `pid` + `comm`), so the `comm` string is
//! built exactly once per process per cycle.

use metrics_framework::{Collector, ItemKind, MetricItem, Number, SampleGroup};
use opentelemetry::KeyValue;
use smallvec::smallvec;
use std::time::Duration;
use tracing::debug;

static ITEMS: &[MetricItem] = &[
    MetricItem {
        name: "process_io_rchar_bytes_total",
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Bytes read from storage by the process (rchar).",
    },
    MetricItem {
        name: "process_io_wchar_bytes_total",
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Bytes written to storage by the process (wchar).",
    },
    MetricItem {
        name: "process_io_read_bytes_total",
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Bytes actually read from disk by the process.",
    },
    MetricItem {
        name: "process_io_write_bytes_total",
        kind: ItemKind::CounterU64,
        unit: "By",
        description: "Bytes actually written to disk by the process.",
    },
    MetricItem {
        name: "process_io_syscr_total",
        kind: ItemKind::CounterU64,
        unit: "{operations}",
        description: "Read syscalls issued by the process.",
    },
    MetricItem {
        name: "process_io_syscw_total",
        kind: ItemKind::CounterU64,
        unit: "{operations}",
        description: "Write syscalls issued by the process.",
    },
];

pub struct ProcessCollector {
    interval: Duration,
}

impl ProcessCollector {
    pub fn new(interval: Duration) -> Self {
        Self { interval }
    }
}

impl Collector for ProcessCollector {
    fn name(&self) -> &'static str {
        "process"
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    fn items(&self) -> &'static [MetricItem] {
        ITEMS
    }

    fn collect(&mut self, out: &mut Vec<SampleGroup>) {
        let Ok(processes) = procfs::process::get_all_processes() else {
            return;
        };

        for process in processes {
            // comm is the gate: if it cannot be read the process is gone.
            let Ok(comm) = process.comm() else {
                continue;
            };
            let comm = comm_to_string(&comm);

            let mut group = SampleGroup::with_attrs(smallvec![
                KeyValue::new("pid", process.pid() as i64),
                KeyValue::new("comm", comm),
            ]);

            match process.io() {
                Ok(io) => {
                    group.push("process_io_rchar_bytes_total", Number::U64(io.rchar));
                    group.push("process_io_wchar_bytes_total", Number::U64(io.wchar));
                    group.push("process_io_syscr_total", Number::U64(io.syscr));
                    group.push("process_io_syscw_total", Number::U64(io.syscw));
                    group.push("process_io_read_bytes_total", Number::U64(io.read_bytes));
                    group.push("process_io_write_bytes_total", Number::U64(io.write_bytes));
                    out.push(group);
                }
                Err(err) => {
                    debug!("read io for pid {} failed: {}", process.pid(), err);
                }
            }
        }
    }
}

/// Convert a fixed-size `comm` (NUL/newline padded) into an owned `String`.
fn comm_to_string(comm: &[u8; 16]) -> String {
    let end = comm
        .iter()
        .position(|&b| b == 0 || b == b'\n')
        .unwrap_or(comm.len());
    String::from_utf8_lossy(&comm[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::Value;

    #[test]
    fn comm_trims_padding() {
        assert_eq!(
            comm_to_string(b"bash\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"),
            "bash"
        );
        assert_eq!(
            comm_to_string(b"bash\n\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"),
            "bash"
        );
        assert_eq!(comm_to_string(b"1234567890123456"), "1234567890123456");
    }

    #[test]
    fn process_collect_contains_self_io() {
        let mut collector = ProcessCollector::new(Duration::from_secs(1));
        let mut out = Vec::new();
        collector.collect(&mut out);

        let pid = std::process::id() as i64;
        assert!(
            out.iter().any(|g| g.attrs.iter().any(|kv| {
                kv.key.as_str() == "pid" && matches!(&kv.value, Value::I64(v) if *v == pid)
            })),
            "own pid {pid} missing from process sample"
        );
        assert!(
            out.iter()
                .flat_map(|g| g.values.iter())
                .any(|v| v.name == "process_io_rchar_bytes_total")
        );
    }
}
