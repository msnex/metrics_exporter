//! System-wide memory, swap and huge-page statistics from `/proc/meminfo`.
//!
//! Values are kept in the raw units reported by the kernel (kilobytes for
//! memory/swap sizes, pages for huge-page counts); unit conversion happens
//! in the collection layer.

use crate::parse::parse_u64;
use crate::traits::FileRead;
use crate::{LINUX_PROC_PATH, ProcResult};
use std::path::Path;

/// Snapshot of `/proc/meminfo`.
#[derive(Debug, Default, Clone)]
pub struct MemInfo {
    /// Total usable memory, in kilobytes.
    pub mem_total_kb: u64,
    /// Free memory, in kilobytes.
    pub mem_free_kb: u64,
    /// Estimated memory available for starting new applications, in kilobytes.
    pub mem_available_kb: u64,
    /// Total swap space, in kilobytes.
    pub swap_total_kb: u64,
    /// Free swap space, in kilobytes.
    pub swap_free_kb: u64,
    /// Total number of huge pages configured.
    pub huge_pages_total: u64,
    /// Number of free huge pages.
    pub huge_pages_free: u64,
    /// Size of one huge page, in kilobytes.
    pub huge_page_size_kb: u64,
}

impl FileRead for MemInfo {
    fn from_file<P>(path: P) -> ProcResult<Self>
    where
        P: AsRef<Path>,
    {
        // Read raw bytes directly: `/proc/meminfo` is ASCII and parsing from
        // `&[u8]` avoids a UTF-8 validation pass and intermediate `String`s.
        let content = std::fs::read(path)?;
        Self::parse_meminfo(&content)
    }
}

impl MemInfo {
    /// Parse `/proc/meminfo` content.
    ///
    /// Lines use `Key: value [unit]`; unknown keys are ignored. The core
    /// fields MemTotal, MemFree, SwapTotal and SwapFree are required, so a
    /// missing or malformed core field fails the whole snapshot. Optional
    /// fields (MemAvailable and the huge-page entries) default to 0.
    pub fn parse_meminfo(content: &[u8]) -> ProcResult<Self> {
        let mut mem = Self::default();

        for line in content.split(|&byte| byte == b'\n') {
            let Some(colon) = line.iter().position(|&byte| byte == b':') else {
                continue;
            };

            let key = line[..colon].trim_ascii();
            let value_token = line[colon + 1..]
                .split(u8::is_ascii_whitespace)
                .find(|token| !token.is_empty());

            let value = match parse_u64(value_token) {
                Ok(value) => value,
                Err(err) if is_required(key) => return Err(err),
                Err(_) => continue,
            };

            match key {
                b"MemTotal" => {
                    mem.mem_total_kb = value;
                }
                b"MemFree" => {
                    mem.mem_free_kb = value;
                }
                b"MemAvailable" => mem.mem_available_kb = value,
                b"SwapTotal" => {
                    mem.swap_total_kb = value;
                }
                b"SwapFree" => {
                    mem.swap_free_kb = value;
                }
                b"HugePages_Total" => mem.huge_pages_total = value,
                b"HugePages_Free" => mem.huge_pages_free = value,
                b"Hugepagesize" => mem.huge_page_size_kb = value,
                _ => {}
            }
        }

        Ok(mem)
    }
}

/// Whether `key` is one of the required `/proc/meminfo` fields.
fn is_required(key: &[u8]) -> bool {
    matches!(key, b"MemTotal" | b"MemFree" | b"SwapTotal" | b"SwapFree")
}

/// Read the current memory statistics from `/proc/meminfo`.
pub fn meminfo() -> ProcResult<MemInfo> {
    let path = Path::new(LINUX_PROC_PATH).join("meminfo");
    MemInfo::from_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"MemTotal:        16177868 kB
MemFree:          1000000 kB
MemAvailable:     8000000 kB
Buffers:            123456 kB
Cached:            4567890 kB
SwapTotal:        4194304 kB
SwapFree:         2097152 kB
HugePages_Total:     128
HugePages_Free:       64
Hugepagesize:       2048 kB
";

    #[test]
    fn parse_meminfo_parses_sample() {
        let mem = MemInfo::parse_meminfo(SAMPLE).unwrap();
        assert_eq!(mem.mem_total_kb, 16_177_868);
        assert_eq!(mem.mem_free_kb, 1_000_000);
        assert_eq!(mem.mem_available_kb, 8_000_000);
        assert_eq!(mem.swap_total_kb, 4_194_304);
        assert_eq!(mem.swap_free_kb, 2_097_152);
        assert_eq!(mem.huge_pages_total, 128);
        assert_eq!(mem.huge_pages_free, 64);
        assert_eq!(mem.huge_page_size_kb, 2048);
    }

    #[test]
    fn parse_meminfo_defaults_optional_fields() {
        // Older kernels may omit MemAvailable and huge-page lines.
        let content = b"MemTotal: 1000 kB
MemFree: 200 kB
SwapTotal: 300 kB
SwapFree: 100 kB
";
        let mem = MemInfo::parse_meminfo(content).unwrap();
        assert_eq!(mem.mem_total_kb, 1000);
        assert_eq!(mem.mem_available_kb, 0);
        assert_eq!(mem.huge_pages_total, 0);
        assert_eq!(mem.huge_pages_free, 0);
        assert_eq!(mem.huge_page_size_kb, 0);
    }

    #[test]
    fn parse_meminfo_ignores_unknown_keys_and_units() {
        let content = b"MemTotal: 1000 kB
MemFree: 200 kB
SwapTotal: 300 kB
SwapFree: 100 kB
TotallyUnknown: 999 kB
";
        let mem = MemInfo::parse_meminfo(content).unwrap();
        assert_eq!(mem.mem_total_kb, 1000);
        assert_eq!(mem.mem_free_kb, 200);
    }

    #[test]
    fn meminfo_test() {
        let mem = meminfo().unwrap();
        assert!(mem.mem_total_kb > 0);
        assert!(mem.mem_total_kb >= mem.mem_free_kb);
        assert!(mem.swap_total_kb >= mem.swap_free_kb);
    }
}
