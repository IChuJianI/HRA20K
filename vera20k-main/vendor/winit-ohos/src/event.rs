//! Window and device events.

use std::path::PathBuf;
use std::time::Instant;

use crate::dpi::{PhysicalPosition, PhysicalSize, Size};
use crate::error::ExternalError;
pub use crate::keyboard::{KeyEvent, Modifiers};
use crate::window::WindowId;

/// A device identifier. OHOS delivers keyboard and pointer events from the
/// window's own input stream, so every event uses this id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId(pub u64);

impl DeviceId {
    pub const fn dummy() -> Self {
        Self(0)
    }
}

/// An axis identifier for joystick-style motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AxisId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementState {
    Pressed,
    Released,
}

impl ElementState {
    pub fn is_pressed(self) -> bool {
        self == Self::Pressed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseScrollDelta {
    LineDelta(f32, f32),
    PixelDelta(PhysicalPosition<f64>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Force {
    Calibrated(f32),
    Normalized(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Touch {
    pub device_id: DeviceId,
    pub phase: TouchPhase,
    pub location: PhysicalPosition<f64>,
    pub force: Option<Force>,
    pub id: u64,
}

/// An input-method event.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ime {
    Enabled,
    Preedit(String, Option<(usize, usize)>),
    Commit(String),
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }
}

/// Lets a scale-factor handler propose a new inner size.
#[derive(Debug)]
pub struct InnerSizeWriter {
    size: PhysicalSize<u32>,
}

impl InnerSizeWriter {
    pub fn new(size: PhysicalSize<u32>) -> Self {
        Self { size }
    }

    pub fn request_inner_size(&mut self, new_inner_size: impl Into<Size>) -> Result<(), ExternalError> {
        self.size = new_inner_size.into().to_physical(1.0);
        Ok(())
    }
}

/// Reasons the event loop started a new dispatch cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartCause {
    Init,
    Poll,
    WaitCancelled { start: Instant, requested_resume: Option<Instant> },
    ResumeTimeReached { start: Instant, requested_resume: Instant },
}

/// Everything a window can report.
#[derive(Debug)]
pub enum WindowEvent {
    Resized(PhysicalSize<u32>),
    Moved(PhysicalPosition<i32>),
    CloseRequested,
    Destroyed,
    DroppedFile(PathBuf),
    HoveredFile(PathBuf),
    HoveredFileCancelled,
    Focused(bool),
    KeyboardInput { device_id: DeviceId, event: KeyEvent, is_synthetic: bool },
    ModifiersChanged(Modifiers),
    Ime(Ime),
    CursorMoved { device_id: DeviceId, position: PhysicalPosition<f64> },
    CursorEntered { device_id: DeviceId },
    CursorLeft { device_id: DeviceId },
    MouseWheel { device_id: DeviceId, delta: MouseScrollDelta, phase: TouchPhase },
    MouseInput { device_id: DeviceId, state: ElementState, button: MouseButton },
    PinchGesture { device_id: DeviceId, delta: f64, phase: TouchPhase },
    PanGesture { device_id: DeviceId, delta: PhysicalPosition<f32>, phase: TouchPhase },
    DoubleTapGesture { device_id: DeviceId },
    RotationGesture { device_id: DeviceId, delta: f32, phase: TouchPhase },
    TouchpadPressure { device_id: DeviceId, pressure: f32, stage: i64 },
    AxisMotion { device_id: DeviceId, axis: AxisId, value: f64 },
    Touch(Touch),
    ScaleFactorChanged { scale_factor: f64, inner_size_writer: InnerSizeWriter },
    ThemeChanged(Theme),
    Occluded(bool),
    RedrawRequested,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeviceEvent {
    Added,
    Removed,
    MouseMotion { delta: (f64, f64) },
    MouseAbsoluteMotion { delta: (f64, f64) },
    MouseButton { button: MouseButton, state: ElementState },
    MouseWheel { delta: MouseScrollDelta },
    Motion { axis: AxisId, value: f64 },
    Text { codepoint: char },
}

/// An application-level event, delivered by `EventLoop::run`-style loops.
#[derive(Debug)]
pub enum Event<T = ()> {
    NewEvents(StartCause),
    WindowEvent { window_id: WindowId, event: WindowEvent },
    DeviceEvent { device_id: DeviceId, event: DeviceEvent },
    UserEvent(T),
    Suspended,
    Resumed,
    MemoryWarning,
    AboutToWait,
    LoopExiting,
}
