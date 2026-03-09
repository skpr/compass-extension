use crate::function_observer::observe_function_end;
use crate::util::{get_cli_command, get_pid, get_sapi_module_name, init_and_get_server};

use once_cell::sync::Lazy;
use phper::sys;
use probe::probe_lazy;
use std::ffi::CString;

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
    let command = get_cli_command(server);
    let command_cstr = CString::new(command).unwrap_or_else(|_| CString::default());

    probe_lazy!(compass, cli_init, pid, command_cstr.as_ptr());
}

pub fn shutdown() {
    if !is_cli() {
        return;
    }

    let pid = get_pid();

    probe_lazy!(compass, cli_shutdown, pid);
}
