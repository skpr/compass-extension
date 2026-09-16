mod canary;
mod cli;
mod clock;
mod config;
mod drupal_cache;
mod enabled;
mod fpm;
mod function_observer;
mod observer;
mod probe_str;
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

    // The path only, not the settings behind it: those live in the file so they can be
    // changed on a running fleet, and the file is optional.
    module.add_ini(
        config::INI_CONFIG,
        config::DEFAULT_CONFIG_FILE.to_owned(),
        Policy::All,
    );

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

    // Same reason, and it needs the clock: the first read of the live config happens
    // here rather than inside whichever request would otherwise have paid for it.
    config::init();

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

    // After the canary rather than before it: checking the live config costs a syscall
    // once an interval, and there is nothing to suspend while no tracer is listening.
    if config::is_suspended() {
        return;
    }

    fpm::init();
    cli::init();
}

pub fn on_request_shutdown() {
    if !enabled::is_enabled() {
        return;
    }

    if canary::probe_enabled() && !config::is_suspended() {
        fpm::shutdown();
        cli::shutdown();
    }

    // After the shutdown probes have read them, and outside the canary check: a worker
    // reuses this thread, so anything left here would be reported against the next
    // request if a tracer attaches part way through it.
    util::clear_request_values();
}
