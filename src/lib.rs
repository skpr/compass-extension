mod canary;
mod cli;
mod cli_function;
mod cli_request;
mod drupal_cache;
mod enabled;
mod fpm;
mod fpm_function;
mod observer;
mod request;
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

    unsafe {
        sys::zend_observer_fcall_register(Some(observer::observer_instrument));
    }
}

pub fn on_request_init() {
    if !enabled::is_enabled() {
        return;
    }

    if !canary::probe_enabled() {
        return;
    }

    request::init();
    cli_request::init();
}

pub fn on_request_shutdown() {
    if !enabled::is_enabled() {
        return;
    }

    if !canary::probe_enabled() {
        return;
    }

    request::shutdown();
    cli_request::shutdown();
}
