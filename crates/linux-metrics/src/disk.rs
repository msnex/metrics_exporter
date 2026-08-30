//! Block-device I/O metrics (`disk_*`) from `/proc/diskstats`.
//!
//! Each sampling cycle reads `/proc/diskstats` once and emits one sample
//! group per collected device, with the attributes `hostname` + `device`.
//! Sector counters are converted to bytes (512-byte units) and times from
//! milliseconds to seconds, matching the conventions used for the other
//! metric families.

use metrics_framework::{Number, SampleGroup};
use opentelemetry::KeyValue;
use procfs::diskstats::DiskStat;
use smallvec::smallvec;
use std::collections::HashSet;
use std::path::Path;

/// Bytes per sector as reported by the kernel in `/proc/diskstats`.
const SECTOR_SIZE: u64 = 512;
/// Milliseconds per second, used to convert `/proc/diskstats` times.
const MS_PER_SEC: f64 = 1000.0;

/// Filter for which block devices to collect.
///
/// With no explicit devices the default policy collects whole disks only
/// (partitions are excluded); an explicit list is matched exactly and skips
/// the partition check, so partitions can be requested by name.
#[derive(Default, Clone, bon::Builder)]
pub struct DiskFilter {
    disks: Option<HashSet<String>>,
}

/// Read `/proc/diskstats` and append one sample group per matching device.
pub fn collect_disk_metrics(hostname: &KeyValue, filter: &DiskFilter, out: &mut Vec<SampleGroup>) {
    let Ok(stats) = procfs::diskstats::disk_stats() else {
        return;
    };
    collect_disk_devices(hostname, filter, stats.disks, out);
}

/// Append one sample group per device passing the filter.
fn collect_disk_devices(
    hostname: &KeyValue,
    filter: &DiskFilter,
    disks: Vec<DiskStat>,
    out: &mut Vec<SampleGroup>,
) {
    for stat in disks {
        if !should_collect(filter, &stat.name, is_partition) {
            continue;
        }

        let mut group = SampleGroup::with_attrs(smallvec![
            hostname.clone(),
            KeyValue::new("device", stat.name),
        ]);
        group.push(
            "disk_read_bytes_total",
            Number::U64(stat.sectors_read * SECTOR_SIZE),
        );
        group.push(
            "disk_reads_completed_total",
            Number::U64(stat.reads_completed),
        );
        group.push("disk_reads_merged_total", Number::U64(stat.reads_merged));
        group.push(
            "disk_read_time_seconds_total",
            Number::F64(stat.read_ms as f64 / MS_PER_SEC),
        );
        group.push(
            "disk_write_bytes_total",
            Number::U64(stat.sectors_written * SECTOR_SIZE),
        );
        group.push(
            "disk_writes_completed_total",
            Number::U64(stat.writes_completed),
        );
        group.push("disk_writes_merged_total", Number::U64(stat.writes_merged));
        group.push(
            "disk_write_time_seconds_total",
            Number::F64(stat.write_ms as f64 / MS_PER_SEC),
        );
        group.push("disk_io_now", Number::U64(stat.io_in_progress));
        group.push(
            "disk_io_time_seconds_total",
            Number::F64(stat.io_ms as f64 / MS_PER_SEC),
        );
        group.push(
            "disk_io_time_weighted_seconds_total",
            Number::F64(stat.weighted_io_ms as f64 / MS_PER_SEC),
        );
        group.push(
            "disk_discard_bytes_total",
            Number::U64(stat.sectors_discarded * SECTOR_SIZE),
        );
        group.push(
            "disk_discards_completed_total",
            Number::U64(stat.discards_completed),
        );
        group.push(
            "disk_discards_merged_total",
            Number::U64(stat.discards_merged),
        );
        group.push(
            "disk_discard_time_seconds_total",
            Number::F64(stat.discard_ms as f64 / MS_PER_SEC),
        );
        group.push(
            "disk_flush_requests_total",
            Number::U64(stat.flush_requests),
        );
        group.push(
            "disk_flush_time_seconds_total",
            Number::F64(stat.flush_ms as f64 / MS_PER_SEC),
        );
        out.push(group);
    }
}

/// Decide whether a device passes the filter.
///
/// `is_partition` is injected so the default whole-disk policy is testable
/// without depending on the host's sysfs contents.
fn should_collect(filter: &DiskFilter, name: &str, is_partition: impl Fn(&str) -> bool) -> bool {
    match filter.disks.as_ref() {
        Some(disks) => disks.contains(name),
        None => !is_partition(name),
    }
}

/// True when the device is a partition, detected via the sysfs marker
/// `/sys/class/block/<name>/partition`. When sysfs is unavailable the
/// device is treated as a whole disk so real devices are never dropped.
fn is_partition(name: &str) -> bool {
    Path::new("/sys/class/block")
        .join(name)
        .join("partition")
        .exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn sample_disks() -> Vec<DiskStat> {
        vec![
            DiskStat {
                name: "sda".into(),
                reads_completed: 10,
                reads_merged: 2,
                sectors_read: 100,
                read_ms: 20,
                writes_completed: 30,
                writes_merged: 4,
                sectors_written: 200,
                write_ms: 40,
                io_in_progress: 1,
                io_ms: 60,
                weighted_io_ms: 70,
                discards_completed: 5,
                discards_merged: 1,
                sectors_discarded: 50,
                discard_ms: 6,
                flush_requests: 7,
                flush_ms: 8,
            },
            DiskStat {
                name: "sda1".into(),
                sectors_read: 10,
                ..Default::default()
            },
            DiskStat {
                name: "nvme0n1".into(),
                ..Default::default()
            },
            DiskStat {
                name: "nvme0n1p1".into(),
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

    fn group<'a>(out: &'a [SampleGroup], device: &str) -> &'a SampleGroup {
        out.iter()
            .find(|group| {
                group
                    .attrs
                    .iter()
                    .any(|kv| kv.key.as_str() == "device" && kv.value.as_str() == device)
            })
            .expect("device group missing")
    }

    #[test]
    fn collect_disk_devices_emits_all_metrics() {
        let hostname = KeyValue::new("hostname", Arc::from("test-host"));
        let mut whole_disks = HashSet::new();
        for disk in sample_disks() {
            whole_disks.insert(disk.name);
        }
        let filter = DiskFilter::builder().maybe_disks(Some(whole_disks)).build();

        let mut out = Vec::new();
        collect_disk_devices(&hostname, &filter, sample_disks(), &mut out);

        assert_eq!(out.len(), 4);
        let sda = group(&out, "sda");
        assert!(
            sda.attrs
                .iter()
                .any(|kv| kv.key.as_str() == "hostname" && kv.value.as_str() == "test-host")
        );
        assert_eq!(
            value(sda, "disk_read_bytes_total"),
            Number::U64(100 * SECTOR_SIZE)
        );
        assert_eq!(value(sda, "disk_reads_completed_total"), Number::U64(10));
        assert_eq!(value(sda, "disk_read_merged_total"), Number::U64(2));
        assert_eq!(
            value(sda, "disk_read_time_seconds_total"),
            Number::F64(0.02)
        );
        assert_eq!(
            value(sda, "disk_write_bytes_total"),
            Number::U64(200 * SECTOR_SIZE)
        );
        assert_eq!(value(sda, "disk_writes_completed_total"), Number::U64(30));
        assert_eq!(value(sda, "disk_write_merged_total"), Number::U64(4));
        assert_eq!(
            value(sda, "disk_write_time_seconds_total"),
            Number::F64(0.04)
        );
        assert_eq!(value(sda, "disk_io_now"), Number::U64(1));
        assert_eq!(value(sda, "disk_io_time_seconds_total"), Number::F64(0.06));
        assert_eq!(
            value(sda, "disk_io_time_weighted_seconds_total"),
            Number::F64(0.07)
        );
        assert_eq!(
            value(sda, "disk_discard_bytes_total"),
            Number::U64(50 * SECTOR_SIZE)
        );
        assert_eq!(value(sda, "disk_discards_completed_total"), Number::U64(5));
        assert_eq!(value(sda, "disk_discards_merged_total"), Number::U64(1));
        assert_eq!(
            value(sda, "disk_discard_time_seconds_total"),
            Number::F64(0.006)
        );
        assert_eq!(value(sda, "disk_flush_requests_total"), Number::U64(7));
        assert_eq!(
            value(sda, "disk_flush_time_seconds_total"),
            Number::F64(0.008)
        );
    }

    #[test]
    fn should_collect_defaults_to_whole_disks() {
        let filter = DiskFilter::default();
        let is_partition = |name: &str| matches!(name, "sda1" | "nvme0n1p1" | "mmcblk0p1");

        assert!(should_collect(&filter, "sda", is_partition));
        assert!(should_collect(&filter, "nvme0n1", is_partition));
        assert!(should_collect(&filter, "mmcblk0", is_partition));
        assert!(!should_collect(&filter, "sda1", is_partition));
        assert!(!should_collect(&filter, "nvme0n1p1", is_partition));
        assert!(!should_collect(&filter, "mmcblk0p1", is_partition));
    }

    #[test]
    fn should_collect_explicit_devices_skips_partition_check() {
        let mut disks = HashSet::new();
        disks.insert("sda1".to_string());
        let filter = DiskFilter::builder().maybe_disks(Some(disks)).build();

        // The injected predicate would exclude sda1, but the explicit list
        // takes precedence.
        let is_partition = |_: &str| true;
        assert!(should_collect(&filter, "sda1", is_partition));
        assert!(!should_collect(&filter, "sda", is_partition));
    }

    #[test]
    fn collect_disk_devices_filters_exact_devices() {
        let hostname = KeyValue::new("hostname", Arc::from("test-host"));
        let mut disks = HashSet::new();
        disks.insert("sda1".to_string());
        let filter = DiskFilter::builder().maybe_disks(Some(disks)).build();

        let mut out = Vec::new();
        collect_disk_devices(&hostname, &filter, sample_disks(), &mut out);

        assert_eq!(out.len(), 1);
        assert!(
            out[0]
                .attrs
                .iter()
                .any(|kv| kv.key.as_str() == "device" && kv.value.as_str() == "sda1")
        );
    }
}
