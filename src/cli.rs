use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Open a native live preview for a Markdown or SVG file"
)]
pub struct Args {
    /// Spawn the viewer as a detached child process and return the shell prompt immediately.
    #[arg(long)]
    pub detach: bool,

    /// File to preview.
    pub file: Option<PathBuf>,

    /// Additional files in the viewer queue. Used internally for detached launches.
    #[arg(long, hide = true)]
    pub queue_file: Vec<PathBuf>,
}
