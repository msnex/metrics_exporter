//! Linux metric collectors.
//!
//! This is the only crate allowed to couple with `procfs` (the atomic
//! proc-filesystem access layer). The collection framework and the exporter
//! application never see `procfs` types.
//!
//! Currently implements uptime, per-process I/O, and network interface
//! metrics. Each sampling cycle reads each `/proc/<pid>/` file at most once;
//! per-tick allocations are limited to one owned `String` per process.

mod host;
mod net;
mod process;

pub use host::HostCollector;
pub use host::HostCollectorCfg;
pub use net::NetFilter;
pub use process::ProcessFilter;
