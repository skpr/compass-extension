mod canary;
mod cli;
mod database;
mod dispatch;
mod drupal_cache;
mod enabled;
mod fpm;
mod function_observer;
mod observer;
mod threshold;
mod util;

use phper::{ini::Policy, modules::Module, php_get_module, sys};

// The name PHP knows this extension by. Spelled out rather than taken from
// CARGO_CRATE_NAME, which would register it as "compass_extension" - so `php -m`,
// `php --ri compass` and extension_loaded('compass') all disagreed with the
// installed compass.so, the `extension=compass` line in 00_compass.ini and the
// `compass` USDT provider.
const MODULE_NAME: &str = "compass";

// This is the entrypoint of the PHP extension.
#[php_get_module]
pub fn get_module() -> Module {
    let mut module = Module::new(
        MODULE_NAME,
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

    unsafe {
        sys::zend_observer_fcall_register(Some(observer::observer_instrument));
    }
}

pub fn on_request_init() {
    if !enabled::is_enabled() {
        return;
    }

    // Outside the tracer gate on purpose: a request during which the tracer
    // detached still needs its orphaned frame timings dropped.
    function_observer::reset();

    let was_traced = canary::is_traced();

    if !canary::refresh() {
        return;
    }

    // A tracer that has just attached never saw the earlier db_query_text probes,
    // so re-announce the SQL behind the ids it is about to receive.
    if !was_traced {
        database::forget_interned_sql();
    }

    fpm::init();
    cli::init();
}

pub fn on_request_shutdown() {
    if !enabled::is_enabled() {
        return;
    }

    // Reads the latch set at request init rather than re-sampling the canary, so
    // every request_init probe is followed by exactly one request_shutdown. A
    // mid-request attach used to emit a shutdown with no init, and a mid-request
    // detach an init with no shutdown - both break rollup downstream.
    if canary::is_traced() {
        fpm::shutdown();
        cli::shutdown();
    }

    function_observer::reset();
    canary::clear();
}
