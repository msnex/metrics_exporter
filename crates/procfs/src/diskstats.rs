use crate::parse::parse_u64;
use crate::traits::FileRead;
use crate::{LINUX_PROC_PATH, ProcResult};
use std::path::Path;

/// Block-device statistics for one device from `/proc/diskstats`.
#[derive(Debug, Default, Clone)]
pub struct DiskStat {
    /// Device name, e.g. `sda`, `nvme0n1` or partition `sda1`.
    pub name: String,
    /// Reads completed successfully.
    pub reads_completed: u64,
    /// Reads merged.
    pub reads_merged: u64,
    /// Sectors read, in 512-byte units.
    pub sectors_read: u64,
    /// Time spent reading, in milliseconds.
    pub read_ms: u64,
    /// Writes completed successfully.
    pub writes_completed: u64,
    /// Writes merged.
    pub writes_merged: u64,
    /// Sectors written, in 512-byte units.
    pub sectors_written: u64,
    /// Time spent writing, in milliseconds.
    pub write_ms: u64,
    /// I/Os currently in progress.
    pub io_in_progress: u64,
    /// Time spent doing I/Os, in milliseconds.
    pub io_ms: u64,
    /// Weighted time spent doing I/Os, in milliseconds.
    pub weighted_io_ms: u64,
    /// Discards completed successfully.
    pub discards_completed: u64,
    /// Discards merged.
    pub discards_merged: u64,
    /// Sectors discarded, in 512-byte units.
    pub sectors_discarded: u64,
    /// Time spent discarding, in milliseconds.
    pub discard_ms: u64,
    /// Flush requests completed successfully.
    pub flush_requests: u64,
    /// Time spent flushing, in milliseconds.
    pub flush_ms: u64,
}

/// Snapshot of all block-device statistics from `/proc/diskstats`.
#[derive(Debug, Default, Clone)]
pub struct DiskStats {
    /// One entry per device line in `/proc/diskstats`.
    pub disks: Vec<DiskStat>,
}

impl FileRead for DiskStats {
    fn from_file<P>(path: P) -> ProcResult<Self>
    where
        P: AsRef<Path>,
    {
        // Read raw bytes directly: `/proc/diskstats` is ASCII and parsing
        // from `&[u8]` avoids a UTF-8 validation pass and intermediate
        // `String`s.
        let content = std::fs::read(path)?;
        Ok(Self::parse_diskstats(&content))
    }
}

impl DiskStats {
    /// Parse `/proc/diskstats` content into block-device statistics.
    ///
    /// Every line starts with the major, minor and device name followed by
    /// up to 17 numeric fields. Trailing fields missing on older kernels
    /// (discards, flushes) default to 0; a line with a malformed numeric
    /// token is skipped, matching the tolerant parsing style used elsewhere
    /// in this crate.
    pub fn parse_diskstats(content: &[u8]) -> Self {
        // Typical hosts expose a small number of block devices; reserve up
        // front to avoid repeated reallocations while parsing.
        let mut stats = Self {
            disks: Vec::with_capacity(16),
        };

        'lines: for line in content.split(|&b| b == b'\n') {
            let mut tokens = line
                .split(u8::is_ascii_whitespace)
                .filter(|token| !token.is_empty());

            let Some(major) = tokens.next() else {
                continue;
            };
            let Some(minor) = tokens.next() else {
                continue;
            };
            let Some(name) = tokens.next() else {
                continue;
            };

            // Validate major/minor numbers so non-device lines (e.g. a
            // stray header) are skipped like any other malformed line.
            if parse_u64(Some(major)).is_err() || parse_u64(Some(minor)).is_err() {
                continue;
            }

            let mut fields = [0u64; 17];
            for (slot, token) in fields.iter_mut().zip(tokens) {
                let Ok(value) = parse_u64(Some(token)) else {
                    continue 'lines;
                };
                *slot = value;
            }

            stats.disks.push(DiskStat {
                name: String::from_utf8_lossy(name).into_owned(),
                reads_completed: fields[0],
                reads_merged: fields[1],
                sectors_read: fields[2],
                read_ms: fields[3],
                writes_completed: fields[4],
                writes_merged: fields[5],
                sectors_written: fields[6],
                write_ms: fields[7],
                io_in_progress: fields[8],
                io_ms: fields[9],
                weighted_io_ms: fields[10],
                discards_completed: fields[11],
                discards_merged: fields[12],
                sectors_discarded: fields[13],
                discard_ms: fields[14],
                flush_requests: fields[15],
                flush_ms: fields[16],
            });
        }

        stats
    }
}

/// Read the current block-device statistics from `/proc/diskstats`.
pub fn disk_stats() -> ProcResult<DiskStats> {
    let path = Path::new(LINUX_PROC_PATH).join("diskstats");
    DiskStats::from_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"8       0 sda 429980 165336 16149224 110718 400903 1473565 33577416 266291 0 142304 377009 0 0 0 0 0 0
8       1 sda1 190 22 6178 25 13 7 160 19 0 28 45 0 0 0 0 0 0
11      0 sr0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
";

    #[test]
    fn parse_diskstats_parses_sample() {
        let stats = DiskStats::parse_diskstats(SAMPLE);
        assert_eq!(stats.disks.len(), 3);

        let sda = &stats.disks[0];
        assert_eq!(sda.name, "sda");
        assert_eq!(sda.reads_completed, 429980);
        assert_eq!(sda.reads_merged, 165336);
        assert_eq!(sda.sectors_read, 16149224);
        assert_eq!(sda.read_ms, 110718);
        assert_eq!(sda.writes_completed, 400903);
        assert_eq!(sda.writes_merged, 1473565);
        assert_eq!(sda.sectors_written, 33577416);
        assert_eq!(sda.write_ms, 266291);
        assert_eq!(sda.io_in_progress, 0);
        assert_eq!(sda.io_ms, 142304);
        assert_eq!(sda.weighted_io_ms, 377009);
        assert_eq!(sda.discards_completed, 0);
        assert_eq!(sda.discards_merged, 0);
        assert_eq!(sda.sectors_discarded, 0);
        assert_eq!(sda.discard_ms, 0);
        assert_eq!(sda.flush_requests, 0);
        assert_eq!(sda.flush_ms, 0);

        let sda1 = &stats.disks[1];
        assert_eq!(sda1.name, "sda1");
        assert_eq!(sda1.reads_completed, 190);
        assert_eq!(sda1.write_ms, 19);
    }

    #[test]
    fn parse_diskstats_parses_discard_and_flush_fields() {
        let content = b"8 0 sda 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17\n";
        let stats = DiskStats::parse_diskstats(content);
        assert_eq!(stats.disks.len(), 1);

        let sda = &stats.disks[0];
        assert_eq!(sda.discards_completed, 12);
        assert_eq!(sda.discards_merged, 13);
        assert_eq!(sda.sectors_discarded, 14);
        assert_eq!(sda.discard_ms, 15);
        assert_eq!(sda.flush_requests, 16);
        assert_eq!(sda.flush_ms, 17);
    }

    #[test]
    fn parse_diskstats_defaults_missing_fields() {
        // Pre-4.18 kernels stop after the weighted I/O time field.
        let content = b"8 0 sda 1 2 3 4 5 6 7 8 9 10 11\n";
        let stats = DiskStats::parse_diskstats(content);
        assert_eq!(stats.disks.len(), 1);

        let sda = &stats.disks[0];
        assert_eq!(sda.reads_completed, 1);
        assert_eq!(sda.weighted_io_ms, 11);
        assert_eq!(sda.discards_completed, 0);
        assert_eq!(sda.discard_ms, 0);
        assert_eq!(sda.flush_requests, 0);
        assert_eq!(sda.flush_ms, 0);
    }

    #[test]
    fn parse_diskstats_skips_malformed_lines() {
        let content = b"8 0 sda 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17\n
8 1 sdb bad 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17\n
8 2 sdc 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17\n
not a device line\n
";
        let stats = DiskStats::parse_diskstats(content);
        assert_eq!(stats.disks.len(), 2);
        assert_eq!(stats.disks[0].name, "sda");
        assert_eq!(stats.disks[1].name, "sdc");
    }

    #[test]
    fn disk_stats_is_non_empty() {
        let stats = disk_stats().unwrap();
        assert!(
            !stats.disks.is_empty(),
            "no block devices found in /proc/diskstats"
        );
    }
}
