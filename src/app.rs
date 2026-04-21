use std::path::{Path, PathBuf};

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
    let _watcher = watcher::watch_file(file.clone(), proxy)?;

    let title = format!("mdglance - {}", display_name(&file));
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

    let html = render::render_document(&file, &config)?;
    let webview = WebViewBuilder::new()
        .with_html(html)
        .with_ipc_handler({
            let proxy = event_loop.create_proxy();
            move |message| match message.body().as_str() {
                "close" => {
                    let _ = proxy.send_event(UserEvent::Close);
                }
                _ => {}
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
            TaoEvent::UserEvent(UserEvent::Reload) => match render::render_body(&file, &config) {
                Ok(rendered) => {
                    let payload = serde_json::json!({
                        "title": display_name(&file),
                        "body": rendered.body,
                        "toc": rendered.toc,
                    });
                    let script = format!("window.__mdglanceUpdate({payload});");
                    if let Err(err) = webview.evaluate_script(&script) {
                        eprintln!("failed to update preview: {err}");
                    }
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
            },
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
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .with_context(|| format!("failed to launch browser for {url}"))?;
    Ok(())
}

pub fn display_name(file: &Path) -> String {
    file.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Markdown")
        .to_string()
}
