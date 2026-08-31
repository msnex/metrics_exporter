//! Block-device I/O metrics (`disk_*`) from `/proc/diskstats`.
//!
//! Each sampling cycle reads `/proc/diskstats` once and emits one sample
//! group per collected device, with the attributes `hostname` + `device`.
//! Sector counters are converted to bytes (512-byte units) and times from
//! milliseconds to seconds, matching the conventions used for the other
//! metric families.

use crate::names::{
    NAME_DISK_DISCARD_BYTES_TOTAL, NAME_DISK_DISCARD_TIME_MS_TOTAL,
    NAME_DISK_DISCARDS_COMPLETED_TOTAL, NAME_DISK_DISCARDS_MERGED_TOTAL,
    NAME_DISK_FLUSH_REQUESTS_TOTAL, NAME_DISK_FLUSH_TIME_MS_TOTAL, NAME_DISK_IO_IN_PROGRESS,
    NAME_DISK_IO_TIME_MS_TOTAL, NAME_DISK_IO_TIME_WEIGHTED_MS_TOTAL, NAME_DISK_READ_BYTES_TOTAL,
    NAME_DISK_READ_TIME_MS_TOTAL, NAME_DISK_READS_COMPLETED_TOTAL, NAME_DISK_READS_MERGED_TOTAL,
    NAME_DISK_WRITE_BYTES_TOTAL, NAME_DISK_WRITE_TIME_MS_TOTAL, NAME_DISK_WRITES_COMPLETED_TOTAL,
    NAME_DISK_WRITES_MERGED_TOTAL,
};
use metrics_framework::{Number, SampleGroup};
use opentelemetry::KeyValue;
use procfs::diskstats::DiskStat;
use regex::RegexSet;
use smallvec::smallvec;

/// Bytes per sector as reported by the kernel in `/proc/diskstats`.
const SECTOR_SIZE: u64 = 512;

/// Filter for which block devices to collect.
///
/// With no explicit devices the default policy collects whole disks only
/// (partitions are excluded); an explicit list is matched exactly and skips
/// the partition check, so partitions can be requested by name.
#[derive(Default, Clone, bon::Builder)]
pub struct DiskFilter {
    exclude_devices_regex: Option<RegexSet>,
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
        if let Some(exclude_regex) = filter.exclude_devices_regex.as_ref()
            && exclude_regex.is_match(&stat.name)
        {
            continue;
        }

        let mut group = SampleGroup::with_attrs(smallvec![
            hostname.clone(),
            KeyValue::new("device", stat.name),
        ]);
        group.push(
            NAME_DISK_READ_BYTES_TOTAL,
            Number::U64(stat.sectors_read * SECTOR_SIZE),
        );
        group.push(
            NAME_DISK_READS_COMPLETED_TOTAL,
            Number::U64(stat.reads_completed),
        );
        group.push(NAME_DISK_READS_MERGED_TOTAL, Number::U64(stat.reads_merged));
        group.push(NAME_DISK_READ_TIME_MS_TOTAL, Number::U64(stat.read_ms));
        group.push(
            NAME_DISK_WRITE_BYTES_TOTAL,
            Number::U64(stat.sectors_written * SECTOR_SIZE),
        );
        group.push(
            NAME_DISK_WRITES_COMPLETED_TOTAL,
            Number::U64(stat.writes_completed),
        );
        group.push(
            NAME_DISK_WRITES_MERGED_TOTAL,
            Number::U64(stat.writes_merged),
        );
        group.push(NAME_DISK_WRITE_TIME_MS_TOTAL, Number::U64(stat.write_ms));
        group.push(NAME_DISK_IO_IN_PROGRESS, Number::U64(stat.io_in_progress));
        group.push(NAME_DISK_IO_TIME_MS_TOTAL, Number::U64(stat.io_ms));
        group.push(
            NAME_DISK_IO_TIME_WEIGHTED_MS_TOTAL,
            Number::U64(stat.weighted_io_ms),
        );
        group.push(
            NAME_DISK_DISCARD_BYTES_TOTAL,
            Number::U64(stat.sectors_discarded * SECTOR_SIZE),
        );
        group.push(
            NAME_DISK_DISCARDS_COMPLETED_TOTAL,
            Number::U64(stat.discards_completed),
        );
        group.push(
            NAME_DISK_DISCARDS_MERGED_TOTAL,
            Number::U64(stat.discards_merged),
        );
        group.push(
            NAME_DISK_DISCARD_TIME_MS_TOTAL,
            Number::U64(stat.discard_ms),
        );
        group.push(
            NAME_DISK_FLUSH_REQUESTS_TOTAL,
            Number::U64(stat.flush_requests),
        );
        group.push(NAME_DISK_FLUSH_TIME_MS_TOTAL, Number::U64(stat.flush_ms));
        out.push(group);
    }
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
    fn collect_all_disk_devices() {
        let hostname = KeyValue::new("hostname", Arc::from("test-host"));
        let filter = DiskFilter::builder().build();

        let mut out = Vec::new();
        collect_disk_devices(&hostname, &filter, sample_disks(), &mut out);

        assert_eq!(out.len(), 4);
    }

    #[test]
    fn collect_excluded_disk_devices() {
        let hostname = KeyValue::new("hostname", Arc::from("test-host"));
        let exclude_regex = RegexSet::new(&["sda\\d+", "nvme0n1p\\d+"]).unwrap();
        let filter = DiskFilter::builder()
            .exclude_devices_regex(exclude_regex)
            .build();

        let mut out = Vec::new();
        collect_disk_devices(&hostname, &filter, sample_disks(), &mut out);

        assert_eq!(out.len(), 2);
        let sda = group(&out, "sda");
        assert!(
            sda.attrs
                .iter()
                .any(|kv| kv.key.as_str() == "hostname" && kv.value.as_str() == "test-host")
        );
        assert_eq!(
            value(sda, NAME_DISK_READ_BYTES_TOTAL),
            Number::U64(100 * SECTOR_SIZE)
        );
        assert_eq!(value(sda, NAME_DISK_READS_COMPLETED_TOTAL), Number::U64(10));
        assert_eq!(value(sda, NAME_DISK_READS_MERGED_TOTAL), Number::U64(2));
        assert_eq!(value(sda, NAME_DISK_READ_TIME_MS_TOTAL), Number::F64(0.02));
        assert_eq!(
            value(sda, NAME_DISK_WRITE_BYTES_TOTAL),
            Number::U64(200 * SECTOR_SIZE)
        );
        assert_eq!(
            value(sda, NAME_DISK_WRITES_COMPLETED_TOTAL),
            Number::U64(30)
        );
        assert_eq!(value(sda, NAME_DISK_WRITES_MERGED_TOTAL), Number::U64(4));
        assert_eq!(value(sda, NAME_DISK_WRITE_TIME_MS_TOTAL), Number::F64(0.04));
        assert_eq!(value(sda, NAME_DISK_IO_IN_PROGRESS), Number::U64(1));
        assert_eq!(value(sda, NAME_DISK_IO_TIME_MS_TOTAL), Number::F64(0.06));
        assert_eq!(
            value(sda, NAME_DISK_IO_TIME_WEIGHTED_MS_TOTAL),
            Number::F64(0.07)
        );
        assert_eq!(
            value(sda, NAME_DISK_DISCARD_BYTES_TOTAL),
            Number::U64(50 * SECTOR_SIZE)
        );
        assert_eq!(
            value(sda, NAME_DISK_DISCARDS_COMPLETED_TOTAL),
            Number::U64(5)
        );
        assert_eq!(value(sda, NAME_DISK_DISCARDS_MERGED_TOTAL), Number::U64(1));
        assert_eq!(
            value(sda, NAME_DISK_DISCARD_TIME_MS_TOTAL),
            Number::F64(0.006)
        );
        assert_eq!(value(sda, NAME_DISK_FLUSH_REQUESTS_TOTAL), Number::U64(7));
        assert_eq!(
            value(sda, NAME_DISK_FLUSH_TIME_MS_TOTAL),
            Number::F64(0.008)
        );
    }
}
