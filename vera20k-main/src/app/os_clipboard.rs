//! The system clipboard, for VERA's random-map share codes.
//!
//! Desktop builds reach it through the `arboard` crate: the Win32 clipboard
//! on Windows, NSPasteboard on macOS and the X11 selection on Linux (Wayland
//! sessions through XWayland). One handle lives as long as the interface
//! thread, because an X11 selection is served by the program that set it and
//! ends with its handle. A clipboard that cannot be opened or holds no text is
//! a `false` or a `None`, which the dialog reports; nothing here can stop the
//! game. Other builds (the browser, Android) have no clipboard.
//!
//! HarmonyOS writes through the system pasteboard. Reads there need the
//! restricted `READ_PASTEBOARD` permission, so the page owns them: it lays a
//! system PasteButton over the dialog's Paste row while [`update_paste_zone`]
//! publishes that row, and the text it reads arrives in [`platform::text`].

/// Put `text` on the clipboard; false when the clipboard refused it.
pub(crate) fn set_text(text: &str) -> bool {
    platform::set_text(text)
}

/// The clipboard's text, trimmed; `None` when it holds none.
pub(crate) fn text() -> Option<String> {
    platform::text().map(|text| text.trim().to_string())
}

/// Publishes the random-map Extras view's Paste row while it is on screen, so
/// the page can cover it with a system PasteButton. No-op off HarmonyOS.
pub(crate) fn update_paste_zone(state: &crate::app::state::AppState) {
    #[cfg(target_env = "ohos")]
    {
        winit::platform::ohos::set_paste_zone(paste_zone(state));
    }
    #[cfg(not(target_env = "ohos"))]
    let _ = state;
}

/// The Paste row's rect in surface pixels, when the dialog shows its Extras
/// page and no other box covers it.
#[cfg(target_env = "ohos")]
fn paste_zone(state: &crate::app::state::AppState) -> Option<[f32; 4]> {
    use crate::app::shell_route::ShellRoute;

    if !matches!(
        state.frontend.shell_route,
        ShellRoute::Skirmish | ShellRoute::NetworkSetup
    ) {
        return None;
    }
    let shell = &state.frontend.skirmish_shell_state;
    let modal = shell.random_map_setup_modal.as_ref()?;
    if modal.extras_page != Some(0)
        || shell.saved_seed_browser.is_some()
        || shell.setups_browser.is_some()
        || shell.validation_modal.is_some()
        || shell.game_options.is_some()
    {
        return None;
    }
    // The Extras view reuses the Advanced sliders' rows; Paste is row 1.
    let layout = crate::app::frontend::shell_layout::random_map_setup_layout(state);
    let rect = layout.advanced_control_rects[1];
    Some([rect.x as f32, rect.y as f32, rect.w as f32, rect.h as f32])
}

#[cfg(target_env = "ohos")]
mod platform {
    pub(super) fn set_text(text: &str) -> bool {
        winit::platform::ohos::set_clipboard_text(text)
    }

    pub(super) fn text() -> Option<String> {
        winit::platform::ohos::take_paste_text()
    }
}

#[cfg(all(
    not(target_env = "ohos"),
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
mod platform {
    use std::cell::RefCell;

    thread_local! {
        /// Opened on first use and kept, so a selection outlives the call.
        static CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
    }

    /// Run `act` on the clipboard, opening it first if need be.
    fn with_clipboard<T>(act: impl FnOnce(&mut arboard::Clipboard) -> Option<T>) -> Option<T> {
        CLIPBOARD.with(|cell| {
            let mut handle = cell.try_borrow_mut().ok()?;
            if handle.is_none() {
                match arboard::Clipboard::new() {
                    Ok(clipboard) => *handle = Some(clipboard),
                    Err(error) => {
                        log::warn!("The clipboard could not be opened: {error}");
                        return None;
                    }
                }
            }
            act(handle.as_mut()?)
        })
    }

    pub(super) fn set_text(text: &str) -> bool {
        with_clipboard(|clipboard| clipboard.set_text(text).ok()).is_some()
    }

    pub(super) fn text() -> Option<String> {
        with_clipboard(|clipboard| clipboard.get_text().ok())
    }
}

#[cfg(not(any(
    target_env = "ohos",
    target_os = "windows",
    target_os = "macos",
    target_os = "linux"
)))]
mod platform {
    pub(super) fn set_text(_text: &str) -> bool {
        false
    }

    pub(super) fn text() -> Option<String> {
        None
    }
}