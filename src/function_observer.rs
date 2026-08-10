use crate::canary;
use crate::threshold;
use phper::strings::ZString;
use phper::{sys, values::ExecuteData};
use quanta::Instant;
use std::cell::RefCell;

// Frames retained across a request boundary. One pathological request should not
// pin a large buffer for the rest of a worker's life.
const MAX_RETAINED: usize = 256;

thread_local! {
    static FUNCTION_TIMES: RefCell<Vec<(usize, Instant)>> = RefCell::new(Vec::with_capacity(32));
}

// Drops frame timings left over from a previous request.
//
// observer_begin pushes a frame that only its matching observer_end pops. PHP
// does still run end handlers while unwinding an exception, but a zend_bailout -
// fatal error, memory limit, execution timeout, and exit()/die() on PHP 8.3 and
// earlier - longjmps straight past them and orphans every live frame. Drupal hits
// exit() on redirects and the batch API. FPM workers serve thousands of requests,
// so without this the vector grows without bound and the rposition scan below
// degrades from O(1) to O(n).
//
// Call this unconditionally at request init, not behind the tracer gate: a
// request during which the tracer detached still needs its leftovers dropped.
pub fn reset() {
    FUNCTION_TIMES.with(|stack| {
        if let Ok(mut stack) = stack.try_borrow_mut() {
            stack.clear();
            stack.shrink_to(MAX_RETAINED);
        }
    });
}

// try_borrow_mut rather than borrow_mut throughout: a BorrowMutError would be the
// only panic this crate can raise, and panic = "abort" would take the PHP worker
// down with it. Skipping a measurement is the strictly better failure.
#[inline(always)]
pub fn set_function_time(exec_ptr: *mut sys::zend_execute_data, now: Instant) {
    let key = exec_ptr as usize;
    FUNCTION_TIMES.with(|stack| {
        if let Ok(mut stack) = stack.try_borrow_mut() {
            stack.push((key, now));
        }
    });
}

// Pops this frame's start time and returns the elapsed nanoseconds.
//
// Searches from the top: for a balanced stack the match is the last entry, and
// with recursion or reused execute_data addresses (fibers, generators) the
// innermost frame is the right one to attribute.
#[inline(always)]
pub fn take_elapsed(exec_ptr: *mut sys::zend_execute_data) -> Option<u64> {
    let key = exec_ptr as usize;
    FUNCTION_TIMES.with(|stack| {
        let mut stack = stack.try_borrow_mut().ok()?;
        let pos = stack.iter().rposition(|(k, _)| *k == key)?;
        let (_, start) = stack.swap_remove(pos);
        Some(start.elapsed().as_nanos() as u64)
    })
}

#[inline(always)]
pub fn take_elapsed_if_over_threshold(exec_ptr: *mut sys::zend_execute_data) -> Option<u64> {
    let elapsed = take_elapsed(exec_ptr)?;
    threshold::is_over_function_threshold(elapsed).then_some(elapsed)
}

pub unsafe extern "C" fn observer_begin(execute_data: *mut sys::zend_execute_data) {
    // The gate lives here rather than in observer_instrument, because PHP
    // memoises the handler decision per zend_function and never revisits it.
    if !canary::is_traced() {
        return;
    }
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
