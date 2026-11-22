//! The bridge from XComponent callbacks to the game event loop.
//!
//! ArkUI calls the XComponent callbacks on the process's UI thread. They may
//! arrive before the game thread has started its loop, so the latest surface
//! state is also kept here and replayed when a loop installs its sink.

use std::sync::{Arc, Mutex, OnceLock};

use smol_str::SmolStr;

use crate::event::{ElementState, Ime, MouseButton};
use crate::keyboard::{
    Key, KeyCode, KeyEvent, KeyEventExtra, KeyLocation, ModifiersState, NamedKey, PhysicalKey,
};

/// The OHNativeWindow owned by the XComponent and its pixel size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OhosSurfaceInfo {
    pub native_window: usize,
    pub width: u32,
    pub height: u32,
}

/// Surface lifecycle and input events forwarded from the UI thread.
#[derive(Debug, Clone, PartialEq)]
pub enum OhosEvent {
    SurfaceCreated(OhosSurfaceInfo),
    SurfaceChanged(OhosSurfaceInfo),
    SurfaceDestroyed,
    CloseRequested,
    Focus(bool),
    Hover(bool),
    RedrawRequested,
    /// Pointer position in the surface's pixels.
    CursorMoved { x: f32, y: f32 },
    MouseButton { button: MouseButton, pressed: bool },
    MouseWheel { dx: f32, dy: f32 },
    /// A raw XComponent key event; [`key_event`] turns it into a `KeyEvent`.
    Key {
        code: i32,
        state: ElementState,
        repeat: bool,
        modifiers: ModifiersState,
        caps_lock: bool,
    },
    /// A notification from the system input method client.
    Ime(Ime),
}

type Sink = Arc<dyn Fn(OhosEvent) + Send + Sync>;

struct Bridge {
    latest_surface: Option<OhosSurfaceInfo>,
    sink: Option<Sink>,
}

fn bridge() -> &'static Mutex<Bridge> {
    static BRIDGE: OnceLock<Mutex<Bridge>> = OnceLock::new();
    BRIDGE.get_or_init(|| Mutex::new(Bridge { latest_surface: None, sink: None }))
}

/// Publishes the main window id ArkTS read from
/// `window.getWindowProperties().id`. Custom cursors requested before it
/// arrives are held and applied when it does.
pub fn set_custom_cursor_window_id(window_id: i32) {
    crate::platform::cursor::set_window_id(window_id);
}

/// Publishes the same window id to the input method client, whose text
/// configuration names the window the candidate UI should follow.
pub fn set_ime_window_id(window_id: i32) {
    crate::platform::ime::set_window_id(window_id);
}

/// Registers the file the page polls for the engine's Paste row.
pub fn set_paste_zone_path(path: std::path::PathBuf) {
    crate::platform::clipboard::set_paste_zone_path(path);
}

/// Publishes the engine's Paste row in surface pixels, or clears it.
pub fn set_paste_zone(zone: Option<[f32; 4]>) {
    crate::platform::clipboard::set_paste_zone(zone);
}

/// Hands the page's clipboard text to the engine and replays the row click.
pub fn deliver_paste(text: String) {
    crate::platform::clipboard::deliver_paste(text);
}

/// The clipboard text waiting for the engine's paste handler, taken once.
pub fn take_paste_text() -> Option<String> {
    crate::platform::clipboard::take_paste_text()
}

/// Puts `text` on the system pasteboard as plain text.
pub fn set_clipboard_text(text: &str) -> bool {
    crate::platform::clipboard::set_clipboard_text(text)
}

/// Records the surface the XComponent currently owns. Called on the UI thread.
pub fn set_latest_surface(surface: Option<OhosSurfaceInfo>) {
    bridge().lock().unwrap().latest_surface = surface;
}

pub fn latest_surface() -> Option<OhosSurfaceInfo> {
    bridge().lock().unwrap().latest_surface
}

/// Forwards an event to the running game loop, if there is one. The sink is
/// called without the bridge lock held because it reads the surface back.
pub fn send_event(event: OhosEvent) {
    let sink = bridge().lock().unwrap().sink.clone();
    if let Some(sink) = sink {
        sink(event);
    }
}

/// Installs the game loop's event sink. Replays the latest surface so a surface
/// created before the loop started is not lost.
pub(crate) fn install_sink(sink: Sink) {
    let latest = {
        let mut guard = bridge().lock().unwrap();
        guard.sink = Some(sink.clone());
        guard.latest_surface
    };
    if let Some(surface) = latest {
        sink(OhosEvent::SurfaceCreated(surface));
    }
}

pub(crate) fn uninstall_sink() {
    bridge().lock().unwrap().sink = None;
}

enum KeySpec {
    Character { physical: KeyCode, base: char, shifted: char, letters: bool },
    Named { physical: KeyCode, named: NamedKey, text: Option<&'static str> },
}

/// The XComponent key codes are OpenHarmony's own numbering, not Linux input
/// codes; these are the values from native_xcomponent_key_event.h.
fn key_spec(code: i32) -> Option<(KeySpec, KeyLocation)> {
    use KeyCode::*;
    let named = |physical: KeyCode, named: NamedKey| KeySpec::Named { physical, named, text: None };
    let modifiers = |physical: KeyCode, named: NamedKey| {
        (KeySpec::Named { physical, named, text: None }, KeyLocation::Standard)
    };
    let character = |physical: KeyCode, base: char, shifted: char| {
        KeySpec::Character { physical, base, shifted, letters: false }
    };
    let spec = match code {
        2000..=2009 => {
            const DIGITS: [KeyCode; 10] = [
                Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8,
                Digit9,
            ];
            const SHIFTED: [char; 10] = [')', '!', '@', '#', '$', '%', '^', '&', '*', '('];
            let index = (code - 2000) as usize;
            return Some((
                KeySpec::Character {
                    physical: DIGITS[index],
                    base: (b'0' + index as u8) as char,
                    shifted: SHIFTED[index],
                    letters: false,
                },
                KeyLocation::Standard,
            ));
        }
        2012 => named(ArrowUp, NamedKey::ArrowUp),
        2013 => named(ArrowDown, NamedKey::ArrowDown),
        2014 => named(ArrowLeft, NamedKey::ArrowLeft),
        2015 => named(ArrowRight, NamedKey::ArrowRight),
        2016 => named(Enter, NamedKey::Enter),
        2017..=2042 => {
            const LETTERS: [KeyCode; 26] = [
                KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM,
                KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
            ];
            let index = (code - 2017) as usize;
            let base = (b'a' + index as u8) as char;
            let shifted = (b'A' + index as u8) as char;
            return Some((
                KeySpec::Character { physical: LETTERS[index], base, shifted, letters: true },
                KeyLocation::Standard,
            ));
        }
        2043 => character(Comma, ',', '<'),
        2044 => character(Period, '.', '>'),
        2045 => return Some(modifiers(AltLeft, NamedKey::Alt)),
        2046 => return Some((KeySpec::Named { physical: AltRight, named: NamedKey::Alt, text: None }, KeyLocation::Right)),
        2047 => return Some((KeySpec::Named { physical: ShiftLeft, named: NamedKey::Shift, text: None }, KeyLocation::Left)),
        2048 => return Some((KeySpec::Named { physical: ShiftRight, named: NamedKey::Shift, text: None }, KeyLocation::Right)),
        2049 => named(Tab, NamedKey::Tab),
        2050 => KeySpec::Named { physical: Space, named: NamedKey::Space, text: Some(" ") },
        2054 => named(Enter, NamedKey::Enter),
        2055 => named(Backspace, NamedKey::Backspace),
        2056 => character(Backquote, '`', '~'),
        2057 => character(Minus, '-', '_'),
        2058 => character(Equal, '=', '+'),
        2059 => character(BracketLeft, '[', '{'),
        2060 => character(BracketRight, ']', '}'),
        2061 => character(Backslash, '\\', '|'),
        2062 => character(Semicolon, ';', ':'),
        2063 => character(Quote, '\'', '"'),
        2064 => character(Slash, '/', '?'),
        2067 => KeySpec::Named { physical: ContextMenu, named: NamedKey::ContextMenu, text: None },
        2068 => named(PageUp, NamedKey::PageUp),
        2069 => named(PageDown, NamedKey::PageDown),
        2070 => named(Escape, NamedKey::Escape),
        2071 => named(Delete, NamedKey::Delete),
        2072 => return Some((KeySpec::Named { physical: ControlLeft, named: NamedKey::Control, text: None }, KeyLocation::Left)),
        2073 => return Some((KeySpec::Named { physical: ControlRight, named: NamedKey::Control, text: None }, KeyLocation::Right)),
        2074 => named(CapsLock, NamedKey::CapsLock),
        2075 => named(ScrollLock, NamedKey::ScrollLock),
        2076 => return Some((KeySpec::Named { physical: SuperLeft, named: NamedKey::Super, text: None }, KeyLocation::Left)),
        2077 => return Some((KeySpec::Named { physical: SuperRight, named: NamedKey::Super, text: None }, KeyLocation::Right)),
        2078 => named(Fn, NamedKey::Fn),
        2079 => named(PrintScreen, NamedKey::PrintScreen),
        2080 => named(Pause, NamedKey::Pause),
        2081 => named(Home, NamedKey::Home),
        2082 => named(End, NamedKey::End),
        2083 => named(Insert, NamedKey::Insert),
        2090..=2101 => {
            const FUNCTION: [KeyCode; 12] = [
                F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
            ];
            const FUNCTION_NAMES: [NamedKey; 12] = [
                NamedKey::F1, NamedKey::F2, NamedKey::F3, NamedKey::F4, NamedKey::F5,
                NamedKey::F6, NamedKey::F7, NamedKey::F8, NamedKey::F9, NamedKey::F10,
                NamedKey::F11, NamedKey::F12,
            ];
            let index = (code - 2090) as usize;
            return Some((
                KeySpec::Named { physical: FUNCTION[index], named: FUNCTION_NAMES[index], text: None },
                KeyLocation::Standard,
            ));
        }
        2102 => named(NumLock, NamedKey::NumLock),
        2103..=2112 => {
            const NUMPAD: [KeyCode; 10] = [
                Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7,
                Numpad8, Numpad9,
            ];
            let index = (code - 2103) as usize;
            let digit = (b'0' + index as u8) as char;
            return Some((
                KeySpec::Character { physical: NUMPAD[index], base: digit, shifted: digit, letters: false },
                KeyLocation::Numpad,
            ));
        }
        2113 => return Some((KeySpec::Character { physical: NumpadDivide, base: '/', shifted: '/', letters: false }, KeyLocation::Numpad)),
        2114 => return Some((KeySpec::Character { physical: NumpadMultiply, base: '*', shifted: '*', letters: false }, KeyLocation::Numpad)),
        2115 => return Some((KeySpec::Character { physical: NumpadSubtract, base: '-', shifted: '-', letters: false }, KeyLocation::Numpad)),
        2116 => return Some((KeySpec::Character { physical: NumpadAdd, base: '+', shifted: '+', letters: false }, KeyLocation::Numpad)),
        2117 => return Some((KeySpec::Character { physical: NumpadDecimal, base: '.', shifted: '.', letters: false }, KeyLocation::Numpad)),
        2118 => return Some((KeySpec::Character { physical: NumpadComma, base: ',', shifted: ',', letters: false }, KeyLocation::Numpad)),
        2119 => return Some((KeySpec::Named { physical: NumpadEnter, named: NamedKey::Enter, text: None }, KeyLocation::Numpad)),
        2120 => return Some((KeySpec::Character { physical: NumpadEqual, base: '=', shifted: '=', letters: false }, KeyLocation::Numpad)),
        _ => return None,
    };
    Some((spec, KeyLocation::Standard))
}

/// Build the winit key event for a raw XComponent key. `modifiers` and
/// `caps_lock` come from the platform event when it reported them.
pub(crate) fn key_event(
    code: i32,
    state: ElementState,
    repeat: bool,
    modifiers: ModifiersState,
    caps_lock: bool,
) -> Option<KeyEvent> {
    let (spec, location) = key_spec(code)?;
    let (physical, logical_key, text, without_modifiers) = match spec {
        KeySpec::Character { physical, base, shifted, letters } => {
            let shifted_active = if letters { modifiers.shift_key() ^ caps_lock } else { modifiers.shift_key() };
            let base_text = SmolStr::new(base.to_string());
            let active = if shifted_active { SmolStr::new(shifted.to_string()) } else { base_text.clone() };
            let text = if modifiers.control_key() || modifiers.alt_key() {
                None
            } else {
                Some(active.clone())
            };
            let without = if shifted_active { Key::Character(base_text) } else { Key::Character(active.clone()) };
            (physical, Key::Character(active), text, without)
        }
        KeySpec::Named { physical, named, text } => {
            let text = text.map(SmolStr::new);
            (physical, Key::Named(named), text.clone(), Key::Named(named))
        }
    };
    Some(KeyEvent {
        physical_key: PhysicalKey::Code(physical),
        logical_key,
        text: text.clone(),
        location,
        state,
        repeat,
        platform_specific: KeyEventExtra {
            text_with_all_modifiers: text,
            key_without_modifiers: Some(Box::new(without_modifiers)),
        },
    })
}
