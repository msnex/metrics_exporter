use crate::LINUX_PROC_PATH;
use crate::ProcResult;
use crate::traits::FileRead;
use std::path::Path;
use std::path::PathBuf;

mod io;
mod task;
mod util;

pub use io::Io;

const COMM_MAX_LEN: usize = 16;
pub type Comm = [u8; COMM_MAX_LEN];

pub struct Process {
    pid: i32,
    root: PathBuf,
}

impl Process {
    pub fn with_pid(pid: i32) -> Self {
        let pid_str = lexical::to_string(pid);
        let root = Path::new(LINUX_PROC_PATH).join(pid_str);
        Self { pid, root }
    }

    #[inline]
    pub fn pid(&self) -> i32 {
        self.pid
    }

    #[inline]
    pub fn comm(&self) -> ProcResult<Comm> {
        let path = self.root.join("comm");
        util::parse_comm(&path)
    }

    #[inline]
    pub fn cmdline(&self) -> ProcResult<String> {
        let path = self.root.join("cmdline");
        util::parse_cmdline(&path)
    }

    #[inline]
    pub fn io(&self) -> ProcResult<Io> {
        let path = self.root.join("io");
        FileRead::from_file(&path)
    }

    #[inline]
    pub fn tasks(&self) -> ProcResult<Vec<Self>> {
        let path = self.root.join("task");
        task::get_process_tasks(&path, Some(self.pid))
    }
}

#[inline]
pub fn get_all_processes() -> ProcResult<Vec<Process>> {
    let proc_dir = Path::new(LINUX_PROC_PATH);
    task::get_process_tasks(&proc_dir, None)
}
