//! XComponent lifecycle callbacks, following the official OpenHarmony flow:
//! ArkTS declares `libraryname`, the module Init unwraps the native XComponent,
//! and the window passed to on_surface_created is handed to the game's winit
//! shim, which builds the wgpu/Vulkan surface from it.

use core::ffi::c_void;
use std::collections::HashSet;
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use winit::event::{ElementState, MouseButton};
use winit::keyboard::ModifiersState;
use winit::platform::ohos::{self, OhosEvent, OhosSurfaceInfo};

#[repr(C)]
pub struct OH_NativeXComponent {
    _private: [u8; 0],
}

#[repr(C)]
struct OH_NativeXComponent_KeyEvent {
    _private: [u8; 0],
}

#[repr(C)]
struct ArkUI_UIInputEvent {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OH_NativeXComponent_Callback {
    pub on_surface_created: Option<unsafe extern "C" fn(*mut OH_NativeXComponent, *mut c_void)>,
    pub on_surface_changed: Option<unsafe extern "C" fn(*mut OH_NativeXComponent, *mut c_void)>,
    pub on_surface_destroyed: Option<unsafe extern "C" fn(*mut OH_NativeXComponent, *mut c_void)>,
    pub dispatch_touch_event: Option<unsafe extern "C" fn(*mut OH_NativeXComponent, *mut c_void)>,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct OH_NativeXComponent_MouseEvent {
    x: f32,
    y: f32,
    screen_x: f32,
    screen_y: f32,
    timestamp: i64,
    action: i32,
    button: i32,
}

impl OH_NativeXComponent_MouseEvent {
    fn zeroed() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            screen_x: 0.0,
            screen_y: 0.0,
            timestamp: 0,
            action: 0,
            button: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct OH_NativeXComponent_MouseEvent_Callback {
    dispatch_mouse_event: Option<unsafe extern "C" fn(*mut OH_NativeXComponent, *mut c_void)>,
    dispatch_hover_event: Option<unsafe extern "C" fn(*mut OH_NativeXComponent, bool)>,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct OH_NativeXComponent_TouchPoint {
    id: i32,
    screen_x: f32,
    screen_y: f32,
    x: f32,
    y: f32,
    r#type: i32,
    size: f64,
    force: f32,
    timestamp: i64,
    is_pressed: bool,
}

impl OH_NativeXComponent_TouchPoint {
    fn zeroed() -> Self {
        Self {
            id: 0,
            screen_x: 0.0,
            screen_y: 0.0,
            x: 0.0,
            y: 0.0,
            r#type: 0,
            size: 0.0,
            force: 0.0,
            timestamp: 0,
            is_pressed: false,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct OH_NativeXComponent_TouchEvent {
    id: i32,
    screen_x: f32,
    screen_y: f32,
    x: f32,
    y: f32,
    r#type: i32,
    size: f64,
    force: f32,
    device_id: i64,
    timestamp: i64,
    touch_points: [OH_NativeXComponent_TouchPoint; 10],
    num_points: u32,
}

impl OH_NativeXComponent_TouchEvent {
    fn zeroed() -> Self {
        Self {
            id: 0,
            screen_x: 0.0,
            screen_y: 0.0,
            x: 0.0,
            y: 0.0,
            r#type: 0,
            size: 0.0,
            force: 0.0,
            device_id: 0,
            timestamp: 0,
            touch_points: [OH_NativeXComponent_TouchPoint::zeroed(); 10],
            num_points: 0,
        }
    }
}

const MOUSE_ACTION_PRESS: i32 = 1;
const MOUSE_ACTION_RELEASE: i32 = 2;
const MOUSE_ACTION_MOVE: i32 = 3;
/// The platform aborts an in-flight gesture on focus loss, app switching or
/// a system pop-up; any button the presses left down must come up.
const MOUSE_ACTION_CANCEL: i32 = 4;

const TOUCH_DOWN: i32 = 0;
const TOUCH_UP: i32 = 1;
const TOUCH_MOVE: i32 = 2;
const TOUCH_CANCEL: i32 = 3;

const KEY_ACTION_DOWN: i32 = 0;
const KEY_ACTION_UP: i32 = 1;

const AXIS_EVENT_UPDATE: i32 = 2;
const UI_INPUT_EVENT_TYPE_AXIS: i32 = 2;

#[link(name = "ace_ndk.z")]
unsafe extern "C" {
    fn OH_NativeXComponent_RegisterCallback(
        component: *mut OH_NativeXComponent,
        callback: *const OH_NativeXComponent_Callback,
    ) -> i32;
    fn OH_NativeXComponent_GetXComponentSize(
        component: *mut OH_NativeXComponent,
        window: *const c_void,
        width: *mut u64,
        height: *mut u64,
    ) -> i32;
    fn OH_NativeXComponent_RegisterMouseEventCallback(
        component: *mut OH_NativeXComponent,
        callback: *const OH_NativeXComponent_MouseEvent_Callback,
    ) -> i32;
    fn OH_NativeXComponent_GetMouseEvent(
        component: *mut OH_NativeXComponent,
        window: *const c_void,
        mouse_event: *mut OH_NativeXComponent_MouseEvent,
    ) -> i32;
    fn OH_NativeXComponent_GetTouchEvent(
        component: *mut OH_NativeXComponent,
        window: *const c_void,
        touch_event: *mut OH_NativeXComponent_TouchEvent,
    ) -> i32;
    fn OH_NativeXComponent_RegisterKeyEventCallback(
        component: *mut OH_NativeXComponent,
        callback: Option<unsafe extern "C" fn(*mut OH_NativeXComponent, *mut c_void)>,
    ) -> i32;
    fn OH_NativeXComponent_GetKeyEvent(
        component: *mut OH_NativeXComponent,
        key_event: *mut *mut OH_NativeXComponent_KeyEvent,
    ) -> i32;
    fn OH_NativeXComponent_GetKeyEventAction(
        key_event: *mut OH_NativeXComponent_KeyEvent,
        action: *mut i32,
    ) -> i32;
    fn OH_NativeXComponent_GetKeyEventCode(
        key_event: *mut OH_NativeXComponent_KeyEvent,
        code: *mut i32,
    ) -> i32;
    fn OH_NativeXComponent_GetKeyEventModifierKeyStates(
        key_event: *mut OH_NativeXComponent_KeyEvent,
        keys: *mut u64,
    ) -> i32;
    fn OH_NativeXComponent_GetKeyEventCapsLockState(
        key_event: *mut OH_NativeXComponent_KeyEvent,
        is_caps_lock_on: *mut bool,
    ) -> i32;
    fn OH_NativeXComponent_RegisterFocusEventCallback(
        component: *mut OH_NativeXComponent,
        callback: Option<unsafe extern "C" fn(*mut OH_NativeXComponent, *mut c_void)>,
    ) -> i32;
    fn OH_NativeXComponent_RegisterBlurEventCallback(
        component: *mut OH_NativeXComponent,
        callback: Option<unsafe extern "C" fn(*mut OH_NativeXComponent, *mut c_void)>,
    ) -> i32;
    fn OH_NativeXComponent_RegisterUIInputEventCallback(
        component: *mut OH_NativeXComponent,
        callback: Option<
            unsafe extern "C" fn(*mut OH_NativeXComponent, *mut ArkUI_UIInputEvent, i32),
        >,
        event_type: i32,
    ) -> i32;
    fn OH_ArkUI_UIInputEvent_GetAction(event: *const ArkUI_UIInputEvent) -> i32;
    fn OH_ArkUI_AxisEvent_GetVerticalAxisValue(event: *const ArkUI_UIInputEvent) -> f64;
    fn OH_ArkUI_AxisEvent_GetScrollStep(event: *const ArkUI_UIInputEvent) -> i32;
}

static CALLBACK: OH_NativeXComponent_Callback = OH_NativeXComponent_Callback {
    on_surface_created: Some(on_surface_created),
    on_surface_changed: Some(on_surface_changed),
    on_surface_destroyed: Some(on_surface_destroyed),
    dispatch_touch_event: Some(on_touch_event),
};

static MOUSE_CALLBACK: OH_NativeXComponent_MouseEvent_Callback = OH_NativeXComponent_MouseEvent_Callback {
    dispatch_mouse_event: Some(on_mouse_event),
    dispatch_hover_event: Some(on_hover_event),
};
static FIRST_MOUSE_EVENT: AtomicBool = AtomicBool::new(true);
static FIRST_TOUCH_EVENT: AtomicBool = AtomicBool::new(true);
static TOUCH_PRESSED: AtomicBool = AtomicBool::new(false);
static FIRST_KEY_EVENT: AtomicBool = AtomicBool::new(true);
static PRESSED_BUTTONS: AtomicU8 = AtomicU8::new(0);
static FOCUSED: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct KeyState {
    pressed: HashSet<i32>,
    modifiers: ModifiersState,
    caps_lock: bool,
}

fn key_state() -> &'static Mutex<KeyState> {
    static STATE: OnceLock<Mutex<KeyState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(KeyState::default()))
}

/// Binds to the component ArkUI created for this library and starts waiting for
/// its surface. Runs on the ArkUI UI thread during module initialization.
pub fn attach(component: *mut OH_NativeXComponent) {
    if component.is_null() {
        log::error!("XComponent attach received a null component");
        return;
    }
    // SAFETY: CALLBACK is a file-static with a stable address, and the native
    // side copies it during registration.
    let status = unsafe { OH_NativeXComponent_RegisterCallback(component, &raw const CALLBACK) };
    if status != 0 {
        log::error!("OH_NativeXComponent_RegisterCallback failed with status {status}");
    } else {
        log::info!("XComponent callbacks registered");
    }
    let status = unsafe { OH_NativeXComponent_RegisterMouseEventCallback(component, &raw const MOUSE_CALLBACK) };
    if status != 0 {
        log::error!("OH_NativeXComponent_RegisterMouseEventCallback failed with status {status}");
    }
    let status = unsafe { OH_NativeXComponent_RegisterKeyEventCallback(component, Some(on_key_event)) };
    if status != 0 {
        log::error!("OH_NativeXComponent_RegisterKeyEventCallback failed with status {status}");
    }
    let status = unsafe { OH_NativeXComponent_RegisterFocusEventCallback(component, Some(on_focus)) };
    if status != 0 {
        log::error!("OH_NativeXComponent_RegisterFocusEventCallback failed with status {status}");
    }
    let status = unsafe { OH_NativeXComponent_RegisterBlurEventCallback(component, Some(on_blur)) };
    if status != 0 {
        log::error!("OH_NativeXComponent_RegisterBlurEventCallback failed with status {status}");
    }
    let status = unsafe {
        OH_NativeXComponent_RegisterUIInputEventCallback(
            component,
            Some(on_axis_event),
            UI_INPUT_EVENT_TYPE_AXIS,
        )
    };
    if status != 0 {
        log::error!("OH_NativeXComponent_RegisterUIInputEventCallback failed with status {status}");
    }
    crate::engine::start();
}

fn surface_info(component: *mut OH_NativeXComponent, window: *mut c_void) -> OhosSurfaceInfo {
    let mut width: u64 = 0;
    let mut height: u64 = 0;
    // SAFETY: both pointers come from the XComponent callback.
    let status = unsafe {
        OH_NativeXComponent_GetXComponentSize(component, window, &mut width, &mut height)
    };
    if status != 0 {
        log::warn!("OH_NativeXComponent_GetXComponentSize failed with status {status}");
    }
    OhosSurfaceInfo {
        native_window: window as usize,
        width: width.min(u32::MAX as u64) as u32,
        height: height.min(u32::MAX as u64) as u32,
    }
}

unsafe extern "C" fn on_surface_created(component: *mut OH_NativeXComponent, window: *mut c_void) {
    let info = surface_info(component, window);
    log::info!(
        "XComponent surface created at {}x{} window 0x{:x}",
        info.width,
        info.height,
        info.native_window
    );
    ohos::set_latest_surface(Some(info));
    ohos::send_event(OhosEvent::SurfaceCreated(info));
}

unsafe extern "C" fn on_surface_changed(component: *mut OH_NativeXComponent, window: *mut c_void) {
    let info = surface_info(component, window);
    log::info!(
        "XComponent surface changed to {}x{} window 0x{:x}",
        info.width,
        info.height,
        info.native_window
    );
    ohos::set_latest_surface(Some(info));
    ohos::send_event(OhosEvent::SurfaceChanged(info));
}

unsafe extern "C" fn on_surface_destroyed(_component: *mut OH_NativeXComponent, _window: *mut c_void) {
    log::info!("XComponent surface destroyed");
    ohos::set_latest_surface(None);
    ohos::send_event(OhosEvent::SurfaceDestroyed);
}

/// The five buttons as (mask bit, button) pairs; the mask lets a release that
/// arrives without its button still release what a press left down.
const BUTTONS: [(u8, MouseButton); 5] = [
    (0x01, MouseButton::Left),
    (0x02, MouseButton::Right),
    (0x04, MouseButton::Middle),
    (0x08, MouseButton::Back),
    (0x10, MouseButton::Forward),
];

fn mouse_button(button: i32) -> Option<MouseButton> {
    BUTTONS.iter().find(|(bit, _)| i32::from(*bit) == button).map(|(_, button)| *button)
}

fn button_bit(button: MouseButton) -> u8 {
    BUTTONS.iter().find(|(_, b)| *b == button).map_or(0, |(bit, _)| *bit)
}

fn release_pressed_buttons() {
    let mask = PRESSED_BUTTONS.swap(0, Ordering::Relaxed);
    for (bit, button) in BUTTONS {
        if mask & bit != 0 {
            ohos::send_event(OhosEvent::MouseButton { button, pressed: false });
        }
    }
}

/// A real input edge proves this surface is receiving input. Screenshot and
/// recorder overlays can blur the component without delivering the matching
/// focus event, leaving the engine's input gate closed for the rest of the
/// session; re-open it before the input that exposed the gap.
fn ensure_input_focus() {
    if !FOCUSED.swap(true, Ordering::Relaxed) {
        log::info!("input arrived without XComponent focus; re-asserting focus");
        ohos::send_event(OhosEvent::Focus(true));
    }
}

unsafe extern "C" fn on_mouse_event(component: *mut OH_NativeXComponent, window: *mut c_void) {
    let mut event = OH_NativeXComponent_MouseEvent::zeroed();
    // SAFETY: both handles come from the XComponent callback.
    let status = unsafe { OH_NativeXComponent_GetMouseEvent(component, window, &mut event) };
    if status != 0 {
        log::warn!("OH_NativeXComponent_GetMouseEvent failed with status {status}");
        return;
    }
    if FIRST_MOUSE_EVENT.swap(false, Ordering::Relaxed) {
        log::info!(
            "First mouse event at {}x{} px: action {} button {}",
            event.x,
            event.y,
            event.action,
            event.button
        );
    }
    // The XComponent's mouse callback reports surface pixels, not the vp the
    // header documents: measured against the surface size, dividing by the
    // display density put the game's cursor a density factor past the pointer.
    let x = event.x;
    let y = event.y;
    match event.action {
        MOUSE_ACTION_PRESS => {
            ensure_input_focus();
            ohos::send_event(OhosEvent::CursorMoved { x, y });
            if let Some(button) = mouse_button(event.button) {
                let bit = button_bit(button);
                if PRESSED_BUTTONS.fetch_or(bit, Ordering::Relaxed) & bit == 0 {
                    ohos::send_event(OhosEvent::MouseButton { button, pressed: true });
                }
            }
        }
        MOUSE_ACTION_RELEASE => {
            ohos::send_event(OhosEvent::CursorMoved { x, y });
            match mouse_button(event.button) {
                Some(button) => {
                    let bit = button_bit(button);
                    if PRESSED_BUTTONS.fetch_and(!bit, Ordering::Relaxed) & bit != 0 {
                        ohos::send_event(OhosEvent::MouseButton { button, pressed: false });
                    }
                }
                // A release without a button is the platform cancelling the
                // gesture; drop everything the presses left down.
                None => release_pressed_buttons(),
            }
        }
        MOUSE_ACTION_MOVE => ohos::send_event(OhosEvent::CursorMoved { x, y }),
        MOUSE_ACTION_CANCEL => {
            release_pressed_buttons();
            release_touch_left();
        }
        _ => {}
    }
}

unsafe extern "C" fn on_hover_event(_component: *mut OH_NativeXComponent, is_hover: bool) {
    ohos::send_event(OhosEvent::Hover(is_hover));
}

/// Touch has no native winit event on this platform, so a finger drives the
/// same synthetic pointer the mouse does: movement follows the contact, and a
/// press covers the contact from down to up. A tap therefore lands wherever
/// the finger lands, instead of ArkUI's default click in the middle.
unsafe extern "C" fn on_touch_event(component: *mut OH_NativeXComponent, window: *mut c_void) {
    let mut event = OH_NativeXComponent_TouchEvent::zeroed();
    // SAFETY: both handles come from the XComponent callback, and `event`
    // matches the SDK's OH_NativeXComponent_TouchEvent layout.
    let status = unsafe { OH_NativeXComponent_GetTouchEvent(component, window, &mut event) };
    if status != 0 {
        log::warn!("OH_NativeXComponent_GetTouchEvent failed with status {status}");
        return;
    }
    if FIRST_TOUCH_EVENT.swap(false, Ordering::Relaxed) {
        log::info!(
            "First touch event at {}x{} px: type {} points {}",
            event.x,
            event.y,
            event.r#type,
            event.num_points
        );
    }
    // Touch x/y share the surface-pixel space measured for the mouse callback.
    let x = event.x;
    let y = event.y;
    match event.r#type {
        TOUCH_DOWN => {
            ensure_input_focus();
            ohos::send_event(OhosEvent::CursorMoved { x, y });
            TOUCH_PRESSED.store(true, Ordering::Relaxed);
            let bit = button_bit(MouseButton::Left);
            if PRESSED_BUTTONS.fetch_or(bit, Ordering::Relaxed) & bit == 0 {
                ohos::send_event(OhosEvent::MouseButton { button: MouseButton::Left, pressed: true });
            }
        }
        TOUCH_MOVE => ohos::send_event(OhosEvent::CursorMoved { x, y }),
        TOUCH_UP => {
            ohos::send_event(OhosEvent::CursorMoved { x, y });
            release_touch_left();
        }
        TOUCH_CANCEL => release_touch_left(),
        _ => {}
    }
}

fn release_touch_left() {
    if !TOUCH_PRESSED.swap(false, Ordering::Relaxed) {
        return;
    }
    let bit = button_bit(MouseButton::Left);
    if PRESSED_BUTTONS.fetch_and(!bit, Ordering::Relaxed) & bit != 0 {
        ohos::send_event(OhosEvent::MouseButton { button: MouseButton::Left, pressed: false });
    }
}

unsafe extern "C" fn on_key_event(component: *mut OH_NativeXComponent, _window: *mut c_void) {
    let mut raw: *mut OH_NativeXComponent_KeyEvent = core::ptr::null_mut();
    // SAFETY: the key event belongs to ArkUI and is valid for this callback.
    let status = unsafe { OH_NativeXComponent_GetKeyEvent(component, &mut raw) };
    if status != 0 || raw.is_null() {
        log::warn!("OH_NativeXComponent_GetKeyEvent failed with status {status}");
        return;
    }
    let mut action = -1;
    let mut code = -1;
    let mut keys = 0u64;
    let mut caps_lock = false;
    let modifiers_known = unsafe {
        OH_NativeXComponent_GetKeyEventAction(raw, &mut action);
        OH_NativeXComponent_GetKeyEventCode(raw, &mut code);
        OH_NativeXComponent_GetKeyEventModifierKeyStates(raw, &mut keys) == 0
    };
    let caps_known = unsafe { OH_NativeXComponent_GetKeyEventCapsLockState(raw, &mut caps_lock) } == 0;
    let state = match action {
        KEY_ACTION_DOWN => ElementState::Pressed,
        KEY_ACTION_UP => ElementState::Released,
        _ => return,
    };
    if FIRST_KEY_EVENT.swap(false, Ordering::Relaxed) {
        log::info!("First key event: code {code} action {action}");
    }
    let pressed = state.is_pressed();
    if pressed {
        ensure_input_focus();
    }

    let mut guard = key_state().lock().unwrap();
    let repeat = if pressed { !guard.pressed.insert(code) } else { guard.pressed.remove(&code); false };
    // Key edges are also the fallback modifier source when an older platform
    // does not fill the event's modifier mask.
    match code {
        2047 | 2048 => guard.modifiers.set(ModifiersState::SHIFT, pressed),
        2072 | 2073 => guard.modifiers.set(ModifiersState::CONTROL, pressed),
        2045 | 2046 => guard.modifiers.set(ModifiersState::ALT, pressed),
        2076 | 2077 => guard.modifiers.set(ModifiersState::SUPER, pressed),
        2074 if pressed => guard.caps_lock = !guard.caps_lock,
        _ => {}
    }
    let mut modifiers = guard.modifiers;
    if modifiers_known {
        modifiers = ModifiersState::empty();
        modifiers.set(ModifiersState::CONTROL, keys & 0x1 != 0);
        modifiers.set(ModifiersState::SHIFT, keys & 0x2 != 0);
        modifiers.set(ModifiersState::ALT, keys & 0x4 != 0);
        guard.modifiers = modifiers;
    }
    let caps_lock = if caps_known { caps_lock } else { guard.caps_lock };
    drop(guard);

    ohos::send_event(OhosEvent::Key { code, state, repeat, modifiers, caps_lock });
}

unsafe extern "C" fn on_focus(_component: *mut OH_NativeXComponent, _window: *mut c_void) {
    log::info!("XComponent focused");
    FOCUSED.store(true, Ordering::Relaxed);
    key_state().lock().unwrap().pressed.clear();
    ohos::send_event(OhosEvent::Focus(true));
}

unsafe extern "C" fn on_blur(_component: *mut OH_NativeXComponent, _window: *mut c_void) {
    log::info!("XComponent lost focus");
    FOCUSED.store(false, Ordering::Relaxed);
    key_state().lock().unwrap().pressed.clear();
    // Clear the press masks too: a click held across the switch would
    // otherwise suppress the next press as a duplicate until it is released.
    release_pressed_buttons();
    release_touch_left();
    ohos::send_event(OhosEvent::Focus(false));
}

unsafe extern "C" fn on_axis_event(
    _component: *mut OH_NativeXComponent,
    event: *mut ArkUI_UIInputEvent,
    event_type: i32,
) {
    if event.is_null() || event_type != UI_INPUT_EVENT_TYPE_AXIS {
        return;
    }
    let action = unsafe { OH_ArkUI_UIInputEvent_GetAction(event) };
    if action != AXIS_EVENT_UPDATE {
        return;
    }
    let vertical = unsafe { OH_ArkUI_AxisEvent_GetVerticalAxisValue(event) };
    let step = unsafe { OH_ArkUI_AxisEvent_GetScrollStep(event) }.max(1);
    // The SDK reports degrees that already include the user's scroll step, so
    // dividing by both normalises one notch to one line; touchpad deltas are
    // px and land at 15 x step px per line.
    let lines = (vertical / (15.0 * f64::from(step))).clamp(-3.0, 3.0) as f32;
    if lines != 0.0 {
        ensure_input_focus();
        ohos::send_event(OhosEvent::MouseWheel { dx: 0.0, dy: lines });
    }
}

/// The NAPI module's registration entry point. ArkUI loads the library named by
/// the ArkTS `libraryname` and hands it the XComponent through `exports`.
pub unsafe extern "C" fn init(
    env: crate::napi::napi_env,
    exports: crate::napi::napi_value,
) -> crate::napi::napi_value {
    // The header defines OH_NATIVE_XCOMPONENT_OBJ as "__NATIVE_XCOMPONENT_OBJ__".
    let key = CString::new("__NATIVE_XCOMPONENT_OBJ__").expect("static property name");
    let mut export_instance: crate::napi::napi_value = core::ptr::null_mut();
    let status = unsafe {
        crate::napi::napi_get_named_property(env, exports, key.as_ptr(), &mut export_instance)
    };
    if status != crate::napi::NAPI_OK {
        log::error!("no native XComponent object in this module instance ({status})");
        return exports;
    }
    let mut component: *mut c_void = core::ptr::null_mut();
    let status = unsafe { crate::napi::napi_unwrap(env, export_instance, &mut component) };
    if status != crate::napi::NAPI_OK {
        log::error!("napi_unwrap failed with status {status}");
        return exports;
    }
    attach(component.cast::<OH_NativeXComponent>());
    exports
}