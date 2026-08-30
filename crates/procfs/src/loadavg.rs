use crate::parse::{parse_f64, parse_u32};
use crate::traits::FileRead;
use crate::{LINUX_PROC_PATH, ProcResult};
use std::path::Path;

/// Load averages and task statistics from `/proc/loadavg`.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LoadAvg {
    /// Load average over the last 1 minute.
    pub load1: f64,
    /// Load average over the last 5 minutes.
    pub load5: f64,
    /// Load average over the last 15 minutes.
    pub load15: f64,
    /// Number of currently running tasks.
    pub running: u32,
    /// Total number of tasks.
    pub total: u32,
    /// Last PID allocated by the kernel.
    pub last_pid: u32,
}

impl FileRead for LoadAvg {
    fn from_file<P>(path: P) -> ProcResult<Self>
    where
        P: AsRef<Path>,
    {
        // Read raw bytes directly: `/proc/loadavg` is ASCII and parsing from
        // `&[u8]` avoids a UTF-8 validation pass and intermediate `String`s.
        let content = std::fs::read(path)?;
        Self::parse_loadavg(&content)
    }
}

impl LoadAvg {
    /// Parse `/proc/loadavg` content.
    ///
    /// Strict parsing: all five whitespace-separated fields are required and
    /// a missing or malformed token returns `Err`, so partial or zeroed data
    /// is never reported as a valid snapshot. Values are parsed in place from
    /// the byte buffer with no copies or intermediate `String`s.
    pub fn parse_loadavg(content: &[u8]) -> ProcResult<Self> {
        let mut tokens = content
            .split(u8::is_ascii_whitespace)
            .filter(|token| !token.is_empty());

        let load1 = parse_f64(tokens.next())?;
        let load5 = parse_f64(tokens.next())?;
        let load15 = parse_f64(tokens.next())?;
        let (running, total) = parse_tasks(tokens.next())?;
        let last_pid = parse_u32(tokens.next())?;

        Ok(Self {
            load1,
            load5,
            load15,
            running,
            total,
            last_pid,
        })
    }
}

/// Read the current load averages from `/proc/loadavg`.
pub fn loadavg() -> ProcResult<LoadAvg> {
    let path = Path::new(LINUX_PROC_PATH).join("loadavg");
    LoadAvg::from_file(path)
}

/// Parse the `running/total` tasks token.
fn parse_tasks(token: Option<&[u8]>) -> ProcResult<(u32, u32)> {
    let mut parts = token.unwrap_or_default().split(|&byte| byte == b'/');
    let running = parse_u32(parts.next())?;
    let total = parse_u32(parts.next())?;
    Ok((running, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"0.00 0.01 0.05 1/123 4567";

    #[test]
    fn parse_loadavg_parses_sample() {
        let load = LoadAvg::parse_loadavg(SAMPLE).unwrap();
        assert_eq!(load.load1, 0.0);
        assert_eq!(load.load5, 0.01);
        assert_eq!(load.load15, 0.05);
        assert_eq!(load.running, 1);
        assert_eq!(load.total, 123);
        assert_eq!(load.last_pid, 4567);
    }

    #[test]
    fn parse_loadavg_tolerates_surrounding_whitespace() {
        let load = LoadAvg::parse_loadavg(b"  0.00 0.01 0.05 1/123 4567\n").unwrap();
        assert_eq!(load.total, 123);
    }

    #[test]
    fn parse_loadavg_rejects_missing_tokens() {
        let cases: &[&[u8]] = &[b"", b"0.00", b"0.00 0.01 0.05", b"0.00 0.01 0.05 1/123"];
        for content in cases {
            assert!(
                LoadAvg::parse_loadavg(content).is_err(),
                "expected error for content: {content:?}"
            );
        }
    }

    #[test]
    fn parse_loadavg_rejects_malformed_tokens() {
        let cases: &[&[u8]] = &[
            b"x 0.01 0.05 1/123 4567",     // bad load1
            b"0.00 0.01 0.05 1/123 bad",   // bad last_pid
            b"0.00 0.01 0.05 1 4567",      // tasks without '/'
            b"0.00 0.01 0.05 1/ 4567",     // empty total
            b"0.00 0.01 0.05 /123 4567",   // empty running
            b"0.00 0.01 0.05 1/123 -4567", // negative last_pid
        ];
        for content in cases {
            assert!(
                LoadAvg::parse_loadavg(content).is_err(),
                "expected error for content: {content:?}"
            );
        }
    }

    #[test]
    fn loadavg_test() {
        let load = loadavg().unwrap();
        assert!(load.load1 >= 0.0);
        assert!(load.load5 >= 0.0);
        assert!(load.load15 >= 0.0);
        assert!(load.total > 0);
        assert!(load.running <= load.total);
        assert!(load.last_pid > 0);
    }
}
