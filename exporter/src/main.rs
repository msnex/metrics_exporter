use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info};

mod args;
mod config;
mod log;

pub static RUNNING: std::sync::OnceLock<AtomicBool> = std::sync::OnceLock::new();

fn init_ctrlc() -> Result<()> {
    let _ = RUNNING.set(AtomicBool::new(true));

    ctrlc::set_handler(move || {
        info!("Received termination signal to exit");
        if let Some(r) = RUNNING.get() {
            r.store(false, Ordering::SeqCst);
        } else {
            let _ = RUNNING.set(AtomicBool::new(false));
        }
    })?;

    Ok(())
}

fn main() -> Result<()> {
    let args = args::args();
    let config = config::config();

    log::init_log(&config.log)?;
    debug!("args: {:?}, config: {:?}", args, config);

    init_ctrlc()?;

    let processes = procfs::process::get_all_processes()?;
    for process in processes {
        println!(
            "pid: {} comm: {}, cmdline: {}",
            process.pid(),
            str::from_utf8(&process.comm()?)
                .unwrap_or("")
                .trim_end_matches("\0"),
            process.cmdline()?
        );

        for child in process.tasks()? {
            println!(
                "\tpid: {} comm: {}",
                child.pid(),
                str::from_utf8(&child.comm()?)
                    .unwrap_or("")
                    .trim_end_matches("\0")
            );
        }
    }

    while RUNNING.get().unwrap().load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    Ok(())
}
