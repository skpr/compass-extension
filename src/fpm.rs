use crate::function_observer::observe_function_end;
use crate::probe_str::{ProbeStr, compass_probe};
use crate::util::{
    cache_request_values, get_request_method, get_request_uri, get_sapi_module_name,
    init_and_get_server, with_request_id,
};

use once_cell::sync::Lazy;
use phper::sys;

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

    with_request_id(|request_id| {
        compass_probe!(
            fpm_function,
            request_id,
            ProbeStr::from_zend_str(&obs.function_name),
            obs.elapsed,
            obs.memory,
        );
    });
}

pub fn init() {
    if !is_fpm() {
        return;
    }

    let server = match init_and_get_server() {
        Some(s) => s,
        None => return,
    };

    cache_request_values(server);

    let uri = get_request_uri(server);
    let method = get_request_method(server);

    with_request_id(|request_id| {
        compass_probe!(
            fpm_request_init,
            request_id,
            ProbeStr::from(&uri),
            ProbeStr::from(&method),
        );
    });
}

pub fn shutdown() {
    if !is_fpm() {
        return;
    }

    with_request_id(|request_id| {
        compass_probe!(fpm_request_shutdown, request_id);
    });
}
