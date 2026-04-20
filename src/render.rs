use std::path::Path;

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd, html};

use crate::{app, assets};

pub fn render_document(file: &Path) -> Result<String> {
    let body = render_body(file)?;
    let display_name = app::display_name(file);
    let title = html_escape::encode_text(&display_name);
    let mermaid_js = assets::js_string_literal(assets::MERMAID_JS)?;
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
  <div id="search-bar" class="hud hidden">
    <span class="search-prefix">/</span>
    <input id="search-input" autocomplete="off" spellcheck="false" aria-label="Search text">
    <span id="search-status"></span>
  </div>
  <div id="help-overlay" class="help hidden">
    <div class="help-panel">
      <h2>Keybindings</h2>
      <dl>
        <dt>j / k</dt><dd>Scroll down / up</dd>
        <dt>h / l</dt><dd>Scroll left / right</dd>
        <dt>d / u</dt><dd>Half page down / up</dd>
        <dt>Space</dt><dd>Page down</dd>
        <dt>g / G</dt><dd>Top / bottom</dd>
        <dt>/</dt><dd>Search</dd>
        <dt>Enter</dt><dd>Accept search</dd>
        <dt>n / N</dt><dd>Next / previous search hit</dd>
        <dt>?</dt><dd>Show this help</dd>
        <dt>Esc</dt><dd>Close search or help</dd>
        <dt>q</dt><dd>Quit</dd>
      </dl>
    </div>
  </div>
  <script>
    window.__MDVIEW_MERMAID_SOURCE = {mermaid_js};
  </script>
  <script type="module">{app_js}</script>
</body>
</html>"#,
        app_js = assets::APP_JS,
        css = assets::STYLE_CSS
    ))
}

pub fn render_body(file: &Path) -> Result<String> {
    let markdown = std::fs::read_to_string(file)
        .with_context(|| format!("failed to read {}", file.display()))?;
    let base_dir = file
        .parent()
        .context("cannot render a file without a parent directory")?;
    Ok(markdown_to_html(&markdown, base_dir))
}

fn markdown_to_html(markdown: &str, base_dir: &Path) -> String {
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
                events.push(rewrite_local_image(other, base_dir).into_static());
            }
            _ => {}
        }
    }

    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    out
}

fn rewrite_local_image<'a>(event: Event<'a>, base_dir: &Path) -> Event<'a> {
    let Event::Start(Tag::Image {
        link_type,
        dest_url,
        title,
        id,
    }) = event
    else {
        return event;
    };

    if is_external_url(&dest_url) || dest_url.starts_with("data:") {
        return Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        });
    }

    let image_path = base_dir.join(dest_url.as_ref());
    let Some(data_url) = image_data_url(&image_path) else {
        return Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        });
    };

    Event::Start(Tag::Image {
        link_type,
        dest_url: CowStr::Boxed(data_url.into_boxed_str()),
        title,
        id,
    })
}

fn is_external_url(url: &str) -> bool {
    url.contains("://") || url.starts_with("//") || url.starts_with('#')
}

fn image_data_url(path: &Path) -> Option<String> {
    let mime = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("bmp") => "image/bmp",
        _ => return None,
    };

    let bytes = std::fs::read(path).ok()?;
    Some(format!(
        "data:{mime};base64,{}",
        general_purpose::STANDARD.encode(bytes)
    ))
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

fn path_to_file_url(path: &Path) -> Result<String> {
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))?;
    let mut url = String::from("file://");
    url.push_str(&path.to_string_lossy().replace(' ', "%20"));
    Ok(url)
}
