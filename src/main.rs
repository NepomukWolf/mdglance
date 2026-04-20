mod app;
mod assets;
mod cli;
mod render;
mod watcher;

use anyhow::Result;
use clap::Parser as _;

fn main() -> Result<()> {
    let args = cli::Args::parse();
    app::run(args.file)
}
