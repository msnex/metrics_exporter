use crate::ProcFsResult;
use std::path::Path;

pub trait FileRead: Sized {
    fn from_file<P>(path: P) -> ProcFsResult<Self>
    where
        P: AsRef<Path>;
}
