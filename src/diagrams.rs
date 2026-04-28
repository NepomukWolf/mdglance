use std::{
    collections::HashMap,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{LazyLock, Mutex},
};

use sha2::{Digest, Sha256};

pub enum DiagramRender {
    Html(String),
    Fallback,
}

pub fn render_diagram_html(language: Option<&str>, source: &str) -> Option<DiagramRender> {
    let language = first_language_token(language?)?;

    if MERMAID_RENDERER.supports_language(language) {
        return Some(MERMAID_RENDERER.render(source));
    }

    if is_plantuml_lang(language) {
        return Some(PLANTUML_RENDERER.render(source));
    }

    None
}

trait DiagramRenderer: Sync {
    fn supports_language(&self, language: &str) -> bool;
    fn render(&self, source: &str) -> DiagramRender;
}

struct MermaidRenderer;

static MERMAID_RENDERER: MermaidRenderer = MermaidRenderer;

impl DiagramRenderer for MermaidRenderer {
    fn supports_language(&self, language: &str) -> bool {
        first_language_token(language).is_some_and(|name| name.eq_ignore_ascii_case("mermaid"))
    }

    fn render(&self, source: &str) -> DiagramRender {
        let escaped = html_escape::encode_text(source);
        DiagramRender::Html(format!(r#"<pre class="mermaid">{escaped}</pre>"#))
    }
}

struct PlantUmlRenderer {
    command: Option<String>,
    cache_dir: Option<PathBuf>,
    memory_cache: Mutex<HashMap<String, String>>,
}

static PLANTUML_RENDERER: LazyLock<PlantUmlRenderer> = LazyLock::new(PlantUmlRenderer::new);

impl PlantUmlRenderer {
    fn new() -> Self {
        Self {
            command: detect_plantuml_command(),
            cache_dir: build_cache_dir(),
            memory_cache: Mutex::new(HashMap::new()),
        }
    }

    fn render_svg(&self, source: &str) -> Option<String> {
        let command = self.command.as_deref()?;
        let cache_key = hash_source(source);

        if let Some(svg) = self.read_memory_cache(&cache_key) {
            return Some(svg);
        }

        if let Some(svg) = self.read_disk_cache(&cache_key) {
            self.store_memory_cache(cache_key, svg.clone());
            return Some(svg);
        }

        let svg = run_plantuml(command, source)?;
        self.store_memory_cache(cache_key.clone(), svg.clone());
        self.write_disk_cache(&cache_key, &svg);
        Some(svg)
    }

    fn read_memory_cache(&self, cache_key: &str) -> Option<String> {
        self.memory_cache.lock().ok()?.get(cache_key).cloned()
    }

    fn store_memory_cache(&self, cache_key: String, svg: String) {
        if let Ok(mut cache) = self.memory_cache.lock() {
            cache.insert(cache_key, svg);
        }
    }

    fn read_disk_cache(&self, cache_key: &str) -> Option<String> {
        let path = self.cache_path(cache_key)?;
        std::fs::read_to_string(path).ok()
    }

    fn write_disk_cache(&self, cache_key: &str, svg: &str) {
        let Some(path) = self.cache_path(cache_key) else {
            return;
        };
        let _ = std::fs::write(path, svg);
    }

    fn cache_path(&self, cache_key: &str) -> Option<PathBuf> {
        Some(self.cache_dir.as_ref()?.join(format!("{cache_key}.svg")))
    }
}

impl DiagramRenderer for PlantUmlRenderer {
    fn supports_language(&self, language: &str) -> bool {
        is_plantuml_lang(language)
    }

    fn render(&self, source: &str) -> DiagramRender {
        match self.render_svg(source) {
            Some(svg) => DiagramRender::Html(format!(
                r#"<div class="diagram-block plantuml-diagram">{svg}</div>"#
            )),
            None => DiagramRender::Fallback,
        }
    }
}

fn first_language_token(language: &str) -> Option<&str> {
    language
        .split_whitespace()
        .next()
        .filter(|token| !token.is_empty())
}

fn is_plantuml_lang(language: &str) -> bool {
    language.eq_ignore_ascii_case("plantuml") || language.eq_ignore_ascii_case("puml")
}

fn detect_plantuml_command() -> Option<String> {
    let status = Command::new("plantuml")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;

    status.success().then(|| String::from("plantuml"))
}

fn build_cache_dir() -> Option<PathBuf> {
    let base = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
    let path = base.join("mdglance").join("plantuml");
    std::fs::create_dir_all(&path).ok()?;
    Some(path)
}

fn hash_source(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let digest = hasher.finalize();
    let mut hash = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hash, "{byte:02x}");
    }
    hash
}

fn run_plantuml(command: &str, source: &str) -> Option<String> {
    let mut process = Command::new(command);
    process
        .args(["--svg", "--pipe", "--no-error-image"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let output = process.output_with_stdin(source.as_bytes()).ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    normalize_svg(stdout)
}

fn normalize_svg(svg: String) -> Option<String> {
    let mut svg = svg.trim().to_owned();

    while svg.starts_with("<?xml") {
        let end = svg.find("?>")?;
        svg = svg[end + 2..].trim_start().to_owned();
    }

    if svg.starts_with("<!DOCTYPE") || svg.starts_with("<!doctype") {
        let end = svg.find('>')?;
        svg = svg[end + 1..].trim_start().to_owned();
    }

    svg.contains("<svg").then_some(svg)
}

trait CommandOutputExt {
    fn output_with_stdin(self, stdin: &[u8]) -> std::io::Result<std::process::Output>;
}

impl CommandOutputExt for Command {
    fn output_with_stdin(mut self, stdin: &[u8]) -> std::io::Result<std::process::Output> {
        use std::io::Write as _;

        let mut child = self.spawn()?;
        if let Some(mut child_stdin) = child.stdin.take() {
            child_stdin.write_all(stdin)?;
        }
        child.wait_with_output()
    }
}

#[cfg(test)]
mod tests {
    use super::{first_language_token, hash_source, normalize_svg};

    #[test]
    fn recognizes_first_info_token() {
        assert_eq!(
            first_language_token("plantuml title=demo"),
            Some("plantuml")
        );
        assert_eq!(first_language_token(""), None);
    }

    #[test]
    fn hashes_change_with_content() {
        assert_ne!(hash_source("alice"), hash_source("bob"));
    }

    #[test]
    fn strips_xml_preamble_from_svg() {
        let svg = String::from(r#"<?xml version="1.0"?><svg viewBox="0 0 10 10"></svg>"#);
        assert_eq!(
            normalize_svg(svg).as_deref(),
            Some(r#"<svg viewBox="0 0 10 10"></svg>"#)
        );
    }
}
