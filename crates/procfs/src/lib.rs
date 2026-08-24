pub mod error;
pub mod process;
mod traits;

pub(crate) const LINUX_PROC_PATH: &str = "/proc";

pub type ProcResult<T> = Result<T, error::ProcError>;
