use crate::probe_str::{ProbeStr, compass_probe};
use crate::util::{with_request_id, z_val_to_string};
use phper::strings::ZString;
use phper::values::ZVal;
use phper::{sys, values::ExecuteData};
use std::ffi::{CStr, CString};

// Used when a function has no caller (top-level execution)
const NO_CALLER: &CStr = c"(no caller)";

// Used when the argument's class name cannot be read.
const UNKNOWN_TYPE: &CStr = c"(unknown)";

// Extracts the caller (class::method or function name) from the previous execute_data frame.
// Returns None when there is no previous frame (top-level execution).
//
// The owning ZString is returned rather than a ProbeStr: the name has to stay alive until
// the probe has read it, and only the call site knows how long that is.
#[inline]
fn get_caller_name(execute_data: *mut sys::zend_execute_data) -> Option<ZString> {
    let prev_ptr = unsafe { (*execute_data).prev_execute_data };
    let prev = unsafe { ExecuteData::try_from_mut_ptr(prev_ptr) }?;
    Some(prev.func().get_function_or_method_name())
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

// The cacheability fields both observers report. Reading them lives in one place so a
// correction cannot land in one observer and quietly miss the other.
struct CacheableMetadata {
    max_age: i64,
    tags: CString,
    contexts: CString,
}

// Reads the cacheability metadata off an observer's return value. A missing or unreadable
// value yields the defaults the probes have always reported: -1 and empty strings.
//
// # Safety
//
// `return_value` must be null or point to a zval that stays valid for the call.
unsafe fn extract_cacheable_metadata(return_value: *mut sys::zval) -> CacheableMetadata {
    let mut max_age: i64 = -1;
    let mut tags = String::new();
    let mut contexts = String::new();

    if !return_value.is_null()
        && let Some(ret) = unsafe { ZVal::try_from_mut_ptr(return_value) }
    {
        if let Some(zobj) = ret.as_z_obj() {
            let max_age_zv = zobj.get_property("cacheMaxAge");
            if let Some(v) = max_age_zv.as_long() {
                max_age = v;
            }
        }
        tags = extract_string_array_property(ret, "cacheTags");
        contexts = extract_string_array_property(ret, "cacheContexts");
    }

    // Converted here rather than at the probe so the owner outlives the probe call. A PHP
    // string is binary safe, so a value with an interior NUL has no C representation and
    // becomes empty rather than panicking in a request path.
    CacheableMetadata {
        max_age,
        tags: CString::new(tags).unwrap_or_default(),
        contexts: CString::new(contexts).unwrap_or_default(),
    }
}

pub unsafe extern "C" fn cacheablemetadata_createfromrenderarray_observer_end(
    execute_data: *mut sys::zend_execute_data,
    return_value: *mut sys::zval,
) {
    // `caller_name` owns the bytes the probe reads, so it has to stay bound until after
    // compass_probe! fires.
    let caller_name = get_caller_name(execute_data);
    let caller = match &caller_name {
        Some(name) => ProbeStr::from_zend_str(name),
        None => ProbeStr::from(NO_CALLER),
    };

    let metadata = unsafe { extract_cacheable_metadata(return_value) };

    with_request_id(|request_id| {
        compass_probe!(
            drupal_cacheablemetadata_createfromrenderarray,
            request_id,
            caller,
            metadata.max_age,
            ProbeStr::from(&metadata.tags),
            ProbeStr::from(&metadata.contexts),
        );
    });
}

pub unsafe extern "C" fn cacheablemetadata_createfromobject_observer_end(
    execute_data: *mut sys::zend_execute_data,
    return_value: *mut sys::zval,
) {
    // Extract caller before shadowing execute_data. `caller_name` owns the bytes the
    // probe reads, so it has to stay bound until after compass_probe! fires.
    let caller_name = get_caller_name(execute_data);
    let caller = match &caller_name {
        Some(name) => ProbeStr::from_zend_str(name),
        None => ProbeStr::from(NO_CALLER),
    };

    let execute_data = match unsafe { ExecuteData::try_from_mut_ptr(execute_data) } {
        Some(data) => data,
        None => return,
    };

    let arg_type = get_arg_type_name(execute_data);

    let metadata = unsafe { extract_cacheable_metadata(return_value) };

    with_request_id(|request_id| {
        compass_probe!(
            drupal_cacheablemetadata_createfromobject,
            request_id,
            caller,
            metadata.max_age,
            arg_type,
            ProbeStr::from(&metadata.tags),
            ProbeStr::from(&metadata.contexts),
        );
    });
}
