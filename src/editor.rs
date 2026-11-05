use std::path::Path;

use anyhow::Result;

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

#[cfg(unix)]
pub fn open_in_editor(file: &Path) -> Result<()> {
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    let (program, args) = resolve_editor_command();
    let err = Command::new(&program).args(&args).arg(file).exec();
    Err(anyhow::Error::new(err).context(format!("failed to launch `{program}`")))
}

#[cfg(not(unix))]
pub fn open_in_editor(_file: &Path) -> Result<()> {
    anyhow::bail!("opening files in an external editor isn't supported on this platform yet")
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
