//! System-wide and per-core CPU time metrics (`cpu_seconds_total`).
//!
//! Each sampling cycle reads `/proc/stat` once and emits one sample group
//! per (cpu, mode) pair, with the attributes `hostname` + `cpu` + `mode`.
//! Kernel ticks are converted to seconds using `USER_HZ = 100`.

use metrics_framework::{Number, SampleGroup};
use opentelemetry::KeyValue;
use procfs::cpu::CpuTimes;
use smallvec::smallvec;

/// Ticks per second for `/proc/stat` CPU times (USER_HZ on Linux).
const USER_HZ: f64 = 100.0;

/// CPU modes, in the order `/proc/stat` reports them.
const MODES: [&str; 10] = [
    "user",
    "nice",
    "system",
    "idle",
    "iowait",
    "irq",
    "softirq",
    "steal",
    "guest",
    "guest_nice",
];

/// Read `/proc/stat` and append CPU time samples.
pub fn collect_cpu_metrics(hostname: &KeyValue, per_core: bool, out: &mut Vec<SampleGroup>) {
    let Ok(stats) = procfs::cpu::cpu_stat() else {
        return;
    };
    collect_cpu_times(hostname, per_core, stats.cpus, out);
}

/// Append one sample group per (cpu, mode).
///
/// The aggregate `cpu` line is always emitted; per-core lines (`cpu0`, ...)
/// are emitted only when `per_core` is set.
fn collect_cpu_times(
    hostname: &KeyValue,
    per_core: bool,
    cpus: Vec<CpuTimes>,
    out: &mut Vec<SampleGroup>,
) {
    for entry in cpus {
        if entry.name != "cpu" && !per_core {
            continue;
        }

        // Shared per-core attribute: one allocation per cpu line, cloned into
        // each of its mode groups.
        let cpu_attr = KeyValue::new("cpu", entry.name);
        let times = [
            entry.user,
            entry.nice,
            entry.system,
            entry.idle,
            entry.iowait,
            entry.irq,
            entry.softirq,
            entry.steal,
            entry.guest,
            entry.guest_nice,
        ];

        for (mode, ticks) in MODES.iter().zip(times) {
            let mut group = SampleGroup::with_attrs(smallvec![
                hostname.clone(),
                cpu_attr.clone(),
                KeyValue::new("mode", *mode),
            ]);
            group.push("cpu_seconds_total", Number::F64(ticks as f64 / USER_HZ));
            out.push(group);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn sample_cpus() -> Vec<CpuTimes> {
        vec![
            CpuTimes {
                name: "cpu".into(),
                user: 100,
                system: 50,
                idle: 850,
                ..Default::default()
            },
            CpuTimes {
                name: "cpu0".into(),
                user: 30,
                ..Default::default()
            },
            CpuTimes {
                name: "cpu1".into(),
                user: 70,
                ..Default::default()
            },
        ]
    }

    fn value(group: &SampleGroup, name: &str) -> Number {
        group
            .values
            .iter()
            .find(|value| value.name == name)
            .map(|value| value.value)
            .expect("metric value missing")
    }

    fn group<'a>(out: &'a [SampleGroup], cpu: &str, mode: &str) -> &'a SampleGroup {
        out.iter()
            .find(|group| {
                group
                    .attrs
                    .iter()
                    .any(|kv| kv.key.as_str() == "cpu" && kv.value.as_str() == cpu)
                    && group
                        .attrs
                        .iter()
                        .any(|kv| kv.key.as_str() == "mode" && kv.value.as_str() == mode)
            })
            .expect("cpu/mode group missing")
    }

    #[test]
    fn collect_cpu_times_emits_aggregate_modes() {
        let hostname = KeyValue::new("hostname", Arc::from("test-host"));
        let mut out = Vec::new();
        collect_cpu_times(&hostname, false, sample_cpus(), &mut out);

        // Only the aggregate cpu line, one group per mode.
        assert_eq!(out.len(), 10);

        let user = group(&out, "cpu", "user");
        assert_eq!(value(user, "cpu_seconds_total"), Number::F64(1.0));
        assert!(
            user.attrs
                .iter()
                .any(|kv| kv.key.as_str() == "hostname" && kv.value.as_str() == "test-host")
        );
        assert_eq!(
            value(group(&out, "cpu", "system"), "cpu_seconds_total"),
            Number::F64(0.5)
        );
        assert_eq!(
            value(group(&out, "cpu", "idle"), "cpu_seconds_total"),
            Number::F64(8.5)
        );
        assert_eq!(
            value(group(&out, "cpu", "guest"), "cpu_seconds_total"),
            Number::F64(0.0)
        );
    }

    #[test]
    fn collect_cpu_times_emits_per_core_when_enabled() {
        let hostname = KeyValue::new("hostname", Arc::from("test-host"));
        let mut out = Vec::new();
        collect_cpu_times(&hostname, true, sample_cpus(), &mut out);

        // Aggregate + cpu0 + cpu1, each with 10 mode groups.
        assert_eq!(out.len(), 30);
        assert_eq!(
            value(group(&out, "cpu0", "user"), "cpu_seconds_total"),
            Number::F64(0.3)
        );
        assert_eq!(
            value(group(&out, "cpu1", "user"), "cpu_seconds_total"),
            Number::F64(0.7)
        );
    }
}
