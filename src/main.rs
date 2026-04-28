mod app;
mod assets;
mod cli;
mod config;
mod diagrams;
mod render;
mod watcher;

use anyhow::{Context, Result, bail};
use clap::Parser as _;
use std::{
    io::{self, IsTerminal as _, Read as _},
    path::PathBuf,
    process::{Command, Stdio},
};

fn main() -> Result<()> {
    let args = cli::Args::parse();
    let cli::Args {
        detach,
        file,
        queue_file,
    } = args;
    let launch = resolve_launch_input(file, queue_file)?;

    if detach {
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg(&launch.file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for file in &launch.queue {
            command.arg("--queue-file").arg(file);
        }
        let child = command.spawn()?;
        let _ = child.id();
        return Ok(());
    }

    app::run(launch.file, launch.queue)
}

struct LaunchInput {
    file: PathBuf,
    queue: Vec<PathBuf>,
}

fn resolve_launch_input(file: Option<PathBuf>, queue: Vec<PathBuf>) -> Result<LaunchInput> {
    if let Some(file) = file {
        let file = canonicalize_existing_file(&file)?;
        let mut queue = queue
            .into_iter()
            .map(|path| canonicalize_existing_file(&path))
            .collect::<Result<Vec<_>>>()?;
        queue.retain(|candidate| candidate != &file);
        return Ok(LaunchInput { file, queue });
    }

    if !io::stdin().is_terminal() {
        return read_launch_input_from_stdin();
    }

    bail!("no input file provided")
}

fn read_launch_input_from_stdin() -> Result<LaunchInput> {
    let mut stdin = String::new();
    io::stdin()
        .read_to_string(&mut stdin)
        .context("failed to read file list from stdin")?;

    let mut files = stdin
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .map(|path| canonicalize_existing_file(&path))
        .collect::<Result<Vec<_>>>()?;

    let Some(file) = files.first().cloned() else {
        bail!("stdin did not contain any file paths");
    };

    files.remove(0);
    files.retain(|candidate| candidate != &file);
    Ok(LaunchInput { file, queue: files })
}

fn canonicalize_existing_file(path: &PathBuf) -> Result<PathBuf> {
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))?;

    if !path.is_file() {
        bail!("{} is not a file", path.display());
    }

    Ok(path)
}
