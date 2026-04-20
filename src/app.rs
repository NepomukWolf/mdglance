use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tao::{
    dpi::LogicalSize,
    event::{Event as TaoEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use url::Url;
use wry::WebViewBuilder;

use crate::{render, watcher};

#[derive(Debug, Clone)]
pub enum UserEvent {
    Reload,
    Close,
    OpenExternal(String),
    WatchError(String),
}

pub fn run(file: PathBuf) -> Result<()> {
    let file = file
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", file.display()))?;

    if !file.is_file() {
        anyhow::bail!("{} is not a file", file.display());
    }

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let _watcher = watcher::watch_file(file.clone(), proxy)?;

    let title = format!("mdview - {}", display_name(&file));
    let window = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(LogicalSize::new(1080.0, 860.0))
        .build(&event_loop)
        .context("failed to create window")?;

    let html = render::render_document(&file)?;
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

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            TaoEvent::UserEvent(UserEvent::Reload) => match render::render_body(&file) {
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
            TaoEvent::UserEvent(UserEvent::OpenExternal(url)) => {
                if let Err(err) = open_external_url(&url) {
                    eprintln!("failed to open {url}: {err}");
                }
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
