use crate::names::{
    NAME_PROCESS_IO_CANCELLED_WRITE_BYTES_TOTAL, NAME_PROCESS_IO_RCHAR_TOTAL,
    NAME_PROCESS_IO_READ_BYTES_TOTAL, NAME_PROCESS_IO_SYSCR_TOTAL, NAME_PROCESS_IO_SYSCW_TOTAL,
    NAME_PROCESS_IO_WCHAR_TOTAL, NAME_PROCESS_IO_WRITE_BYTES_TOTAL,
};
use metrics_framework::Number;
use metrics_framework::SampleGroup;
use opentelemetry::KeyValue;
use procfs::process::Process;
use regex::RegexSet;
use smallvec::smallvec;
use tracing::debug;

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

        let mut group = SampleGroup::with_attrs(smallvec![
            hostname.clone(),
            KeyValue::new("pid", process.pid() as i64),
            KeyValue::new("comm", comm),
        ]);

        match process.io() {
            Ok(io) => {
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
                out.push(group);
            }
            Err(err) => {
                debug!("read io for pid {} failed: {}", process.pid(), err);
            }
        }

        if cfg.thread && collector != CollectorType::Thread {
            let Ok(tasks) = process.tasks() else {
                continue;
            };
            collect_process(hostname, cfg, &tasks, CollectorType::Thread, out);
        }
    }
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
    use regex::RegexSet;

    #[test]
    fn comm_regex_test() {
        let comm_regex = RegexSet::new(&["W#.*", "rust"]).unwrap();
        assert_eq!(comm_regex.is_match("W#eno1"), true);
        assert_eq!(comm_regex.is_match("rust-analyzer"), true);
        assert_eq!(comm_regex.is_match("w#eno1"), false);
    }
}
