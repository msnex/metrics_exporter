use crate::ProcResult;
use crate::process::Process;
use std::fs::read_dir;
use std::path::Path;

pub fn get_process_tasks(root: &Path, ppid: Option<i32>) -> ProcResult<Vec<Process>> {
    let mut processes = Vec::new();

    for entry in read_dir(&root)? {
        if entry.is_err() {
            continue;
        }
        let dir_entry = entry.unwrap();

        let dir_path = dir_entry.path();
        if !dir_path.is_dir() {
            continue;
        }
        let dir_name = dir_entry.file_name();

        if let Ok(pid) = dir_name.to_string_lossy().parse::<i32>() {
            if let Some(id) = ppid {
                if pid == id {
                    continue;
                }
            }
            processes.push(Process::with_pid(pid));
        }
    }

    Ok(processes)
}
