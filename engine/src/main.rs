use std::path::PathBuf;

use clap::Parser;
use engine::config::Config;

#[derive(Parser)]
#[command(version, about = "Streaming engine for Streamline")]
struct Cli {
    #[arg(long)]
    config: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Config::load(&cli.config)?;
    config.secret_bytes()?;
    Ok(())
}
