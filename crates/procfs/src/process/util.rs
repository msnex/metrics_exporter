use crate::ProcResult;
use crate::error::ProcError;
use crate::process::Comm;
use std::path::Path;

pub fn parse_comm(path: &Path) -> ProcResult<Comm> {
    match std::fs::read(&path) {
        Ok(content) => {
            let mut comm = Comm::default();
            let content = content.strip_suffix(b"\n").unwrap_or(&content);
            let len = content.len().min(comm.len());
            comm[..len].copy_from_slice(&content[..len]);
            Ok(comm)
        }
        Err(err) => Err(ProcError::IOError(err)),
    }
}

pub fn parse_cmdline(path: &Path) -> ProcResult<String> {
    match std::fs::read(&path) {
        Ok(content) => {
            let mut args = String::with_capacity(content.len());
            for arg in content.split(|&c| c == 0).filter(|arg| !arg.is_empty()) {
                if !args.is_empty() {
                    args.push(' ');
                }
                args.push_str(&String::from_utf8_lossy(arg));
            }
            Ok(args)
        }
        Err(err) => Err(ProcError::IOError(err)),
    }
}
