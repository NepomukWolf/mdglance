mod app;
mod assets;
mod cli;
mod config;
mod render;
mod watcher;

use anyhow::Result;
use clap::Parser as _;
use std::process::{Command, Stdio};

fn main() -> Result<()> {
    let args = cli::Args::parse();
    if args.detach {
        let file = args.file.canonicalize().unwrap_or(args.file.clone());
        let child = Command::new(std::env::current_exe()?)
            .arg(file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let _ = child.id();
        return Ok(());
    }

    app::run(args.file)
}
