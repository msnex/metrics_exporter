//! Parent PID, state and memory fields from `/proc/<pid>/status`
//! (per-thread rows come from `/proc/<pid>/task/<tid>/status`).
//!
//! A single `status` file carries every field this crate exposes about a
//! process: parent PID, state character, virtual/peak/anonymous/
//! file/shmem resident breakdown, swap and hugetlb usage, fd table size,
//! thread count and context-switch counters — no other file needs to be
//! read for these values. `PPid` reports `real_parent->tgid`, the same
//! value as field 4 of `stat`; kernel thread creation inherits
//! `real_parent`, so thread rows report the same parent as their process.
//!
//! Values are kept in the raw units reported by the kernel (kilobytes for
//! memory, counts for threads/fd slots/context switches); unit conversion
//! happens in the collection layer.
//!
//! Parsing is zero-copy: the file is read once into a single `Vec<u8>`
//! buffer and every line, key and value is handled as a byte sub-slice —
//! no UTF-8 validation pass, no per-line or per-field allocations.

use crate::error::ProcError;
use crate::parse::parse_i32;
use crate::parse::parse_u64;
use crate::traits::FileRead;
use crate::{LINUX_PROC_PATH, ProcResult};
use std::path::Path;

/// Snapshot of the identity, state and memory fields of
/// `/proc/<pid>/status`.
///
/// Process and thread rows share the same layout; on thread rows `State`
/// is the task's own state while the mm-backed fields (`Vm*`, `Rss*`,
/// `HugetlbPages`) and `Threads` describe the whole thread group.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Status {
    /// Parent process ID (`PPid`). For thread rows this is the parent of
    /// the whole thread group.
    pub ppid: i32,
    /// Process state character (`State`), e.g. `R`, `S`, `D`, `T`, `Z` or
    /// `t`; the parenthesized description in the file is dropped.
    pub state: char,
    /// Total virtual memory size, in kilobytes (`VmSize`).
    pub vm_size_kb: u64,
    /// Resident (physical) memory size, in kilobytes (`VmRSS`).
    pub vm_rss_kb: u64,
    /// Historical peak resident memory, in kilobytes (`VmHWM`; kernel >=
    /// 2.6.18, 0 when unavailable).
    pub vm_hwm_kb: u64,
    /// Resident anonymous pages, in kilobytes (`RssAnon`; kernel >= 4.5,
    /// 0 when unavailable).
    pub rss_anon_kb: u64,
    /// Resident file-backed pages (shared libraries, mappings), in
    /// kilobytes (`RssFile`; kernel >= 4.5, 0 when unavailable).
    pub rss_file_kb: u64,
    /// Resident shmem/tmpfs pages, in kilobytes (`RssShmem`; kernel >= 4.5,
    /// 0 when unavailable).
    pub rss_shmem_kb: u64,
    /// Swapped-out memory, in kilobytes (`VmSwap`; 0 when unavailable).
    pub vm_swap_kb: u64,
    /// Resident hugetlb memory, in kilobytes (`HugetlbPages`; 0 when
    /// unavailable).
    pub hugetlb_pages_kb: u64,
    /// Number of file descriptor slots currently allocated (`FDSize`;
    /// kernel >= 2.6.24). This is the table size, not the number of open
    /// file descriptors.
    pub fd_size: u64,
    /// Number of threads in the thread group (`Threads`). On thread rows
    /// this is the process-wide count.
    pub threads: u64,
    /// Voluntary context switch count (`voluntary_ctxt_switches`; kernel
    /// >= 2.6.23, 0 when unavailable).
    pub voluntary_ctxt_switches: u64,
    /// Non-voluntary context switch count (`nonvoluntary_ctxt_switches`;
    /// kernel >= 2.6.23, 0 when unavailable).
    pub nonvoluntary_ctxt_switches: u64,
}

impl FileRead for Status {
    fn from_file<P>(path: P) -> ProcResult<Self>
    where
        P: AsRef<Path>,
    {
        // Read raw bytes directly: `/proc/<pid>/status` is ASCII and
        // parsing from `&[u8]` avoids a UTF-8 validation pass and
        // intermediate `String`s.
        let content = std::fs::read(path)?;
        Self::parse_status(&content)
    }
}

impl Status {
    pub fn with_pid(pid: &str) -> ProcResult<Self> {
        let path = Path::new(LINUX_PROC_PATH).join(pid).join("status");
        FileRead::from_file(&path)
    }

    /// Parse `/proc/<pid>/status` content.
    ///
    /// Lines use `Key: value kB`; unknown keys are ignored. The core fields
    /// PPid, State, VmSize and VmRSS are required, so a snapshot missing one
    /// of them — or carrying a malformed value — fails as a whole. Optional
    /// fields (VmHWM, RssAnon, FDSize, Threads, the context-switch
    /// counters, RssFile, RssShmem, VmSwap, HugetlbPages) default to 0.
    pub fn parse_status(content: &[u8]) -> ProcResult<Self> {
        let mut status = Self::default();
        let mut ppid_seen = false;
        let mut state_seen = false;
        let mut vm_size_seen = false;
        let mut vm_rss_seen = false;

        for line in content.split(|&byte| byte == b'\n') {
            let Some(colon) = line.iter().position(|&byte| byte == b':') else {
                continue;
            };

            let key = line[..colon].trim_ascii();
            let value_token = line[colon + 1..]
                .split(u8::is_ascii_whitespace)
                .find(|token| !token.is_empty());

            match key {
                b"PPid" => {
                    status.ppid = parse_i32(value_token)?;
                    ppid_seen = true;
                }
                b"State" => {
                    // The state is the first character of the value, e.g.
                    // `S` in `S (sleeping)`; the description is dropped.
                    let Some(&state) = value_token.and_then(|token| token.first()) else {
                        return Err(required_field_error());
                    };
                    status.state = char::from(state);
                    state_seen = true;
                }
                b"VmSize" => {
                    status.vm_size_kb = parse_u64(value_token)?;
                    vm_size_seen = true;
                }
                b"VmRSS" => {
                    status.vm_rss_kb = parse_u64(value_token)?;
                    vm_rss_seen = true;
                }
                b"VmHWM" => {
                    if let Ok(value) = parse_u64(value_token) {
                        status.vm_hwm_kb = value;
                    }
                }
                b"RssAnon" => {
                    if let Ok(value) = parse_u64(value_token) {
                        status.rss_anon_kb = value;
                    }
                }
                b"RssFile" => {
                    if let Ok(value) = parse_u64(value_token) {
                        status.rss_file_kb = value;
                    }
                }
                b"RssShmem" => {
                    if let Ok(value) = parse_u64(value_token) {
                        status.rss_shmem_kb = value;
                    }
                }
                b"VmSwap" => {
                    if let Ok(value) = parse_u64(value_token) {
                        status.vm_swap_kb = value;
                    }
                }
                b"HugetlbPages" => {
                    if let Ok(value) = parse_u64(value_token) {
                        status.hugetlb_pages_kb = value;
                    }
                }
                b"FDSize" => {
                    if let Ok(value) = parse_u64(value_token) {
                        status.fd_size = value;
                    }
                }
                b"Threads" => {
                    if let Ok(value) = parse_u64(value_token) {
                        status.threads = value;
                    }
                }
                b"voluntary_ctxt_switches" => {
                    if let Ok(value) = parse_u64(value_token) {
                        status.voluntary_ctxt_switches = value;
                    }
                }
                b"nonvoluntary_ctxt_switches" => {
                    if let Ok(value) = parse_u64(value_token) {
                        status.nonvoluntary_ctxt_switches = value;
                    }
                }
                _ => {}
            }
        }

        if !ppid_seen || !state_seen || !vm_size_seen || !vm_rss_seen {
            return Err(required_field_error());
        }

        Ok(status)
    }
}

/// Lexical error for a required field whose line is missing or empty:
/// parsing an empty token never succeeds.
fn required_field_error() -> ProcError {
    parse_u64(None).unwrap_err()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::Process;

    /// Real-world shaped sample: tab-aligned values, `kB` units, a `Name`
    /// containing spaces and parentheses, and plenty of unknown keys.
    const SAMPLE: &[u8] = b"Name:\tfoo (bar) baz
Umask:\t0022
State:\tS (sleeping)
Tgid:\t1234
Ngid:\t0
Pid:\t1234
PPid:\t42
TracerPid:\t0
Uid:\t1000\t1000\t1000\t1000
Gid:\t1000\t1000\t1000\t1000
FDSize:\t256
Groups:\t10 20
VmPeak:\t   17452 kB
VmSize:\t   16384 kB
VmLck:\t       0 kB
VmPin:\t       0 kB
VmHWM:\t    6456 kB
VmRSS:\t    6220 kB
RssAnon:\t    2016 kB
RssFile:\t    4204 kB
RssShmem:\t       0 kB
VmData:\t   12996 kB
VmStk:\t     136 kB
VmExe:\t      28 kB
VmLib:\t    2040 kB
VmPTE:\t      88 kB
VmSwap:\t       512 kB
HugetlbPages:\t  2048 kB
CoreDumping:\t0
THP_enabled:\t1
Threads:\t4
SigQ:\t0/61455
SigPnd:\t0000000000000000
ShdPnd:\t0000000000000000
SigBlk:\t0000000000000000
SigIgn:\t0000000000000001
SigCgt:\t00000001800004e8
CapInh:\t0000000000000000
CapPrm:\t0000000000000000
CapEff:\t0000000000000000
CapBnd:\t000001ffffffffff
CapAmb:\t0000000000000000
NoNewPrivs:\t0
Seccomp:\t2
Seccomp_filters:\t1
Cpus_allowed:\tff
Cpus_allowed_list:\t0-7
Mems_allowed:\t1
Mems_allowed_list:\t0
voluntary_ctxt_switches:\t14
nonvoluntary_ctxt_switches:\t3
";

    #[test]
    fn parse_status_parses_sample() {
        let status = Status::parse_status(SAMPLE).unwrap();
        assert_eq!(status.ppid, 42);
        assert_eq!(status.state, 'S');
        assert_eq!(status.vm_size_kb, 16_384);
        assert_eq!(status.vm_rss_kb, 6_220);
        assert_eq!(status.vm_hwm_kb, 6_456);
        assert_eq!(status.rss_anon_kb, 2_016);
        assert_eq!(status.rss_file_kb, 4_204);
        assert_eq!(status.rss_shmem_kb, 0);
        assert_eq!(status.vm_swap_kb, 512);
        assert_eq!(status.hugetlb_pages_kb, 2_048);
        assert_eq!(status.fd_size, 256);
        assert_eq!(status.threads, 4);
        assert_eq!(status.voluntary_ctxt_switches, 14);
        assert_eq!(status.nonvoluntary_ctxt_switches, 3);
    }

    #[test]
    fn parse_status_defaults_optional_fields() {
        // Older kernels omit the resident-page breakdown, VmSwap and
        // HugetlbPages lines; every optional field defaults to 0.
        let content = b"PPid:\t7
State:\tS (sleeping)
VmSize:\t1000 kB
VmRSS:\t500 kB
";
        let status = Status::parse_status(content).unwrap();
        assert_eq!(status.ppid, 7);
        assert_eq!(status.state, 'S');
        assert_eq!(status.vm_size_kb, 1000);
        assert_eq!(status.vm_rss_kb, 500);
        assert_eq!(status.vm_hwm_kb, 0);
        assert_eq!(status.rss_anon_kb, 0);
        assert_eq!(status.rss_file_kb, 0);
        assert_eq!(status.rss_shmem_kb, 0);
        assert_eq!(status.vm_swap_kb, 0);
        assert_eq!(status.hugetlb_pages_kb, 0);
        assert_eq!(status.fd_size, 0);
        assert_eq!(status.threads, 0);
        assert_eq!(status.voluntary_ctxt_switches, 0);
        assert_eq!(status.nonvoluntary_ctxt_switches, 0);
    }

    #[test]
    fn parse_status_fails_without_required_fields() {
        // Missing PPid line.
        assert!(
            Status::parse_status(b"State:\tS (sleeping)\nVmSize:\t1000 kB\nVmRSS:\t500 kB\n")
                .is_err(),
            "missing PPid must fail the snapshot"
        );
        // Missing State line.
        assert!(
            Status::parse_status(b"PPid:\t7\nVmSize:\t1000 kB\nVmRSS:\t500 kB\n").is_err(),
            "missing State must fail the snapshot"
        );
        // Missing VmSize line.
        assert!(
            Status::parse_status(b"PPid:\t7\nState:\tS (sleeping)\nVmRSS:\t500 kB\n").is_err(),
            "missing VmSize must fail the snapshot"
        );
        // Missing VmRSS line.
        assert!(
            Status::parse_status(b"PPid:\t7\nState:\tS (sleeping)\nVmSize:\t1000 kB\n").is_err(),
            "missing VmRSS must fail the snapshot"
        );
        // Empty content.
        assert!(Status::parse_status(b"").is_err());
    }

    #[test]
    fn parse_status_fails_on_malformed_required_values() {
        assert!(
            Status::parse_status(
                b"PPid:\tabc\nState:\tS (sleeping)\nVmSize:\t1000 kB\nVmRSS:\t500 kB\n"
            )
            .is_err()
        );
        assert!(
            Status::parse_status(
                b"PPid:\t7\nState:\tS (sleeping)\nVmSize:\tnope kB\nVmRSS:\t500 kB\n"
            )
            .is_err()
        );
        // Empty State value.
        assert!(
            Status::parse_status(b"PPid:\t7\nState:\t\nVmSize:\t1000 kB\nVmRSS:\t500 kB\n")
                .is_err()
        );
    }

    #[test]
    fn parse_status_skips_malformed_optional_values() {
        let content = b"PPid:\t7
State:\tS (sleeping)
VmSize:\t1000 kB
VmRSS:\t500 kB
VmHWM:\tbroken kB
RssAnon:\t-1 kB
RssFile:\tbroken kB
Threads:\tmany
VmSwap:\t-3 kB
";
        let status = Status::parse_status(content).unwrap();
        assert_eq!(status.vm_hwm_kb, 0);
        assert_eq!(status.rss_anon_kb, 0);
        assert_eq!(status.rss_file_kb, 0);
        assert_eq!(status.threads, 0);
        assert_eq!(status.vm_swap_kb, 0);
        assert_eq!(status.state, 'S');
    }

    #[test]
    fn status_live_self_matches_stat_ppid() {
        let pid = std::process::id() as i32;
        let status = Process::with_pid(pid).status().unwrap();
        assert!(
            status.ppid > 0,
            "ppid must be positive, got {}",
            status.ppid
        );
        assert!(
            matches!(
                status.state,
                'R' | 'S' | 'D' | 'T' | 't' | 'Z' | 'X' | 'x' | 'K' | 'W' | 'P' | 'I'
            ),
            "state must be a valid kernel state char, got {:?}",
            status.state
        );
        assert!(status.vm_size_kb > 0);
        assert!(status.vm_rss_kb > 0);
        assert!(status.vm_hwm_kb > 0);
        assert!(status.rss_anon_kb > 0);
        assert!(status.fd_size > 0);
        assert!(status.threads >= 1);

        // PPid in `status` is sourced from the same `real_parent->tgid` as
        // field 4 of `stat`; spot-check the equivalence on live data.
        let stat_path = Path::new(LINUX_PROC_PATH)
            .join(pid.to_string())
            .join("stat");
        let stat = std::fs::read_to_string(stat_path).unwrap();
        let after_comm = &stat[stat.rfind(')').unwrap() + 1..];
        let stat_ppid: i32 = after_comm
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(status.ppid, stat_ppid);
    }

    #[test]
    fn status_live_threads_report_process_ppid() {
        use std::sync::mpsc;
        use std::time::Duration;

        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            // Signal that the thread exists, then park until the main
            // thread has scanned the task directory.
            ready_tx.send(()).unwrap();
            let _ = stop_rx.recv_timeout(Duration::from_secs(5));
        });
        ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        let pid = std::process::id() as i32;
        let process_status = Process::with_pid(pid).status().unwrap();
        let task_root = Path::new(LINUX_PROC_PATH)
            .join(pid.to_string())
            .join("task");

        let mut threads_seen = 0usize;
        for entry in std::fs::read_dir(task_root).unwrap() {
            let tid = entry.unwrap().file_name();
            let tid = tid.to_string_lossy().parse::<i32>().unwrap();
            // The parallel test harness spawns and joins threads of this
            // process, so a task may exit between `read_dir` and the read
            // below; tolerate vanished rows and assert on the survivors.
            let Ok(status) = Process::with_pid(tid).status() else {
                continue;
            };
            assert_eq!(
                status.ppid, process_status.ppid,
                "thread {} must report the process ppid",
                tid
            );
            // `Threads` is the group-wide count, but the test harness runs
            // tests in parallel threads of this same process, so the count
            // may change between reads. Only assert the deterministic part:
            // the running main thread plus the parked thread below.
            assert!(
                status.threads >= 2,
                "thread {} must report at least 2 group threads, got {}",
                tid,
                status.threads
            );
            assert!(
                matches!(
                    status.state,
                    'R' | 'S' | 'D' | 'T' | 't' | 'Z' | 'X' | 'x' | 'K' | 'W' | 'P' | 'I'
                ),
                "thread {} state must be a valid kernel state char, got {:?}",
                tid,
                status.state
            );
            threads_seen += 1;
        }
        // The main thread and the parked thread below are alive for the
        // whole scan, so at least two rows must have parsed.
        assert!(
            threads_seen >= 2,
            "expected at least 2 live task rows, saw {}",
            threads_seen
        );

        stop_tx.send(()).unwrap();
        handle.join().unwrap();
    }
}
