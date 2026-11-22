//! A source-compatible subset of winit 0.30 for OpenHarmony 2in1.
//!
//! ArkUI owns the process's only UI thread, so `EventLoop::run_app` does not
//! pump the OS queue itself: XComponent callbacks (surface lifecycle, input,
//! focus) arrive on the UI thread and are forwarded through
//! [`platform::ohos::send_event`]. The game loop runs on the thread that calls
//! `run_app`, normally a dedicated render thread started by the NAPI module.
//!
//! On every other target this crate is only a facade: it re-exports the
//! official winit unchanged, so the workspace can name one canonical `winit`
//! dependency for all targets.

#[cfg(not(target_env = "ohos"))]
pub use winit_official::*;

#[cfg(target_env = "ohos")]
pub mod application;
#[cfg(target_env = "ohos")]
pub mod dpi;
#[cfg(target_env = "ohos")]
pub mod error;
#[cfg(target_env = "ohos")]
pub mod event;
#[cfg(target_env = "ohos")]
pub mod event_loop;
#[cfg(target_env = "ohos")]
pub mod keyboard;
#[cfg(target_env = "ohos")]
pub mod monitor;
#[cfg(target_env = "ohos")]
pub mod platform;
#[cfg(target_env = "ohos")]
pub mod window;

#[cfg(target_env = "ohos")]
pub use event::WindowEvent;
#[cfg(target_env = "ohos")]
pub use event_loop::EventLoop;
#[cfg(target_env = "ohos")]
pub use window::Window;
