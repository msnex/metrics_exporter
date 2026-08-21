use crate::exporter::MetricsExporter;
use anyhow::{Result, bail};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tracing::{debug, info};

mod affinity;
mod args;
mod collector;
mod config;
mod exporter;
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

fn init_async_runtime() -> Result<tokio::runtime::Runtime> {
    let cpu_cores = crate::affinity::get_cpu_cores();
    let worker_threads = config::config().runtime.worker_threads.min(cpu_cores);

    if worker_threads == 0 {
        bail!("async runtime worker_threads must be greater than 0");
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .enable_all()
        .build()?;

    info!("async runtime worker threads: {}", worker_threads);

    Ok(rt)
}

fn main() -> Result<()> {
    let config = config::config();

    log::init_log(&config.log)?;
    debug!("config: {:?}", config);

    init_ctrlc()?;
    let async_rt = init_async_runtime()?;

    let exporter = MetricsExporter::new(&config.otlp, &async_rt)?;
    let meter = exporter.meter();
    let mut registry = collector::register_collectors(&config.metrics, &meter)?;

    while RUNNING.get().unwrap().load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    registry.shutdown();
    exporter.shutdown();
    async_rt.shutdown_timeout(Duration::from_secs(5));

    Ok(())
}
