use crate::traits::FileRead;
use crate::{LINUX_PROC_PATH, ProcFsResult};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct Io {
    pub rchar: usize,
    pub wchar: usize,
    pub syscr: usize,
    pub syscw: usize,
    pub read_bytes: usize,
    pub write_bytes: usize,
    pub cancelled_write_bytes: usize,
}

impl FileRead for Io {
    fn from_file<P>(path: P) -> ProcFsResult<Self>
    where
        P: AsRef<Path>,
    {
        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        Self::parse_io(reader)
    }
}

impl Io {
    pub fn with_pid(pid: &str) -> ProcFsResult<Self> {
        let path = Path::new(LINUX_PROC_PATH).join(pid).join("io");
        FileRead::from_file(&path)
    }

    fn parse_io(reader: BufReader<File>) -> ProcFsResult<Self> {
        let mut io = Self::default();

        for line in reader.lines() {
            if line.is_err() {
                continue;
            }
            let line = line.unwrap();

            let columns = line.split_once(':');
            if columns.is_none() {
                continue;
            }
            let columns = columns.unwrap();

            let key = columns.0.trim();
            let value = columns.1.trim();
            if key.is_empty() || value.is_empty() {
                continue;
            }

            let value = if let Ok(v) = lexical::parse(value) {
                v
            } else {
                continue;
            };

            match key {
                "rchar" => io.rchar = value,
                "wchar" => io.wchar = value,
                "syscr" => io.syscr = value,
                "syscw" => io.syscw = value,
                "read_bytes" => io.read_bytes = value,
                "write_bytes" => io.read_bytes = value,
                "cancelled_write_bytes" => io.cancelled_write_bytes = value,
                _ => {}
            }
        }
        Ok(io)
    }
}
