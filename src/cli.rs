use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(version, about = "Open a native live preview for a Markdown file")]
pub struct Args {
    /// Spawn the viewer as a detached child process and return the shell prompt immediately.
    #[arg(long)]
    pub detach: bool,

    /// Markdown file to preview.
    pub file: PathBuf,
}
