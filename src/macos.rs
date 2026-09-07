use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationPresentationOptions};

pub struct FullscreenPresentation {
    normal_options: NSApplicationPresentationOptions,
    active: bool,
}

impl FullscreenPresentation {
    pub fn new() -> Self {
        Self {
            normal_options: application().presentationOptions(),
            active: false,
        }
    }

    pub fn sync(&mut self, fullscreen: bool) {
        if fullscreen {
            let app = application();
            app.setPresentationOptions(native_fullscreen_options(app.presentationOptions()));
            self.active = true;
        } else if self.active {
            application().setPresentationOptions(self.normal_options);
            self.active = false;
        }
    }
}

fn application() -> objc2::rc::Retained<NSApplication> {
    let main_thread = MainThreadMarker::new()
        .expect("macOS presentation options must be updated on the main thread");
    NSApplication::sharedApplication(main_thread)
}

fn native_fullscreen_options(
    mut options: NSApplicationPresentationOptions,
) -> NSApplicationPresentationOptions {
    options.remove(
        NSApplicationPresentationOptions::HideDock | NSApplicationPresentationOptions::HideMenuBar,
    );
    options.insert(
        NSApplicationPresentationOptions::AutoHideDock
            | NSApplicationPresentationOptions::AutoHideMenuBar,
    );
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_hidden_chrome_with_auto_hidden_chrome() {
        let original = NSApplicationPresentationOptions::FullScreen
            | NSApplicationPresentationOptions::HideDock
            | NSApplicationPresentationOptions::HideMenuBar
            | NSApplicationPresentationOptions::DisableAppleMenu;

        let updated = native_fullscreen_options(original);

        assert!(updated.contains(NSApplicationPresentationOptions::FullScreen));
        assert!(updated.contains(NSApplicationPresentationOptions::AutoHideDock));
        assert!(updated.contains(NSApplicationPresentationOptions::AutoHideMenuBar));
        assert!(updated.contains(NSApplicationPresentationOptions::DisableAppleMenu));
        assert!(!updated.contains(NSApplicationPresentationOptions::HideDock));
        assert!(!updated.contains(NSApplicationPresentationOptions::HideMenuBar));
    }
}
