use probe::probe_lazy;
use std::cell::Cell;

thread_local! {
    // const-initialised so reads skip the lazy-init check, and Cell<bool> has no
    // Drop so no thread-local destructor is registered.
    static TRACING: Cell<bool> = const { Cell::new(false) };
}

// Samples the canary probe and latches the result for this request.
// Returns true if a tracer is attached.
//
// Call this from request init only. probe_lazy! fires the canary probe as a side
// effect - there is no read-only variant - so it must never run on a per-function
// path. Every other probe site has its own private semaphore that we cannot read,
// which is the whole reason the canary exists.
pub fn refresh() -> bool {
    let enabled = probe_lazy!(compass, canary);
    TRACING.set(enabled);
    enabled
}

// True if a tracer was attached when this request started.
//
// Latched for the request on purpose. The value cannot change between an
// observer_begin and its matching observer_end, so gating both orphans no
// frames in the timing stack. Re-sampling mid-request would.
#[inline(always)]
pub fn is_traced() -> bool {
    TRACING.get()
}

pub fn clear() {
    TRACING.set(false);
}
