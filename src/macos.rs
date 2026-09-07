use std::ptr::NonNull;

use block2::RcBlock;
use objc2::{
    MainThreadMarker,
    rc::Retained,
    runtime::{AnyObject, NSObjectProtocol, ProtocolObject},
};
use objc2_app_kit::{
    NSApplication, NSApplicationPresentationOptions, NSWindowDidEnterFullScreenNotification,
    NSWindowDidExitFullScreenNotification,
};
use objc2_foundation::{NSNotification, NSNotificationCenter};
use tao::{platform::macos::WindowExtMacOS, window::Window};

pub struct FullscreenPresentation {
    normal_options: NSApplicationPresentationOptions,
    observers: Option<FullscreenObservers>,
}

struct FullscreenObservers {
    center: Retained<NSNotificationCenter>,
    enter: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    exit: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl FullscreenPresentation {
    pub fn new() -> Self {
        Self {
            normal_options: application().presentationOptions(),
            observers: None,
        }
    }

    pub fn observe(mut self, window: &Window) -> Self {
        let center = NSNotificationCenter::defaultCenter();
        let ns_window = unsafe { &*window.ns_window().cast::<AnyObject>() };
        let enter_block = RcBlock::new(|_: NonNull<NSNotification>| {
            let app = application();
            app.setPresentationOptions(native_fullscreen_options(app.presentationOptions()));
        });
        let normal_options = self.normal_options;
        let exit_block = RcBlock::new(move |_: NonNull<NSNotification>| {
            application().setPresentationOptions(normal_options);
        });

        let enter = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidEnterFullScreenNotification),
                Some(ns_window),
                None,
                &enter_block,
            )
        };
        let exit = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidExitFullScreenNotification),
                Some(ns_window),
                None,
                &exit_block,
            )
        };
        self.observers = Some(FullscreenObservers {
            center,
            enter,
            exit,
        });
        self
    }
}

impl Drop for FullscreenObservers {
    fn drop(&mut self) {
        let enter: &AnyObject = AsRef::<AnyObject>::as_ref(&*self.enter);
        let exit: &AnyObject = AsRef::<AnyObject>::as_ref(&*self.exit);
        unsafe {
            self.center.removeObserver(enter);
            self.center.removeObserver(exit);
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
