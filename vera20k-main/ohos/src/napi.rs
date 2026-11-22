//! Minimal NAPI declarations used by the XComponent module.
#![allow(non_camel_case_types, non_snake_case, dead_code)]

use core::ffi::{c_char, c_int, c_uint, c_void};

pub type napi_env = *mut c_void;
pub type napi_value = *mut c_void;
pub type napi_callback_info = *mut c_void;
pub type napi_status = c_int;

pub const NAPI_OK: napi_status = 0;

pub type napi_callback =
    Option<unsafe extern "C" fn(env: napi_env, info: napi_callback_info) -> napi_value>;

#[repr(C)]
pub struct napi_module {
    pub nm_version: c_int,
    pub nm_flags: c_uint,
    pub nm_filename: *const c_char,
    pub nm_register_func: Option<unsafe extern "C" fn(env: napi_env, exports: napi_value) -> napi_value>,
    pub nm_modname: *const c_char,
    pub nm_priv: *mut c_void,
    pub reserved: [*mut c_void; 4],
}

#[link(name = "ace_napi.z")]
unsafe extern "C" {
    pub fn napi_module_register(module: *const napi_module);
    pub fn napi_get_named_property(
        env: napi_env,
        object: napi_value,
        utf8_name: *const c_char,
        result: *mut napi_value,
    ) -> napi_status;
    pub fn napi_unwrap(
        env: napi_env,
        js_object: napi_value,
        result: *mut *mut c_void,
    ) -> napi_status;
    pub fn napi_create_function(
        env: napi_env,
        utf8_name: *const c_char,
        length: usize,
        callback: napi_callback,
        data: *mut c_void,
        result: *mut napi_value,
    ) -> napi_status;
    pub fn napi_set_named_property(
        env: napi_env,
        object: napi_value,
        utf8_name: *const c_char,
        value: napi_value,
    ) -> napi_status;
    pub fn napi_get_cb_info(
        env: napi_env,
        info: napi_callback_info,
        argc: *mut usize,
        argv: *mut napi_value,
        this_arg: *mut napi_value,
        data: *mut *mut c_void,
    ) -> napi_status;
    pub fn napi_get_value_string_utf8(
        env: napi_env,
        value: napi_value,
        buf: *mut c_char,
        bufsize: usize,
        result: *mut usize,
    ) -> napi_status;
    pub fn napi_get_value_int32(
        env: napi_env,
        value: napi_value,
        result: *mut i32,
    ) -> napi_status;
    pub fn napi_get_value_bool(
        env: napi_env,
        value: napi_value,
        result: *mut bool,
    ) -> napi_status;
}
