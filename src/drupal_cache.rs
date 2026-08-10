use crate::util::{bytes_to_cstring, get_request_id, get_request_server, join_z_arr_strings};
use phper::strings::ZString;
use phper::values::ZVal;
use phper::{sys, values::ExecuteData};
use probe::probe_lazy;
use std::ffi::{CStr, CString, c_char};
use std::ptr;

// Used when a function has no caller (top-level execution)
const NO_CALLER: &CStr = c"(no caller)";

// Extracts the caller (class::method or function name) from the previous execute_data frame.
// Returns the owned name, or None if there is no previous frame.
//
// This returns the ZString rather than a pointer into it on purpose.
// get_function_or_method_name() hands back an owned ZString (EBox<ZStr>, and
// ZStr releases the underlying zend_string on drop), so a pointer taken from a
// local here would dangle before the probe ever read it. The caller must keep
// the returned value alive across the probe call - see caller_ptr below.
#[inline]
fn get_caller_name(execute_data: *mut sys::zend_execute_data) -> Option<ZString> {
    if execute_data.is_null() {
        return None;
    }
    let prev_ptr = unsafe { (*execute_data).prev_execute_data };
    let prev = unsafe { ExecuteData::try_from_mut_ptr(prev_ptr) }?;
    Some(prev.func().get_function_or_method_name())
}

// Borrows a probe-ready pointer from a caller name, falling back to NO_CALLER.
// The returned pointer is only valid while `caller` is alive.
#[inline]
fn caller_ptr(caller: &Option<ZString>) -> *const c_char {
    match caller {
        Some(name) => name.as_c_str_ptr(),
        None => NO_CALLER.as_ptr(),
    }
}

// Extracts the type/class name from the first argument of createFromObject.
// Returns a pointer to a C string representing either the class name (for objects)
// or the base type name (for primitives).
#[inline]
fn get_arg_type_name(execute_data: &ExecuteData) -> *const c_char {
    let arg0 = execute_data.get_parameter(0);
    let ti = arg0.get_type_info().get_base_type();

    if ti.is_object() {
        arg0.as_z_obj()
            .map(|obj| obj.get_class().get_name().as_c_str_ptr())
            .unwrap_or(ptr::null())
    } else {
        ti.get_base_type_name().as_ptr()
    }
}

// Extracts an array property from a ZVal object and joins its string values with a space delimiter.
// Returns an empty CString if the property doesn't exist or isn't an array.
#[inline]
fn extract_string_array_property(zval: &ZVal, property_name: &str) -> CString {
    let bytes = zval
        .as_z_obj()
        .map(|zobj| {
            let prop = zobj.get_property(property_name);
            prop.as_z_arr().map(join_z_arr_strings).unwrap_or_default()
        })
        .unwrap_or_default();

    bytes_to_cstring(&bytes)
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

    // Extract caller before shadowing execute_data. Held as an owned value for
    // the rest of this function so the probe can read through it.
    let caller = get_caller_name(execute_data);

    let _execute_data = match unsafe { ExecuteData::try_from_mut_ptr(execute_data) } {
        Some(data) => data,
        None => return,
    };

    // Bound to locals so they outlive the probe call below.
    let mut cache_max_age: i64 = -1;
    let mut cache_tags = CString::default();
    let mut cache_contexts = CString::default();

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

    probe_lazy!(
        compass,
        drupal_cacheablemetadata_createfromrenderarray,
        request_id.as_ptr(),
        caller_ptr(&caller),
        cache_max_age,
        cache_tags.as_ptr(),
        cache_contexts.as_ptr(),
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

    // Extract caller before shadowing execute_data. Held as an owned value for
    // the rest of this function so the probe can read through it.
    let caller = get_caller_name(execute_data);

    let execute_data = match unsafe { ExecuteData::try_from_mut_ptr(execute_data) } {
        Some(data) => data,
        None => return,
    };

    let arg_type_cstr_ptr = get_arg_type_name(execute_data);

    // Bound to locals so they outlive the probe call below.
    let mut cache_max_age: i64 = -1;
    let mut cache_tags = CString::default();
    let mut cache_contexts = CString::default();

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

    probe_lazy!(
        compass,
        drupal_cacheablemetadata_createfromobject,
        request_id.as_ptr(),
        caller_ptr(&caller),
        cache_max_age,
        arg_type_cstr_ptr,
        cache_tags.as_ptr(),
        cache_contexts.as_ptr(),
    );
}
