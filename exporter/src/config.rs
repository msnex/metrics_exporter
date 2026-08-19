use crate::args;
use anyhow::Result;
use serde::Deserialize;
use std::sync::OnceLock;

static CONFIG: OnceLock<Config> = OnceLock::new();

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct OTLPCfg {
    pub protocol: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogFileCfg {
    pub rotation: String,
    pub max_log_files: usize,
    pub directory: String,
    pub filename: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogCfg {
    pub writer: String,
    pub level: String,
    pub file: Option<LogFileCfg>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub otlp: OTLPCfg,
    pub log: LogCfg,
}

impl Config {
    pub fn new(path: &str) -> Result<Self> {
        let cfg = config::Config::builder()
            .add_source(config::File::with_name(path).required(true))
            .add_source(
                config::Environment::with_prefix("EXPORTER")
                    .prefix_separator("_")
                    .separator("__"),
            )
            .build()?;
        Ok(cfg.try_deserialize()?)
    }
}

#[inline(always)]
pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| {
        let cfg = Config::new(&args::args().config);
        if let Err(err) = cfg {
            panic!("Failed to parse config: {}", err.to_string());
        }
        cfg.unwrap()
    })
}
