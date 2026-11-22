//! OpenHarmony XComponent host for VERA20K.
//!
//! ArkUI loads this cdylib through `XComponent({ libraryname: "vera20k_ohos" })`,
//! hands it the native XComponent in the NAPI module's Init, and the game loop
//! then runs on its own thread while surface and input callbacks arrive from
//! the UI thread.

mod engine;
mod log;
mod napi;
mod xcomponent;

use core::ffi::c_void;

static mut MODULE: napi::napi_module = napi::napi_module {
    nm_version: 1,
    nm_flags: 0,
    nm_filename: core::ptr::null(),
    nm_register_func: Some(xcomponent::init),
    nm_modname: c"vera20k_ohos".as_ptr(),
    nm_priv: core::ptr::null_mut(),
    reserved: [core::ptr::null_mut(); 4],
};

extern "C" fn register_module() {
    log::init();
    // SAFETY: the module struct is a static with a stable address and the NAPI
    // runtime only reads it during registration.
    unsafe { napi::napi_module_register(&raw const MODULE) };
}

#[used]
#[unsafe(link_section = ".init_array")]
static REGISTER_MODULE: extern "C" fn() = register_module;

/// Some OpenHarmony loaders look this symbol up directly instead of running the
/// constructor; keep both paths.
#[unsafe(no_mangle)]
pub extern "C" fn napi_register_module_v1(
    env: napi::napi_env,
    exports: napi::napi_value,
) -> napi::napi_value {
    // SAFETY: the NAPI runtime guarantees both handles for the call.
    unsafe { xcomponent::init(env, exports) }
}

#[allow(dead_code)]
fn assert_ffi_types() {
    let _: *mut c_void = core::ptr::null_mut();
}
