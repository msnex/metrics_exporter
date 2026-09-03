//! System-wide memory, swap and huge-page metrics from `/proc/meminfo`.
//!
//! One sample group per snapshot with the `host` attribute. Raw
//! kilobyte values are converted to bytes in this layer; huge-page bytes
//! are computed from the page count and page size rather than emitted as
//! raw page counts.

use crate::BYTES_PER_KB;
use crate::names::*;
use metrics_framework::{Number, SampleGroup};
use opentelemetry::KeyValue;
use procfs::meminfo::MemInfo;
use smallvec::smallvec;

/// Read `/proc/meminfo` and append host memory samples.
pub fn collect_mem_metrics(hostname: &KeyValue, out: &mut Vec<SampleGroup>) {
    let Ok(mem) = procfs::meminfo::meminfo() else {
        return;
    };
    collect_mem_values(hostname, &mem, out);
}

/// Append one host-level sample group from a `/proc/meminfo` snapshot.
///
/// Memory, swap and huge-page sizes are converted from KiB to bytes; huge
/// page totals/free are computed as `count * size`, so raw page counts are
/// never emitted.
fn collect_mem_values(hostname: &KeyValue, mem: &MemInfo, out: &mut Vec<SampleGroup>) {
    let mut group = SampleGroup::with_attrs(smallvec![hostname.clone()]);
    group.push(
        NAME_MEM_TOTAL_BYTES,
        Number::U64(mem.mem_total_kb * BYTES_PER_KB),
    );
    group.push(
        NAME_MEM_FREE_BYTES,
        Number::U64(mem.mem_free_kb * BYTES_PER_KB),
    );
    group.push(
        NAME_MEM_AVAILABLE_BYTES,
        Number::U64(mem.mem_available_kb * BYTES_PER_KB),
    );
    group.push(
        NAME_SWAP_TOTAL_BYTES,
        Number::U64(mem.swap_total_kb * BYTES_PER_KB),
    );
    group.push(
        NAME_SWAP_FREE_BYTES,
        Number::U64(mem.swap_free_kb * BYTES_PER_KB),
    );
    group.push(
        NAME_HUGE_PAGES_TOTAL_BYTES,
        Number::U64(mem.huge_pages_total * mem.huge_page_size_kb * BYTES_PER_KB),
    );
    group.push(
        NAME_HUGE_PAGES_FREE_BYTES,
        Number::U64(mem.huge_pages_free * mem.huge_page_size_kb * BYTES_PER_KB),
    );
    out.push(group);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn value(group: &SampleGroup, name: &str) -> Number {
        group
            .values
            .iter()
            .find(|value| value.name == name)
            .map(|value| value.value)
            .expect("metric value missing")
    }

    #[test]
    fn collect_mem_values_converts_and_computes_bytes() {
        let hostname = KeyValue::new("host", Arc::from("test-host"));
        let mem = MemInfo {
            mem_total_kb: 16_000,
            mem_free_kb: 4_000,
            mem_available_kb: 8_000,
            swap_total_kb: 2_000,
            swap_free_kb: 1_000,
            huge_pages_total: 10,
            huge_pages_free: 4,
            huge_page_size_kb: 2048,
        };
        let mut out = Vec::new();
        collect_mem_values(&hostname, &mem, &mut out);

        assert_eq!(out.len(), 1);
        let group = &out[0];
        assert!(
            group
                .attrs
                .iter()
                .any(|kv| kv.key.as_str() == "host" && kv.value.as_str() == "test-host")
        );
        assert_eq!(
            value(group, NAME_MEM_TOTAL_BYTES),
            Number::U64(16_000 * 1024)
        );
        assert_eq!(value(group, NAME_MEM_FREE_BYTES), Number::U64(4_000 * 1024));
        assert_eq!(
            value(group, NAME_MEM_AVAILABLE_BYTES),
            Number::U64(8_000 * 1024)
        );
        assert_eq!(
            value(group, NAME_SWAP_TOTAL_BYTES),
            Number::U64(2_000 * 1024)
        );
        assert_eq!(
            value(group, NAME_SWAP_FREE_BYTES),
            Number::U64(1_000 * 1024)
        );
        assert_eq!(
            value(group, NAME_HUGE_PAGES_TOTAL_BYTES),
            Number::U64(10 * 2048 * 1024)
        );
        assert_eq!(
            value(group, NAME_HUGE_PAGES_FREE_BYTES),
            Number::U64(4 * 2048 * 1024)
        );
    }
}
