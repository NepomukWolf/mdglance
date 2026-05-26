use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
#[cfg(target_os = "macos")]
use tao::platform::macos::WindowExtMacOS;
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
    ToggleFullscreen,
    Refocus,
    OpenExternal(String),
    OpenMarkdown { href: String, scroll_ratio: f64 },
    Back { scroll_ratio: f64 },
    Forward { scroll_ratio: f64 },
    PreviousQueuedFile { scroll_ratio: f64 },
    NextQueuedFile { scroll_ratio: f64 },
    WatchError(String),
}

pub fn run(file: PathBuf, queued_files: Vec<PathBuf>) -> Result<()> {
    let config = Config::load()?;

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let mut watcher = watcher::watch_file(file.clone(), proxy)?;
    let mut current_file = file;
    let mut current_watch_dir = current_file.parent().map(Path::to_path_buf);
    let mut back_stack = Vec::<PathBuf>::new();
    let mut forward_stack = Vec::<PathBuf>::new();
    let mut scroll_positions = HashMap::<PathBuf, f64>::new();
    let queued_files = build_file_queue(current_file.clone(), queued_files);
    let mut queue_index = 0usize;

    let title = window_title(&current_file, Some((queue_index, queued_files.len())));
    let mut window_builder =
        WindowBuilder::new()
            .with_title(title)
            .with_inner_size(LogicalSize::new(
                f64::from(config.window.width),
                f64::from(config.window.height),
            ));
    if config.window.fullscreen && !cfg!(target_os = "macos") {
        window_builder = window_builder.with_fullscreen(Some(Fullscreen::Borderless(None)));
    }
    let window = window_builder
        .build(&event_loop)
        .context("failed to create window")?;
    apply_initial_fullscreen(&window, &config);

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
                        IpcMessage::ToggleFullscreen => {
                            let _ = proxy.send_event(UserEvent::ToggleFullscreen);
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
                        IpcMessage::PreviousFile { scroll_ratio } => {
                            let _ =
                                proxy.send_event(UserEvent::PreviousQueuedFile { scroll_ratio });
                        }
                        IpcMessage::NextFile { scroll_ratio } => {
                            let _ = proxy.send_event(UserEvent::NextQueuedFile { scroll_ratio });
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

    let refocus_script = String::from(
        "window.focus(); document.getElementById('presentation-root')?.focus(); document.getElementById('content')?.focus();",
    );

    let mut current_modifiers = ModifiersState::empty();
    let refocus_proxy = event_loop.create_proxy();

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
                            "document_kind": rendered.document_kind,
                            "presentation": rendered.presentation,
                        });
                        let script = format!("window.__mdglanceUpdate({payload});");
                        if let Err(err) = webview.evaluate_script(&script) {
                            eprintln!("failed to update preview: {err}");
                        }
                        window.set_title(&window_title(
                            &current_file,
                            display_queue_state(&current_file, &queued_files, queue_index),
                        ));
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
            TaoEvent::UserEvent(UserEvent::ToggleFullscreen) => {
                toggle_fullscreen(&window);
                refocus_window(&window, &webview, &refocus_script);
                schedule_refocus(&refocus_proxy);
            }
            TaoEvent::UserEvent(UserEvent::Refocus) => {
                refocus_window(&window, &webview, &refocus_script);
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
                    &queued_files,
                    queue_index,
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
                    &queued_files,
                    queue_index,
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
                    &queued_files,
                    queue_index,
                    HistoryDirection::Forward,
                    &config,
                    &window,
                    &webview,
                ) {
                    eprintln!("failed to go forward: {err}");
                }
            }
            TaoEvent::UserEvent(UserEvent::PreviousQueuedFile { scroll_ratio }) => {
                if let Err(err) = navigate_queue(
                    scroll_ratio,
                    &mut current_file,
                    &mut current_watch_dir,
                    &mut watcher,
                    &mut scroll_positions,
                    &queued_files,
                    &mut queue_index,
                    QueueDirection::Previous,
                    &config,
                    &window,
                    &webview,
                ) {
                    eprintln!("failed to go to previous queued file: {err}");
                }
            }
            TaoEvent::UserEvent(UserEvent::NextQueuedFile { scroll_ratio }) => {
                if let Err(err) = navigate_queue(
                    scroll_ratio,
                    &mut current_file,
                    &mut current_watch_dir,
                    &mut watcher,
                    &mut scroll_positions,
                    &queued_files,
                    &mut queue_index,
                    QueueDirection::Next,
                    &config,
                    &window,
                    &webview,
                ) {
                    eprintln!("failed to go to next queued file: {err}");
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
                event: WindowEvent::Focused(true),
                ..
            } => {
                refocus_window(&window, &webview, &refocus_script);
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
    ToggleFullscreen,
    OpenExternal { href: String },
    OpenMarkdown { href: String, scroll_ratio: f64 },
    Back { scroll_ratio: f64 },
    Forward { scroll_ratio: f64 },
    PreviousFile { scroll_ratio: f64 },
    NextFile { scroll_ratio: f64 },
}

enum HistoryDirection {
    Back,
    Forward,
}

enum QueueDirection {
    Previous,
    Next,
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
    queued_files: &[PathBuf],
    queue_index: usize,
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
    let queue_state = display_queue_state(&target_file, queued_files, queue_index);
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
        queue_state,
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
    queued_files: &[PathBuf],
    queue_index: usize,
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

    let queue_state = display_queue_state(&target, queued_files, queue_index);
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
        queue_state,
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
    queue_state: Option<(usize, usize)>,
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
        "document_kind": rendered.document_kind,
        "presentation": rendered.presentation,
        "anchor": anchor,
        "scroll_ratio": scroll_positions.get(current_file).copied().unwrap_or(0.0),
    });
    let script = format!("window.__mdglanceUpdate({payload});");
    webview
        .evaluate_script(&script)
        .context("failed to update preview")?;
    window.set_title(&window_title(current_file, queue_state));
    Ok(())
}

fn navigate_queue(
    scroll_ratio: f64,
    current_file: &mut PathBuf,
    current_watch_dir: &mut Option<PathBuf>,
    watcher: &mut notify::RecommendedWatcher,
    scroll_positions: &mut HashMap<PathBuf, f64>,
    queued_files: &[PathBuf],
    queue_index: &mut usize,
    direction: QueueDirection,
    config: &Config,
    window: &tao::window::Window,
    webview: &wry::WebView,
) -> Result<()> {
    if queued_files.len() <= 1 {
        return Ok(());
    }

    let next_index = match direction {
        QueueDirection::Previous if *queue_index > 0 => *queue_index - 1,
        QueueDirection::Next if *queue_index + 1 < queued_files.len() => *queue_index + 1,
        _ => return Ok(()),
    };

    let target = queued_files[next_index].clone();
    if target == *current_file {
        *queue_index = next_index;
        return Ok(());
    }

    scroll_positions.insert(current_file.clone(), scroll_ratio);
    *queue_index = next_index;
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
        Some((*queue_index, queued_files.len())),
    )
}

fn build_file_queue(current_file: PathBuf, queued_files: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut queue = Vec::with_capacity(queued_files.len() + 1);
    queue.push(current_file);

    for file in queued_files {
        if !queue.contains(&file) {
            queue.push(file);
        }
    }

    queue
}

fn display_queue_state(
    current_file: &Path,
    queued_files: &[PathBuf],
    queue_index: usize,
) -> Option<(usize, usize)> {
    if queued_files
        .get(queue_index)
        .is_some_and(|file| file == current_file)
    {
        Some((queue_index, queued_files.len()))
    } else {
        None
    }
}

fn window_title(current_file: &Path, queue_state: Option<(usize, usize)>) -> String {
    let mut title = format!("mdglance - {}", display_name(current_file));
    if let Some((index, total)) = queue_state
        && total > 1
    {
        title.push_str(&format!(" ({}/{})", index + 1, total));
    }
    title
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

fn apply_initial_fullscreen(window: &tao::window::Window, config: &Config) {
    if !config.window.fullscreen {
        return;
    }

    #[cfg(target_os = "macos")]
    {
        let _ = window.set_simple_fullscreen(true);
    }

    #[cfg(not(target_os = "macos"))]
    {
        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
    }
}

fn toggle_fullscreen(window: &tao::window::Window) {
    #[cfg(target_os = "macos")]
    {
        let _ = window.set_simple_fullscreen(!window.simple_fullscreen());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let next = if window.fullscreen().is_some() {
            None
        } else {
            Some(Fullscreen::Borderless(None))
        };
        window.set_fullscreen(next);
    }
}

fn refocus_window(window: &tao::window::Window, webview: &wry::WebView, script: &str) {
    window.set_focus();
    let _ = webview.focus();
    let _ = webview.evaluate_script(script);
}

fn schedule_refocus(proxy: &tao::event_loop::EventLoopProxy<UserEvent>) {
    for delay_ms in [80u64, 220, 420] {
        let proxy = proxy.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(delay_ms));
            let _ = proxy.send_event(UserEvent::Refocus);
        });
    }
}

pub fn display_name(file: &Path) -> String {
    file.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Markdown")
        .to_string()
}
