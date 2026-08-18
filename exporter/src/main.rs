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

    while RUNNING.get().unwrap().load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    Ok(())
}
