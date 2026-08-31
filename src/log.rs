use crate::config::LogCfg;
use anyhow::{Result, bail};
use time::macros::format_description;
use tracing::level_filters::LevelFilter;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt::time::LocalTime};

fn log_level(level: &str) -> LevelFilter {
    match level {
        "trace" => LevelFilter::TRACE,
        "debug" => LevelFilter::DEBUG,
        "info" => LevelFilter::INFO,
        "warn" => LevelFilter::WARN,
        "error" => LevelFilter::ERROR,
        "off" => LevelFilter::OFF,
        _ => LevelFilter::OFF,
    }
}

fn log_file_rotation(rotation: &str) -> Rotation {
    match rotation {
        "minutely" => Rotation::MINUTELY,
        "hourly" => Rotation::HOURLY,
        "daily" => Rotation::DAILY,
        "weekly" => Rotation::WEEKLY,
        "never" => Rotation::NEVER,
        _ => Rotation::NEVER,
    }
}

pub fn init_log(cfg: &LogCfg) -> Result<()> {
    let level = log_level(&cfg.level);
    let subscriber = tracing_subscriber::fmt()
        .with_level(true)
        .with_timer(LocalTime::new(format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond]"
        )))
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(level.into())
                .with_regex(false)
                .from_env_lossy(),
        );

    if cfg.writer == "stdout" {
        subscriber.with_writer(std::io::stdout).init();
    } else {
        if let Some(file_cfg) = cfg.file.as_ref() {
            let max_log_files = if file_cfg.max_log_files <= 1 {
                2
            } else {
                file_cfg.max_log_files + 1
            };

            let file_appender = RollingFileAppender::builder()
                .rotation(log_file_rotation(&file_cfg.rotation))
                .max_log_files(max_log_files)
                .filename_prefix(&file_cfg.filename)
                .build(&file_cfg.directory)?;

            subscriber.with_writer(file_appender).init();
        } else {
            bail!("log file writer requires log.file configure.");
        }
    }

    Ok(())
}
