use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

use anyhow::{Context, Result};
use clap::Parser as ClapParser;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd, html};
use tao::{
    dpi::LogicalSize,
    event::{Event as TaoEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

#[derive(Debug, ClapParser)]
#[command(version, about = "Open a native live preview for a Markdown file")]
struct Args {
    /// Markdown file to preview.
    file: PathBuf,
}

#[derive(Debug, Clone)]
enum UserEvent {
    Reload,
    Close,
    WatchError(String),
}

fn main() -> Result<()> {
    let args = Args::parse();
    let file = args
        .file
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", args.file.display()))?;

    if !file.is_file() {
        anyhow::bail!("{} is not a file", file.display());
    }

    run(file)
}

fn run(file: PathBuf) -> Result<()> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let _watcher = watch_file(file.clone(), proxy)?;

    let title = format!("mdview - {}", display_name(&file));
    let window = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(LogicalSize::new(1080.0, 860.0))
        .build(&event_loop)
        .context("failed to create window")?;

    let html = render_document(&file)?;
    let webview = WebViewBuilder::new()
        .with_html(html)
        .with_ipc_handler({
            let proxy = event_loop.create_proxy();
            move |message| {
                if message.body() == "close" {
                    let _ = proxy.send_event(UserEvent::Close);
                }
            }
        })
        .build(&window)
        .context("failed to create webview")?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            TaoEvent::UserEvent(UserEvent::Reload) => match render_body(&file) {
                Ok(body) => {
                    let payload = serde_json::json!({
                        "title": display_name(&file),
                        "body": body,
                    });
                    let script = format!("window.__mdviewUpdate({payload});");
                    if let Err(err) = webview.evaluate_script(&script) {
                        eprintln!("failed to update preview: {err}");
                    }
                }
                Err(err) => {
                    let message = err.to_string();
                    let escaped = html_escape::encode_text(&message);
                    let script = format!(
                        "window.__mdviewShowError({});",
                        serde_json::to_string(&escaped.to_string()).unwrap_or_default()
                    );
                    let _ = webview.evaluate_script(&script);
                }
            },
            TaoEvent::UserEvent(UserEvent::WatchError(message)) => {
                eprintln!("watch error: {message}");
            }
            TaoEvent::UserEvent(UserEvent::Close) => {
                *control_flow = ControlFlow::Exit;
            }
            TaoEvent::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });

    #[allow(unreachable_code)]
    Ok(())
}

fn watch_file(file: PathBuf, proxy: EventLoopProxy<UserEvent>) -> Result<RecommendedWatcher> {
    let watch_dir = file
        .parent()
        .context("cannot watch a file without a parent directory")?
        .to_path_buf();
    let file_name = file.file_name().map(|name| name.to_owned());
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .context("failed to create file watcher")?;

    watcher
        .watch(&watch_dir, RecursiveMode::NonRecursive)
        .with_context(|| format!("failed to watch {}", watch_dir.display()))?;

    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            match event {
                Ok(event) => {
                    let touches_target = event.paths.is_empty()
                        || event.paths.iter().any(|path| {
                            path == &file
                                || path.file_name().is_some()
                                    && file_name.as_deref() == path.file_name()
                        });

                    if touches_target {
                        let _ = proxy.send_event(UserEvent::Reload);
                    }
                }
                Err(err) => {
                    let _ = proxy.send_event(UserEvent::WatchError(err.to_string()));
                }
            }
        }
    });

    Ok(watcher)
}

fn render_document(file: &Path) -> Result<String> {
    let body = render_body(file)?;
    let display_name = display_name(file);
    let title = html_escape::encode_text(&display_name);
    let base = file
        .parent()
        .map(path_to_file_url)
        .transpose()?
        .unwrap_or_default();

    Ok(format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <base href="{base}/">
  <title>{title}</title>
  <style>{css}</style>
</head>
<body>
  <main id="content">{body}</main>
  <script type="module">
    import mermaid from "https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs";

    mermaid.initialize({{ startOnLoad: false, securityLevel: "loose" }});

    async function renderMermaid() {{
      const nodes = document.querySelectorAll("pre.mermaid");
      for (const node of nodes) {{
        node.removeAttribute("data-processed");
      }}
      await mermaid.run({{ nodes }});
    }}

    window.__mdviewUpdate = async function(payload) {{
      const scrollRatio = window.scrollY / Math.max(1, document.body.scrollHeight - window.innerHeight);
      document.title = "mdview - " + payload.title;
      document.getElementById("content").innerHTML = payload.body;
      await renderMermaid();
      window.scrollTo(0, scrollRatio * Math.max(1, document.body.scrollHeight - window.innerHeight));
    }};

    window.__mdviewShowError = function(message) {{
      document.getElementById("content").innerHTML = `<pre class="error">${{message}}</pre>`;
    }};

    function isTypingTarget(element) {{
      return element && (
        element.isContentEditable ||
        ["INPUT", "TEXTAREA", "SELECT"].includes(element.tagName)
      );
    }}

    window.addEventListener("keydown", event => {{
      if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey || isTypingTarget(event.target)) {{
        return;
      }}

      const line = Math.max(48, Math.round(window.innerHeight * 0.08));
      const page = Math.max(120, Math.round(window.innerHeight * 0.82));

      switch (event.key) {{
        case "j":
          event.preventDefault();
          window.scrollBy({{ top: line, behavior: "instant" }});
          break;
        case "k":
          event.preventDefault();
          window.scrollBy({{ top: -line, behavior: "instant" }});
          break;
        case "h":
          event.preventDefault();
          window.scrollBy({{ left: -line, behavior: "instant" }});
          break;
        case "l":
          event.preventDefault();
          window.scrollBy({{ left: line, behavior: "instant" }});
          break;
        case "d":
          event.preventDefault();
          window.scrollBy({{ top: page / 2, behavior: "instant" }});
          break;
        case "u":
          event.preventDefault();
          window.scrollBy({{ top: -page / 2, behavior: "instant" }});
          break;
        case " ":
          event.preventDefault();
          window.scrollBy({{ top: page, behavior: "instant" }});
          break;
        case "g":
          event.preventDefault();
          window.scrollTo({{ top: 0, behavior: "instant" }});
          break;
        case "G":
          event.preventDefault();
          window.scrollTo({{ top: document.body.scrollHeight, behavior: "instant" }});
          break;
        case "q":
          event.preventDefault();
          window.ipc.postMessage("close");
          break;
      }}
    }});

    renderMermaid();
  </script>
</body>
</html>"#,
        css = CSS
    ))
}

fn render_body(file: &Path) -> Result<String> {
    let markdown = std::fs::read_to_string(file)
        .with_context(|| format!("failed to read {}", file.display()))?;
    Ok(markdown_to_html(&markdown))
}

fn markdown_to_html(markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, markdown_options());
    let mut events = Vec::new();
    let mut in_mermaid = false;
    let mut mermaid = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) if is_mermaid_lang(&lang) => {
                in_mermaid = true;
                mermaid.clear();
            }
            Event::End(TagEnd::CodeBlock) if in_mermaid => {
                let escaped = html_escape::encode_text(&mermaid);
                events.push(Event::Html(CowStr::Boxed(
                    format!(r#"<pre class="mermaid">{escaped}</pre>"#).into_boxed_str(),
                )));
                in_mermaid = false;
            }
            Event::Text(text) if in_mermaid => {
                mermaid.push_str(&text);
            }
            Event::Code(text) if in_mermaid => {
                mermaid.push_str(&text);
            }
            other if !in_mermaid => {
                events.push(other.into_static());
            }
            _ => {}
        }
    }

    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    out
}

fn markdown_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
}

fn is_mermaid_lang(lang: &str) -> bool {
    lang.split_whitespace()
        .next()
        .is_some_and(|name| name.eq_ignore_ascii_case("mermaid"))
}

fn display_name(file: &Path) -> String {
    file.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Markdown")
        .to_string()
}

fn path_to_file_url(path: &Path) -> Result<String> {
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))?;
    let mut url = String::from("file://");
    url.push_str(&path.to_string_lossy().replace(' ', "%20"));
    Ok(url)
}

const CSS: &str = r#"
:root {
  color-scheme: light dark;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  line-height: 1.55;
  background: Canvas;
  color: CanvasText;
}

body {
  margin: 0;
}

main {
  max-width: 900px;
  margin: 0 auto;
  padding: 32px 24px 56px;
}

h1, h2, h3, h4, h5, h6 {
  line-height: 1.2;
  margin: 1.6em 0 0.5em;
}

h1 {
  border-bottom: 1px solid color-mix(in srgb, CanvasText 20%, transparent);
  padding-bottom: 0.25em;
}

a {
  color: LinkText;
}

pre {
  overflow: auto;
  padding: 14px;
  border-radius: 8px;
  background: color-mix(in srgb, CanvasText 8%, Canvas);
}

code {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 0.92em;
}

:not(pre) > code {
  padding: 0.15em 0.35em;
  border-radius: 5px;
  background: color-mix(in srgb, CanvasText 8%, Canvas);
}

blockquote {
  margin-left: 0;
  padding-left: 1em;
  border-left: 4px solid color-mix(in srgb, CanvasText 22%, transparent);
  color: color-mix(in srgb, CanvasText 74%, Canvas);
}

table {
  border-collapse: collapse;
  width: 100%;
  display: block;
  overflow-x: auto;
}

th, td {
  border: 1px solid color-mix(in srgb, CanvasText 18%, transparent);
  padding: 6px 10px;
}

img, svg {
  max-width: 100%;
}

.mermaid {
  text-align: center;
  background: transparent;
}

.error {
  color: #b00020;
  white-space: pre-wrap;
}
"#;
