use crate::parse::parse_u64;
use crate::traits::FileRead;
use crate::{LINUX_PROC_PATH, ProcResult};
use std::path::Path;

/// Statistics for one network interface from `/proc/net/dev`.
#[derive(Default, Clone)]
pub struct IfaceStats {
    pub name: String,
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub rx_errs: u64,
    pub rx_drop: u64,
    pub rx_fifo: u64,
    pub rx_frame: u64,
    pub rx_compressed: u64,
    pub rx_multicast: u64,
    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_errs: u64,
    pub tx_drop: u64,
    pub tx_fifo: u64,
    pub tx_colls: u64,
    pub tx_carrier: u64,
    pub tx_compressed: u64,
}

/// Snapshot of all network interface statistics from `/proc/net/dev`.
#[derive(Default, Clone)]
pub struct DevStats {
    pub interfaces: Vec<IfaceStats>,
}

impl FileRead for DevStats {
    fn from_file<P>(path: P) -> ProcResult<Self>
    where
        P: AsRef<Path>,
    {
        // Read raw bytes directly: `/proc/net/dev` is ASCII and parsing from
        // `&[u8]` avoids a UTF-8 validation pass and intermediate `String`s.
        let content = std::fs::read(path)?;
        Ok(Self::parse_dev(&content))
    }
}

impl DevStats {
    /// Parse `/proc/net/dev` content into interface statistics.
    ///
    /// Header lines (without a `:`) and malformed interface lines are skipped,
    /// matching the tolerant parsing style used elsewhere in this crate.
    pub fn parse_dev(content: &[u8]) -> Self {
        // Typical hosts have a small number of interfaces; reserve up front to
        // avoid repeated reallocations while parsing.
        let mut dev = Self {
            interfaces: Vec::with_capacity(8),
        };

        for line in content.split(|&b| b == b'\n') {
            let Some(colon) = line.iter().position(|&b| b == b':') else {
                continue;
            };

            let iface_name = line[..colon].trim_ascii();
            if iface_name.is_empty() {
                continue;
            }

            let mut tokens = line[colon + 1..]
                .split(|b| b.is_ascii_whitespace())
                .filter(|token| !token.is_empty());

            let Ok(rx_bytes) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(rx_packets) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(rx_errs) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(rx_drop) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(rx_fifo) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(rx_frame) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(rx_compressed) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(rx_multicast) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(tx_bytes) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(tx_packets) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(tx_errs) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(tx_drop) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(tx_fifo) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(tx_colls) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(tx_carrier) = parse_u64(tokens.next()) else {
                continue;
            };
            let Ok(tx_compressed) = parse_u64(tokens.next()) else {
                continue;
            };

            dev.interfaces.push(IfaceStats {
                name: String::from_utf8_lossy(iface_name).into_owned(),
                rx_bytes,
                rx_packets,
                rx_errs,
                rx_drop,
                rx_fifo,
                rx_frame,
                rx_compressed,
                rx_multicast,
                tx_bytes,
                tx_packets,
                tx_errs,
                tx_drop,
                tx_fifo,
                tx_colls,
                tx_carrier,
                tx_compressed,
            });
        }

        dev
    }
}

/// Read the current network device statistics.
pub fn dev_stats() -> ProcResult<DevStats> {
    let path = Path::new(LINUX_PROC_PATH).join("net").join("dev");
    DevStats::from_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16
 enp89s0: 100 200 0 0 0 0 0 0 300 400 0 0 0 0 0 0
";

    #[test]
    fn parse_dev_parses_sample() {
        let dev = DevStats::parse_dev(SAMPLE);
        assert_eq!(dev.interfaces.len(), 2);

        let lo = &dev.interfaces[0];
        assert_eq!(lo.name, "lo");
        assert_eq!(lo.rx_bytes, 1);
        assert_eq!(lo.rx_packets, 2);
        assert_eq!(lo.rx_errs, 3);
        assert_eq!(lo.rx_drop, 4);
        assert_eq!(lo.rx_fifo, 5);
        assert_eq!(lo.rx_frame, 6);
        assert_eq!(lo.rx_compressed, 7);
        assert_eq!(lo.rx_multicast, 8);
        assert_eq!(lo.tx_bytes, 9);
        assert_eq!(lo.tx_packets, 10);
        assert_eq!(lo.tx_errs, 11);
        assert_eq!(lo.tx_drop, 12);
        assert_eq!(lo.tx_fifo, 13);
        assert_eq!(lo.tx_colls, 14);
        assert_eq!(lo.tx_carrier, 15);
        assert_eq!(lo.tx_compressed, 16);

        let enp = &dev.interfaces[1];
        assert_eq!(enp.name, "enp89s0");
        assert_eq!(enp.rx_bytes, 100);
        assert_eq!(enp.tx_bytes, 300);
    }

    #[test]
    fn parse_dev_skips_malformed_lines() {
        let content = b"Inter-|   Receive
 face |bytes
    lo: 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15
 docker0: bad 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16
 eth0: 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16
";
        let dev = DevStats::parse_dev(content);
        assert_eq!(dev.interfaces.len(), 1);
        assert_eq!(dev.interfaces[0].name, "eth0");
    }

    #[test]
    fn dev_stats_contains_loopback() {
        let dev = dev_stats().unwrap();
        assert!(
            dev.interfaces
                .iter()
                .any(|interface| interface.name == "lo"),
            "loopback interface missing from /proc/net/dev"
        );
    }
}
