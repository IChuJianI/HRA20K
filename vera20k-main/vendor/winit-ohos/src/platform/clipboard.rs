//! The system pasteboard and the ArkUI paste overlay bridge.
//!
//! Writing is unrestricted: `OH_Pasteboard_SetData` takes a UDMF plain-text
//! record. Reading is not: since API 12 the system requires
//! `ohos.permission.READ_PASTEBOARD`, a restricted permission this app's
//! profile does not carry. The page therefore puts a system `PasteButton`
//! security component over the engine's Paste row; a tap grants temporary
//! read access, the page writes the text to `vera20k.pastecode`, the host
//! watcher hands it here, and this module replays the row's click so the
//! engine's own paste path runs untouched.

use core::ffi::{c_char, c_int, c_void};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::event::MouseButton;
use crate::platform::ohos::{self, OhosEvent};

#[link(name = "pasteboard")]
unsafe extern "C" {
    fn OH_Pasteboard_Create() -> *mut c_void;
    fn OH_Pasteboard_Destroy(pasteboard: *mut c_void);
    fn OH_Pasteboard_SetData(pasteboard: *mut c_void, data: *mut c_void) -> c_int;
}

#[link(name = "udmf")]
unsafe extern "C" {
    fn OH_UdmfData_Create() -> *mut c_void;
    fn OH_UdmfData_Destroy(data: *mut c_void);
    fn OH_UdmfData_AddRecord(data: *mut c_void, record: *mut c_void) -> c_int;
    fn OH_UdmfRecord_Create() -> *mut c_void;
    fn OH_UdmfRecord_Destroy(record: *mut c_void);
    fn OH_UdmfRecord_AddPlainText(record: *mut c_void, plain_text: *mut c_void) -> c_int;
    fn OH_UdsPlainText_Create() -> *mut c_void;
    fn OH_UdsPlainText_Destroy(plain_text: *mut c_void);
    fn OH_UdsPlainText_SetContent(plain_text: *mut c_void, content: *const c_char) -> c_int;
}

struct Bridge {
    /// The engine's Paste row in surface pixels, while its Extras page shows.
    zone: Option<[f32; 4]>,
    /// Where the page reads the row from; registered by the host at start-up.
    zone_path: Option<PathBuf>,
    /// Clipboard text the page read through the PasteButton.
    pending: Option<String>,
}

fn bridge() -> &'static Mutex<Bridge> {
    static BRIDGE: OnceLock<Mutex<Bridge>> = OnceLock::new();
    BRIDGE.get_or_init(|| {
        Mutex::new(Bridge {
            zone: None,
            zone_path: None,
            pending: None,
        })
    })
}

/// Registers the file the page polls for the Paste row. Drops a stale zone
/// from the previous run.
pub(crate) fn set_paste_zone_path(path: PathBuf) {
    let mut bridge = bridge().lock().unwrap();
    let _ = std::fs::remove_file(&path);
    bridge.zone = None;
    bridge.zone_path = Some(path);
}

/// Publishes the Paste row, or clears it when the row is off screen. Runs on
/// the game loop every turn; the file is only touched when the rect changes.
pub(crate) fn set_paste_zone(zone: Option<[f32; 4]>) {
    let mut bridge = bridge().lock().unwrap();
    if bridge.zone == zone {
        return;
    }
    bridge.zone = zone;
    let Some(path) = bridge.zone_path.clone() else {
        return;
    };
    match zone {
        Some([x, y, w, h]) => {
            if let Err(error) = std::fs::write(&path, format!("{x} {y} {w} {h}")) {
                log::warn!("Could not publish the paste row: {error}");
            }
        }
        None => {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Hands over text the page read through the PasteButton and replays the
/// click on the Paste row, so the engine adopts the code through its normal
/// path. Dropped when the row is not on screen.
pub(crate) fn deliver_paste(text: String) {
    let zone = {
        let mut bridge = bridge().lock().unwrap();
        let Some(zone) = bridge.zone else {
            log::warn!("Dropped a paste: the Paste row is not on screen");
            return;
        };
        bridge.pending = Some(text);
        zone
    };
    let x = zone[0] + zone[2] / 2.0;
    let y = zone[1] + zone[3] / 2.0;
    ohos::send_event(OhosEvent::CursorMoved { x, y });
    ohos::send_event(OhosEvent::MouseButton {
        button: MouseButton::Left,
        pressed: true,
    });
    ohos::send_event(OhosEvent::MouseButton {
        button: MouseButton::Left,
        pressed: false,
    });
}

/// The clipboard text waiting for the engine's paste handler, taken once.
pub(crate) fn take_paste_text() -> Option<String> {
    bridge().lock().unwrap().pending.take()
}

/// Puts `text` on the system pasteboard as plain text.
pub(crate) fn set_clipboard_text(text: &str) -> bool {
    let Ok(text) = std::ffi::CString::new(text) else {
        return false;
    };
    // SAFETY: every object is created and destroyed here, and the strings
    // outlive the calls that borrow them.
    unsafe {
        let pasteboard = OH_Pasteboard_Create();
        if pasteboard.is_null() {
            log::warn!("OH_Pasteboard_Create returned null");
            return false;
        }
        let data = OH_UdmfData_Create();
        let record = OH_UdmfRecord_Create();
        let plain = OH_UdsPlainText_Create();
        let built = !data.is_null()
            && !record.is_null()
            && !plain.is_null()
            && OH_UdsPlainText_SetContent(plain, text.as_ptr()) == 0
            && OH_UdmfRecord_AddPlainText(record, plain) == 0
            && OH_UdmfData_AddRecord(data, record) == 0;
        let status = if built {
            OH_Pasteboard_SetData(pasteboard, data)
        } else {
            -1
        };
        if !plain.is_null() {
            OH_UdsPlainText_Destroy(plain);
        }
        if !record.is_null() {
            OH_UdmfRecord_Destroy(record);
        }
        if !data.is_null() {
            OH_UdmfData_Destroy(data);
        }
        OH_Pasteboard_Destroy(pasteboard);
        if status != 0 {
            log::warn!("The pasteboard refused the text ({status})");
        }
        status == 0
    }
}