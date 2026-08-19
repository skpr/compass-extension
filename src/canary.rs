use crate::probe_str::compass_probe;
use quanta::Instant;
use std::cell::Cell;
use std::time::Duration;

// How long an answer is reused before the canary probe is fired again. Attaching or
// detaching a tracer therefore takes effect within a second.
const TTL: Duration = Duration::from_secs(1);

thread_local! {
    // Deliberately per-thread rather than a shared cache: observer_instrument consults
    // this on every PHP function call, so anything synchronised would put a lock on the
    // hottest path in the extension. Empty until the first call of a thread, which is
    // what makes that call fire the probe.
    static STATE: Cell<Option<(bool, Instant)>> = const { Cell::new(None) };
}

// Reports whether a tracer is listening, cheaply enough to ask on every function call.
#[inline]
pub fn probe_enabled() -> bool {
    STATE.with(|state| {
        let now = Instant::now();

        if let Some((enabled, checked_at)) = state.get()
            && now.duration_since(checked_at) < TTL
        {
            return enabled;
        }

        let enabled = compass_probe!(canary);
        state.set(Some((enabled, now)));
        enabled
    })
}
