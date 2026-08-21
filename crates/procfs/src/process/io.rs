use crate::traits::FileRead;
use crate::{LINUX_PROC_PATH, ProcFsResult};
use std::fs::File;
use std::io::{BufRead, BufReader};
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
    fn from_file<P>(path: P) -> ProcFsResult<Self>
    where
        P: AsRef<Path>,
    {
        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        Ok(Self::parse_io(reader))
    }
}

impl Io {
    pub fn with_pid(pid: &str) -> ProcFsResult<Self> {
        let path = Path::new(LINUX_PROC_PATH).join(pid).join("io");
        FileRead::from_file(&path)
    }

    pub fn parse_io<R: BufRead>(reader: R) -> Self {
        let mut io = Self::default();

        for line in reader.lines() {
            let Ok(line) = line else { continue };
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
