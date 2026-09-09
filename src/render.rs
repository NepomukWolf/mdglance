use std::{collections::HashMap, path::Path, sync::LazyLock};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose};
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html,
};
use serde::Serialize;
use syntect::{html::ClassedHTMLGenerator, parsing::SyntaxSet, util::LinesWithEndings};

use crate::{
    app, assets,
    config::Config,
    diagrams::{self, DiagramRender},
    theme,
    trust::TrustState,
};

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

#[derive(Debug, Clone, Serialize)]
pub struct RenderedContent {
    pub body: String,
    pub toc: Vec<TocItem>,
    pub document_kind: DocumentKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct TocItem {
    pub id: String,
    pub title: String,
    pub level: u8,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    Markdown,
    Svg,
}

pub fn render_document(
    file: &Path,
    workspace: &Path,
    trust_state: TrustState,
    config: &Config,
) -> Result<String> {
    let rendered = render_body(file, workspace, trust_state, config)?;
    let display_name = app::display_name(file);
    let title = html_escape::encode_text(&display_name).to_string();
    let trusted = trust_state == TrustState::Trusted;
    let mermaid_js = trusted
        .then(|| assets::js_string_literal(assets::MERMAID_JS))
        .transpose()?
        .unwrap_or_else(|| "null".to_string());
    let theme_css = config.theme.css()?;
    let app_config = inline_json(&config.web_config())?;
    let initial_state = inline_json(&InitialState {
        title: display_name.clone(),
        toc: rendered.toc.clone(),
        document_kind: rendered.document_kind,
        trust_state,
        workspace: workspace.display().to_string(),
    })?;
    let base = if trusted {
        file.parent()
            .map(path_to_file_url)
            .transpose()?
            .unwrap_or_default()
    } else {
        String::new()
    };
    let csp = if trusted {
        String::new()
    } else {
        r#"<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'; script-src 'unsafe-inline'">"#.to_string()
    };

    Ok(format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  {csp}
  <base href="{base}/">
  <title>{title}</title>
  <style>{css}</style>
  <style id="theme-style">{theme_css}</style>
</head>
<body>
  <button id="trust-indicator" class="trust-indicator hidden" type="button"></button>
  <div id="app-shell" class="app-shell">
    <aside id="toc-panel" class="toc-panel" tabindex="-1" aria-label="Table of contents">
      <div class="toc-header">
        <h2>Contents</h2>
        <kbd id="toc-toggle-hint" class="toc-toggle-hint hidden" aria-label="Table of contents toggle shortcut"></kbd>
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
  <div id="trust-overlay" class="trust-overlay hidden" role="dialog" aria-modal="true" aria-labelledby="trust-title">
    <div class="trust-dialog">
      <h2 id="trust-title">Do you trust this folder?</h2>
      <p>Trusted documents may run embedded HTML and diagrams, load remote images, and use project configuration.</p>
      <code id="trust-workspace"></code>
      <p id="trust-error" class="trust-error hidden" role="alert"></p>
      <div class="trust-actions">
        <button id="trust-folder" type="button">Trust Folder</button>
        <button id="open-restricted" type="button">Open Restricted</button>
        <button id="close-trust" type="button">Close</button>
      </div>
    </div>
  </div>
  <div id="theme-overlay" class="theme-overlay hidden" role="dialog" aria-modal="true" aria-labelledby="theme-title">
    <div class="theme-dialog">
      <h2 id="theme-title">Choose theme</h2>
      <div id="theme-list" class="theme-list" role="listbox" tabindex="0"></div>
      <p id="theme-error" class="theme-error hidden" role="alert"></p>
      <div class="theme-actions">
        <button id="apply-theme" type="button">Apply</button>
        <button id="cancel-theme" type="button">Cancel</button>
      </div>
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
        css = assets::STYLE_CSS,
        theme_css = theme_css
    ))
}

pub fn render_body(
    file: &Path,
    workspace: &Path,
    trust_state: TrustState,
    config: &Config,
) -> Result<RenderedContent> {
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("failed to read {}", file.display()))?;
    if is_svg_file(file) {
        return render_svg(&source, trust_state == TrustState::Trusted);
    }

    let base_dir = file
        .parent()
        .context("cannot render a file without a parent directory")?;
    Ok(markdown_to_html(
        &source,
        base_dir,
        workspace,
        trust_state == TrustState::Trusted,
        config,
    ))
}

fn markdown_to_html(
    markdown: &str,
    base_dir: &Path,
    workspace: &Path,
    trusted: bool,
    config: &Config,
) -> RenderedContent {
    let parser = Parser::new_ext(markdown, markdown_options());
    let mut events = Vec::new();
    let mut toc = Vec::new();
    let mut current_block = None::<CodeBlockCapture>;
    let mut current_heading = None::<HeadingCapture>;
    let mut slug_counts = HashMap::new();

    for event in parser {
        if let Some(block) = current_block.as_mut() {
            match event {
                Event::End(TagEnd::CodeBlock) => {
                    let block = current_block.take().expect("code block state must exist");
                    events.push(Event::Html(CowStr::Boxed(
                        render_code_block_html(block, trusted, config).into_boxed_str(),
                    )));
                }
                Event::Text(text) | Event::Code(text) => {
                    block.text.push_str(&text);
                }
                Event::SoftBreak | Event::HardBreak => {
                    block.text.push('\n');
                }
                _ => {}
            }
            continue;
        }

        if current_heading.is_some() {
            match event {
                Event::End(TagEnd::Heading(_level)) => {
                    let heading = current_heading.take().expect("heading state must exist");
                    let title = collapse_whitespace(&heading.text);
                    let slug_source = heading.original_id.as_deref().unwrap_or(&title);
                    let final_id = unique_heading_id(slug_source, &mut slug_counts);
                    let level_number = heading_level_number(heading.level);

                    if level_number <= config.toc.max_depth {
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
                    heading
                        .events
                        .push(restrict_author_html(other, trusted).into_static());
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
            Event::Start(Tag::CodeBlock(kind)) => {
                current_block = Some(CodeBlockCapture::new(kind));
            }
            other => {
                let event = restrict_author_html(other, trusted);
                events.push(rewrite_local_image(event, base_dir, workspace, trusted).into_static());
            }
        }
    }

    let mut body = String::new();
    html::push_html(&mut body, events.into_iter());
    RenderedContent {
        body,
        toc,
        document_kind: DocumentKind::Markdown,
    }
}

fn render_svg(source: &str, trusted: bool) -> Result<RenderedContent> {
    if !trusted {
        let source = html_escape::encode_text(source);
        return Ok(RenderedContent {
            body: format!(
                r#"<div class="restricted-document"><p>SVG preview is disabled in restricted mode.</p><pre class="code-block"><code>{source}</code></pre></div>"#
            ),
            toc: Vec::new(),
            document_kind: DocumentKind::Markdown,
        });
    }
    let svg = normalize_svg_document(source).context("failed to normalize SVG document")?;
    let body = format!(
        r#"<div class="svg-shell"><div id="svg-viewport" class="svg-viewport"><div id="svg-stage" class="svg-stage">{svg}</div></div></div>"#
    );

    Ok(RenderedContent {
        body,
        toc: Vec::new(),
        document_kind: DocumentKind::Svg,
    })
}

fn rewrite_local_image<'a>(
    event: Event<'a>,
    base_dir: &Path,
    workspace: &Path,
    trusted: bool,
) -> Event<'a> {
    let Event::Start(Tag::Image {
        link_type,
        dest_url,
        title,
        id,
    }) = event
    else {
        return event;
    };

    if trusted && (is_external_url(&dest_url) || dest_url.starts_with("data:")) {
        return Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        });
    }

    if !trusted && (is_external_url(&dest_url) || dest_url.starts_with("data:")) {
        return blocked_image(link_type, title, id);
    }

    let image_path = base_dir.join(dest_url.as_ref());
    if !trusted {
        let Ok(canonical) = image_path.canonicalize() else {
            return blocked_image(link_type, title, id);
        };
        if !canonical.starts_with(workspace) || is_svg_file(&canonical) {
            return blocked_image(link_type, title, id);
        }
    }
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

fn blocked_image<'a>(
    link_type: pulldown_cmark::LinkType,
    title: CowStr<'a>,
    id: CowStr<'a>,
) -> Event<'a> {
    Event::Start(Tag::Image {
        link_type,
        dest_url: CowStr::Borrowed("data:,"),
        title,
        id,
    })
}

fn restrict_author_html<'a>(event: Event<'a>, trusted: bool) -> Event<'a> {
    if trusted {
        return event;
    }
    match event {
        Event::Html(value) | Event::InlineHtml(value) => Event::Text(value),
        other => other,
    }
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

fn render_code_block_html(block: CodeBlockCapture, trusted: bool, config: &Config) -> String {
    if trusted
        && let Some(diagram) =
            diagrams::render_diagram_html(block.language(), &block.text, config.diagrams.plantuml)
    {
        match diagram {
            DiagramRender::Html(html) => return html,
            DiagramRender::Fallback => {}
        }
    }

    let language_class = block
        .language()
        .map(|language| {
            format!(
                " language-{}",
                html_escape::encode_double_quoted_attribute(language)
            )
        })
        .unwrap_or_default();

    let code_html = block
        .language()
        .and_then(|language| classed_code_html(language, &block.text))
        .unwrap_or_else(|| html_escape::encode_text(&block.text).into_owned());

    format!(
        r#"<pre class="code-block"><code class="syntect-code{language_class}">{code_html}</code></pre>"#
    )
}

fn classed_code_html(language: &str, source: &str) -> Option<String> {
    let syntax = SYNTAX_SET.find_syntax_by_token(language)?;
    let mut generator = ClassedHTMLGenerator::new_with_class_style(
        syntax,
        &SYNTAX_SET,
        theme::syntax_class_style(),
    );

    for line in LinesWithEndings::from(source) {
        generator
            .parse_html_for_line_which_includes_newline(line)
            .ok()?;
    }

    Some(generator.finalize())
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
    document_kind: DocumentKind,
    trust_state: TrustState,
    workspace: String,
}

struct HeadingCapture {
    level: HeadingLevel,
    original_id: Option<String>,
    classes: Vec<String>,
    attrs: Vec<(CowStr<'static>, Option<CowStr<'static>>)>,
    events: Vec<Event<'static>>,
    text: String,
}

fn is_svg_file(file: &Path) -> bool {
    file.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
}

fn normalize_svg_document(source: &str) -> Option<String> {
    let mut svg = source.trim().to_owned();

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

struct CodeBlockCapture {
    kind: CapturedCodeBlockKind,
    text: String,
}

enum CapturedCodeBlockKind {
    Indented,
    Fenced(String),
}

impl CodeBlockCapture {
    fn new(kind: CodeBlockKind<'_>) -> Self {
        let kind = match kind {
            CodeBlockKind::Indented => CapturedCodeBlockKind::Indented,
            CodeBlockKind::Fenced(info) => CapturedCodeBlockKind::Fenced(info.into_string()),
        };

        Self {
            kind,
            text: String::new(),
        }
    }

    fn language(&self) -> Option<&str> {
        let CapturedCodeBlockKind::Fenced(info) = &self.kind else {
            return None;
        };

        info.split_whitespace()
            .next()
            .filter(|token| !token.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlighted_code_uses_prefixed_scope_classes() {
        let rendered = markdown_to_html(
            "```rust\nfn main() {}\n```",
            Path::new("."),
            Path::new("."),
            true,
            &Config::default(),
        );

        assert!(
            rendered
                .body
                .contains("class=\"syntect-code language-rust\"")
        );
        assert!(rendered.body.contains("syntect-"));
        assert!(!rendered.body.contains("style=\"color:"));
    }

    #[test]
    fn unknown_languages_fall_back_to_escaped_plain_code() {
        let rendered = markdown_to_html(
            "```not-a-language\n<a>&\n```",
            Path::new("."),
            Path::new("."),
            true,
            &Config::default(),
        );

        assert!(rendered.body.contains("&lt;a&gt;&amp;"));
        assert!(
            rendered
                .body
                .contains("class=\"syntect-code language-not-a-language\"")
        );
    }

    #[test]
    fn restricted_mode_escapes_html_and_leaves_diagrams_as_code() {
        let rendered = markdown_to_html(
            "<script>alert('x')</script>\n\n```mermaid\ngraph TD; A-->B\n```",
            Path::new("."),
            Path::new("."),
            false,
            &Config::default(),
        );

        assert!(rendered.body.contains("&lt;script&gt;"));
        assert!(!rendered.body.contains("<script>"));
        assert!(!rendered.body.contains("class=\"mermaid\""));
        assert!(rendered.body.contains("graph TD; A--&gt;B"));
    }

    #[test]
    fn restricted_mode_does_not_embed_remote_images() {
        let rendered = markdown_to_html(
            "![tracking](https://example.com/pixel.png)",
            Path::new("."),
            Path::new("."),
            false,
            &Config::default(),
        );

        assert!(!rendered.body.contains("https://example.com"));
        assert!(rendered.body.contains("src=\"data:,\""));
    }

    #[test]
    fn restricted_svg_is_shown_as_source() {
        let rendered = render_svg("<svg><script>alert(1)</script></svg>", false).unwrap();
        assert!(matches!(rendered.document_kind, DocumentKind::Markdown));
        assert!(rendered.body.contains("&lt;svg&gt;"));
        assert!(!rendered.body.contains("<script>"));
    }

    #[test]
    fn restricted_document_has_csp_and_does_not_embed_mermaid_runtime() {
        let temp = std::env::temp_dir().join(format!(
            "mdglance-restricted-document-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        let file = temp.join("README.md");
        std::fs::write(&file, "# Safe preview").unwrap();

        let html = render_document(&file, &temp, TrustState::Pending, &Config::default()).unwrap();
        assert!(html.contains("Content-Security-Policy"));
        assert!(html.contains("window.__MDVIEW_MERMAID_SOURCE = null"));
        assert!(html.contains("Do you trust this folder?"));
        let _ = std::fs::remove_dir_all(temp);
    }
}
