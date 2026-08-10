use crate::function_observer::observe_function_end;
use crate::util::{
    ensure_request_id, get_request_id, get_request_method, get_request_server, get_request_uri,
    get_sapi_module_name, init_and_get_server,
};

use once_cell::sync::Lazy;
use phper::sys;
use probe::probe_lazy;
use tracing::error;

static IS_FPM: Lazy<bool> = Lazy::new(|| get_sapi_module_name().to_bytes() == b"fpm-fcgi");

#[inline]
pub fn is_fpm() -> bool {
    *IS_FPM
}

pub unsafe extern "C" fn observer_end(
    execute_data: *mut sys::zend_execute_data,
    _return_value: *mut sys::zval,
) {
    // Cheapest possible exit when nobody is listening: one thread-local load.
    // Everything below allocates.
    if !crate::canary::is_traced() {
        return;
    }

    let obs = match observe_function_end(execute_data) {
        Some(o) => o,
        None => return,
    };

    let server = match get_request_server() {
        Ok(s) => s,
        Err(_) => return, // Avoid logging in hot path
    };

    let request_id = get_request_id(server);

    probe_lazy!(
        compass,
        fpm_function,
        request_id.as_ptr(),
        obs.function_name.as_c_str_ptr(),
        obs.elapsed,
        obs.memory,
    );
}

pub fn init() {
    if !is_fpm() {
        return;
    }

    // Perform JIT initialization to ensure $_SERVER is materialized,
    // then generate a request ID if one was not provided via header.
    if init_and_get_server().is_none() {
        return;
    }
    ensure_request_id();

    // Re-fetch server as an immutable reference after the mutable
    // ensure_request_id() call to avoid aliasing issues.
    let server = match get_request_server() {
        Ok(s) => s,
        Err(_) => return,
    };

    let request_id = get_request_id(server);
    let uri = get_request_uri(server);
    let method = get_request_method(server);

    probe_lazy!(
        compass,
        fpm_request_init,
        request_id.as_ptr(),
        uri.as_ptr(),
        method.as_ptr()
    );
}

pub fn shutdown() {
    if !is_fpm() {
        return;
    }

    let server_result = get_request_server();

    let server = match server_result {
        Ok(carrier) => carrier,
        Err(_err) => {
            error!("unable to get server info: {}", _err);
            return;
        }
    };

    let request_id = get_request_id(server);

    probe_lazy!(compass, fpm_request_shutdown, request_id.as_ptr());
}
