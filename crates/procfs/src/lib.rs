pub mod error;
pub mod process;
mod traits;

pub(crate) const LINUX_PROC_PATH: &str = "/proc";

pub type ProcFsResult<T> = Result<T, error::ProcFsError>;
