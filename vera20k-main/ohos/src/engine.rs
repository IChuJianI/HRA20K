//! The VERA20K engine on the XComponent surface.
//!
//! The ArkUI UI thread hands surfaces and lifecycle events to the winit shim;
//! this module owns the game thread, the app data directory and the engine's
//! single event loop, in the same order the Android host uses: data directory,
//! start-up banner, loop, exit report.

use std::path::PathBuf;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use vera20k::app::frontend::launch::AppLaunchMode;
use vera20k::app::frontend::startup_options::RetailStartupOptions;

#[link(name = "ability_runtime")]
unsafe extern "C" {
    fn OH_AbilityRuntime_ApplicationContextGetFilesDir(
        buffer: *mut core::ffi::c_char,
        buffer_size: i32,
        write_length: *mut i32,
    ) -> i32;
}

static STARTED: AtomicBool = AtomicBool::new(false);

/// The app's private files directory (`context.filesDir` in ArkTS). Falls
/// back to the sandbox's known location when the runtime context is not
/// available yet.
pub(crate) fn files_dir() -> PathBuf {
    const BUFFER: usize = 4096;
    let mut buffer = [0u8; BUFFER];
    let mut written: i32 = 0;
    // SAFETY: the buffer is valid for BUFFER bytes, the runtime writes at most
    // that many, and both lengths are in range.
    let status = unsafe {
        OH_AbilityRuntime_ApplicationContextGetFilesDir(
            buffer.as_mut_ptr().cast(),
            BUFFER as i32,
            &mut written,
        )
    };
    if status != 0 || written <= 0 || written as usize > BUFFER {
        log::warn!(
            "OH_AbilityRuntime_ApplicationContextGetFilesDir returned {status}; using {}",
            crate::log::FILES_DIR
        );
        return PathBuf::from(crate::log::FILES_DIR);
    }
    let bytes = &buffer[..written as usize];
    let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
    match core::str::from_utf8(&bytes[..end]) {
        Ok(text) if !text.is_empty() => PathBuf::from(text),
        _ => PathBuf::from(crate::log::FILES_DIR),
    }
}

pub fn start() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let dir = files_dir();
    winit::platform::ohos::set_paste_zone_path(dir.join("vera20k.pastezone"));
    // A paste from the previous run must not fire at this run's dialog.
    let _ = std::fs::remove_file(dir.join("vera20k.pastecode"));
    // The sandbox has no LANG, so the page's locale list is the game's only
    // source for the system's language until the player picks one.
    vera20k::i18n::install_system_locales(system_locales);
    match thread::Builder::new()
        .name("vera20k-loop".to_owned())
        .spawn(run)
    {
        Ok(_) => log::info!("Started the VERA20K engine thread"),
        Err(error) => log::error!("Could not start the engine thread: {error}"),
    }
    match thread::Builder::new()
        .name("vera20k-windowid".to_owned())
        .spawn(watch_window_id)
    {
        Ok(_) => {}
        Err(error) => log::error!("Could not start the window id watcher: {error}"),
    }
    match thread::Builder::new()
        .name("vera20k-paste".to_owned())
        .spawn(watch_paste_requests)
    {
        Ok(_) => {}
        Err(error) => log::error!("Could not start the paste watcher: {error}"),
    }
}

/// ArkTS cannot hand the native game a value directly, so it publishes the
/// main window id in the data directory; the cursor APIs need that id to
/// place a custom pointer. Polls until the page has written it.
fn watch_window_id() {
    let path = files_dir().join("vera20k.windowid");
    for _ in 0..80 {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(window_id) = text.trim().parse::<i32>() {
                winit::platform::ohos::set_custom_cursor_window_id(window_id);
                winit::platform::ohos::set_ime_window_id(window_id);
                return;
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
    log::warn!("No window id in {}; custom cursors stay off", path.display());
}

/// The system's preferred language tags, published by the page before the
/// surface exists: one BCP 47 tag per line, most preferred first.
fn system_locales() -> Vec<String> {
    let path = files_dir().join("vera20k.locale");
    match std::fs::read_to_string(&path) {
        Ok(text) => text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// The page's PasteButton reads the clipboard on the player's tap, which the
/// engine itself cannot do without a permission it does not hold; the page
/// leaves the text in a file and the press is replayed on the Paste row, so
/// the dialog's own handler runs unchanged.
fn watch_paste_requests() {
    let path = files_dir().join("vera20k.pastecode");
    loop {
        if path.exists() {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let _ = std::fs::remove_file(&path);
            winit::platform::ohos::deliver_paste(text.trim().to_owned());
        }
        thread::sleep(Duration::from_millis(150));
    }
}

fn run() {
    let dir = files_dir();
    if let Err(error) = vera20k::entry::adopt_ohos_data_dir(&dir) {
        log::error!("cannot use the app data directory: {error:#}");
    }
    vera20k::entry::log_banner(None);

    let event_loop = match winit::event_loop::EventLoop::builder().build() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            log::error!("cannot create the OHOS event loop: {error}");
            return;
        }
    };
    // ArkUI has no retail command line: the page starts an ordinary
    // interactive game.
    let launch_mode = AppLaunchMode::Interactive(RetailStartupOptions::default());
    let status = vera20k::entry::run_with_loop(launch_mode, false, event_loop);
    match &status {
        Ok(0) => {}
        Ok(code) => log::warn!("VERA20K exited with status {code}"),
        Err(error) => log::error!("VERA20K stopped: {error:#}"),
    }
    log::logger().flush();
    // Only ArkUI can close the ability, so a clean exit hands the page this
    // signal; it calls terminateSelf and the desktop comes back.
    if matches!(status, Ok(0)) {
        if let Err(error) = std::fs::write(dir.join("vera20k.exit"), "1") {
            log::warn!("Could not publish the exit signal: {error}");
        }
    }
}