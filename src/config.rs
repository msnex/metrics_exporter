use crate::args;
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OtlpProtocol {
    Grpc,
    #[serde(alias = "http/protobuf")]
    Http,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeCfg {
    pub worker_threads: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtlpCfg {
    pub protocol: OtlpProtocol,
    #[serde(default = "default_otlp_timeout")]
    pub timeout: u64,
    #[serde(default = "default_otlp_interval_secs")]
    pub interval_secs: u64,
    pub endpoint: String,
    pub headers: Option<HashMap<String, String>>,
}

fn default_otlp_timeout() -> u64 {
    5
}

fn default_otlp_interval_secs() -> u64 {
    1
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

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsHostProcessCfg {
    pub enable: bool,
    pub thread: bool,
    #[serde(default)]
    pub comms: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsHostNetCfg {
    pub enable: bool,
    #[serde(default)]
    pub ifaces: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsHostCpuCfg {
    pub enable: bool,
    #[serde(default)]
    pub per_core: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsHostDiskCfg {
    pub enable: bool,
    /// Explicit device allow-list; empty means whole disks only.
    #[serde(default)]
    pub exclude_devices: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsHostCfg {
    pub enable: bool,
    #[serde(default = "default_interval_secs")]
    pub interval_secs: u64,
    pub cpu: Option<MetricsHostCpuCfg>,
    pub disk: Option<MetricsHostDiskCfg>,
    pub process: Option<MetricsHostProcessCfg>,
    pub net: Option<MetricsHostNetCfg>,
}

fn default_interval_secs() -> u64 {
    1
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsCfg {
    pub host: Option<MetricsHostCfg>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub runtime: RuntimeCfg,
    pub otlp: OtlpCfg,
    pub log: LogCfg,
    pub metrics: MetricsCfg,
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
            panic!("Failed to parse config: {}", err);
        }
        cfg.unwrap()
    })
}
