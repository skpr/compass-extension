use crate::function_observer::observe_function_end;
use crate::probe_str::{ProbeStr, compass_probe};
use crate::util::{
    get_request_id, get_request_method, get_request_server, get_request_uri, get_sapi_module_name,
    init_and_get_server,
};

use once_cell::sync::Lazy;
use phper::sys;
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
    let obs = match observe_function_end(execute_data) {
        Some(o) => o,
        None => return,
    };

    let server = match get_request_server() {
        Ok(s) => s,
        Err(_) => return, // Avoid logging in hot path
    };

    let request_id = get_request_id(server);

    compass_probe!(
        fpm_function,
        ProbeStr::from(&request_id),
        ProbeStr::from_zend_str(&obs.function_name),
        obs.elapsed,
        obs.memory,
    );
}

pub fn init() {
    if !is_fpm() {
        return;
    }

    let server = match init_and_get_server() {
        Some(s) => s,
        None => return,
    };

    let request_id = get_request_id(server);
    let uri = get_request_uri(server);
    let method = get_request_method(server);

    compass_probe!(
        fpm_request_init,
        ProbeStr::from(&request_id),
        ProbeStr::from(&uri),
        ProbeStr::from(&method),
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

    compass_probe!(fpm_request_shutdown, ProbeStr::from(&request_id));
}
