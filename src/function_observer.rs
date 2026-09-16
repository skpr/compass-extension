use crate::clock;
use crate::threshold;
use phper::strings::ZString;
use phper::{sys, values::ExecuteData};
use std::cell::RefCell;

thread_local! {
    static FUNCTION_TIMES: RefCell<Vec<(usize, u64)>> = RefCell::new(Vec::with_capacity(32));
}

// Depth past which start times are no longer recorded. Lookup is a linear scan of the
// stack on every function return, so a runaway recursion that grew this without bound
// would slow down the rest of the request. Losing timings for the frames beyond the cap
// is the cheaper failure.
const MAX_TRACKED_FRAMES: usize = 1024;

#[inline(always)]
pub fn set_function_time(exec_ptr: *mut sys::zend_execute_data, now: u64) {
    let key = exec_ptr as usize;
    FUNCTION_TIMES.with(|stack| {
        let mut stack = stack.borrow_mut();
        if stack.len() < MAX_TRACKED_FRAMES {
            stack.push((key, now));
        }
    });
}

// Drops any start times left over from an earlier request, keeping the allocation.
//
// Entries are removed by the matching end handler, but zend_bailout (fatal error,
// exit()) longjmps straight past it, so every frame live at that moment leaks. An FPM
// worker serves thousands of requests on one thread, so without this the leaks pile up
// for the life of the worker. The keys are raw execute_data addresses, which the engine
// reuses — a stale entry matched by an unrelated later frame reports a nonsense elapsed
// time, not just a slower scan.
pub fn reset() {
    FUNCTION_TIMES.with(|stack| stack.borrow_mut().clear());
}

#[inline(always)]
pub fn take_elapsed_if_over_threshold(exec_ptr: *mut sys::zend_execute_data) -> Option<u64> {
    let key = exec_ptr as usize;
    FUNCTION_TIMES.with(|stack| {
        let mut stack = stack.borrow_mut();
        if let Some(pos) = stack.iter().rposition(|(k, _)| *k == key) {
            let (_, start) = stack.swap_remove(pos);
            let elapsed = clock::delta_nanos(start, clock::raw());
            if threshold::is_over_function_threshold(elapsed) {
                return Some(elapsed);
            }
        }
        None
    })
}

pub unsafe extern "C" fn observer_begin(execute_data: *mut sys::zend_execute_data) {
    set_function_time(execute_data, clock::raw());
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
