use crate::ProcResult;
use std::path::Path;

pub trait FileRead: Sized {
    fn from_file<P>(path: P) -> ProcResult<Self>
    where
        P: AsRef<Path>;
}
