//! The application callbacks driven by `EventLoop::run_app`.

use crate::event::{StartCause, WindowEvent};
use crate::event_loop::ActiveEventLoop;
use crate::window::WindowId;

pub trait ApplicationHandler<T: 'static = ()> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop);

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent);

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: StartCause) {}

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: T) {}

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _device_id: crate::event::DeviceId, _event: crate::event::DeviceEvent) {}

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {}

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {}

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {}

    fn memory_warning(&mut self, _event_loop: &ActiveEventLoop) {}
}
