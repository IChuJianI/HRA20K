//! The system input method client for the XComponent surface.
//!
//! ArkUI's triple key dispatch offers keys to the input method before the
//! focused component sees them, so a game that wants pinyin composition has to
//! register an editor with the input method framework itself. The public NDK
//! client (`libohinputmethod.so`) does exactly that: `Attach` binds an editor
//! proxy to the input method, `SetPreviewText` reports the composition and
//! `InsertText` reports the finished text.
//!
//! The engine already consumes winit's `Ime` events, so this module only
//! translates: preview text becomes `Ime::Preedit`, inserted text becomes
//! `Ime::Commit`, and the keys the input method takes over (enter, delete,
//! arrows) come back as ordinary key events so the game's own edit keeps
//! working. Text configuration is filled on the thread that calls `Attach`
//! (the engine thread), while the other callbacks run on the client's IPC
//! thread; both only publish through [`super::ohos::send_event`], which is
//! thread safe.
//!
//! The window id is required by the text configuration and only ArkTS can read
//! it, so the host publishes it through the same file the cursor uses.

use core::ffi::c_void;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::event::Ime;
use crate::keyboard::ModifiersState;
use crate::platform::ohos::{self, OhosEvent};

// InputMethod_TextInputType and InputMethod_EnterKeyType values.
const TEXT_INPUT_TYPE_TEXT: i32 = 0;
const ENTER_KEY_SEND: i32 = 4;
const KEYBOARD_STATUS_HIDE: i32 = 1;

const IME_ERR_OK: i32 = 0;

// XComponent key codes (native_xcomponent_key_event.h), the same numbering the
// window uses for hardware keys.
const KEY_ENTER: i32 = 2016;
const KEY_ARROW_UP: i32 = 2012;
const KEY_ARROW_DOWN: i32 = 2013;
const KEY_ARROW_LEFT: i32 = 2014;
const KEY_ARROW_RIGHT: i32 = 2015;
const KEY_BACKSPACE: i32 = 2055;
const KEY_DELETE: i32 = 2071;

#[link(name = "ohinputmethod")]
unsafe extern "C" {
    fn OH_InputMethodController_Attach(
        proxy: *mut c_void,
        options: *mut c_void,
        input_method_proxy: *mut *mut c_void,
    ) -> i32;
    fn OH_InputMethodController_Detach(input_method_proxy: *mut c_void) -> i32;

    fn OH_TextEditorProxy_Create() -> *mut c_void;
    fn OH_TextEditorProxy_Destroy(proxy: *mut c_void);
    fn OH_TextEditorProxy_SetGetTextConfigFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>) -> i32;
    fn OH_TextEditorProxy_SetInsertTextFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void, *const u16, usize)>) -> i32;
    fn OH_TextEditorProxy_SetDeleteForwardFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void, i32)>) -> i32;
    fn OH_TextEditorProxy_SetDeleteBackwardFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void, i32)>) -> i32;
    fn OH_TextEditorProxy_SetSendKeyboardStatusFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void, i32)>) -> i32;
    fn OH_TextEditorProxy_SetSendEnterKeyFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void, i32)>) -> i32;
    fn OH_TextEditorProxy_SetMoveCursorFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void, i32)>) -> i32;
    fn OH_TextEditorProxy_SetHandleSetSelectionFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void, i32, i32)>) -> i32;
    fn OH_TextEditorProxy_SetHandleExtendActionFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void, i32)>) -> i32;
    fn OH_TextEditorProxy_SetGetLeftTextOfCursorFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void, i32, *mut u16, *mut usize)>) -> i32;
    fn OH_TextEditorProxy_SetGetRightTextOfCursorFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void, i32, *mut u16, *mut usize)>) -> i32;
    fn OH_TextEditorProxy_SetGetTextIndexAtCursorFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void) -> i32>) -> i32;
    fn OH_TextEditorProxy_SetReceivePrivateCommandFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void, *mut *mut c_void, usize) -> i32>) -> i32;
    fn OH_TextEditorProxy_SetSetPreviewTextFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void, *const u16, usize, i32, i32) -> i32>) -> i32;
    fn OH_TextEditorProxy_SetFinishTextPreviewFunc(proxy: *mut c_void, func: Option<unsafe extern "C" fn(*mut c_void)>) -> i32;

    fn OH_AttachOptions_Create(show_keyboard: bool) -> *mut c_void;
    fn OH_AttachOptions_Destroy(options: *mut c_void);

    fn OH_TextConfig_Create() -> *mut c_void;
    fn OH_TextConfig_Destroy(config: *mut c_void);
    fn OH_TextConfig_SetInputType(config: *mut c_void, input_type: i32) -> i32;
    fn OH_TextConfig_SetEnterKeyType(config: *mut c_void, enter_key_type: i32) -> i32;
    fn OH_TextConfig_SetPreviewTextSupport(config: *mut c_void, supported: bool) -> i32;
    fn OH_TextConfig_SetSelection(config: *mut c_void, start: i32, end: i32) -> i32;
    fn OH_TextConfig_SetWindowId(config: *mut c_void, window_id: i32) -> i32;

    fn OH_InputMethodProxy_NotifyCursorUpdate(input_method_proxy: *mut c_void, cursor_info: *mut c_void) -> i32;

    fn OH_CursorInfo_Create(left: f64, top: f64, width: f64, height: f64) -> *mut c_void;
    fn OH_CursorInfo_Destroy(cursor_info: *mut c_void);
}

/// The main window id, published by ArkTS; `-1` until it arrives.
static WINDOW_ID: AtomicI32 = AtomicI32::new(-1);

#[derive(Default)]
struct State {
    allowed: bool,
    focused: bool,
    attached: bool,
    proxy: *mut c_void,
    options: *mut c_void,
    config: *mut c_void,
    input_method_proxy: *mut c_void,
    /// Cursor rectangle in window pixels.
    area: Option<(f64, f64, f64, f64)>,
}

unsafe impl Send for State {}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(State {
            // The game runs in the foreground; a focus loss detaches.
            focused: true,
            ..State::default()
        })
    })
}

/// ArkTS published the main window id.
pub fn set_window_id(window_id: i32) {
    WINDOW_ID.store(window_id, Ordering::SeqCst);
    let mut state = state().lock().unwrap();
    attach_if_wanted(&mut state);
}

pub(crate) fn set_allowed(allowed: bool) {
    let mut state = state().lock().unwrap();
    if state.allowed == allowed {
        return;
    }
    state.allowed = allowed;
    if allowed {
        attach_if_wanted(&mut state);
    } else {
        detach(&mut state);
    }
}

/// Keeps the client bound only while the window has focus, so switching away
/// never leaves this process holding the system input method.
pub(crate) fn set_focused(focused: bool) {
    let mut state = state().lock().unwrap();
    if state.focused == focused {
        return;
    }
    state.focused = focused;
    if focused {
        attach_if_wanted(&mut state);
    } else {
        detach(&mut state);
    }
}

/// The edit rectangle, in window pixels; the framework wants screen pixels,
/// and the game window is the full immersive screen.
pub(crate) fn set_cursor_area(x: f64, y: f64, width: f64, height: f64) {
    let mut state = state().lock().unwrap();
    state.area = Some((x, y, width, height));
    if state.attached {
        push_cursor(&state);
    }
}

fn attach_if_wanted(state: &mut State) {
    if state.attached || !state.allowed || !state.focused {
        return;
    }
    let window_id = WINDOW_ID.load(Ordering::SeqCst);
    if window_id < 0 {
        log::warn!("IME is wanted but the window id has not arrived yet");
        return;
    }

    // SAFETY: every pointer below is checked before use and released in the
    // same function on failure.
    unsafe {
        let proxy = OH_TextEditorProxy_Create();
        if proxy.is_null() {
            log::error!("OH_TextEditorProxy_Create returned null");
            return;
        }
        OH_TextEditorProxy_SetGetTextConfigFunc(proxy, Some(get_text_config));
        OH_TextEditorProxy_SetInsertTextFunc(proxy, Some(insert_text));
        OH_TextEditorProxy_SetDeleteForwardFunc(proxy, Some(delete_forward));
        OH_TextEditorProxy_SetDeleteBackwardFunc(proxy, Some(delete_backward));
        OH_TextEditorProxy_SetSendKeyboardStatusFunc(proxy, Some(send_keyboard_status));
        OH_TextEditorProxy_SetSendEnterKeyFunc(proxy, Some(send_enter_key));
        OH_TextEditorProxy_SetMoveCursorFunc(proxy, Some(move_cursor));
        OH_TextEditorProxy_SetHandleSetSelectionFunc(proxy, Some(set_selection));
        OH_TextEditorProxy_SetHandleExtendActionFunc(proxy, Some(extend_action));
        OH_TextEditorProxy_SetGetLeftTextOfCursorFunc(proxy, Some(get_left_text));
        OH_TextEditorProxy_SetGetRightTextOfCursorFunc(proxy, Some(get_right_text));
        OH_TextEditorProxy_SetGetTextIndexAtCursorFunc(proxy, Some(get_text_index));
        OH_TextEditorProxy_SetReceivePrivateCommandFunc(proxy, Some(receive_private_command));
        OH_TextEditorProxy_SetSetPreviewTextFunc(proxy, Some(set_preview_text));
        OH_TextEditorProxy_SetFinishTextPreviewFunc(proxy, Some(finish_text_preview));

        // A hardware keyboard is the normal case on a 2-in-1; never cover the
        // game with the on-screen keyboard.
        let options = OH_AttachOptions_Create(false);
        let config = OH_TextConfig_Create();
        if !config.is_null() {
            OH_TextConfig_SetInputType(config, TEXT_INPUT_TYPE_TEXT);
            OH_TextConfig_SetEnterKeyType(config, ENTER_KEY_SEND);
            OH_TextConfig_SetPreviewTextSupport(config, true);
            OH_TextConfig_SetSelection(config, 0, 0);
            OH_TextConfig_SetWindowId(config, window_id);
        }

        let mut input_method_proxy: *mut c_void = core::ptr::null_mut();
        let status = OH_InputMethodController_Attach(proxy, options, &mut input_method_proxy);
        if status == IME_ERR_OK && !input_method_proxy.is_null() {
            state.attached = true;
            state.proxy = proxy;
            state.options = options;
            state.config = config;
            state.input_method_proxy = input_method_proxy;
            log::info!("IME attached to window {window_id}");
            push_cursor(state);
        } else {
            log::error!("OH_InputMethodController_Attach failed with {status}");
            OH_TextEditorProxy_Destroy(proxy);
            OH_AttachOptions_Destroy(options);
            if !config.is_null() {
                OH_TextConfig_Destroy(config);
            }
        }
    }
}

fn detach(state: &mut State) {
    if !state.attached {
        return;
    }
    // SAFETY: the handles were produced by Attach and are released once.
    unsafe {
        let status = OH_InputMethodController_Detach(state.input_method_proxy);
        if status != IME_ERR_OK {
            log::warn!("OH_InputMethodController_Detach returned {status}");
        }
        OH_TextEditorProxy_Destroy(state.proxy);
        OH_AttachOptions_Destroy(state.options);
        if !state.config.is_null() {
            OH_TextConfig_Destroy(state.config);
        }
    }
    state.attached = false;
    state.proxy = core::ptr::null_mut();
    state.options = core::ptr::null_mut();
    state.config = core::ptr::null_mut();
    state.input_method_proxy = core::ptr::null_mut();
    log::info!("IME detached");
    // The open composition no longer has an owner.
    ohos::send_event(OhosEvent::Ime(Ime::Disabled));
}

fn push_cursor(state: &State) {
    let (Some((x, y, width, height)), true) = (state.area, state.input_method_proxy.is_null() == false) else {
        return;
    };
    // SAFETY: cursor info is created and destroyed around the notify call.
    unsafe {
        let info = OH_CursorInfo_Create(x, y, width, height);
        if info.is_null() {
            return;
        }
        OH_InputMethodProxy_NotifyCursorUpdate(state.input_method_proxy, info);
        OH_CursorInfo_Destroy(info);
    }
}

fn send_key(code: i32) {
    for pressed in [true, false] {
        ohos::send_event(OhosEvent::Key {
            code,
            state: if pressed {
                crate::event::ElementState::Pressed
            } else {
                crate::event::ElementState::Released
            },
            repeat: false,
            modifiers: ModifiersState::default(),
            caps_lock: false,
        });
    }
}

fn utf16_to_string(text: *const u16, length: usize) -> String {
    if text.is_null() || length == 0 {
        return String::new();
    }
    // SAFETY: the framework owns the buffer for the duration of this call.
    let units = unsafe { core::slice::from_raw_parts(text, length) };
    char::decode_utf16(units.iter().copied())
        .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

unsafe extern "C" fn get_text_config(_proxy: *mut c_void, config: *mut c_void) {
    // Runs on the thread that called Attach. Only atomics are read here so the
    // attach call can hold the client state.
    if config.is_null() {
        return;
    }
    let window_id = WINDOW_ID.load(Ordering::SeqCst);
    // SAFETY: `config` is a live TextConfig owned by the framework for the
    // duration of the callback.
    unsafe {
        OH_TextConfig_SetInputType(config, TEXT_INPUT_TYPE_TEXT);
        OH_TextConfig_SetEnterKeyType(config, ENTER_KEY_SEND);
        OH_TextConfig_SetPreviewTextSupport(config, true);
        OH_TextConfig_SetSelection(config, 0, 0);
        OH_TextConfig_SetWindowId(config, window_id);
    }
}

unsafe extern "C" fn insert_text(_proxy: *mut c_void, text: *const u16, length: usize) {
    let text = utf16_to_string(text, length);
    if !text.is_empty() {
        ohos::send_event(OhosEvent::Ime(Ime::Commit(text)));
    }
}

unsafe extern "C" fn set_preview_text(
    _proxy: *mut c_void,
    text: *const u16,
    length: usize,
    _start: i32,
    _end: i32,
) -> i32 {
    let text = utf16_to_string(text, length);
    let end = text.len();
    ohos::send_event(OhosEvent::Ime(Ime::Preedit(text, Some((end, end)))));
    0
}

unsafe extern "C" fn finish_text_preview(_proxy: *mut c_void) {
    ohos::send_event(OhosEvent::Ime(Ime::Preedit(String::new(), Some((0, 0)))));
}

unsafe extern "C" fn delete_backward(_proxy: *mut c_void, length: i32) {
    for _ in 0..length.clamp(1, 8) {
        send_key(KEY_BACKSPACE);
    }
}

unsafe extern "C" fn delete_forward(_proxy: *mut c_void, length: i32) {
    for _ in 0..length.clamp(1, 8) {
        send_key(KEY_DELETE);
    }
}

unsafe extern "C" fn send_enter_key(_proxy: *mut c_void, _enter_key_type: i32) {
    send_key(KEY_ENTER);
}

unsafe extern "C" fn move_cursor(_proxy: *mut c_void, direction: i32) {
    let code = match direction {
        1 => KEY_ARROW_UP,
        2 => KEY_ARROW_DOWN,
        3 => KEY_ARROW_LEFT,
        4 => KEY_ARROW_RIGHT,
        _ => return,
    };
    send_key(code);
}

unsafe extern "C" fn send_keyboard_status(_proxy: *mut c_void, status: i32) {
    if status == KEYBOARD_STATUS_HIDE {
        ohos::send_event(OhosEvent::Ime(Ime::Disabled));
    }
}

unsafe extern "C" fn set_selection(_proxy: *mut c_void, _start: i32, _end: i32) {}

unsafe extern "C" fn extend_action(_proxy: *mut c_void, _action: i32) {}

unsafe extern "C" fn get_left_text(
    _proxy: *mut c_void,
    _number: i32,
    _text: *mut u16,
    length: *mut usize,
) {
    if !length.is_null() {
        // SAFETY: the framework passes a writable out parameter.
        unsafe { *length = 0 };
    }
}

unsafe extern "C" fn get_right_text(
    _proxy: *mut c_void,
    _number: i32,
    _text: *mut u16,
    length: *mut usize,
) {
    if !length.is_null() {
        // SAFETY: the framework passes a writable out parameter.
        unsafe { *length = 0 };
    }
}

unsafe extern "C" fn get_text_index(_proxy: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn receive_private_command(
    _proxy: *mut c_void,
    _commands: *mut *mut c_void,
    _size: usize,
) -> i32 {
    0
}