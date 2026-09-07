//! CPU time fields of a task from `/proc/<pid>/stat` (per-thread rows come
//! from `/proc/<pid>/task/<tid>/stat`).
//!
//! Only the scheduling-time fields this crate exposes are parsed: `utime`
//! and `stime` (stat fields 14/15), measured in kernel USER_HZ ticks.
//! Values are kept in the raw units reported by the kernel; conversion to
//! seconds happens in the collection layer. Child process times
//! (`cutime`/`cstime`) are not read: like `top`'s per-process CPU columns,
//! descendants are accounted separately.
//!
//! The `stat` format starts with `pid (comm) state ...`, where `comm` may
//! contain spaces and parentheses; the closing `)` is located with a
//! rightmost search and every field after it is a whitespace-separated
//! token, with `utime`/`stime` at token index 11/12 (fields 14/15).
//!
//! Parsing is zero-copy: the file is read once into a single `Vec<u8>`
//! buffer and parsed as byte sub-slices — no UTF-8 validation pass, no
//! intermediate allocations.

use crate::parse::parse_u64;
use crate::traits::FileRead;
use crate::{LINUX_PROC_PATH, ProcResult};
use std::path::Path;

/// CPU time of one task from `/proc/<pid>/stat`, in kernel ticks (USER_HZ
/// units).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stat {
    /// Time scheduled in user mode, in USER_HZ ticks (stat field 14).
    pub utime_ticks: u64,
    /// Time scheduled in kernel mode, in USER_HZ ticks (stat field 15).
    pub stime_ticks: u64,
}

impl FileRead for Stat {
    fn from_file<P>(path: P) -> ProcResult<Self>
    where
        P: AsRef<Path>,
    {
        // Read raw bytes directly: `/proc/<pid>/stat` is ASCII and parsing
        // from `&[u8]` avoids a UTF-8 validation pass and intermediate
        // `String`s.
        let content = std::fs::read(path)?;
        Self::parse_stat(&content)
    }
}

impl Stat {
    pub fn with_pid(pid: &str) -> ProcResult<Self> {
        let path = Path::new(LINUX_PROC_PATH).join(pid).join("stat");
        FileRead::from_file(&path)
    }

    /// Parse `/proc/<pid>/stat` content.
    ///
    /// The token after the `)` closing the comm field carries the state
    /// (field 3); utime and stime are the 12th and 13th tokens after it
    /// (fields 14/15). A snapshot without a closing `)` or without enough
    /// numeric tokens for both fields fails as a whole.
    pub fn parse_stat(content: &[u8]) -> ProcResult<Self> {
        let Some(comm_end) = content.iter().rposition(|&byte| byte == b')') else {
            return Err(parse_u64(None).unwrap_err());
        };

        let mut tokens = content[comm_end + 1..]
            .split(u8::is_ascii_whitespace)
            .filter(|token| !token.is_empty());

        // Token 0 is the state, tokens 1..10 are ppid through cmajflt,
        // token 11 is utime (field 14) and token 12 is stime (field 15).
        let Some(utime) = tokens.nth(11) else {
            return Err(parse_u64(None).unwrap_err());
        };
        let Some(stime) = tokens.next() else {
            return Err(parse_u64(None).unwrap_err());
        };

        Ok(Self {
            utime_ticks: parse_u64(Some(utime))?,
            stime_ticks: parse_u64(Some(stime))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::Process;

    /// Real-world shaped sample: the comm contains spaces and parentheses,
    /// fields after it follow the kernel field order (state, ppid, pgrp,
    /// session, tty_nr, tpgid, flags, minflt, cminflt, majflt, cmajflt,
    /// utime, stime, ...).
    const SAMPLE: &[u8] = b"12345 (multithreaded proc (svc)) S 1 12345 12345 0 -1 4194304 1234 0 567 0 150 250 0 0 20 0 1 0 2279338 12345 18446744073709551615 4194304 140734576435200 140734576414176 140737488343040 140734575300608 140734575300672 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
";

    #[test]
    fn parse_stat_parses_sample() {
        let stat = Stat::parse_stat(SAMPLE).unwrap();
        assert_eq!(stat.utime_ticks, 150);
        assert_eq!(stat.stime_ticks, 250);
    }

    #[test]
    fn parse_stat_handles_comm_without_special_chars() {
        let content = b"1 (init) S 0 1 1 0 -1 4194560 10 0 0 0 7 3 0 0 20 0 1 0 1 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n";
        let stat = Stat::parse_stat(content).unwrap();
        assert_eq!(stat.utime_ticks, 7);
        assert_eq!(stat.stime_ticks, 3);
    }

    #[test]
    fn parse_stat_fails_without_closing_paren() {
        assert!(Stat::parse_stat(b"12345 comm broken S 1 2 3").is_err());
    }

    #[test]
    fn parse_stat_fails_without_enough_fields() {
        // Valid comm close, but fewer than 13 trailing tokens.
        assert!(Stat::parse_stat(b"12345 (svc) R 1 2 3 4 5 6 7 8 9 10").is_err());
        assert!(Stat::parse_stat(b"").is_err());
    }

    #[test]
    fn parse_stat_fails_on_malformed_time_tokens() {
        assert!(Stat::parse_stat(b"1 (svc) R 1 2 3 4 5 6 7 8 9 10 abc 3 0").is_err());
        assert!(Stat::parse_stat(b"1 (svc) R 1 2 3 4 5 6 7 8 9 10 7 -x 0").is_err());
    }

    #[test]
    fn stat_live_self_parses() {
        let pid = std::process::id() as i32;
        // A just-started test task may not have consumed a full kernel
        // tick yet, so only parseability is asserted here; the fixture
        // tests pin the exact field values.
        let _ = Process::with_pid(pid).stat().unwrap();
    }

    #[test]
    fn stat_live_threads_parse() {
        use std::sync::mpsc;
        use std::time::Duration;

        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            ready_tx.send(()).unwrap();
            let _ = stop_rx.recv_timeout(Duration::from_secs(5));
        });
        ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        let pid = std::process::id() as i32;
        let task_root = Path::new(LINUX_PROC_PATH)
            .join(pid.to_string())
            .join("task");

        // A task may exit between `read_dir` and the read below (the
        // parallel test harness spawns and joins threads of this process);
        // tolerate vanished rows and assert on the survivors.
        let mut parsed = 0usize;
        for entry in std::fs::read_dir(task_root).unwrap() {
            let tid = entry.unwrap().file_name();
            let tid = tid.to_string_lossy().parse::<i32>().unwrap();
            let Ok(_stat) = Process::with_pid(tid).stat() else {
                continue;
            };
            parsed += 1;
        }
        assert!(
            parsed >= 2,
            "expected at least 2 live task rows, saw {}",
            parsed
        );

        stop_tx.send(()).unwrap();
        handle.join().unwrap();
    }
}
