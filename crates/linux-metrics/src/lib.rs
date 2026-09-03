//! Linux metric collectors.
//!
//! This is the only crate allowed to couple with `procfs` (the atomic
//! proc-filesystem access layer). The collection framework and the exporter
//! application never see `procfs` types.
//!
//! Currently implements uptime, load average, CPU time, per-process I/O,
//! memory and task metrics, plus network interface metrics. Each sampling
//! cycle reads each `/proc/<pid>/` file at most once; per-tick allocations
//! are limited to one owned `String` per process.

/// Bytes per kilobyte as reported by the proc filesystem.
pub(crate) const BYTES_PER_KB: u64 = 1024;

mod cpu;
mod disk;
mod host;
mod mem;
mod names;
mod net;
mod process;

pub use disk::DiskFilter;
pub use host::HostCollector;
pub use host::HostCollectorCfg;
pub use net::NetFilter;
pub use process::ProcessFilter;
