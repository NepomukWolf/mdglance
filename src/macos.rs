use std::ptr::NonNull;

use block2::RcBlock;
use objc2::{
    MainThreadMarker,
    rc::Retained,
    runtime::{AnyObject, NSObjectProtocol, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSApplication, NSApplicationPresentationOptions, NSMenu, NSMenuItem,
    NSWindowDidEnterFullScreenNotification, NSWindowDidExitFullScreenNotification,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, ns_string};
use tao::{platform::macos::WindowExtMacOS, window::Window};

/// Installs a minimal `Edit` menu so macOS routes Cmd+C/Cmd+A to the webview's
/// native `copy:`/`selectAll:` responder actions. Without any app menu at all,
/// those key equivalents have nowhere to go and are silently dropped.
pub fn install_edit_menu() {
    let app = application();
    let mtm = MainThreadMarker::new().expect("macOS menu setup must happen on the main thread");

    let copy_item = NSMenuItem::new(mtm);
    copy_item.setTitle(ns_string!("Copy"));
    copy_item.setKeyEquivalent(ns_string!("c"));
    unsafe { copy_item.setAction(Some(sel!(copy:))) };

    let select_all_item = NSMenuItem::new(mtm);
    select_all_item.setTitle(ns_string!("Select All"));
    select_all_item.setKeyEquivalent(ns_string!("a"));
    unsafe { select_all_item.setAction(Some(sel!(selectAll:))) };

    let edit_menu = NSMenu::new(mtm);
    edit_menu.setTitle(ns_string!("Edit"));
    edit_menu.addItem(&copy_item);
    edit_menu.addItem(&select_all_item);

    let edit_menu_item = NSMenuItem::new(mtm);
    edit_menu_item.setSubmenu(Some(&edit_menu));

    // AppKit always renames the title of the main menu's first item to the
    // running process's name, so a placeholder application-menu item is
    // needed here to keep that renaming from landing on the Edit menu.
    let app_menu_item = NSMenuItem::new(mtm);
    app_menu_item.setSubmenu(Some(&NSMenu::new(mtm)));

    let main_menu = NSMenu::new(mtm);
    main_menu.addItem(&app_menu_item);
    main_menu.addItem(&edit_menu_item);

    app.setMainMenu(Some(&main_menu));
}

/// Hides the app (equivalent to Cmd+H) rather than just the window. On macOS,
/// merely ordering a window out doesn't hand keyboard focus to anything else —
/// the app can stay the "active" one with no visible window to receive input.
/// Hiding the whole app deactivates it and returns focus to whatever was
/// active immediately before it (the terminal mdg was launched from).
pub fn hide_app() {
    application().hide(None);
}

/// Reverses [`hide_app`]: unhides and reactivates mdg so its window is both
/// visible and the one receiving keyboard input again.
pub fn unhide_and_activate_app() {
    let app = application();
    app.unhide(None);
    app.activate();
}

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
