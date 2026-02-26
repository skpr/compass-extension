use crate::canary::probe_enabled;
use crate::fpm::is_fpm;
use phper::{sys, values::ExecuteData};
use std::ffi::CStr;

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
    if !probe_enabled() || !is_fpm() {
        return handlers(None, None);
    }

    let data = match unsafe { ExecuteData::try_from_mut_ptr(execute_data) } {
        Some(d) => d,
        None => {
            return handlers(None, None);
        }
    };

    let name = data.func().get_function_or_method_name();

    // Convert ZStr -> &[u8] via CStr using the raw pointer.
    let name_bytes: &[u8] = unsafe { CStr::from_ptr(name.as_c_str_ptr()) }.to_bytes();

    // Used to determine what max age headers we are getting from Drupal objects.
    if name_bytes == b"Drupal\\Core\\Cache\\CacheableMetadata::createFromObject" {
        return handlers(
            None, // No need to capture start time for this probe
            Some(crate::drupal_cache::cacheablemetadata_createfromobject_observer_end),
        );
    }

    // Used to determine what max age headers we are getting from Drupal objects.
    if name_bytes == b"Drupal\\Core\\Cache\\CacheableMetadata::createFromRenderArray" {
        return handlers(
            None, // No need to capture start time for this probe
            Some(crate::drupal_cache::cacheablemetadata_createfromrenderarray_observer_end),
        );
    }

    // Default function instrumentation
    handlers(
        Some(crate::php_function::observer_begin),
        Some(crate::php_function::observer_end),
    )
}
