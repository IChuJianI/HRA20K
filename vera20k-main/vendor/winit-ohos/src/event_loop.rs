//! The event loop and its control handles.

use std::any::Any;
use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

use raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle};

use crate::application::ApplicationHandler;
use crate::dpi::PhysicalSize;
use crate::error::{EventLoopError, OsError};
use crate::dpi::PhysicalPosition;
use crate::event::{DeviceId, MouseScrollDelta, StartCause, TouchPhase, WindowEvent};
use crate::monitor::MonitorHandle;
use crate::platform::ohos::{self, OhosEvent, OhosSurfaceInfo};
use crate::window::{default_monitor, CustomCursor, CustomCursorSource, Window, WindowAttributes, WindowId};

/// What the loop does once it has no more events to dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlFlow {
    Poll,
    Wait,
    WaitUntil(Instant),
}

/// A handle to an OS display, owned by the event loop.
#[derive(Debug, Clone, Copy)]
pub struct OwnedDisplayHandle;

impl HasDisplayHandle for OwnedDisplayHandle {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::ohos())
    }
}

/// State shared by the loop, its windows and its proxies.
pub(crate) struct LoopShared {
    window: Mutex<Option<Window>>,
    surface: Mutex<Option<OhosSurfaceInfo>>,
    exiting: AtomicBool,
    redraw: AtomicBool,
    thread: Mutex<Option<Thread>>,
    control_flow: Mutex<ControlFlow>,
    scale_factor_bits: AtomicU64,
    next_window_id: AtomicU64,
}

impl LoopShared {
    fn new() -> Self {
        Self {
            window: Mutex::new(None),
            surface: Mutex::new(None),
            exiting: AtomicBool::new(false),
            redraw: AtomicBool::new(false),
            thread: Mutex::new(None),
            control_flow: Mutex::new(ControlFlow::Wait),
            scale_factor_bits: AtomicU64::new(1.0f64.to_bits()),
            next_window_id: AtomicU64::new(0),
        }
    }

    /// Wakes the loop for a message or a state change from another thread.
    /// A callback on the loop thread itself must not unpark: the token would
    /// make the next `park_timeout` return at once, and the loop would spin
    /// through `about_to_wait` without ever resting.
    pub(crate) fn wake(&self) {
        let thread = self.thread.lock().unwrap().clone();
        if let Some(thread) = thread {
            if thread.id() != std::thread::current().id() {
                thread.unpark();
            }
        }
    }

    pub(crate) fn request_redraw(&self) {
        self.redraw.store(true, Ordering::SeqCst);
        self.wake();
    }

    pub(crate) fn scale_factor(&self) -> f64 {
        f64::from_bits(self.scale_factor_bits.load(Ordering::Relaxed))
    }

    pub(crate) fn window(&self) -> Option<Window> {
        self.window.lock().unwrap().clone()
    }

    fn surface(&self) -> Option<OhosSurfaceInfo> {
        *self.surface.lock().unwrap()
    }

    fn set_surface(&self, surface: Option<OhosSurfaceInfo>) {
        *self.surface.lock().unwrap() = surface;
    }

    fn exiting(&self) -> bool {
        self.exiting.load(Ordering::SeqCst)
    }

    fn monitor(&self) -> MonitorHandle {
        let size = self
            .surface()
            .map(|surface| PhysicalSize::new(surface.width, surface.height))
            .unwrap_or(PhysicalSize::new(1920, 1200));
        default_monitor(size)
    }
}

enum LoopMsg<T> {
    Ohos(OhosEvent),
    User(T),
}

/// A proxy that can wake the loop from another thread and deliver a user event.
pub struct EventLoopProxy<T: 'static> {
    tx: Sender<LoopMsg<T>>,
    shared: Arc<LoopShared>,
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self { tx: self.tx.clone(), shared: self.shared.clone() }
    }
}

impl<T: 'static> fmt::Debug for EventLoopProxy<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventLoopProxy").finish_non_exhaustive()
    }
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.shared.wake();
        match self.tx.send(LoopMsg::User(event)) {
            Ok(()) => {
                self.shared.wake();
                Ok(())
            }
            Err(error) => match error.0 {
                LoopMsg::User(event) => Err(EventLoopClosed(event)),
                LoopMsg::Ohos(_) => unreachable!("the proxy only sends user events"),
            },
        }
    }
}

/// The user event could not be delivered because the loop has exited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventLoopClosed<T>(pub T);

/// Builder for [`EventLoop`].
pub struct EventLoopBuilder<T: 'static> {
    _marker: PhantomData<T>,
}

impl<T: 'static> fmt::Debug for EventLoopBuilder<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventLoopBuilder").finish_non_exhaustive()
    }
}

impl<T: 'static> EventLoopBuilder<T> {
    pub fn build(&mut self) -> Result<EventLoop<T>, EventLoopError> {
        EventLoop::new_internal()
    }

    /// Kept for source compatibility with desktop-only call sites.
    pub fn with_any_thread(&mut self, _any_thread: bool) -> &mut Self {
        self
    }
}

struct AnySender(Box<dyn Any + Send + Sync>);

/// The loop's handle passed to application callbacks.
pub struct ActiveEventLoop {
    shared: Arc<LoopShared>,
    sender: AnySender,
}

impl fmt::Debug for ActiveEventLoop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActiveEventLoop").finish_non_exhaustive()
    }
}

impl ActiveEventLoop {
    pub fn create_window(&self, attributes: WindowAttributes) -> Result<Window, OsError> {
        let surface = self
            .shared
            .surface()
            .ok_or_else(|| OsError::new(-1, "the XComponent surface is not available yet"))?;
        let id = WindowId::new(self.shared.next_window_id.fetch_add(1, Ordering::SeqCst) + 1);
        let window = Window::new(id, Some(surface), &attributes, self.shared.clone());
        *self.shared.window.lock().unwrap() = Some(window.clone());
        log::info!(
            "Created OHOS window {} at {}x{}",
            window.id().raw(),
            surface.width,
            surface.height
        );
        Ok(window)
    }

    pub fn exit(&self) {
        self.shared.exiting.store(true, Ordering::SeqCst);
        self.shared.wake();
    }

    pub fn exiting(&self) -> bool {
        self.shared.exiting()
    }

    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        *self.shared.control_flow.lock().unwrap() = control_flow;
        self.shared.wake();
    }

    pub fn control_flow(&self) -> ControlFlow {
        *self.shared.control_flow.lock().unwrap()
    }

    pub fn create_custom_cursor(&self, custom_cursor: CustomCursorSource) -> CustomCursor {
        CustomCursor::new(custom_cursor)
    }

    pub fn create_proxy<T: Send + 'static>(&self) -> EventLoopProxy<T> {
        let tx = self
            .sender
            .0
            .downcast_ref::<Sender<LoopMsg<T>>>()
            .expect("the event loop user event type")
            .clone();
        EventLoopProxy { tx, shared: self.shared.clone() }
    }

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        Some(self.shared.monitor())
    }

    pub fn available_monitors(&self) -> impl Iterator<Item = MonitorHandle> {
        std::iter::once(self.shared.monitor())
    }

    pub fn owned_display_handle(&self) -> OwnedDisplayHandle {
        OwnedDisplayHandle
    }
}

/// The event loop, created once per process.
pub struct EventLoop<T: 'static> {
    shared: Arc<LoopShared>,
    tx: Sender<LoopMsg<T>>,
    rx: Receiver<LoopMsg<T>>,
    _marker: PhantomData<T>,
}

impl<T: 'static> fmt::Debug for EventLoop<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventLoop").finish_non_exhaustive()
    }
}

impl<T: 'static> EventLoop<T> {
    pub fn builder() -> EventLoopBuilder<T> {
        EventLoopBuilder { _marker: PhantomData }
    }

    pub fn with_user_event() -> EventLoopBuilder<T> {
        Self::builder()
    }

    fn new_internal() -> Result<Self, EventLoopError> {
        let (tx, rx) = mpsc::channel();
        Ok(Self {
            shared: Arc::new(LoopShared::new()),
            tx,
            rx,
            _marker: PhantomData,
        })
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T>
    where
        T: Send,
    {
        EventLoopProxy { tx: self.tx.clone(), shared: self.shared.clone() }
    }

    pub fn owned_display_handle(&self) -> OwnedDisplayHandle {
        OwnedDisplayHandle
    }

    pub fn run_app<A: ApplicationHandler<T>>(self, app: &mut A) -> Result<(), EventLoopError>
    where
        T: Send,
    {
        let thread = thread::current();
        *self.shared.thread.lock().unwrap() = Some(thread.clone());

        let tx = self.tx.clone();
        let shared = self.shared.clone();
        ohos::install_sink(Arc::new(move |event| {
            match event {
                OhosEvent::SurfaceCreated(surface) | OhosEvent::SurfaceChanged(surface) => {
                    ohos::set_latest_surface(Some(surface));
                    *shared.surface.lock().unwrap() = Some(surface);
                }
                OhosEvent::SurfaceDestroyed => {
                    ohos::set_latest_surface(None);
                    *shared.surface.lock().unwrap() = None;
                }
                _ => {}
            }
            let _ = tx.send(LoopMsg::Ohos(event));
            shared.wake();
        }));

        let active = ActiveEventLoop {
            shared: self.shared.clone(),
            sender: AnySender(Box::new(self.tx.clone())),
        };

        let mut resumed = false;
        // Like winit, announce every new batch of events; startup uses one
        // admitted loop pass per slice of its work.
        let mut cause = StartCause::Init;

        loop {
            app.new_events(&active, cause);
            cause = StartCause::Poll;
            let mut handled_any = false;
            loop {
                let message = match self.rx.try_recv() {
                    Ok(message) => message,
                    Err(_) => break,
                };
                handled_any = true;
                match message {
                    LoopMsg::User(event) => app.user_event(&active, event),
                    LoopMsg::Ohos(OhosEvent::SurfaceCreated(surface)) => {
                        self.shared.set_surface(Some(surface));
                        if let Some(window) = self.shared.window() {
                            window.update_surface(Some(surface));
                            if resumed {
                                app.window_event(
                                    &active,
                                    window.id(),
                                    WindowEvent::Resized(PhysicalSize::new(surface.width, surface.height)),
                                );
                                self.shared.request_redraw();
                            }
                        }
                        if !resumed {
                            resumed = true;
                            app.resumed(&active);
                            if let Some(window) = self.shared.window() {
                                app.window_event(&active, window.id(), WindowEvent::Resized(window.inner_size()));
                                self.shared.request_redraw();
                            }
                        }
                    }
                    LoopMsg::Ohos(OhosEvent::SurfaceChanged(surface)) => {
                        self.shared.set_surface(Some(surface));
                        if let Some(window) = self.shared.window() {
                            window.update_surface(Some(surface));
                            app.window_event(
                                &active,
                                window.id(),
                                WindowEvent::Resized(PhysicalSize::new(surface.width, surface.height)),
                            );
                        }
                        self.shared.request_redraw();
                    }
                    LoopMsg::Ohos(OhosEvent::SurfaceDestroyed) => {
                        self.shared.set_surface(None);
                        if let Some(window) = self.shared.window() {
                            window.update_surface(None);
                        }
                        log::warn!("The XComponent surface was destroyed; rendering pauses");
                    }
                    LoopMsg::Ohos(OhosEvent::CloseRequested) => {
                        if let Some(window) = self.shared.window() {
                            app.window_event(&active, window.id(), WindowEvent::CloseRequested);
                        }
                    }
                    LoopMsg::Ohos(OhosEvent::Focus(focused)) => {
                        crate::platform::ime::set_focused(focused);
                        if let Some(window) = self.shared.window() {
                            window.set_focused(focused);
                            app.window_event(&active, window.id(), WindowEvent::Focused(focused));
                        }
                    }
                    LoopMsg::Ohos(OhosEvent::Hover(hovered)) => {
                        if let Some(window) = self.shared.window() {
                            let event = if hovered {
                                WindowEvent::CursorEntered { device_id: DeviceId::dummy() }
                            } else {
                                WindowEvent::CursorLeft { device_id: DeviceId::dummy() }
                            };
                            app.window_event(&active, window.id(), event);
                        }
                    }
                    LoopMsg::Ohos(OhosEvent::CursorMoved { x, y }) => {
                        if let Some(window) = self.shared.window() {
                            app.window_event(
                                &active,
                                window.id(),
                                WindowEvent::CursorMoved {
                                    device_id: DeviceId::dummy(),
                                    position: PhysicalPosition::new(x as f64, y as f64),
                                },
                            );
                        }
                    }
                    LoopMsg::Ohos(OhosEvent::MouseButton { button, pressed }) => {
                        if let Some(window) = self.shared.window() {
                            let state = if pressed {
                                crate::event::ElementState::Pressed
                            } else {
                                crate::event::ElementState::Released
                            };
                            app.window_event(
                                &active,
                                window.id(),
                                WindowEvent::MouseInput {
                                    device_id: DeviceId::dummy(),
                                    state,
                                    button,
                                },
                            );
                        }
                    }
                    LoopMsg::Ohos(OhosEvent::MouseWheel { dx, dy }) => {
                        if let Some(window) = self.shared.window() {
                            app.window_event(
                                &active,
                                window.id(),
                                WindowEvent::MouseWheel {
                                    device_id: DeviceId::dummy(),
                                    delta: MouseScrollDelta::LineDelta(dx, dy),
                                    phase: TouchPhase::Moved,
                                },
                            );
                        }
                    }
                    LoopMsg::Ohos(OhosEvent::Key { code, state, repeat, modifiers, caps_lock }) => {
                        if let Some(window) = self.shared.window()
                            && let Some(event) =
                                ohos::key_event(code, state, repeat, modifiers, caps_lock)
                        {
                            app.window_event(
                                &active,
                                window.id(),
                                WindowEvent::KeyboardInput {
                                    device_id: DeviceId::dummy(),
                                    event,
                                    is_synthetic: false,
                                },
                            );
                        }
                    }
                    LoopMsg::Ohos(OhosEvent::Ime(ime)) => {
                        if let Some(window) = self.shared.window() {
                            app.window_event(&active, window.id(), WindowEvent::Ime(ime));
                        }
                    }
                    LoopMsg::Ohos(OhosEvent::RedrawRequested) => self.shared.request_redraw(),
                }
            }

            if self.shared.redraw.swap(false, Ordering::SeqCst) {
                // A dispatched redraw is work this turn: `about_to_wait` must
                // run and set the next deadline before the loop rests. The
                // deadline it asked with can be stale (`Wait`), and honoring
                // it here would sleep through the pass that services and
                // requests the next draw.
                handled_any = true;
                if let Some(window) = self.shared.window() {
                    app.window_event(&active, window.id(), WindowEvent::RedrawRequested);
                }
            }

            if self.shared.exiting() {
                break;
            }

            if !handled_any {
                let (timeout, requested_resume) =
                    match *self.shared.control_flow.lock().unwrap() {
                        ControlFlow::Poll => (Duration::from_millis(1), None),
                        ControlFlow::Wait => (Duration::from_millis(50), None),
                        ControlFlow::WaitUntil(deadline) => (
                            deadline
                                .saturating_duration_since(Instant::now())
                                .min(Duration::from_millis(50)),
                            Some(deadline),
                        ),
                    };
                let start = Instant::now();
                thread::park_timeout(timeout);
                let now = Instant::now();
                cause = match requested_resume {
                    Some(deadline) if now >= deadline => StartCause::ResumeTimeReached {
                        start,
                        requested_resume: deadline,
                    },
                    Some(deadline) => StartCause::WaitCancelled {
                        start,
                        requested_resume: Some(deadline),
                    },
                    None => StartCause::WaitCancelled {
                        start,
                        requested_resume: None,
                    },
                };
            }

            app.about_to_wait(&active);
        }

        app.exiting(&active);
        ohos::uninstall_sink();
        Ok(())
    }
}
