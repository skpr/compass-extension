use crate::util::{get_cli_command, get_pid, get_request_server, jit_initialization};

use crate::cli::is_cli;

use probe::probe_lazy;
use std::ffi::CString;
use tracing::error;

pub fn init() {
    if !is_cli() {
        return;
    }

    jit_initialization();

    let server_result = get_request_server();

    let server = match server_result {
        Ok(carrier) => carrier,
        Err(_err) => {
            error!("unable to get server info: {}", _err);
            return;
        }
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
