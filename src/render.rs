use std::{collections::HashMap, path::Path};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose};
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html,
};
use serde::Serialize;

use crate::{app, assets, config::Config};

#[derive(Debug, Clone, Serialize)]
pub struct RenderedContent {
    pub body: String,
    pub toc: Vec<TocItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TocItem {
    pub id: String,
    pub title: String,
    pub level: u8,
}

pub fn render_document(file: &Path, config: &Config) -> Result<String> {
    let rendered = render_body(file, config)?;
    let display_name = app::display_name(file);
    let title = html_escape::encode_text(&display_name).to_string();
    let mermaid_js = assets::js_string_literal(assets::MERMAID_JS)?;
    let app_config = inline_json(&config.web_config())?;
    let initial_state = inline_json(&InitialState {
        title: display_name.clone(),
        toc: rendered.toc.clone(),
    })?;
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
  <div id="app-shell" class="app-shell">
    <aside id="toc-panel" class="toc-panel" tabindex="-1" aria-label="Table of contents">
      <div class="toc-header">
        <h2>Contents</h2>
      </div>
      <nav id="toc-nav" class="toc-nav" aria-label="Table of contents"></nav>
      <p id="toc-empty" class="toc-empty hidden">No headings in this document.</p>
    </aside>
    <main id="content" tabindex="-1">{body}</main>
  </div>
  <div id="search-bar" class="hud hidden">
    <span class="search-prefix">/</span>
    <input id="search-input" autocomplete="off" spellcheck="false" aria-label="Search text">
    <span id="search-status"></span>
  </div>
  <div id="help-overlay" class="help hidden">
    <div class="help-panel">
      <h2>Keybindings</h2>
      <dl id="help-list"></dl>
    </div>
  </div>
  <script>
    window.__MDVIEW_MERMAID_SOURCE = {mermaid_js};
    window.__MDGLANCE_CONFIG = {app_config};
    window.__MDGLANCE_INITIAL_STATE = {initial_state};
  </script>
  <script type="module">{app_js}</script>
</body>
</html>"#,
        app_js = assets::APP_JS,
        body = rendered.body,
        css = assets::STYLE_CSS
    ))
}

pub fn render_body(file: &Path, config: &Config) -> Result<RenderedContent> {
    let markdown = std::fs::read_to_string(file)
        .with_context(|| format!("failed to read {}", file.display()))?;
    let base_dir = file
        .parent()
        .context("cannot render a file without a parent directory")?;
    Ok(markdown_to_html(&markdown, base_dir, config.toc.max_depth))
}

fn markdown_to_html(markdown: &str, base_dir: &Path, max_toc_depth: u8) -> RenderedContent {
    let parser = Parser::new_ext(markdown, markdown_options());
    let mut events = Vec::new();
    let mut toc = Vec::new();
    let mut in_mermaid = false;
    let mut mermaid = String::new();
    let mut current_heading = None::<HeadingCapture>;
    let mut slug_counts = HashMap::new();

    for event in parser {
        if current_heading.is_some() {
            match event {
                Event::End(TagEnd::Heading(_level)) => {
                    let heading = current_heading.take().expect("heading state must exist");
                    let title = collapse_whitespace(&heading.text);
                    let slug_source = heading.original_id.as_deref().unwrap_or(&title);
                    let final_id = unique_heading_id(slug_source, &mut slug_counts);
                    let level_number = heading_level_number(heading.level);

                    if level_number <= max_toc_depth {
                        toc.push(TocItem {
                            id: final_id.clone(),
                            title: if title.is_empty() {
                                format!("Section {final_id}")
                            } else {
                                title
                            },
                            level: level_number,
                        });
                    }

                    events.push(Event::Start(Tag::Heading {
                        level: heading.level,
                        id: Some(CowStr::Boxed(final_id.into_boxed_str())),
                        classes: heading
                            .classes
                            .into_iter()
                            .map(|class| CowStr::Boxed(class.into_boxed_str()))
                            .collect(),
                        attrs: heading
                            .attrs
                            .into_iter()
                            .chain([
                                (
                                    CowStr::Borrowed("data-mdglance-heading"),
                                    Some(CowStr::Borrowed("true")),
                                ),
                                (
                                    CowStr::Borrowed("data-level"),
                                    Some(CowStr::Boxed(
                                        heading_level_number(heading.level)
                                            .to_string()
                                            .into_boxed_str(),
                                    )),
                                ),
                            ])
                            .collect(),
                    }));
                    events.extend(heading.events);
                    events.push(Event::End(TagEnd::Heading(heading.level)));
                }
                Event::Text(text) => {
                    let heading = current_heading.as_mut().expect("heading state must exist");
                    heading.text.push_str(&text);
                    heading.events.push(Event::Text(text.into_static()));
                }
                Event::Code(text) => {
                    let heading = current_heading.as_mut().expect("heading state must exist");
                    heading.text.push_str(&text);
                    heading.events.push(Event::Code(text.into_static()));
                }
                Event::SoftBreak | Event::HardBreak => {
                    let heading = current_heading.as_mut().expect("heading state must exist");
                    heading.text.push(' ');
                    heading.events.push(event.into_static());
                }
                other => {
                    let heading = current_heading.as_mut().expect("heading state must exist");
                    heading.events.push(other.into_static());
                }
            }
            continue;
        }

        match event {
            Event::Start(Tag::Heading {
                level,
                id,
                classes,
                attrs,
            }) => {
                current_heading = Some(HeadingCapture {
                    level,
                    original_id: id.map(CowStr::into_string),
                    classes: classes.into_iter().map(CowStr::into_string).collect(),
                    attrs: attrs
                        .into_iter()
                        .filter(|(name, _)| name.as_ref() != "data-mdglance-heading")
                        .map(|(name, value)| (name.into_static(), value.map(CowStr::into_static)))
                        .collect(),
                    events: Vec::new(),
                    text: String::new(),
                });
            }
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

    let mut body = String::new();
    html::push_html(&mut body, events.into_iter());
    RenderedContent { body, toc }
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

fn heading_level_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn unique_heading_id(source: &str, seen: &mut HashMap<String, usize>) -> String {
    let base = slugify(source);
    let count = seen.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base
    } else {
        format!("{base}-{}", *count)
    }
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            slug.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            pending_dash = true;
        }
    }

    if slug.is_empty() {
        String::from("section")
    } else {
        slug
    }
}

fn path_to_file_url(path: &Path) -> Result<String> {
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))?;
    let mut url = String::from("file://");
    url.push_str(&path.to_string_lossy().replace(' ', "%20"));
    Ok(url)
}

fn inline_json<T: Serialize>(value: &T) -> Result<String> {
    let json = serde_json::to_string(value)?;
    Ok(json.replace("</", "<\\/"))
}

#[derive(Serialize)]
struct InitialState {
    title: String,
    toc: Vec<TocItem>,
}

struct HeadingCapture {
    level: HeadingLevel,
    original_id: Option<String>,
    classes: Vec<String>,
    attrs: Vec<(CowStr<'static>, Option<CowStr<'static>>)>,
    events: Vec<Event<'static>>,
    text: String,
}
