use crate::probe_str::compass_probe;
use cached::proc_macro::once;
use std::time::Duration;

// Used to check if the canary probe is enabled without hitting the probe every time
// A cached function with TTL of 1 second
#[once(time = 1)]
pub fn probe_enabled() -> bool {
    compass_probe!(canary)
}
