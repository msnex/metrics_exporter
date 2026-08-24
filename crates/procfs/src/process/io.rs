use crate::traits::FileRead;
use crate::{LINUX_PROC_PATH, ProcResult};
use std::path::Path;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Io {
    pub rchar: u64,
    pub wchar: u64,
    pub syscr: u64,
    pub syscw: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub cancelled_write_bytes: u64,
}

impl FileRead for Io {
    fn from_file<P>(path: P) -> ProcResult<Self>
    where
        P: AsRef<Path>,
    {
        // Read the whole file up front so a permission error (e.g. EACCES on
        // another user's /proc/<pid>/io) surfaces as Err instead of making
        // BufRead::lines() loop forever on repeated failed reads.
        let content = std::fs::read_to_string(&path)?;
        Ok(Self::parse_io(&content))
    }
}

impl Io {
    pub fn with_pid(pid: &str) -> ProcResult<Self> {
        let path = Path::new(LINUX_PROC_PATH).join(pid).join("io");
        FileRead::from_file(&path)
    }

    pub fn parse_io(content: &str) -> Self {
        let mut io = Self::default();

        for line in content.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };

            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                continue;
            }

            let Ok(value) = lexical::parse(value) else {
                continue;
            };

            match key {
                "rchar" => io.rchar = value,
                "wchar" => io.wchar = value,
                "syscr" => io.syscr = value,
                "syscw" => io.syscw = value,
                "read_bytes" => io.read_bytes = value,
                "write_bytes" => io.write_bytes = value,
                "cancelled_write_bytes" => io.cancelled_write_bytes = value,
                _ => {}
            }
        }
        io
    }
}
