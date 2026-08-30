use crate::parse::parse_u64;
use crate::traits::FileRead;
use crate::{LINUX_PROC_PATH, ProcResult};
use std::path::Path;

/// CPU time of one line from `/proc/stat`, in kernel ticks (USER_HZ units).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct CpuTimes {
    /// Line name: `cpu` for the aggregate, `cpu0`, `cpu1`, ... per core.
    pub name: String,
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
    pub guest: u64,
    pub guest_nice: u64,
}

/// Snapshot of all CPU time lines from `/proc/stat`.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct CpuStat {
    /// Aggregate line `cpu` first (when present), then `cpu0`, `cpu1`, ...
    pub cpus: Vec<CpuTimes>,
}

impl FileRead for CpuStat {
    fn from_file<P>(path: P) -> ProcResult<Self>
    where
        P: AsRef<Path>,
    {
        // Read raw bytes directly: `/proc/stat` is ASCII and parsing from
        // `&[u8]` avoids a UTF-8 validation pass and intermediate `String`s.
        let content = std::fs::read(path)?;
        Ok(Self::parse_stat(&content))
    }
}

impl CpuStat {
    /// Parse `/proc/stat` content into CPU time snapshots.
    ///
    /// Only lines whose first token starts with `cpu` are kept (the aggregate
    /// line plus per-core lines); other lines such as `intr`, `ctxt` and
    /// `btime` are skipped. Trailing numeric fields missing on older kernels
    /// default to 0; a cpu line with a malformed numeric token is skipped,
    /// matching the tolerant parsing style used elsewhere in this crate.
    pub fn parse_stat(content: &[u8]) -> Self {
        // Typical hosts have a small number of cores; reserve up front to
        // avoid repeated reallocations while parsing.
        let mut stat = Self {
            cpus: Vec::with_capacity(128),
        };

        'lines: for line in content.split(|&b| b == b'\n') {
            let mut tokens = line
                .split(u8::is_ascii_whitespace)
                .filter(|token| !token.is_empty());

            let Some(name) = tokens.next() else {
                continue;
            };
            if !name.starts_with(b"cpu") {
                continue;
            }

            let mut fields = [0u64; 10];
            for (slot, token) in fields.iter_mut().zip(tokens) {
                let Ok(value) = parse_u64(Some(token)) else {
                    continue 'lines;
                };
                *slot = value;
            }

            stat.cpus.push(CpuTimes {
                name: String::from_utf8_lossy(name).into_owned(),
                user: fields[0],
                nice: fields[1],
                system: fields[2],
                idle: fields[3],
                iowait: fields[4],
                irq: fields[5],
                softirq: fields[6],
                steal: fields[7],
                guest: fields[8],
                guest_nice: fields[9],
            });
        }

        stat
    }
}

/// Read the current CPU time statistics from `/proc/stat`.
pub fn cpu_stat() -> ProcResult<CpuStat> {
    let path = Path::new(LINUX_PROC_PATH).join("stat");
    CpuStat::from_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"cpu  87427 8 69803 6681582 5335 0 58161 0 0 0
cpu0 21634 0 19737 1667801 1718 0 24244 0 0 0
cpu1 21234 0 16822 1669871 1016 0 14871 0 0 0
intr 13623451 6 13 0
ctxt 20364211
btime 1788039862
";

    #[test]
    fn parse_stat_parses_cpu_lines() {
        let stat = CpuStat::parse_stat(SAMPLE);
        assert_eq!(stat.cpus.len(), 3);

        let aggregate = &stat.cpus[0];
        assert_eq!(aggregate.name, "cpu");
        assert_eq!(aggregate.user, 87427);
        assert_eq!(aggregate.nice, 8);
        assert_eq!(aggregate.system, 69803);
        assert_eq!(aggregate.idle, 6681582);
        assert_eq!(aggregate.iowait, 5335);
        assert_eq!(aggregate.irq, 0);
        assert_eq!(aggregate.softirq, 58161);
        assert_eq!(aggregate.steal, 0);
        assert_eq!(aggregate.guest, 0);
        assert_eq!(aggregate.guest_nice, 0);

        let cpu0 = &stat.cpus[1];
        assert_eq!(cpu0.name, "cpu0");
        assert_eq!(cpu0.user, 21634);
        assert_eq!(cpu0.idle, 1667801);
    }

    #[test]
    fn parse_stat_defaults_missing_fields() {
        let stat = CpuStat::parse_stat(b"cpu 1 2\ncpu0 3\n");
        assert_eq!(stat.cpus.len(), 2);

        assert_eq!(stat.cpus[0].user, 1);
        assert_eq!(stat.cpus[0].nice, 2);
        assert_eq!(stat.cpus[0].system, 0);
        assert_eq!(stat.cpus[0].idle, 0);

        assert_eq!(stat.cpus[1].name, "cpu0");
        assert_eq!(stat.cpus[1].user, 3);
    }

    #[test]
    fn parse_stat_skips_malformed_cpu_line() {
        let content = b"cpu 1 2 3 4 5 6 7 8 9 10\ncpu0 bad 3\ncpu1 1 2 3 4 5 6 7 8 9 10\n";
        let stat = CpuStat::parse_stat(content);
        assert_eq!(stat.cpus.len(), 2);
        assert_eq!(stat.cpus[0].name, "cpu");
        assert_eq!(stat.cpus[1].name, "cpu1");
    }

    #[test]
    fn cpu_stat_test() {
        let stat = cpu_stat().unwrap();
        let aggregate = stat
            .cpus
            .iter()
            .find(|entry| entry.name == "cpu")
            .expect("aggregate cpu line missing from /proc/stat");
        assert!(aggregate.idle > 0);
    }
}
