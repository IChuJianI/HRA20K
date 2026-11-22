//! Window attributes and the XComponent-backed window.

use std::fmt;
use std::sync::{Arc, Mutex, Weak};

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, OhosNdkWindowHandle,
    RawWindowHandle, WindowHandle,
};

use crate::dpi::{PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{BadImage, ExternalError};
use crate::event::Theme;
use crate::event_loop::LoopShared;
use crate::monitor::MonitorHandle;
use crate::platform::ohos::OhosSurfaceInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(u64);

impl WindowId {
    pub const fn dummy() -> Self {
        Self(u64::MAX)
    }

    pub(crate) fn new(value: u64) -> Self {
        Self(value)
    }

    pub(crate) fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CursorGrabMode {
    None,
    Confined,
    Locked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowLevel {
    AlwaysOnTop,
    Normal,
    AlwaysOnBottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserAttentionType {
    Critical,
    Informational,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResizeDirection {
    East,
    North,
    NorthEast,
    NorthWest,
    South,
    SouthEast,
    SouthWest,
    West,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[allow(non_camel_case_types)]
pub enum CursorIcon {
    #[default]
    Default,
    Crosshair,
    Hand,
    Arrow,
    Move,
    Text,
    Wait,
    Help,
    Progress,
    NotAllowed,
    ContextMenu,
    Cell,
    VerticalText,
    Alias,
    Copy,
    NoDrop,
    Grab,
    Grabbing,
    AllScroll,
    ZoomIn,
    ZoomOut,
    EResize,
    NResize,
    NeResize,
    NwResize,
    SResize,
    SeResize,
    SwResize,
    WResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    ColResize,
    RowResize,
}

/// Image data for a cursor, created before the event loop turns it into a
/// platform [`CustomCursor`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomCursorSource {
    rgba: Vec<u8>,
    width: u16,
    height: u16,
    hotspot_x: u16,
    hotspot_y: u16,
}

impl CustomCursorSource {
    fn from_rgba(
        rgba: Vec<u8>,
        width: u16,
        height: u16,
        hotspot_x: u16,
        hotspot_y: u16,
    ) -> Result<Self, BadImage> {
        if width == 0 || height == 0 || hotspot_x >= width || hotspot_y >= height {
            return Err(BadImage);
        }
        // Returning Err here is how the game learns to paint the cursor into
        // the frame instead of handing it to the window system.
        if !crate::platform::cursor::supports_image(width, height, rgba.len()) {
            return Err(BadImage);
        }
        Ok(Self { rgba, width, height, hotspot_x, hotspot_y })
    }

    pub fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    pub fn hotspot(&self) -> (u16, u16) {
        (self.hotspot_x, self.hotspot_y)
    }

    pub(crate) fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomCursor {
    source: CustomCursorSource,
}

impl CustomCursor {
    /// The alpha channel is assumed to be **not** premultiplied.
    pub fn from_rgba(
        rgba: impl Into<Vec<u8>>,
        width: u16,
        height: u16,
        hotspot_x: u16,
        hotspot_y: u16,
    ) -> Result<CustomCursorSource, BadImage> {
        CustomCursorSource::from_rgba(rgba.into(), width, height, hotspot_x, hotspot_y)
    }

    pub(crate) fn new(source: CustomCursorSource) -> Self {
        Self { source }
    }

    pub fn size(&self) -> (u16, u16) {
        self.source.size()
    }

    pub fn hotspot(&self) -> (u16, u16) {
        self.source.hotspot()
    }

    pub(crate) fn rgba(&self) -> &[u8] {
        self.source.rgba()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cursor {
    Icon(CursorIcon),
    Custom(CustomCursor),
}

impl From<CursorIcon> for Cursor {
    fn from(icon: CursorIcon) -> Self {
        Self::Icon(icon)
    }
}

impl From<CustomCursor> for Cursor {
    fn from(cursor: CustomCursor) -> Self {
        Self::Custom(cursor)
    }
}

impl From<CustomCursorSource> for Cursor {
    fn from(source: CustomCursorSource) -> Self {
        Self::Custom(CustomCursor::new(source))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

impl Icon {
    pub fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, BadImage> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(BadImage)?;
        if width == 0 || height == 0 || rgba.len() != expected {
            return Err(BadImage);
        }
        Ok(Self { rgba, width, height })
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VideoMode {
    pub size: PhysicalSize<u32>,
    pub refresh_rate_millihertz: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fullscreen {
    Exclusive(VideoMode, MonitorHandle),
    Borderless(Option<MonitorHandle>),
}

/// Attributes a window is created with.
#[derive(Debug, Clone)]
pub struct WindowAttributes {
    pub title: String,
    pub inner_size: Option<Size>,
    pub min_inner_size: Option<Size>,
    pub max_inner_size: Option<Size>,
    pub position: Option<Position>,
    pub resizable: bool,
    pub fullscreen: Option<Fullscreen>,
    pub visible: bool,
    pub active: bool,
    pub decorations: bool,
    pub transparent: bool,
    pub window_level: WindowLevel,
    pub cursor: Cursor,
    pub window_icon: Option<Icon>,
    pub preferred_theme: Option<Theme>,
}

impl Default for WindowAttributes {
    fn default() -> Self {
        Self {
            title: String::new(),
            inner_size: None,
            min_inner_size: None,
            max_inner_size: None,
            position: None,
            resizable: true,
            fullscreen: None,
            visible: true,
            active: true,
            decorations: true,
            transparent: false,
            window_level: WindowLevel::Normal,
            cursor: Cursor::Icon(CursorIcon::Default),
            window_icon: None,
            preferred_theme: None,
        }
    }
}

impl WindowAttributes {
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_inner_size(mut self, size: impl Into<Size>) -> Self {
        self.inner_size = Some(size.into());
        self
    }

    pub fn with_min_inner_size(mut self, size: impl Into<Size>) -> Self {
        self.min_inner_size = Some(size.into());
        self
    }

    pub fn with_max_inner_size(mut self, size: impl Into<Size>) -> Self {
        self.max_inner_size = Some(size.into());
        self
    }

    pub fn with_position(mut self, position: impl Into<Position>) -> Self {
        self.position = Some(position.into());
        self
    }

    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    pub fn with_fullscreen(mut self, fullscreen: Option<Fullscreen>) -> Self {
        self.fullscreen = fullscreen;
        self
    }

    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn with_active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn with_decorations(mut self, decorations: bool) -> Self {
        self.decorations = decorations;
        self
    }

    pub fn with_transparent(mut self, transparent: bool) -> Self {
        self.transparent = transparent;
        self
    }

    pub fn with_window_level(mut self, level: WindowLevel) -> Self {
        self.window_level = level;
        self
    }

    pub fn with_cursor(mut self, cursor: impl Into<Cursor>) -> Self {
        self.cursor = cursor.into();
        self
    }

    pub fn with_window_icon(mut self, icon: Option<Icon>) -> Self {
        self.window_icon = icon;
        self
    }

    pub fn with_theme(mut self, theme: Option<Theme>) -> Self {
        self.preferred_theme = theme;
        self
    }
}

pub(crate) struct WindowState {
    pub surface: Option<OhosSurfaceInfo>,
    pub title: String,
    pub visible: bool,
    pub focused: bool,
    pub maximized: bool,
    pub minimized: bool,
    pub fullscreen: Option<Fullscreen>,
    pub cursor_visible: bool,
    pub window_level: WindowLevel,
}

impl WindowState {
    fn new(surface: Option<OhosSurfaceInfo>, attributes: &WindowAttributes) -> Self {
        Self {
            surface,
            title: attributes.title.clone(),
            visible: attributes.visible,
            focused: attributes.active,
            maximized: false,
            minimized: false,
            // The page presents the game on the XComponent surface and keeps
            // the window maximized; there is no windowed mode to track.
            fullscreen: Some(
                attributes
                    .fullscreen
                    .clone()
                    .unwrap_or(Fullscreen::Borderless(None)),
            ),
            cursor_visible: true,
            window_level: attributes.window_level,
        }
    }

    pub(crate) fn size(&self) -> PhysicalSize<u32> {
        self.surface
            .map(|surface| PhysicalSize::new(surface.width, surface.height))
            .unwrap_or(PhysicalSize::new(0, 0))
    }
}

#[derive(Clone)]
pub struct Window {
    id: WindowId,
    state: Arc<Mutex<WindowState>>,
    shared: Weak<LoopShared>,
}

impl Window {
    pub fn default_attributes() -> WindowAttributes {
        WindowAttributes::default()
    }

    pub(crate) fn new(
        id: WindowId,
        surface: Option<OhosSurfaceInfo>,
        attributes: &WindowAttributes,
        shared: Arc<LoopShared>,
    ) -> Self {
        Self {
            id,
            state: Arc::new(Mutex::new(WindowState::new(surface, attributes))),
            shared: Arc::downgrade(&shared),
        }
    }

    pub(crate) fn update_surface(&self, surface: Option<OhosSurfaceInfo>) {
        self.state.lock().unwrap().surface = surface;
    }

    pub(crate) fn set_focused(&self, focused: bool) {
        self.state.lock().unwrap().focused = focused;
    }

    pub fn id(&self) -> WindowId {
        self.id
    }

    pub fn scale_factor(&self) -> f64 {
        self.shared
            .upgrade()
            .map(|shared| shared.scale_factor())
            .unwrap_or(1.0)
    }

    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.state.lock().unwrap().size()
    }

    pub fn outer_size(&self) -> PhysicalSize<u32> {
        self.inner_size()
    }

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, ExternalError> {
        Ok(PhysicalPosition::new(0, 0))
    }

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, ExternalError> {
        Ok(PhysicalPosition::new(0, 0))
    }

    pub fn set_outer_position(&self, _position: impl Into<Position>) {}

    pub fn request_inner_size(&self, size: impl Into<Size>) -> Option<PhysicalSize<u32>> {
        Some(size.into().to_physical(self.scale_factor()))
    }

    pub fn request_redraw(&self) {
        if let Some(shared) = self.shared.upgrade() {
            shared.request_redraw();
        }
    }

    pub fn pre_present_notify(&self) {}

    pub fn set_title(&self, title: &str) {
        self.state.lock().unwrap().title = title.to_owned();
    }

    pub fn title(&self) -> String {
        self.state.lock().unwrap().title.clone()
    }

    /// `None` means the platform cannot report it, as on OHOS where ArkUI
    /// owns the window.
    pub fn is_visible(&self) -> Option<bool> {
        Some(self.state.lock().unwrap().visible)
    }

    pub fn set_visible(&self, visible: bool) {
        self.state.lock().unwrap().visible = visible;
    }

    pub fn has_focus(&self) -> bool {
        self.state.lock().unwrap().focused
    }

    pub fn focus_window(&self) {}

    pub fn fullscreen(&self) -> Option<Fullscreen> {
        self.state.lock().unwrap().fullscreen.clone()
    }

    /// The game is full screen on this platform: ArkUI owns the window mode
    /// and keeps it maximized, and the engine's request to leave would only
    /// desync the Options box from the window the player sees.
    pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        if let Some(fullscreen) = fullscreen {
            self.state.lock().unwrap().fullscreen = Some(fullscreen);
        }
        self.request_redraw();
    }

    pub fn is_maximized(&self) -> bool {
        self.state.lock().unwrap().maximized
    }

    pub fn set_maximized(&self, maximized: bool) {
        self.state.lock().unwrap().maximized = maximized;
    }

    pub fn is_minimized(&self) -> Option<bool> {
        Some(self.state.lock().unwrap().minimized)
    }

    pub fn set_minimized(&self, minimized: bool) {
        self.state.lock().unwrap().minimized = minimized;
    }

    pub fn set_cursor_visible(&self, visible: bool) {
        self.state.lock().unwrap().cursor_visible = visible;
        crate::platform::cursor::set_visible(visible);
    }

    pub fn is_cursor_visible(&self) -> bool {
        self.state.lock().unwrap().cursor_visible
    }

    pub fn set_cursor_grab(&self, _mode: CursorGrabMode) -> Result<(), ExternalError> {
        Ok(())
    }

    pub fn set_cursor(&self, cursor: impl Into<Cursor>) {
        crate::platform::cursor::set_cursor(cursor.into());
    }

    pub fn reset_cursor(&self) {
        crate::platform::cursor::set_cursor(Cursor::Icon(CursorIcon::Default));
    }

    pub fn set_cursor_position(&self, _position: impl Into<Position>) -> Result<(), ExternalError> {
        Ok(())
    }

    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), ExternalError> {
        Ok(())
    }

    pub fn set_ime_allowed(&self, allowed: bool) {
        crate::platform::ime::set_allowed(allowed);
    }

    pub fn set_ime_cursor_area(&self, position: impl Into<Position>, size: impl Into<Size>) {
        let scale = self.scale_factor();
        let position = position.into().to_physical(scale);
        let size = size.into().to_physical(scale);
        crate::platform::ime::set_cursor_area(
            f64::from(position.x),
            f64::from(position.y),
            f64::from(size.width),
            f64::from(size.height),
        );
    }

    pub fn set_window_icon(&self, _icon: Option<Icon>) {}

    pub fn set_taskbar_icon(&self, _icon: Option<Icon>) {}

    pub fn set_window_level(&self, level: WindowLevel) {
        self.state.lock().unwrap().window_level = level;
    }

    pub fn window_level(&self) -> WindowLevel {
        self.state.lock().unwrap().window_level
    }

    pub fn request_user_attention(&self, _request: Option<UserAttentionType>) {}

    pub fn set_theme(&self, _theme: Option<Theme>) {}

    pub fn theme(&self) -> Option<Theme> {
        None
    }

    pub fn set_content_protected(&self, _protected: bool) {}

    pub fn set_transparent(&self, _transparent: bool) {}

    pub fn drag_window(&self) -> Result<(), ExternalError> {
        Ok(())
    }

    pub fn drag_resize_window(&self, _direction: ResizeDirection) -> Result<(), ExternalError> {
        Ok(())
    }

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        Some(default_monitor(self.inner_size()))
    }

    pub fn available_monitors(&self) -> impl Iterator<Item = MonitorHandle> {
        std::iter::once(default_monitor(self.inner_size()))
    }

    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        Some(default_monitor(self.inner_size()))
    }
}

pub(crate) fn default_monitor(size: PhysicalSize<u32>) -> MonitorHandle {
    let size = if size.width == 0 || size.height == 0 {
        PhysicalSize::new(1920, 1200)
    } else {
        size
    };
    MonitorHandle::new(Some("Built-in display".to_owned()), size, PhysicalPosition::new(0, 0), 1.0, Some(60_000))
}

impl fmt::Debug for Window {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Window").field("id", &self.id).finish_non_exhaustive()
    }
}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let native_window = {
            let state = self.state.lock().unwrap();
            state.surface.map(|surface| surface.native_window)
        };
        let native_window = native_window.ok_or(HandleError::NotSupported)?;
        let pointer = std::ptr::NonNull::new(native_window as *mut core::ffi::c_void)
            .ok_or(HandleError::NotSupported)?;
        let handle = OhosNdkWindowHandle::new(pointer);
        // SAFETY: the handle borrows nothing; it names the OHNativeWindow owned
        // by the live XComponent surface.
        unsafe { Ok(WindowHandle::borrow_raw(RawWindowHandle::OhosNdk(handle))) }
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::ohos())
    }
}
