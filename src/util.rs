use anyhow::Context;
use phper::{arrays::ZArr, eg, pg, sys, values::ZVal};
use std::ffi::{CStr, CString};
use tracing::error;
use uuid::Uuid;

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

// Mutable variant of get_request_server for modifying $_SERVER entries.
pub fn get_request_server_mut<'a>() -> anyhow::Result<&'a mut ZArr> {
    unsafe {
        let symbol_table = ZArr::from_mut_ptr(&raw mut eg!(symbol_table));
        let carrier = symbol_table
            .get_mut("_SERVER")
            .and_then(|carrier| carrier.as_mut_z_arr())
            .context("$_SERVER is null")?;
        Ok(carrier)
    }
}

/// Ensures that HTTP_X_REQUEST_ID is set on $_SERVER.
/// If the header is not present, generates a UUID v4 and inserts it.
pub fn ensure_request_id() {
    match get_request_server_mut() {
        Ok(server) => {
            if !server.exists("HTTP_X_REQUEST_ID") {
                let id = Uuid::new_v4().to_string();
                server.insert("HTTP_X_REQUEST_ID", ZVal::from(id.as_str()));
            }
        }
        Err(err) => {
            error!("unable to ensure request id: {}", err);
        }
    }
}

// Copies bytes into a NUL-terminated CString for use as a probe argument.
//
// Every string handed to a probe must go through here. Tracers read these with
// bpf_probe_read_user_str, which scans until a NUL byte, so a pointer into a
// Rust String reads past the end of the allocation into adjacent heap.
//
// Truncates at the first interior NUL rather than failing, which is what the
// consumer would see anyway. Cannot panic: CString::new only fails on an
// interior NUL, and that has already been removed.
pub fn bytes_to_cstring(bytes: &[u8]) -> CString {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    CString::new(&bytes[..end]).unwrap_or_default()
}

// Converts a zval to a NUL-terminated CString, if it holds a string.
//
// Deliberately byte-oriented rather than going via &str: $_SERVER values are not
// guaranteed to be valid UTF-8, and a non-UTF-8 URI should still be reported
// rather than silently falling through to "/unknown".
pub fn z_val_to_cstring(zv: &ZVal) -> Option<CString> {
    zv.as_z_str().map(|zs| bytes_to_cstring(zs.to_bytes()))
}

// Joins the string values of an array with a space, as raw bytes.
pub fn join_z_arr_strings(arr: &ZArr) -> Vec<u8> {
    let mut out = Vec::new();
    for (_, v) in arr.iter() {
        if let Some(zs) = v.as_z_str() {
            if !out.is_empty() {
                out.push(b' ');
            }
            out.extend_from_slice(zs.to_bytes());
        }
    }
    out
}

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

pub fn get_pid() -> u64 {
    std::process::id() as u64
}

pub fn get_cli_command(server: &ZArr) -> CString {
    server
        .get("argv")
        .and_then(|val| val.as_z_arr())
        .map(|arr| bytes_to_cstring(&join_z_arr_strings(arr)))
        .unwrap_or_else(|| {
            server
                .get("SCRIPT_NAME")
                .and_then(z_val_to_cstring)
                .unwrap_or_else(|| c"UNKNOWN".to_owned())
        })
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
