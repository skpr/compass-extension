use crate::function_observer::observe_function_end;
use crate::util::{get_cli_command, get_pid, get_sapi_module_name, init_and_get_server};

use once_cell::sync::Lazy;
use phper::sys;
use probe::probe_lazy;

static IS_CLI: Lazy<bool> = Lazy::new(|| get_sapi_module_name().to_bytes() == b"cli");

#[inline]
pub fn is_cli() -> bool {
    *IS_CLI
}

pub unsafe extern "C" fn observer_end(
    execute_data: *mut sys::zend_execute_data,
    _return_value: *mut sys::zval,
) {
    // Cheapest possible exit when nobody is listening: one thread-local load.
    if !crate::canary::is_traced() {
        return;
    }

    let obs = match observe_function_end(execute_data) {
        Some(o) => o,
        None => return,
    };

    let pid = get_pid();

    probe_lazy!(
        compass,
        cli_function,
        pid,
        obs.function_name.as_c_str_ptr(),
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

    let pid = get_pid();
    // Already NUL terminated and truncated at any interior NUL by get_cli_command.
    let command = get_cli_command(server);

    probe_lazy!(compass, cli_request_init, pid, command.as_ptr());
}

pub fn shutdown() {
    if !is_cli() {
        return;
    }

    let pid = get_pid();

    probe_lazy!(compass, cli_request_shutdown, pid);
}
