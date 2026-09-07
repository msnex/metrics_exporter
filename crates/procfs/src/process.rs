use crate::LINUX_PROC_PATH;
use crate::ProcResult;
use crate::traits::FileRead;
use std::path::Path;
use std::path::PathBuf;

mod io;
mod stat;
mod status;
mod task;
mod util;

pub use io::Io;
pub use stat::Stat;
pub use status::Status;

pub struct Process {
    pid: i32,
    root: PathBuf,
}

impl Process {
    pub fn with_pid(pid: i32) -> Self {
        let mut buf = core::fmt::NumBuffer::new();
        let pid_str = pid.format_into(&mut buf);
        let root = Path::new(LINUX_PROC_PATH).join(pid_str);
        Self { pid, root }
    }

    #[inline]
    pub fn pid(&self) -> i32 {
        self.pid
    }

    #[inline]
    pub fn comm(&self) -> ProcResult<String> {
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
    pub fn status(&self) -> ProcResult<Status> {
        let path = self.root.join("status");
        FileRead::from_file(&path)
    }

    /// Read the CPU time fields from `/proc/<pid>/stat`.
    ///
    /// One file read yields the task's user and system scheduling time in
    /// kernel USER_HZ ticks. Process and thread rows share the parser: for
    /// a `Process` rooted under `/proc/<pid>/task/`, this reads the
    /// thread's own `stat` file.
    #[inline]
    pub fn stat(&self) -> ProcResult<Stat> {
        let path = self.root.join("stat");
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
    task::get_process_tasks(proc_dir, None)
}
