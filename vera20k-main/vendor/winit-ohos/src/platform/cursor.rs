//! The window system's cursor plane on OpenHarmony.
//!
//! ArkUI's XComponent has no cursor API of its own; the multimodal input
//! natives (`OH_Input_SetCustomCursor` and friends) put a PixelMap at the
//! pointer of one of the process's windows. The window id comes from ArkTS
//! (`window.getWindowProperties().id`), because the native side may only name
//! windows of its own process and cannot enumerate them without the screen
//! capture permission.
//!
//! A cursor set before that id is known is held and pushed when the id
//! arrives. Any failure to install a custom cursor makes [`supports_image`]
//! answer false, which makes `CustomCursor::from_rgba` fail; the game reads
//! that as "no window-system cursor here" and paints the cursor into its
//! frames instead.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::window::{Cursor, CustomCursor};

/// MMI's largest accepted custom pointer image, in each dimension.
const MAX_CURSOR_SIZE: u16 = 256;

const INPUT_SUCCESS: i32 = 0;
const INPUT_DEVICE_NOT_SUPPORTED: i32 = 801;
const INPUT_INVALID_WINDOWID: i32 = 26_500_001;
const INPUT_POINTER_STYLE_DEFAULT: i32 = 0;

const PIXEL_FORMAT_RGBA_8888: i32 = 3;
const PIXELMAP_ALPHA_TYPE_UNPREMULTIPLIED: i32 = 3;

#[repr(C)]
struct OH_Pixelmap_InitializationOptions {
    _private: [u8; 0],
}

#[repr(C)]
struct OH_PixelmapNative {
    _private: [u8; 0],
}

#[repr(C)]
struct Input_CustomCursor {
    _private: [u8; 0],
}

#[repr(C)]
struct Input_CursorConfig {
    _private: [u8; 0],
}

#[link(name = "ohinput")]
unsafe extern "C" {
    fn OH_Input_SetCustomCursor(
        window_id: i32,
        custom_cursor: *mut Input_CustomCursor,
        cursor_config: *mut Input_CursorConfig,
    ) -> i32;
    fn OH_Input_CustomCursor_Create(
        pixelmap: *mut OH_PixelmapNative,
        anchor_x: i32,
        anchor_y: i32,
    ) -> *mut Input_CustomCursor;
    fn OH_Input_CustomCursor_Destroy(custom_cursor: *mut *mut Input_CustomCursor);
    fn OH_Input_CursorConfig_Create(follow_system: bool) -> *mut Input_CursorConfig;
    fn OH_Input_CursorConfig_Destroy(cursor_config: *mut *mut Input_CursorConfig);
    fn OH_Input_SetPointerVisible(visible: bool) -> i32;
    fn OH_Input_SetPointerStyle(window_id: i32, pointer_style: i32) -> i32;
}

#[link(name = "pixelmap")]
unsafe extern "C" {
    fn OH_PixelmapInitializationOptions_Create(
        options: *mut *mut OH_Pixelmap_InitializationOptions,
    ) -> i32;
    fn OH_PixelmapInitializationOptions_SetWidth(
        options: *mut OH_Pixelmap_InitializationOptions,
        width: u32,
    ) -> i32;
    fn OH_PixelmapInitializationOptions_SetHeight(
        options: *mut OH_Pixelmap_InitializationOptions,
        height: u32,
    ) -> i32;
    fn OH_PixelmapInitializationOptions_SetPixelFormat(
        options: *mut OH_Pixelmap_InitializationOptions,
        pixel_format: i32,
    ) -> i32;
    fn OH_PixelmapInitializationOptions_SetAlphaType(
        options: *mut OH_Pixelmap_InitializationOptions,
        alpha_type: i32,
    ) -> i32;
    fn OH_PixelmapInitializationOptions_Release(
        options: *mut OH_Pixelmap_InitializationOptions,
    ) -> i32;
    fn OH_PixelmapNative_CreatePixelmap(
        data: *mut u8,
        data_length: usize,
        options: *mut OH_Pixelmap_InitializationOptions,
        pixelmap: *mut *mut OH_PixelmapNative,
    ) -> i32;
    fn OH_PixelmapNative_Release(pixelmap: *mut OH_PixelmapNative) -> i32;
}

#[derive(Default)]
struct State {
    window_id: Option<i32>,
    pending: Option<Cursor>,
    /// The hash of the custom cursor the window currently shows.
    applied: Option<u64>,
    /// The PixelMap behind `applied`, kept alive for the window.
    pixelmap: usize,
    visible: Option<bool>,
    waiting_logged: bool,
    /// A custom cursor could not be installed, so the game draws its own.
    unsupported: bool,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(State::default()))
}

/// Whether the game can hand this image to the window system. False makes
/// `CustomCursor::from_rgba` fail so the game paints the cursor in its frame.
pub(crate) fn supports_image(width: u16, height: u16, rgba_len: usize) -> bool {
    if width == 0 || height == 0 {
        return false;
    }
    if rgba_len < usize::from(width) * usize::from(height) * 4 {
        return false;
    }
    !state().lock().unwrap().unsupported
}

/// Records the main window id ArkTS published; pushes whatever cursor was
/// already requested.
pub(crate) fn set_window_id(window_id: i32) {
    let mut guard = state().lock().unwrap();
    if guard.window_id == Some(window_id) {
        return;
    }
    guard.window_id = Some(window_id);
    guard.applied = None;
    log::info!("Cursor window id: {window_id}");
    if let Some(visible) = guard.visible {
        // SAFETY: the call only touches the pointer's visibility.
        unsafe { OH_Input_SetPointerVisible(visible) };
    }
    apply(&mut guard);
}

pub(crate) fn set_cursor(cursor: Cursor) {
    let mut guard = state().lock().unwrap();
    guard.pending = Some(cursor);
    apply(&mut guard);
}

pub(crate) fn set_visible(visible: bool) {
    let mut guard = state().lock().unwrap();
    if guard.visible == Some(visible) {
        return;
    }
    guard.visible = Some(visible);
    // SAFETY: the call only touches the pointer's visibility.
    let status = unsafe { OH_Input_SetPointerVisible(visible) };
    if status != INPUT_SUCCESS {
        log::warn!("OH_Input_SetPointerVisible({visible}) failed with status {status}");
    }
}

fn apply(guard: &mut State) {
    let Some(window_id) = guard.window_id else {
        if !guard.waiting_logged {
            guard.waiting_logged = true;
            log::info!("Cursor requested before the window id is known; holding it");
        }
        return;
    };
    let Some(pending) = guard.pending.clone() else {
        return;
    };
    match pending {
        Cursor::Icon(_) => {
            guard.applied = None;
            // The game only asks for Default (its arrow state); every other
            // winit icon has no RA2 equivalent, so they all use the system
            // arrow too.
            // SAFETY: the window id came from this process's ArkTS window.
            let status =
                unsafe { OH_Input_SetPointerStyle(window_id, INPUT_POINTER_STYLE_DEFAULT) };
            if status != INPUT_SUCCESS {
                log::warn!("OH_Input_SetPointerStyle failed with status {status}");
            }
        }
        Cursor::Custom(cursor) => {
            let key = hash(&cursor);
            if guard.applied == Some(key) {
                return;
            }
            let image = fit_cursor(&cursor);
            match install(window_id, &image) {
                Ok(pixelmap) => {
                    if guard.pixelmap != 0 {
                        // SAFETY: this is the PixelMap of the cursor being
                        // replaced; the window now holds the new one.
                        unsafe {
                            OH_PixelmapNative_Release(guard.pixelmap as *mut OH_PixelmapNative)
                        };
                    }
                    guard.pixelmap = pixelmap as usize;
                    guard.applied = Some(key);
                }
                Err(status) => {
                    log::warn!(
                        "OH_Input_SetCustomCursor failed with status {status}; \
                         drawing the cursor in the frame"
                    );
                    // An invalid window id is worth retrying once ArkTS
                    // publishes a better one; anything else is permanent.
                    if status != INPUT_INVALID_WINDOWID {
                        guard.unsupported = true;
                        // Keep a pointer on screen until the game's sprite
                        // takes over.
                        // SAFETY: the window id came from this process.
                        unsafe {
                            OH_Input_SetPointerStyle(window_id, INPUT_POINTER_STYLE_DEFAULT)
                        };
                    }
                    guard.applied = None;
                }
            }
        }
    }
}

fn hash(cursor: &CustomCursor) -> u64 {
    let mut hasher = DefaultHasher::new();
    cursor.size().hash(&mut hasher);
    cursor.hotspot().hash(&mut hasher);
    cursor.rgba().hash(&mut hasher);
    hasher.finish()
}

/// The image handed to MMI, at most [`MAX_CURSOR_SIZE`] each way. A zoomed
/// tactical cursor can exceed that; shrink it nearest-neighbour rather than
/// give up the cursor plane for the rest of the run.
struct FittedImage {
    rgba: Vec<u8>,
    width: u16,
    height: u16,
    hotspot_x: u16,
    hotspot_y: u16,
}

fn fit_cursor(cursor: &CustomCursor) -> FittedImage {
    let (width, height) = cursor.size();
    let (hotspot_x, hotspot_y) = cursor.hotspot();
    if width <= MAX_CURSOR_SIZE && height <= MAX_CURSOR_SIZE {
        return FittedImage {
            rgba: cursor.rgba().to_vec(),
            width,
            height,
            hotspot_x,
            hotspot_y,
        };
    }
    static LOGGED: AtomicBool = AtomicBool::new(false);
    if !LOGGED.swap(true, Ordering::Relaxed) {
        log::info!("Cursor image {width}x{height} exceeds {MAX_CURSOR_SIZE}; scaling it down");
    }
    let ratio = (f32::from(MAX_CURSOR_SIZE) / f32::from(width))
        .min(f32::from(MAX_CURSOR_SIZE) / f32::from(height));
    let new_width = ((f32::from(width) * ratio).floor() as u16).max(1);
    let new_height = ((f32::from(height) * ratio).floor() as u16).max(1);
    let source = cursor.rgba();
    let mut rgba = vec![0u8; usize::from(new_width) * usize::from(new_height) * 4];
    for y in 0..usize::from(new_height) {
        let source_y = y * usize::from(height) / usize::from(new_height);
        for x in 0..usize::from(new_width) {
            let source_x = x * usize::from(width) / usize::from(new_width);
            let from = (source_y * usize::from(width) + source_x) * 4;
            let to = (y * usize::from(new_width) + x) * 4;
            rgba[to..to + 4].copy_from_slice(&source[from..from + 4]);
        }
    }
    let scale_hotspot = |value: u16, source: u16, target: u16| {
        let scaled = u32::from(value) * u32::from(target) / u32::from(source);
        (scaled as u16).min(target - 1)
    };
    FittedImage {
        rgba,
        width: new_width,
        height: new_height,
        hotspot_x: scale_hotspot(hotspot_x, width, new_width),
        hotspot_y: scale_hotspot(hotspot_y, height, new_height),
    }
}

/// Creates the PixelMap and the custom cursor, hands it to the window, and
/// returns the PixelMap the window keeps. The cursor and config objects are
/// one-shot and destroyed here; the PixelMap outlives the call.
fn install(window_id: i32, image: &FittedImage) -> Result<*mut OH_PixelmapNative, i32> {
    let width = image.width;
    let height = image.height;
    let mut options: *mut OH_Pixelmap_InitializationOptions = std::ptr::null_mut();
    // SAFETY: the out-parameter starts null and is written by the call.
    let status = unsafe { OH_PixelmapInitializationOptions_Create(&mut options) };
    if status != INPUT_SUCCESS || options.is_null() {
        return Err(status);
    }
    // SAFETY: `options` is live until released below.
    let setup = unsafe {
        OH_PixelmapInitializationOptions_SetWidth(options, u32::from(width))
            .max(OH_PixelmapInitializationOptions_SetHeight(
                options,
                u32::from(height),
            ))
            .max(OH_PixelmapInitializationOptions_SetPixelFormat(
                options,
                PIXEL_FORMAT_RGBA_8888,
            ))
            .max(OH_PixelmapInitializationOptions_SetAlphaType(
                options,
                PIXELMAP_ALPHA_TYPE_UNPREMULTIPLIED,
            ))
    };
    let mut pixelmap: *mut OH_PixelmapNative = std::ptr::null_mut();
    let status = if setup == INPUT_SUCCESS {
        // SAFETY: the RGBA buffer is live for the call; the out-parameter is
        // a valid local.
        unsafe {
            OH_PixelmapNative_CreatePixelmap(
                image.rgba.as_ptr().cast_mut(),
                image.rgba.len(),
                options,
                &mut pixelmap,
            )
        }
    } else {
        setup
    };
    // SAFETY: `options` is no longer used after this release.
    unsafe { OH_PixelmapInitializationOptions_Release(options) };
    if status != INPUT_SUCCESS || pixelmap.is_null() {
        if !pixelmap.is_null() {
            // SAFETY: the handle was created above and is not used again.
            unsafe { OH_PixelmapNative_Release(pixelmap) };
        }
        return Err(status);
    }
    // SAFETY: the PixelMap was just created and the anchors lie within it.
    let mut custom = unsafe {
        OH_Input_CustomCursor_Create(
            pixelmap,
            i32::from(image.hotspot_x),
            i32::from(image.hotspot_y),
        )
    };
    if custom.is_null() {
        // SAFETY: no cursor references the PixelMap yet.
        unsafe { OH_PixelmapNative_Release(pixelmap) };
        return Err(INPUT_DEVICE_NOT_SUPPORTED);
    }
    // SAFETY: a config that keeps the image's own size, as the game scaled it.
    let mut config = unsafe { OH_Input_CursorConfig_Create(false) };
    if config.is_null() {
        // SAFETY: neither object reached a window.
        unsafe { OH_Input_CustomCursor_Destroy(&mut custom) };
        unsafe { OH_PixelmapNative_Release(pixelmap) };
        return Err(INPUT_DEVICE_NOT_SUPPORTED);
    }
    // SAFETY: the window id came from this process; both objects are live.
    let status = unsafe { OH_Input_SetCustomCursor(window_id, custom, config) };
    // SAFETY: both objects were created above and are one-shot.
    unsafe {
        OH_Input_CursorConfig_Destroy(&mut config);
        OH_Input_CustomCursor_Destroy(&mut custom);
    }
    if status != INPUT_SUCCESS {
        // SAFETY: the window rejected the cursor, so nothing holds the
        // PixelMap.
        unsafe { OH_PixelmapNative_Release(pixelmap) };
        return Err(status);
    }
    Ok(pixelmap)
}
