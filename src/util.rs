use anyhow::Context;
use phper::{arrays::ZArr, eg, pg, sys, values::ZVal};
use std::ffi::{CStr, CString};
use tracing::error;

// https://github.com/apache/skywalking-php/blob/master/src/request.rs#L93
pub fn jit_initialization() {
    unsafe {
        let jit_initialization: u8 = pg!(auto_globals_jit).into();
        if jit_initialization != 0 {
            let mut server = "_SERVER".to_string();
            sys::zend_is_auto_global_str(server.as_mut_ptr().cast(), server.len());
        }
    }
}

// https://github.com/apache/skywalking-php/blob/master/src/request.rs#L152
pub fn get_request_server<'a>() -> anyhow::Result<&'a ZArr> {
    unsafe {
        let symbol_table = ZArr::from_mut_ptr(&raw mut eg!(symbol_table));
        let carrier = symbol_table
            .get("_SERVER")
            .and_then(|carrier| carrier.as_z_arr())
            .context("$_SERVER is null")?;
        Ok(carrier)
    }
}

// These accessors return `CString` rather than `String` because their only consumer is
// a USDT probe argument, which the tracer reads as a NUL-terminated C string.

// Based off: https://github.com/apache/skywalking-php/blob/master/src/request.rs#L145C4-L145C27
pub fn get_request_id(server: &ZArr) -> CString {
    server
        .get("HTTP_X_REQUEST_ID")
        .and_then(z_val_to_cstring)
        .unwrap_or_else(|| c"UNKNOWN".to_owned())
}

pub fn get_request_uri(server: &ZArr) -> CString {
    server
        .get("REQUEST_URI")
        .and_then(z_val_to_cstring)
        .or_else(|| server.get("PHP_SELF").and_then(z_val_to_cstring))
        .or_else(|| server.get("SCRIPT_NAME").and_then(z_val_to_cstring))
        .unwrap_or_else(|| c"/unknown".to_owned())
}

pub fn get_request_method(server: &ZArr) -> CString {
    server
        .get("REQUEST_METHOD")
        .and_then(z_val_to_cstring)
        .unwrap_or_else(|| c"UNKNOWN".to_owned())
}

// https://github.com/apache/skywalking-php/blob/master/src/util.rs#L63
pub fn z_val_to_string(zv: &ZVal) -> Option<String> {
    zv.as_z_str()
        .and_then(|zs| zs.to_str().ok())
        .map(|s| s.to_string())
}

// A PHP string is binary safe, so it can hold an interior NUL that has no C string
// representation. Those values are treated as absent so callers fall through to their
// default rather than panicking in a request path.
pub fn z_val_to_cstring(zv: &ZVal) -> Option<CString> {
    zv.as_z_str()
        .and_then(|zs| zs.to_str().ok())
        .and_then(|s| CString::new(s).ok())
}

pub fn get_pid() -> u64 {
    std::process::id() as u64
}

pub fn get_cli_command(server: &ZArr) -> CString {
    server
        .get("argv")
        .and_then(|val| val.as_z_arr())
        .map(|arr| {
            arr.iter()
                .filter_map(|(_, v)| z_val_to_string(v))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .and_then(|command| CString::new(command).ok())
        .or_else(|| server.get("SCRIPT_NAME").and_then(z_val_to_cstring))
        .unwrap_or_else(|| c"UNKNOWN".to_owned())
}

// https://github.com/apache/skywalking-php/blob/master/src/util.rs#L86C1-L88C2
pub fn get_sapi_module_name() -> &'static CStr {
    unsafe { CStr::from_ptr(sys::sapi_module.name) }
}

/// Combines JIT initialization with server retrieval and error logging.
/// Used by both CLI and FPM request init paths.
pub fn init_and_get_server<'a>() -> Option<&'a ZArr> {
    jit_initialization();
    match get_request_server() {
        Ok(server) => Some(server),
        Err(err) => {
            error!("unable to get server info: {}", err);
            None
        }
    }
}
