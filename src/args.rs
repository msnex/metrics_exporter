use clap::Parser;

static ARGS: std::sync::OnceLock<Args> = std::sync::OnceLock::new();

#[derive(Debug, clap::Parser)]
#[command(version, about = "Metrics exporter", long_about = None)]
pub struct Args {
    #[arg(short, long, help = "Config file")]
    pub config: String,
}

pub fn args() -> &'static Args {
    ARGS.get_or_init(Args::parse)
}
