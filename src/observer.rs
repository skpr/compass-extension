use crate::canary::probe_enabled;
use crate::cli::is_cli;
use crate::config;
use crate::fpm::is_fpm;
use phper::{sys, values::ExecuteData};

// The Drupal class both cacheability probes hang off.
const CACHEABLE_METADATA: &[u8] = b"Drupal\\Core\\Cache\\CacheableMetadata";

#[inline(always)]
fn handlers(
    begin: Option<unsafe extern "C" fn(*mut sys::zend_execute_data)>,
    end: Option<unsafe extern "C" fn(*mut sys::zend_execute_data, *mut sys::zval)>,
) -> sys::zend_observer_fcall_handlers {
    sys::zend_observer_fcall_handlers { begin, end }
}

pub unsafe extern "C" fn observer_instrument(
    execute_data: *mut sys::zend_execute_data,
) -> sys::zend_observer_fcall_handlers {
    if !probe_enabled() || config::is_suspended() {
        return handlers(None, None);
    }

    if is_cli() {
        // CLI: only generic function tracing, no Drupal-specific probes.
        return handlers(
            Some(crate::function_observer::observer_begin),
            Some(crate::cli::observer_end),
        );
    }

    if !is_fpm() {
        return handlers(None, None);
    }

    let data = match unsafe { ExecuteData::try_from_mut_ptr(execute_data) } {
        Some(d) => d,
        None => {
            return handlers(None, None);
        }
    };

    let func = data.func();

    // Matched as method name first and class name second, rather than through
    // get_function_or_method_name: that builds "Class::method" into a freshly allocated
    // zend_string, and this runs for every distinct function a request touches. The two
    // comparisons are equivalent to the one they replace, because "::" cannot occur in
    // either half, and they allocate nothing.
    if let Some(method) = func.get_function_name() {
        // Used to determine what max age headers we are getting from Drupal objects.
        let end: Option<unsafe extern "C" fn(*mut sys::zend_execute_data, *mut sys::zval)> =
            match method.to_bytes() {
                b"createFromObject" => {
                    Some(crate::drupal_cache::cacheablemetadata_createfromobject_observer_end)
                }
                b"createFromRenderArray" => {
                    Some(crate::drupal_cache::cacheablemetadata_createfromrenderarray_observer_end)
                }
                _ => None,
            };

        if let Some(end) = end
            && func
                .get_class()
                .is_some_and(|class| class.get_name().to_bytes() == CACHEABLE_METADATA)
        {
            // No need to capture start time for these probes.
            return handlers(None, Some(end));
        }
    }

    // Default function instrumentation
    handlers(
        Some(crate::function_observer::observer_begin),
        Some(crate::fpm::observer_end),
    )
}
