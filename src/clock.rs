//! The extension's own monotonic clock.
//!
//! `quanta::Instant::now()` is a free function, so every call first checks a thread-local
//! clock override and then a global `OnceCell` before it reaches the counter. In a
//! dlopen'd cdylib that thread-local goes through `__tls_get_addr`, which costs more than
//! the timestamp itself. Holding the `Clock` here skips both, and `raw`/`delta_as_nanos`
//! keep the TSC-to-nanosecond scaling off the sub-threshold path.

use once_cell::sync::Lazy;
use quanta::Clock;

static CLOCK: Lazy<Clock> = Lazy::new(Clock::new);

// Builds the clock up front. On x86_64 `Clock::new` calibrates the TSC against the
// monotonic clock, which busy-loops for up to 200ms; left to initialise on first use that
// lands inside whichever request first calls an observed function. Called from module
// init, before FPM forks its workers, so the calibration is paid once per pool at startup
// rather than once per worker inside a live request.
pub fn init() {
    Lazy::force(&CLOCK);
}

#[inline(always)]
pub fn raw() -> u64 {
    CLOCK.raw()
}

#[inline(always)]
pub fn delta_nanos(start: u64, end: u64) -> u64 {
    CLOCK.delta_as_nanos(start, end)
}
