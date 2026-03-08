use crate::fpm_function::{set_function_time, take_elapsed_if_over_threshold};
use crate::util::get_pid;
use phper::{sys, values::ExecuteData};
use probe::probe_lazy;
use quanta::Instant;

pub unsafe extern "C" fn observer_begin(execute_data: *mut sys::zend_execute_data) {
    set_function_time(execute_data, Instant::now());
}

pub unsafe extern "C" fn observer_end(
    execute_data: *mut sys::zend_execute_data,
    _return_value: *mut sys::zval,
) {
    let elapsed = match take_elapsed_if_over_threshold(execute_data) {
        Some(e) => e,
        None => return,
    };

    let pid = get_pid();

    // Explicit unsafe block as required in Rust 2024
    let execute_data = match unsafe { ExecuteData::try_from_mut_ptr(execute_data) } {
        Some(data) => data,
        None => return,
    };

    let function_name = execute_data.func().get_function_or_method_name();

    let memory = unsafe { sys::zend_memory_usage(false) } as u64;

    probe_lazy!(
        compass,
        cli_function,
        pid,
        function_name.as_c_str_ptr(),
        elapsed,
        memory,
    );
}
