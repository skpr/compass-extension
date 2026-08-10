use crate::cli::is_cli;
use crate::dispatch::{self, Target};
use crate::fpm::is_fpm;
use phper::{sys, values::ExecuteData};

#[inline(always)]
fn handlers(
    begin: Option<unsafe extern "C" fn(*mut sys::zend_execute_data)>,
    end: Option<unsafe extern "C" fn(*mut sys::zend_execute_data, *mut sys::zval)>,
) -> sys::zend_observer_fcall_handlers {
    sys::zend_observer_fcall_handlers { begin, end }
}

// Chooses the begin/end handler pair for a function.
//
// Deliberately NOT gated on whether a tracer is attached. PHP calls this once per
// zend_function and memoises the answer in that function's run-time cache as
// ZEND_OBSERVER_NOT_OBSERVED, with no API to invalidate it. Deciding here would
// mean a tracer attaching later never instruments any function that had already
// run. The gate therefore lives inside the handlers, where canary::is_traced() is
// re-evaluated every request; the cost when untraced is a load and a branch.
//
// The SAPI checks are safe to decide here: a process cannot change SAPI.
pub unsafe extern "C" fn observer_instrument(
    execute_data: *mut sys::zend_execute_data,
) -> sys::zend_observer_fcall_handlers {
    let cli = is_cli();

    if !cli && !is_fpm() {
        return handlers(None, None);
    }

    let Some(data) = (unsafe { ExecuteData::try_from_mut_ptr(execute_data) }) else {
        return handlers(None, None);
    };

    let generic = if cli {
        handlers(
            Some(crate::function_observer::observer_begin),
            Some(crate::cli::observer_end),
        )
    } else {
        handlers(
            Some(crate::function_observer::observer_begin),
            Some(crate::fpm::observer_end),
        )
    };

    match dispatch::classify(data.func()) {
        // The Drupal probes report a request id, so they stay FPM-only. Under CLI
        // these fall through to generic function timing.
        Target::DrupalCacheableMetadataFromObject if !cli => handlers(
            None, // No need to capture start time for this probe
            Some(crate::drupal_cache::cacheablemetadata_createfromobject_observer_end),
        ),
        Target::DrupalCacheableMetadataFromRenderArray if !cli => handlers(
            None,
            Some(crate::drupal_cache::cacheablemetadata_createfromrenderarray_observer_end),
        ),
        // Query probes work under both SAPIs: drush cron, queue runners and
        // migrations are prime suspects for slow queries.
        Target::DbStatementExecute => handlers(
            Some(crate::function_observer::observer_begin),
            Some(crate::database::statement_execute_observer_end),
        ),
        Target::DbQueryArg0 => handlers(
            Some(crate::function_observer::observer_begin),
            Some(crate::database::query_arg0_observer_end),
        ),
        _ => generic,
    }
}
