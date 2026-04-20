use std::{path::PathBuf, sync::mpsc};

use anyhow::{Context, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tao::event_loop::EventLoopProxy;

use crate::app::UserEvent;

pub fn watch_file(file: PathBuf, proxy: EventLoopProxy<UserEvent>) -> Result<RecommendedWatcher> {
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
