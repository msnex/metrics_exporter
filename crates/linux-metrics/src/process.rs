use crate::BYTES_PER_KB;
use crate::names::*;
use metrics_framework::Number;
use metrics_framework::SampleGroup;
use opentelemetry::KeyValue;
use procfs::process::Io;
use procfs::process::Process;
use procfs::process::Status;
use regex::RegexSet;
use smallvec::smallvec;
use tracing::trace;

#[derive(PartialEq)]
enum CollectorType {
    Process,
    Thread,
}

#[derive(Default, Clone, bon::Builder)]
pub struct ProcessFilter {
    comms_regex: Option<RegexSet>,
    thread: bool,
}

fn collect_process(
    hostname: &KeyValue,
    cfg: &ProcessFilter,
    processes: &Vec<Process>,
    collector: CollectorType,
    out: &mut Vec<SampleGroup>,
) {
    for process in processes {
        // comm is the gate: if it cannot be read the process is gone.
        let Ok(comm) = process.comm() else {
            continue;
        };

        if let Some(comms_regex) = cfg.comms_regex.as_ref()
            && !comms_regex.is_match(&comm)
        {
            continue;
        }

        let mut group = match collector {
            CollectorType::Process => SampleGroup::with_attrs(smallvec![
                hostname.clone(),
                KeyValue::new("pid", process.pid() as i64),
                KeyValue::new("comm", comm),
                KeyValue::new("thread", false)
            ]),
            CollectorType::Thread => SampleGroup::with_attrs(smallvec![
                hostname.clone(),
                KeyValue::new("pid", process.pid() as i64),
                KeyValue::new("comm", comm),
                KeyValue::new("thread", true)
            ]),
        };
        let mut emitted = false;

        match process.io() {
            Ok(io) => {
                emitted |= collect_io_metrics(&mut group, &io);
            }
            Err(err) => {
                trace!("read io for pid {} failed: {}", process.pid(), err);
            }
        }

        match process.status() {
            Ok(status) => {
                emitted |= collect_status_metrics(&mut group, &status);
            }
            Err(err) => {
                trace!("read status for pid {} failed: {}", process.pid(), err);
            }
        }

        if emitted {
            out.push(group);
        }

        if cfg.thread && collector != CollectorType::Thread {
            let Ok(tasks) = process.tasks() else {
                continue;
            };
            collect_process(hostname, cfg, &tasks, CollectorType::Thread, out);
        }
    }
}

fn collect_io_metrics(group: &mut SampleGroup, io: &Io) -> bool {
    group.push(NAME_PROCESS_IO_RCHAR_TOTAL, Number::U64(io.rchar));
    group.push(NAME_PROCESS_IO_WCHAR_TOTAL, Number::U64(io.wchar));
    group.push(NAME_PROCESS_IO_SYSCR_TOTAL, Number::U64(io.syscr));
    group.push(NAME_PROCESS_IO_SYSCW_TOTAL, Number::U64(io.syscw));
    group.push(NAME_PROCESS_IO_READ_BYTES_TOTAL, Number::U64(io.read_bytes));
    group.push(
        NAME_PROCESS_IO_WRITE_BYTES_TOTAL,
        Number::U64(io.write_bytes),
    );
    group.push(
        NAME_PROCESS_IO_CANCELLED_WRITE_BYTES_TOTAL,
        Number::U64(io.cancelled_write_bytes),
    );
    true
}

/// Append the memory and task samples of one `/proc/<pid>/status` snapshot
/// to `group`, converting kilobyte memory fields to bytes.
///
/// Emits the `process_mem_*_bytes` gauges (virtual, resident, peak,
/// shared, swap and hugetlb) plus `process_fd_size`, `process_threads`
/// and the two cumulative context-switch counters. The shared-memory
/// value is `RssFile + RssShmem`, the same resident file-backed + shmem
/// accounting as `top`'s SHR. Returns whether any sample was appended (a
/// successfully parsed status always does).
fn collect_status_metrics(group: &mut SampleGroup, status: &Status) -> bool {
    group.push(
        NAME_PROCESS_MEM_VIRT_BYTES,
        Number::U64(status.vm_size_kb * BYTES_PER_KB),
    );
    group.push(
        NAME_PROCESS_MEM_RSS_BYTES,
        Number::U64(status.vm_rss_kb * BYTES_PER_KB),
    );
    group.push(
        NAME_PROCESS_MEM_HWM_BYTES,
        Number::U64(status.vm_hwm_kb * BYTES_PER_KB),
    );
    group.push(
        NAME_PROCESS_MEM_SHARED_BYTES,
        Number::U64(status.rss_file_kb.saturating_add(status.rss_shmem_kb) * BYTES_PER_KB),
    );
    group.push(
        NAME_PROCESS_MEM_SWAP_BYTES,
        Number::U64(status.vm_swap_kb * BYTES_PER_KB),
    );
    group.push(
        NAME_PROCESS_MEM_HUGETLB_BYTES,
        Number::U64(status.hugetlb_pages_kb * BYTES_PER_KB),
    );
    group.push(NAME_PROCESS_FD_SIZE, Number::U64(status.fd_size));
    group.push(NAME_PROCESS_THREADS, Number::U64(status.threads));
    group.push(
        NAME_PROCESS_CTXT_SWITCHES_VOLUNTARY_TOTAL,
        Number::U64(status.voluntary_ctxt_switches),
    );
    group.push(
        NAME_PROCESS_CTXT_SWITCHES_NONVOLUNTARY_TOTAL,
        Number::U64(status.nonvoluntary_ctxt_switches),
    );
    true
}

pub fn collect_process_metrics(
    hostname: &KeyValue,
    cfg: &ProcessFilter,
    out: &mut Vec<SampleGroup>,
) {
    let Ok(processes) = procfs::process::get_all_processes() else {
        return;
    };

    collect_process(hostname, cfg, &processes, CollectorType::Process, out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use procfs::process::Status;
    use regex::RegexSet;

    fn value(group: &SampleGroup, name: &str) -> Number {
        group
            .values
            .iter()
            .find(|value| value.name == name)
            .map(|value| value.value)
            .expect("metric value missing")
    }

    fn sample_status() -> Status {
        Status {
            ppid: 42,
            state: 'S',
            vm_size_kb: 16_384,
            vm_rss_kb: 6_220,
            vm_hwm_kb: 6_456,
            rss_anon_kb: 2_016,
            rss_file_kb: 4_204,
            rss_shmem_kb: 100,
            vm_swap_kb: 512,
            hugetlb_pages_kb: 2_048,
            fd_size: 256,
            threads: 4,
            voluntary_ctxt_switches: 14,
            nonvoluntary_ctxt_switches: 3,
        }
    }

    #[test]
    fn push_status_samples_converts_kb_to_bytes_and_sums_shared() {
        let status = sample_status();
        let mut group = SampleGroup::with_attrs(smallvec![]);
        assert!(collect_status_metrics(&mut group, &status));

        assert_eq!(
            value(&group, NAME_PROCESS_MEM_VIRT_BYTES),
            Number::U64(16_384 * 1024)
        );
        assert_eq!(
            value(&group, NAME_PROCESS_MEM_RSS_BYTES),
            Number::U64(6_220 * 1024)
        );
        assert_eq!(
            value(&group, NAME_PROCESS_MEM_HWM_BYTES),
            Number::U64(6_456 * 1024)
        );
        // Shared is the RssFile + RssShmem sum, converted to bytes.
        assert_eq!(
            value(&group, NAME_PROCESS_MEM_SHARED_BYTES),
            Number::U64((4_204 + 100) * 1024)
        );
        assert_eq!(
            value(&group, NAME_PROCESS_MEM_SWAP_BYTES),
            Number::U64(512 * 1024)
        );
        assert_eq!(
            value(&group, NAME_PROCESS_MEM_HUGETLB_BYTES),
            Number::U64(2_048 * 1024)
        );
        assert_eq!(value(&group, NAME_PROCESS_FD_SIZE), Number::U64(256));
        assert_eq!(value(&group, NAME_PROCESS_THREADS), Number::U64(4));
        assert_eq!(
            value(&group, NAME_PROCESS_CTXT_SWITCHES_VOLUNTARY_TOTAL),
            Number::U64(14)
        );
        assert_eq!(
            value(&group, NAME_PROCESS_CTXT_SWITCHES_NONVOLUNTARY_TOTAL),
            Number::U64(3)
        );
        assert_eq!(group.values.len(), 10);
    }

    #[test]
    fn push_status_samples_handles_zero_fields() {
        let status = Status::default();
        let mut group = SampleGroup::with_attrs(smallvec![]);
        assert!(collect_status_metrics(&mut group, &status));

        assert_eq!(value(&group, NAME_PROCESS_MEM_VIRT_BYTES), Number::U64(0));
        assert_eq!(value(&group, NAME_PROCESS_MEM_SHARED_BYTES), Number::U64(0));
        assert_eq!(value(&group, NAME_PROCESS_THREADS), Number::U64(0));
        assert_eq!(
            value(&group, NAME_PROCESS_CTXT_SWITCHES_VOLUNTARY_TOTAL),
            Number::U64(0)
        );
    }

    #[test]
    fn comm_regex_test() {
        let comm_regex = RegexSet::new(&["W#.*", "rust"]).unwrap();
        assert_eq!(comm_regex.is_match("W#eno1"), true);
        assert_eq!(comm_regex.is_match("rust-analyzer"), true);
        assert_eq!(comm_regex.is_match("w#eno1"), false);
    }
}
