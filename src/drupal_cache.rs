use crate::probe_str::{ProbeStr, compass_probe};
use crate::util::{get_request_id, get_request_server, z_val_to_string};
use phper::values::ZVal;
use phper::{sys, values::ExecuteData};
use std::ffi::{CStr, CString};

// Used when a function has no caller (top-level execution)
const NO_CALLER: &CStr = c"(no caller)";

// Used when the argument's class name cannot be read.
const UNKNOWN_TYPE: &CStr = c"(unknown)";

// Extracts the caller (class::method or function name) from the previous execute_data frame.
// Returns the caller's fully-qualified name, or "(no caller)" if no previous frame exists.
#[inline]
fn get_caller_name(execute_data: *mut sys::zend_execute_data) -> ProbeStr<'static> {
    let prev_ptr = unsafe { (*execute_data).prev_execute_data };
    match unsafe { ExecuteData::try_from_mut_ptr(prev_ptr) } {
        Some(prev) => {
            let name = prev.func().get_function_or_method_name();
            // SAFETY: the zend_string behind `name` is NUL-terminated. Its lifetime is
            // NOT actually 'static — `name` is dropped when this arm returns, so the
            // pointer dangles. See plans/02-dangling-caller-pointer-in-drupal-probes.md,
            // which fixes the lifetime; this pass only covers NUL termination.
            unsafe { ProbeStr::from_raw(name.as_c_str_ptr()) }
        }
        None => ProbeStr::from(NO_CALLER),
    }
}

// Extracts the type/class name from the first argument of createFromObject.
// Returns either the class name (for objects) or the base type name (for primitives).
#[inline]
fn get_arg_type_name(execute_data: &ExecuteData) -> ProbeStr<'static> {
    let arg0 = execute_data.get_parameter(0);
    let ti = arg0.get_type_info().get_base_type();

    if ti.is_object() {
        arg0.as_z_obj()
            .map(|obj| {
                // SAFETY: a class entry's name is a NUL-terminated zend_string that
                // lives as long as the class is loaded, which outlives this request.
                unsafe { ProbeStr::from_raw(obj.get_class().get_name().as_c_str_ptr()) }
            })
            .unwrap_or(ProbeStr::from(UNKNOWN_TYPE))
    } else {
        ProbeStr::from(ti.get_base_type_name())
    }
}

// Extracts an array property from a ZVal object and joins its string values with a space delimiter.
// Returns an empty string if the property doesn't exist or isn't an array.
#[inline]
fn extract_string_array_property(zval: &ZVal, property_name: &str) -> String {
    zval.as_z_obj()
        .map(|zobj| {
            let prop = zobj.get_property(property_name);
            prop.as_z_arr()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|(_, v)| z_val_to_string(v))
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

pub unsafe extern "C" fn cacheablemetadata_createfromrenderarray_observer_end(
    execute_data: *mut sys::zend_execute_data,
    return_value: *mut sys::zval,
) {
    let server = match get_request_server() {
        Ok(s) => s,
        Err(_) => return,
    };

    let request_id = get_request_id(server);

    // Extract caller before shadowing execute_data
    let caller = get_caller_name(execute_data);

    let _execute_data = match unsafe { ExecuteData::try_from_mut_ptr(execute_data) } {
        Some(data) => data,
        None => return,
    };

    let mut cache_max_age: i64 = -1;
    let mut cache_tags = String::new();
    let mut cache_contexts = String::new();

    if !return_value.is_null()
        && let Some(ret) = unsafe { ZVal::try_from_mut_ptr(return_value) }
    {
        if let Some(zobj) = ret.as_z_obj() {
            let max_age_zv = zobj.get_property("cacheMaxAge");
            if let Some(v) = max_age_zv.as_long() {
                cache_max_age = v;
            }
        }
        cache_tags = extract_string_array_property(ret, "cacheTags");
        cache_contexts = extract_string_array_property(ret, "cacheContexts");
    }

    // Convert to CStrings for probe - these must outlive the probe call
    // Use unwrap_or_else to handle potential NUL bytes in strings without panicking
    let cache_tags_cstr = CString::new(cache_tags).unwrap_or_else(|_| CString::default());
    let cache_contexts_cstr = CString::new(cache_contexts).unwrap_or_else(|_| CString::default());

    compass_probe!(
        drupal_cacheablemetadata_createfromrenderarray,
        ProbeStr::from(&request_id),
        caller,
        cache_max_age,
        ProbeStr::from(&cache_tags_cstr),
        ProbeStr::from(&cache_contexts_cstr),
    );
}

pub unsafe extern "C" fn cacheablemetadata_createfromobject_observer_end(
    execute_data: *mut sys::zend_execute_data,
    return_value: *mut sys::zval,
) {
    let server = match get_request_server() {
        Ok(s) => s,
        Err(_) => return,
    };

    let request_id = get_request_id(server);

    // Extract caller before shadowing execute_data
    let caller = get_caller_name(execute_data);

    let execute_data = match unsafe { ExecuteData::try_from_mut_ptr(execute_data) } {
        Some(data) => data,
        None => return,
    };

    let arg_type = get_arg_type_name(execute_data);

    let mut cache_max_age: i64 = -1;
    let mut cache_tags = String::new();
    let mut cache_contexts = String::new();

    if !return_value.is_null()
        && let Some(ret) = unsafe { ZVal::try_from_mut_ptr(return_value) }
    {
        if let Some(zobj) = ret.as_z_obj() {
            let max_age_zv = zobj.get_property("cacheMaxAge");
            if let Some(v) = max_age_zv.as_long() {
                cache_max_age = v;
            }
        }
        cache_tags = extract_string_array_property(ret, "cacheTags");
        cache_contexts = extract_string_array_property(ret, "cacheContexts");
    }

    // Convert to CStrings for probe - these must outlive the probe call
    // Use unwrap_or_else to handle potential NUL bytes in strings without panicking
    let cache_tags_cstr = CString::new(cache_tags).unwrap_or_else(|_| CString::default());
    let cache_contexts_cstr = CString::new(cache_contexts).unwrap_or_else(|_| CString::default());

    compass_probe!(
        drupal_cacheablemetadata_createfromobject,
        ProbeStr::from(&request_id),
        caller,
        cache_max_age,
        arg_type,
        ProbeStr::from(&cache_tags_cstr),
        ProbeStr::from(&cache_contexts_cstr),
    );
}
