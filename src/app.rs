use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
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
    render,
    theme::{ThemeCatalog, ThemeConfig},
    trust::{self, TrustState, TrustStore},
    watcher,
};

#[derive(Debug, Clone)]
pub enum UserEvent {
    Reload,
    Close,
    OpenExternal(String),
    OpenMarkdown { href: String, scroll_ratio: f64 },
    Back { scroll_ratio: f64 },
    Forward { scroll_ratio: f64 },
    PreviousQueuedFile { scroll_ratio: f64 },
    NextQueuedFile { scroll_ratio: f64 },
    TrustWorkspace,
    OpenRestricted,
    OpenThemePicker,
    PreviewTheme(String),
    CommitTheme(String),
    WatchError(String),
}

pub fn run(file: PathBuf, queued_files: Vec<PathBuf>) -> Result<()> {
    let trust_store = TrustStore::new()?;
    let mut session_restricted = HashSet::new();
    let mut workspace = workspace_context(&file, &trust_store, &session_restricted)?;

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    #[cfg(target_os = "macos")]
    let fullscreen_presentation = crate::macos::FullscreenPresentation::new();
    #[cfg(target_os = "macos")]
    let event_loop = {
        let mut event_loop = event_loop;
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
        event_loop
    };
    let proxy = event_loop.create_proxy();
    let mut watcher = watcher::watch_file(file.clone(), proxy)?;
    let mut current_file = file;
    let mut current_watch_dir = current_file.parent().map(Path::to_path_buf);
    let mut back_stack = Vec::<PathBuf>::new();
    let mut forward_stack = Vec::<PathBuf>::new();
    let mut scroll_positions = HashMap::<PathBuf, f64>::new();
    let queued_files = build_file_queue(current_file.clone(), queued_files);
    let mut queue_index = 0usize;
    let mut session_theme: Option<ThemeConfig> = None;
    let mut theme_catalog = ThemeCatalog::load(true);

    let title = window_title(&current_file, Some((queue_index, queued_files.len())));
    let mut window_builder = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(LogicalSize::new(
            f64::from(workspace.config.window.width),
            f64::from(workspace.config.window.height),
        ))
        .with_maximized(workspace.config.window.maximized);
    if workspace.config.window.fullscreen {
        window_builder = window_builder.with_fullscreen(Some(Fullscreen::Borderless(None)));
    }
    let window = window_builder
        .build(&event_loop)
        .context("failed to create window")?;
    #[cfg(target_os = "macos")]
    let fullscreen_presentation = fullscreen_presentation.observe(&window);

    let html = render::render_document(
        &current_file,
        &workspace.root,
        workspace.trust_state,
        &workspace.config,
    )?;
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
                        IpcMessage::PreviousFile { scroll_ratio } => {
                            let _ =
                                proxy.send_event(UserEvent::PreviousQueuedFile { scroll_ratio });
                        }
                        IpcMessage::NextFile { scroll_ratio } => {
                            let _ = proxy.send_event(UserEvent::NextQueuedFile { scroll_ratio });
                        }
                        IpcMessage::TrustWorkspace => {
                            let _ = proxy.send_event(UserEvent::TrustWorkspace);
                        }
                        IpcMessage::OpenRestricted => {
                            let _ = proxy.send_event(UserEvent::OpenRestricted);
                        }
                        IpcMessage::OpenThemePicker => {
                            let _ = proxy.send_event(UserEvent::OpenThemePicker);
                        }
                        IpcMessage::PreviewTheme { name } => {
                            let _ = proxy.send_event(UserEvent::PreviewTheme(name));
                        }
                        IpcMessage::CommitTheme { name } => {
                            let _ = proxy.send_event(UserEvent::CommitTheme(name));
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
                    Url::parse(&url)
                        .ok()
                        .is_some_and(|url| matches!(url.scheme(), "about" | "data"))
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
                match render::render_body(
                    &current_file,
                    &workspace.root,
                    workspace.trust_state,
                    &workspace.config,
                ) {
                    Ok(rendered) => {
                        let payload = serde_json::json!({
                            "title": display_name(&current_file),
                            "body": rendered.body,
                            "toc": rendered.toc,
                            "document_kind": rendered.document_kind,
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
            TaoEvent::UserEvent(UserEvent::OpenThemePicker) => {
                theme_catalog = ThemeCatalog::load(true);
                let active = session_theme
                    .as_ref()
                    .map_or(workspace.config.theme.name.as_str(), |theme| {
                        theme.name.as_str()
                    });
                let entries = theme_catalog.picker_entries(active);
                let payload = serde_json::json!({"entries": entries, "active": active});
                let _ =
                    webview.evaluate_script(&format!("window.__mdglanceThemeCatalog({payload});"));
            }
            TaoEvent::UserEvent(UserEvent::PreviewTheme(name)) => {
                if let Ok(css) = theme_catalog.css_for(&name) {
                    let payload = serde_json::json!({"name": name, "css": css});
                    let _ = webview
                        .evaluate_script(&format!("window.__mdglanceApplyTheme({payload});"));
                }
            }
            TaoEvent::UserEvent(UserEvent::CommitTheme(name)) => {
                if let Ok(theme) = ThemeConfig::named(&name) {
                    if let Ok(css) = theme.css() {
                        let payload =
                            serde_json::json!({"name": name, "css": css, "committed": true});
                        let _ = webview
                            .evaluate_script(&format!("window.__mdglanceApplyTheme({payload});"));
                    }
                    workspace.config.theme = theme.clone();
                    workspace.theme_session = true;
                    session_theme = Some(theme);
                }
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
                    &queued_files,
                    queue_index,
                    &mut workspace,
                    &trust_store,
                    &session_restricted,
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
                    &mut workspace,
                    &trust_store,
                    &session_restricted,
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
                    &mut workspace,
                    &trust_store,
                    &session_restricted,
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
                    &mut workspace,
                    &trust_store,
                    &session_restricted,
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
                    &mut workspace,
                    &trust_store,
                    &session_restricted,
                    &window,
                    &webview,
                ) {
                    eprintln!("failed to go to next queued file: {err}");
                }
            }
            TaoEvent::UserEvent(UserEvent::OpenRestricted) => {
                session_restricted.insert(workspace.root.clone());
                workspace.trust_state = TrustState::Restricted;
            }
            TaoEvent::UserEvent(UserEvent::TrustWorkspace) => {
                match (|| -> Result<()> {
                    let mut config = Config::load_for_workspace(&workspace.root, true)?;
                    if let Some(theme) = &session_theme {
                        config.theme = theme.clone();
                    }
                    let html = render::render_document(
                        &current_file,
                        &workspace.root,
                        TrustState::Trusted,
                        &config,
                    )?;
                    trust_store.trust(&workspace.root)?;
                    webview
                        .load_html(&html)
                        .context("failed to reload trusted document")?;
                    apply_window_config(&window, &config);
                    workspace.trust_state = TrustState::Trusted;
                    workspace.config = config;
                    Ok(())
                })() {
                    Ok(()) => {}
                    Err(err) => {
                        let message = serde_json::to_string(&err.to_string()).unwrap_or_default();
                        let _ = webview.evaluate_script(&format!(
                            "window.__mdglanceShowTrustError({message});"
                        ));
                    }
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
            } if workspace
                .config
                .bindings_for(Action::Quit)
                .iter()
                .any(|binding| {
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

        // Keep the AppKit notification observers alive for the event loop's lifetime.
        #[cfg(target_os = "macos")]
        let _ = &fullscreen_presentation;
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
    PreviousFile { scroll_ratio: f64 },
    NextFile { scroll_ratio: f64 },
    TrustWorkspace,
    OpenRestricted,
    OpenThemePicker,
    PreviewTheme { name: String },
    CommitTheme { name: String },
}

struct WorkspaceContext {
    root: PathBuf,
    trust_state: TrustState,
    config: Config,
    theme_session: bool,
}

fn workspace_context(
    file: &Path,
    trust_store: &TrustStore,
    session_restricted: &HashSet<PathBuf>,
) -> Result<WorkspaceContext> {
    let root = trust::workspace_root(file)?;
    let trust_state = if trust_store.is_trusted(&root) {
        TrustState::Trusted
    } else if session_restricted.contains(&root) {
        TrustState::Restricted
    } else {
        TrustState::Pending
    };
    let config = Config::load_for_workspace(&root, trust_state == TrustState::Trusted)?;
    Ok(WorkspaceContext {
        root,
        trust_state,
        config,
        theme_session: false,
    })
}

fn apply_window_config(window: &tao::window::Window, config: &Config) {
    window.set_maximized(config.window.maximized);
    window.set_fullscreen(
        config
            .window
            .fullscreen
            .then(|| Fullscreen::Borderless(None)),
    );
    if !config.window.maximized && !config.window.fullscreen {
        window.set_inner_size(LogicalSize::new(
            f64::from(config.window.width),
            f64::from(config.window.height),
        ));
    }
}

enum HistoryDirection {
    Back,
    Forward,
}

enum QueueDirection {
    Previous,
    Next,
}

#[allow(clippy::too_many_arguments)]
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
    workspace: &mut WorkspaceContext,
    trust_store: &TrustStore,
    session_restricted: &HashSet<PathBuf>,
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
        workspace,
        trust_store,
        session_restricted,
        window,
        webview,
        queue_state,
    )
}

#[allow(clippy::too_many_arguments)]
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
    workspace: &mut WorkspaceContext,
    trust_store: &TrustStore,
    session_restricted: &HashSet<PathBuf>,
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
        workspace,
        trust_store,
        session_restricted,
        window,
        webview,
        queue_state,
    )
}

#[allow(clippy::too_many_arguments)]
fn open_file(
    target_file: PathBuf,
    anchor: Option<String>,
    current_file: &mut PathBuf,
    current_watch_dir: &mut Option<PathBuf>,
    watcher: &mut notify::RecommendedWatcher,
    scroll_positions: &HashMap<PathBuf, f64>,
    workspace: &mut WorkspaceContext,
    trust_store: &TrustStore,
    session_restricted: &HashSet<PathBuf>,
    window: &tao::window::Window,
    webview: &wry::WebView,
    queue_state: Option<(usize, usize)>,
) -> Result<()> {
    let mut next_workspace = workspace_context(&target_file, trust_store, session_restricted)?;
    if workspace.theme_session {
        next_workspace.config.theme = workspace.config.theme.clone();
        next_workspace.theme_session = true;
    }
    let same_security_context = next_workspace.root == workspace.root
        && next_workspace.trust_state == workspace.trust_state;
    let body_update = same_security_context
        .then(|| {
            render::render_body(
                &target_file,
                &next_workspace.root,
                next_workspace.trust_state,
                &next_workspace.config,
            )
        })
        .transpose()?;
    let full_document = (!same_security_context)
        .then(|| {
            render::render_document(
                &target_file,
                &next_workspace.root,
                next_workspace.trust_state,
                &next_workspace.config,
            )
        })
        .transpose()?;
    let next_watch_dir =
        watcher::retarget_watch(watcher, current_watch_dir.as_deref(), &target_file)?;
    *current_watch_dir = Some(next_watch_dir);
    *current_file = target_file;
    if let Some(rendered) = body_update {
        let payload = serde_json::json!({
            "title": display_name(current_file),
            "body": rendered.body,
            "toc": rendered.toc,
            "document_kind": rendered.document_kind,
            "anchor": anchor,
            "scroll_ratio": scroll_positions.get(current_file).copied().unwrap_or(0.0),
        });
        webview
            .evaluate_script(&format!("window.__mdglanceUpdate({payload});"))
            .context("failed to update preview")?;
    } else if let Some(html) = full_document {
        webview.load_html(&html).context("failed to load preview")?;
    }
    *workspace = next_workspace;
    window.set_title(&window_title(current_file, queue_state));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn navigate_queue(
    scroll_ratio: f64,
    current_file: &mut PathBuf,
    current_watch_dir: &mut Option<PathBuf>,
    watcher: &mut notify::RecommendedWatcher,
    scroll_positions: &mut HashMap<PathBuf, f64>,
    queued_files: &[PathBuf],
    queue_index: &mut usize,
    direction: QueueDirection,
    workspace: &mut WorkspaceContext,
    trust_store: &TrustStore,
    session_restricted: &HashSet<PathBuf>,
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
        workspace,
        trust_store,
        session_restricted,
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

pub fn display_name(file: &Path) -> String {
    file.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Markdown")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::external_url;

    #[test]
    fn only_http_urls_can_leave_the_webview() {
        assert!(external_url("https://example.com").is_some());
        assert!(external_url("http://example.com").is_some());
        assert!(external_url("javascript:alert(1)").is_none());
        assert!(external_url("file:///tmp/secret").is_none());
        assert!(external_url("https://example.com\nfile:///tmp/secret").is_none());
    }
}
