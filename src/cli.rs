use crate::function_observer::observe_function_end;
use crate::probe_str::{ProbeStr, compass_probe};
use crate::util::{
    cache_request_values, get_cli_command, get_pid, get_sapi_module_name, init_and_get_server,
};

use once_cell::sync::Lazy;
use phper::sys;

static IS_CLI: Lazy<bool> = Lazy::new(|| get_sapi_module_name().to_bytes() == b"cli");

#[inline]
pub fn is_cli() -> bool {
    *IS_CLI
}

pub unsafe extern "C" fn observer_end(
    execute_data: *mut sys::zend_execute_data,
    _return_value: *mut sys::zval,
) {
    let obs = match observe_function_end(execute_data) {
        Some(o) => o,
        None => return,
    };

    let pid = get_pid();

    compass_probe!(
        cli_function,
        pid,
        ProbeStr::from_zend_str(&obs.function_name),
        obs.elapsed,
        obs.memory,
    );
}

pub fn init() {
    if !is_cli() {
        return;
    }

    let server = match init_and_get_server() {
        Some(s) => s,
        None => return,
    };

    cache_request_values(server);

    let pid = get_pid();
    let command = get_cli_command(server);

    compass_probe!(cli_request_init, pid, ProbeStr::from(&command));
}

pub fn shutdown() {
    if !is_cli() {
        return;
    }

    let pid = get_pid();

    compass_probe!(cli_request_shutdown, pid);
}
