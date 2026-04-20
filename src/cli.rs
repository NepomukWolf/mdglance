use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(version, about = "Open a native live preview for a Markdown file")]
pub struct Args {
    /// Markdown file to preview.
    pub file: PathBuf,
}
