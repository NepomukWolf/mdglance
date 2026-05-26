use std::{collections::HashMap, path::Path, sync::LazyLock};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose};
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html,
};
use serde::Serialize;
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    html::{IncludeBackground, styled_line_to_highlighted_html},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

use crate::{
    app, assets,
    config::Config,
    diagrams::{self, DiagramRender},
};

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static SYNTAX_THEME: LazyLock<Theme> = LazyLock::new(|| {
    ThemeSet::load_defaults()
        .themes
        .get("InspiredGitHub")
        .cloned()
        .expect("default syntect theme must exist")
});

#[derive(Debug, Clone, Serialize)]
pub struct RenderedContent {
    pub body: String,
    pub toc: Vec<TocItem>,
    pub document_kind: DocumentKind,
    pub presentation: Option<PresentationData>,
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

#[derive(Debug, Clone, Serialize)]
pub struct PresentationData {
    pub enabled: bool,
    pub default_mode: PresentationMode,
    pub header: Option<String>,
    pub footer: Option<String>,
    pub page_numbers: bool,
    pub slides: Vec<PresentationSlide>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[allow(dead_code)]
#[serde(rename_all = "snake_case")]
pub enum PresentationMode {
    Markdown,
    Presentation,
}

#[derive(Debug, Clone, Serialize)]
pub struct PresentationSlide {
    pub index: usize,
    pub body: String,
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
        document_kind: rendered.document_kind,
        presentation: rendered.presentation.clone(),
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
    <section id="presentation-root" class="presentation-root hidden" tabindex="-1"></section>
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
    let source = std::fs::read_to_string(file)
        .with_context(|| format!("failed to read {}", file.display()))?;
    if is_svg_file(file) {
        return render_svg(&source);
    }

    let base_dir = file
        .parent()
        .context("cannot render a file without a parent directory")?;
    Ok(render_markdown_document(
        &source,
        base_dir,
        config.toc.max_depth,
    ))
}

fn render_markdown_document(markdown: &str, base_dir: &Path, max_toc_depth: u8) -> RenderedContent {
    let (frontmatter, body) = parse_frontmatter(markdown);
    let is_presentation = frontmatter
        .get("presentation")
        .is_some_and(|value| value.eq_ignore_ascii_case("true"));

    if !is_presentation {
        let mut slug_counts = HashMap::new();
        let fragment = markdown_to_html(body, base_dir, max_toc_depth, &mut slug_counts, None);
        return RenderedContent {
            body: fragment.body,
            toc: fragment.toc,
            document_kind: DocumentKind::Markdown,
            presentation: None,
        };
    }

    let slide_sources = split_presentation_slides(body);
    let mut slug_counts = HashMap::new();
    let mut toc = Vec::new();
    let mut markdown_body = String::new();
    let mut slides = Vec::new();

    for (index, slide_source) in slide_sources.iter().enumerate() {
        let fragment = markdown_to_html(
            slide_source,
            base_dir,
            max_toc_depth,
            &mut slug_counts,
            Some(index),
        );
        toc.extend(fragment.toc);

        if index > 0 {
            markdown_body.push_str(&format!(
                r#"<hr class="presentation-divider" data-slide-divider="{index}">"#
            ));
        }
        markdown_body.push_str(&fragment.body);

        slides.push(PresentationSlide {
            index,
            body: fragment.body,
        });
    }

    RenderedContent {
        body: markdown_body,
        toc,
        document_kind: DocumentKind::Markdown,
        presentation: Some(PresentationData {
            enabled: true,
            default_mode: PresentationMode::Presentation,
            header: frontmatter_text(&frontmatter, "presentation_header"),
            footer: frontmatter_text(&frontmatter, "presentation_footer"),
            page_numbers: frontmatter_bool(&frontmatter, "presentation_page_numbers"),
            slides,
        }),
    }
}

fn markdown_to_html(
    markdown: &str,
    base_dir: &Path,
    max_toc_depth: u8,
    slug_counts: &mut HashMap<String, usize>,
    slide_index: Option<usize>,
) -> RenderedFragment {
    let parser = Parser::new_ext(markdown, markdown_options());
    let mut events = Vec::new();
    let mut toc = Vec::new();
    let mut current_block = None::<CodeBlockCapture>;
    let mut current_heading = None::<HeadingCapture>;

    for event in parser {
        if let Some(block) = current_block.as_mut() {
            match event {
                Event::End(TagEnd::CodeBlock) => {
                    let block = current_block.take().expect("code block state must exist");
                    events.push(Event::Html(CowStr::Boxed(
                        render_code_block_html(block).into_boxed_str(),
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
                    let final_id = unique_heading_id(slug_source, slug_counts);
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
                                (
                                    CowStr::Borrowed("data-slide-index"),
                                    slide_index.map(|index| {
                                        CowStr::Boxed(index.to_string().into_boxed_str())
                                    }),
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
            Event::Start(Tag::CodeBlock(kind)) => {
                current_block = Some(CodeBlockCapture::new(kind));
            }
            other => {
                events.push(rewrite_local_image(other, base_dir).into_static());
            }
        }
    }

    let mut body = String::new();
    html::push_html(&mut body, events.into_iter());
    if let Some(index) = slide_index {
        body = format!(
            r#"<section class="presentation-source-slide" data-presentation-slide="{index}">{body}</section>"#
        );
    }

    RenderedFragment { body, toc }
}

fn render_svg(source: &str) -> Result<RenderedContent> {
    let svg = normalize_svg_document(source).context("failed to normalize SVG document")?;
    let body = format!(
        r#"<div class="svg-shell"><div id="svg-viewport" class="svg-viewport"><div id="svg-stage" class="svg-stage">{svg}</div></div></div>"#
    );

    Ok(RenderedContent {
        body,
        toc: Vec::new(),
        document_kind: DocumentKind::Svg,
        presentation: None,
    })
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

fn render_code_block_html(block: CodeBlockCapture) -> String {
    if let Some(diagram) = diagrams::render_diagram_html(block.language(), &block.text) {
        match diagram {
            DiagramRender::Html(html) => return html,
            DiagramRender::Fallback => {}
        }
    }

    let language_class = block
        .language()
        .map(|language| {
            format!(
                r#" class="language-{}""#,
                html_escape::encode_double_quoted_attribute(language)
            )
        })
        .unwrap_or_default();

    let code_html = block
        .language()
        .and_then(|language| highlighted_code_html(language, &block.text))
        .unwrap_or_else(|| html_escape::encode_text(&block.text).into_owned());

    format!(r#"<pre class="code-block"><code{language_class}>{code_html}</code></pre>"#)
}

fn highlighted_code_html(language: &str, source: &str) -> Option<String> {
    let syntax = SYNTAX_SET.find_syntax_by_token(language)?;
    let mut highlighter = HighlightLines::new(syntax, &SYNTAX_THEME);
    let mut html = String::new();

    for line in LinesWithEndings::from(source) {
        let ranges = highlighter.highlight_line(line, &SYNTAX_SET).ok()?;
        let line_html = styled_line_to_highlighted_html(&ranges[..], IncludeBackground::No).ok()?;
        html.push_str(&line_html);
    }

    Some(html)
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
    presentation: Option<PresentationData>,
}

struct HeadingCapture {
    level: HeadingLevel,
    original_id: Option<String>,
    classes: Vec<String>,
    attrs: Vec<(CowStr<'static>, Option<CowStr<'static>>)>,
    events: Vec<Event<'static>>,
    text: String,
}

struct RenderedFragment {
    body: String,
    toc: Vec<TocItem>,
}

fn is_svg_file(file: &Path) -> bool {
    file.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
}

fn parse_frontmatter(markdown: &str) -> (HashMap<String, String>, &str) {
    let Some(rest) = markdown.strip_prefix("---\n") else {
        return (HashMap::new(), markdown);
    };

    let Some(end) = rest.find("\n---\n") else {
        return (HashMap::new(), markdown);
    };

    let frontmatter = &rest[..end];
    let body = &rest[end + 5..];
    let values = frontmatter
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            Some((
                key.trim().to_string(),
                parse_frontmatter_value(value.trim()),
            ))
        })
        .collect();

    (values, body)
}

fn parse_frontmatter_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        let first = bytes[0];
        let last = bytes[trimmed.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return trimmed[1..trimmed.len() - 1].trim().to_string();
        }
    }

    trimmed.to_string()
}

fn frontmatter_bool(frontmatter: &HashMap<String, String>, key: &str) -> bool {
    frontmatter
        .get(key)
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

fn frontmatter_text(frontmatter: &HashMap<String, String>, key: &str) -> Option<String> {
    frontmatter
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn split_presentation_slides(markdown: &str) -> Vec<&str> {
    let mut slides = Vec::new();
    let mut in_fence = false;
    let mut start = 0usize;
    let mut cursor = 0usize;

    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']).trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence && trimmed == "---" {
            slides.push(markdown[start..cursor].trim());
            start = cursor + line.len();
        }
        cursor += line.len();
    }

    slides.push(markdown[start..].trim());
    let mut slides = slides
        .into_iter()
        .filter(|slide| !slide.is_empty())
        .collect::<Vec<_>>();
    if slides.is_empty() {
        slides.push(markdown.trim());
    }
    slides
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
    use super::{parse_frontmatter, split_presentation_slides};

    #[test]
    fn parses_presentation_frontmatter() {
        let markdown =
            "---\npresentation: true\ntitle: Demo\npresentation_header: \"Deck\"\n---\n\n# Slide";
        let (frontmatter, body) = parse_frontmatter(markdown);

        assert_eq!(
            frontmatter.get("presentation").map(String::as_str),
            Some("true")
        );
        assert_eq!(frontmatter.get("title").map(String::as_str), Some("Demo"));
        assert_eq!(
            frontmatter.get("presentation_header").map(String::as_str),
            Some("Deck")
        );
        assert_eq!(body.trim(), "# Slide");
    }

    #[test]
    fn splits_presentation_slides_on_separator_lines() {
        let markdown = "# One\n\n---\n\n# Two\n";
        let slides = split_presentation_slides(markdown);

        assert_eq!(slides, vec!["# One", "# Two"]);
    }

    #[test]
    fn does_not_split_slides_inside_fenced_code_blocks() {
        let markdown = "# One\n\n```md\n---\n```\n\n---\n\n# Two\n";
        let slides = split_presentation_slides(markdown);

        assert_eq!(slides, vec!["# One\n\n```md\n---\n```", "# Two"]);
    }
}
