use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use tao::{
    dpi::LogicalSize,
    event::{ElementState, Event as TaoEvent, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    keyboard::ModifiersState,
    window::{Fullscreen, WindowBuilder},
};
use url::Url;
use wry::WebViewBuilder;

use crate::{
    config::{Action, Config},
    render, watcher,
};

#[derive(Debug, Clone)]
pub enum UserEvent {
    Reload,
    Close,
    OpenExternal(String),
    OpenMarkdown { href: String, scroll_ratio: f64 },
    Back { scroll_ratio: f64 },
    Forward { scroll_ratio: f64 },
    WatchError(String),
}

pub fn run(file: PathBuf) -> Result<()> {
    let config = Config::load()?;
    let file = file
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", file.display()))?;

    if !file.is_file() {
        anyhow::bail!("{} is not a file", file.display());
    }

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let mut watcher = watcher::watch_file(file.clone(), proxy)?;
    let mut current_file = file;
    let mut current_watch_dir = current_file.parent().map(Path::to_path_buf);
    let mut back_stack = Vec::<PathBuf>::new();
    let mut forward_stack = Vec::<PathBuf>::new();
    let mut scroll_positions = HashMap::<PathBuf, f64>::new();

    let title = format!("mdglance - {}", display_name(&current_file));
    let mut window_builder =
        WindowBuilder::new()
            .with_title(title)
            .with_inner_size(LogicalSize::new(
                f64::from(config.window.width),
                f64::from(config.window.height),
            ));
    if config.window.fullscreen {
        window_builder = window_builder.with_fullscreen(Some(Fullscreen::Borderless(None)));
    }
    let window = window_builder
        .build(&event_loop)
        .context("failed to create window")?;

    let html = render::render_document(&current_file, &config)?;
    let webview = WebViewBuilder::new()
        .with_html(html)
        .with_ipc_handler({
            let proxy = event_loop.create_proxy();
            move |message| {
                if let Ok(event) = serde_json::from_str::<IpcMessage>(message.body()) {
                    match event {
                        IpcMessage::Close => {
                            let _ = proxy.send_event(UserEvent::Close);
                        }
                        IpcMessage::OpenExternal { href } => {
                            let _ = proxy.send_event(UserEvent::OpenExternal(href));
                        }
                        IpcMessage::OpenMarkdown { href, scroll_ratio } => {
                            let _ =
                                proxy.send_event(UserEvent::OpenMarkdown { href, scroll_ratio });
                        }
                        IpcMessage::Back { scroll_ratio } => {
                            let _ = proxy.send_event(UserEvent::Back { scroll_ratio });
                        }
                        IpcMessage::Forward { scroll_ratio } => {
                            let _ = proxy.send_event(UserEvent::Forward { scroll_ratio });
                        }
                    }
                }
            }
        })
        .with_navigation_handler({
            let proxy = event_loop.create_proxy();
            move |url| {
                if let Some(url) = external_url(&url) {
                    let _ = proxy.send_event(UserEvent::OpenExternal(url.to_string()));
                    false
                } else {
                    true
                }
            }
        })
        .build(&window)
        .context("failed to create webview")?;

    let mut current_modifiers = ModifiersState::empty();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            TaoEvent::UserEvent(UserEvent::Reload) => {
                match render::render_body(&current_file, &config) {
                    Ok(rendered) => {
                        let payload = serde_json::json!({
                            "title": display_name(&current_file),
                            "body": rendered.body,
                            "toc": rendered.toc,
                        });
                        let script = format!("window.__mdglanceUpdate({payload});");
                        if let Err(err) = webview.evaluate_script(&script) {
                            eprintln!("failed to update preview: {err}");
                        }
                        window.set_title(&format!("mdglance - {}", display_name(&current_file)));
                    }
                    Err(err) => {
                        let message = err.to_string();
                        let escaped = html_escape::encode_text(&message);
                        let script = format!(
                            "window.__mdglanceShowError({});",
                            serde_json::to_string(&escaped.to_string()).unwrap_or_default()
                        );
                        let _ = webview.evaluate_script(&script);
                    }
                }
            }
            TaoEvent::UserEvent(UserEvent::WatchError(message)) => {
                eprintln!("watch error: {message}");
            }
            TaoEvent::UserEvent(UserEvent::Close) => {
                *control_flow = ControlFlow::Exit;
            }
            TaoEvent::UserEvent(UserEvent::OpenExternal(url)) => {
                if let Err(err) = open_external_url(&url) {
                    eprintln!("failed to open {url}: {err}");
                }
            }
            TaoEvent::UserEvent(UserEvent::OpenMarkdown { href, scroll_ratio }) => {
                if let Err(err) = navigate_to_href(
                    &href,
                    scroll_ratio,
                    &mut current_file,
                    &mut current_watch_dir,
                    &mut watcher,
                    &mut back_stack,
                    &mut forward_stack,
                    &mut scroll_positions,
                    &config,
                    &window,
                    &webview,
                ) {
                    eprintln!("failed to open markdown link `{href}`: {err}");
                }
            }
            TaoEvent::UserEvent(UserEvent::Back { scroll_ratio }) => {
                if let Err(err) = navigate_history(
                    scroll_ratio,
                    &mut current_file,
                    &mut current_watch_dir,
                    &mut watcher,
                    &mut back_stack,
                    &mut forward_stack,
                    &mut scroll_positions,
                    HistoryDirection::Back,
                    &config,
                    &window,
                    &webview,
                ) {
                    eprintln!("failed to go back: {err}");
                }
            }
            TaoEvent::UserEvent(UserEvent::Forward { scroll_ratio }) => {
                if let Err(err) = navigate_history(
                    scroll_ratio,
                    &mut current_file,
                    &mut current_watch_dir,
                    &mut watcher,
                    &mut back_stack,
                    &mut forward_stack,
                    &mut scroll_positions,
                    HistoryDirection::Forward,
                    &config,
                    &window,
                    &webview,
                ) {
                    eprintln!("failed to go forward: {err}");
                }
            }
            TaoEvent::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                logical_key,
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    },
                ..
            } if config.bindings_for(Action::Quit).iter().any(|binding| {
                binding
                    .shortcut
                    .matches_native(&logical_key, current_modifiers)
            }) =>
            {
                *control_flow = ControlFlow::Exit;
            }
            TaoEvent::WindowEvent {
                event: WindowEvent::ModifiersChanged(modifiers),
                ..
            } => {
                current_modifiers = modifiers;
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

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum IpcMessage {
    Close,
    OpenExternal { href: String },
    OpenMarkdown { href: String, scroll_ratio: f64 },
    Back { scroll_ratio: f64 },
    Forward { scroll_ratio: f64 },
}

enum HistoryDirection {
    Back,
    Forward,
}

fn navigate_to_href(
    href: &str,
    scroll_ratio: f64,
    current_file: &mut PathBuf,
    current_watch_dir: &mut Option<PathBuf>,
    watcher: &mut notify::RecommendedWatcher,
    back_stack: &mut Vec<PathBuf>,
    forward_stack: &mut Vec<PathBuf>,
    scroll_positions: &mut HashMap<PathBuf, f64>,
    config: &Config,
    window: &tao::window::Window,
    webview: &wry::WebView,
) -> Result<()> {
    let (target_file, anchor) = resolve_markdown_href(current_file, href)?;
    if target_file == *current_file {
        if let Some(anchor) = anchor {
            let payload = serde_json::json!({ "anchor": anchor });
            let script = format!("window.__mdglanceJumpToAnchor({payload});");
            let _ = webview.evaluate_script(&script);
        }
        return Ok(());
    }

    scroll_positions.insert(current_file.clone(), scroll_ratio);
    back_stack.push(current_file.clone());
    forward_stack.clear();
    open_file(
        target_file,
        anchor,
        current_file,
        current_watch_dir,
        watcher,
        scroll_positions,
        config,
        window,
        webview,
    )
}

fn navigate_history(
    scroll_ratio: f64,
    current_file: &mut PathBuf,
    current_watch_dir: &mut Option<PathBuf>,
    watcher: &mut notify::RecommendedWatcher,
    back_stack: &mut Vec<PathBuf>,
    forward_stack: &mut Vec<PathBuf>,
    scroll_positions: &mut HashMap<PathBuf, f64>,
    direction: HistoryDirection,
    config: &Config,
    window: &tao::window::Window,
    webview: &wry::WebView,
) -> Result<()> {
    let target = match direction {
        HistoryDirection::Back => back_stack.pop(),
        HistoryDirection::Forward => forward_stack.pop(),
    };

    let Some(target) = target else {
        return Ok(());
    };

    scroll_positions.insert(current_file.clone(), scroll_ratio);
    match direction {
        HistoryDirection::Back => forward_stack.push(current_file.clone()),
        HistoryDirection::Forward => back_stack.push(current_file.clone()),
    }

    open_file(
        target,
        None,
        current_file,
        current_watch_dir,
        watcher,
        scroll_positions,
        config,
        window,
        webview,
    )
}

fn open_file(
    target_file: PathBuf,
    anchor: Option<String>,
    current_file: &mut PathBuf,
    current_watch_dir: &mut Option<PathBuf>,
    watcher: &mut notify::RecommendedWatcher,
    scroll_positions: &HashMap<PathBuf, f64>,
    config: &Config,
    window: &tao::window::Window,
    webview: &wry::WebView,
) -> Result<()> {
    let rendered = render::render_body(&target_file, config)?;
    let next_watch_dir =
        watcher::retarget_watch(watcher, current_watch_dir.as_ref(), &target_file)?;
    *current_watch_dir = Some(next_watch_dir);
    *current_file = target_file;
    let payload = serde_json::json!({
        "title": display_name(current_file),
        "body": rendered.body,
        "toc": rendered.toc,
        "anchor": anchor,
        "scroll_ratio": scroll_positions.get(current_file).copied().unwrap_or(0.0),
    });
    let script = format!("window.__mdglanceUpdate({payload});");
    webview
        .evaluate_script(&script)
        .context("failed to update preview")?;
    window.set_title(&format!("mdglance - {}", display_name(current_file)));
    Ok(())
}

fn resolve_markdown_href(current_file: &Path, href: &str) -> Result<(PathBuf, Option<String>)> {
    let (path_part, anchor) = href
        .split_once('#')
        .map_or((href, None), |(path, fragment)| {
            (path, Some(fragment.to_string()))
        });

    let target = if path_part.is_empty() {
        current_file.to_path_buf()
    } else {
        let base = current_file
            .parent()
            .context("cannot resolve a relative link without a parent directory")?;
        base.join(path_part)
            .canonicalize()
            .with_context(|| format!("failed to resolve markdown link {href}"))?
    };

    if !target.is_file() {
        anyhow::bail!("{} is not a file", target.display());
    }

    Ok((target, anchor))
}

fn external_url(url: &str) -> Option<Url> {
    if url.chars().any(char::is_control) {
        return None;
    }

    let parsed = Url::parse(url).ok()?;
    match parsed.scheme() {
        "http" | "https" => Some(parsed),
        _ => None,
    }
}

fn open_external_url(url: &str) -> Result<()> {
    open::that(url).with_context(|| format!("failed to open external URL {url}"))?;
    Ok(())
}

pub fn display_name(file: &Path) -> String {
    file.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Markdown")
        .to_string()
}
