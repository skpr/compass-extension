mod canary;
mod cli;
mod clock;
mod drupal_cache;
mod enabled;
mod fpm;
mod function_observer;
mod observer;
mod probe_str;
mod threshold;
mod util;

use phper::{ini::Policy, modules::Module, php_get_module, sys};

// This is the entrypoint of the PHP extension.
#[php_get_module]
pub fn get_module() -> Module {
    let mut module = Module::new(
        env!("CARGO_CRATE_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_AUTHORS"),
    );

    module.add_ini(enabled::INI_CONFIG, false, Policy::All);
    module.add_ini(threshold::INI_CONFIG, 1_000_000, Policy::All);

    module.on_module_init(on_module_init);

    module.on_request_init(on_request_init);
    module.on_request_shutdown(on_request_shutdown);

    module
}

pub fn on_module_init() {
    if !enabled::is_enabled() {
        return;
    }

    // Ahead of the observer: calibrating the clock inside the first observed call would
    // stall a live request, and doing it here means FPM's workers inherit the result.
    clock::init();

    unsafe {
        sys::zend_observer_fcall_register(Some(observer::observer_instrument));
    }
}

pub fn on_request_init() {
    if !enabled::is_enabled() {
        return;
    }

    // Ahead of the canary check: a previous request may have left timers behind, and
    // that has to be cleaned up whether or not a tracer is attached for this one.
    function_observer::reset();

    if !canary::probe_enabled() {
        return;
    }

    fpm::init();
    cli::init();
}

pub fn on_request_shutdown() {
    if !enabled::is_enabled() {
        return;
    }

    if canary::probe_enabled() {
        fpm::shutdown();
        cli::shutdown();
    }

    // After the shutdown probes have read them, and outside the canary check: a worker
    // reuses this thread, so anything left here would be reported against the next
    // request if a tracer attaches part way through it.
    util::clear_request_values();
}
