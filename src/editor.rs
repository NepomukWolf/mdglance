use std::{
    path::Path,
    process::{Child, Command},
};

use anyhow::{Context, Result};

fn parse_editor_command(command: &str) -> (String, Vec<String>) {
    let mut parts = command.split_whitespace().map(str::to_string);
    let program = parts.next().unwrap_or_else(|| "vi".to_string());
    (program, parts.collect())
}

fn resolve_editor_command() -> (String, Vec<String>) {
    let command = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    parse_editor_command(&command)
}

/// Spawns the editor without blocking. Callers must not wait on it from the
/// main/UI thread: doing so freezes AppKit's run loop, which prevents any
/// window-visibility or focus change from actually being applied until the
/// child exits. Wait on the returned child from a background thread instead.
pub fn spawn_editor(file: &Path) -> Result<Child> {
    let (program, args) = resolve_editor_command();
    Command::new(&program)
        .args(&args)
        .arg(file)
        .spawn()
        .with_context(|| format!("failed to launch `{program}`"))
}

#[cfg(test)]
mod tests {
    use super::parse_editor_command;

    #[test]
    fn splits_program_and_arguments() {
        assert_eq!(
            parse_editor_command("code -w"),
            ("code".to_string(), vec!["-w".to_string()])
        );
    }

    #[test]
    fn handles_a_bare_program_name() {
        assert_eq!(parse_editor_command("hx"), ("hx".to_string(), Vec::new()));
    }
}
