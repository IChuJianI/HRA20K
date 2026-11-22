//! hilog-backed logger, mirrored into the app sandbox so the ArkTS host can
//! show native diagnostics while no debugger is attached.

use std::ffi::{CString, c_char};
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::sync::{Mutex, Once};

use log::{Level, LevelFilter, Log, Metadata, Record};

const LOG_APP: i32 = 0;
const DOMAIN: u32 = 0xD0_0D_20_4B;
/// The HAP module's `files` directory; also the engine's data directory.
pub(crate) const FILES_DIR: &str = "/data/storage/el2/base/haps/entry/files";
const LOG_PATH: &str = "/data/storage/el2/base/haps/entry/files/vera20k.log";

#[link(name = "hilog_ndk.z")]
unsafe extern "C" {
    fn OH_LOG_Print(
        log_type: i32,
        level: i32,
        domain: u32,
        tag: *const c_char,
        format: *const c_char,
        ...
    ) -> i32;
}

struct Hilog;

static HILOG: Hilog = Hilog;
static INIT: Once = Once::new();
static FILE: Mutex<Option<File>> = Mutex::new(None);

pub fn init() {
    INIT.call_once(|| {
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(LOG_PATH)
            .ok();
        if let Ok(mut guard) = FILE.lock() {
            *guard = file;
        }
        let _ = log::set_logger(&HILOG);
        log::set_max_level(LevelFilter::Debug);
        std::panic::set_hook(Box::new(|info| {
            log::error!("PANIC: {info}");
        }));
        log::info!("VERA20K OHOS host starting");
    });
}

impl Log for Hilog {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        // The engine's own file logger runs `info,wgpu_hal=error`; keep the
        // same filter so debug floods and per-frame wgpu_hal warnings stay
        // out of hilog and the mirrored file.
        if metadata.target().starts_with("wgpu_hal") {
            return metadata.level() <= Level::Error;
        }
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let level = match record.level() {
            Level::Error => 6,
            Level::Warn => 5,
            Level::Info => 4,
            Level::Debug => 3,
            Level::Trace => 2,
        };
        let mut message = String::new();
        let _ = write!(message, "{}", record.args());
        let message = message.replace('\0', " ");
        if let Ok(mut guard) = FILE.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = writeln!(file, "[{}] {message}", record.level());
                let _ = file.flush();
            }
        }
        let mut end = message.len().min(3500);
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        let Ok(message) = CString::new(&message[..end]) else {
            return;
        };
        // SAFETY: both strings are NUL-terminated and live for the call.
        unsafe {
            OH_LOG_Print(
                LOG_APP,
                level,
                DOMAIN,
                c"Vera20k".as_ptr(),
                c"%{public}s".as_ptr(),
                message.as_ptr(),
            );
        }
    }

    fn flush(&self) {}
}