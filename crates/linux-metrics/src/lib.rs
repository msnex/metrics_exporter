//! Linux metric collectors.
//!
//! This is the only crate allowed to couple with `procfs` (the atomic
//! proc-filesystem access layer). The collection framework and the exporter
//! application never see `procfs` types.
//!
//! Currently only per-process I/O metrics are implemented. Each sampling
//! cycle reads each `/proc/<pid>/` file at most once; per-tick allocations
//! are limited to one owned `String` (comm) per process.

mod host;

pub use host::HostCollector;
pub use host::ProcessFilter;
