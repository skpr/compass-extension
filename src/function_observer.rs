use crate::threshold;
use phper::strings::ZString;
use phper::{sys, values::ExecuteData};
use quanta::Instant;
use std::cell::RefCell;

thread_local! {
    static FUNCTION_TIMES: RefCell<Vec<(usize, Instant)>> = RefCell::new(Vec::with_capacity(32));
}

#[inline(always)]
pub fn set_function_time(exec_ptr: *mut sys::zend_execute_data, now: Instant) {
    let key = exec_ptr as usize;
    FUNCTION_TIMES.with(|stack| stack.borrow_mut().push((key, now)));
}

#[inline(always)]
pub fn take_elapsed_if_over_threshold(exec_ptr: *mut sys::zend_execute_data) -> Option<u64> {
    let key = exec_ptr as usize;
    FUNCTION_TIMES.with(|stack| {
        let mut stack = stack.borrow_mut();
        if let Some(pos) = stack.iter().rposition(|(k, _)| *k == key) {
            let (_, start) = stack.swap_remove(pos);
            let elapsed = start.elapsed().as_nanos() as u64;
            if threshold::is_over_function_threshold(elapsed) {
                return Some(elapsed);
            }
        }
        None
    })
}

pub unsafe extern "C" fn observer_begin(execute_data: *mut sys::zend_execute_data) {
    set_function_time(execute_data, Instant::now());
}

pub struct FunctionObservation {
    pub elapsed: u64,
    pub function_name: ZString,
    pub memory: u64,
}

pub fn observe_function_end(
    execute_data: *mut sys::zend_execute_data,
) -> Option<FunctionObservation> {
    let elapsed = take_elapsed_if_over_threshold(execute_data)?;

    // Explicit unsafe block as required in Rust 2024
    let execute_data = unsafe { ExecuteData::try_from_mut_ptr(execute_data) }?;

    let function_name = execute_data.func().get_function_or_method_name();

    let memory = unsafe { sys::zend_memory_usage(false) } as u64;

    Some(FunctionObservation {
        elapsed,
        function_name,
        memory,
    })
}
