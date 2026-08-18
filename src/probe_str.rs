//! Typed probe arguments.
//!
//! USDT consumers read string arguments with `bpf_probe_read_user_str` (bpftrace's
//! `str()`), which scans forward from the pointer until it finds a NUL byte. A Rust
//! `String`/`str` is not NUL-terminated, so handing one of those pointers to a probe
//! makes the tracer read past the end of the allocation and append whatever heap bytes
//! happen to follow.
//!
//! Probe arguments therefore go through [`ProbeStr`], which can only be built from
//! something already NUL-terminated, and probes are fired through [`compass_probe!`],
//! which accepts nothing but [`ProbeArg`] implementors. A raw `*const u8` from
//! `String::as_ptr()` has no such implementation, so the bug is a compile error.

use phper::strings::ZStr;
use std::ffi::{CStr, CString, c_char};
use std::marker::PhantomData;

/// A pointer to a NUL-terminated string that stays readable for `'a`.
///
/// The lifetime ties the pointer back to whatever owns the bytes, so the owner cannot
/// be dropped before the probe fires.
#[derive(Copy, Clone)]
pub struct ProbeStr<'a> {
    ptr: *const c_char,
    _owner: PhantomData<&'a CStr>,
}

impl<'a> ProbeStr<'a> {
    /// Wraps a raw pointer that is already known to be NUL-terminated.
    ///
    /// # Safety
    ///
    /// `ptr` must be non-null, NUL-terminated, and valid for reads for all of `'a`.
    #[inline(always)]
    pub const unsafe fn from_raw(ptr: *const c_char) -> Self {
        Self {
            ptr,
            _owner: PhantomData,
        }
    }

    /// Wraps a PHP string. `zend_string` values always carry a trailing NUL.
    #[inline(always)]
    pub fn from_zend_str(zs: &'a ZStr) -> Self {
        // SAFETY: zend_string allocates len + 1 bytes and NUL-terminates the buffer.
        // The borrow keeps the string alive for 'a.
        unsafe { Self::from_raw(zs.as_c_str_ptr()) }
    }

    #[inline(always)]
    pub const fn as_ptr(self) -> *const c_char {
        self.ptr
    }
}

impl<'a> From<&'a CStr> for ProbeStr<'a> {
    #[inline(always)]
    fn from(s: &'a CStr) -> Self {
        // SAFETY: a CStr is NUL-terminated by construction.
        unsafe { Self::from_raw(s.as_ptr()) }
    }
}

impl<'a> From<&'a CString> for ProbeStr<'a> {
    #[inline(always)]
    fn from(s: &'a CString) -> Self {
        Self::from(s.as_c_str())
    }
}

/// Converts a typed probe argument into the raw value passed to the SDT probe.
///
/// The set of implementations is deliberately small: it is what keeps a non-terminated
/// pointer from reaching a probe.
pub trait ProbeArg {
    /// The raw value handed to the probe, which must be castable `as isize`.
    type Raw;

    fn into_probe_arg(self) -> Self::Raw;
}

impl ProbeArg for ProbeStr<'_> {
    type Raw = *const c_char;

    #[inline(always)]
    fn into_probe_arg(self) -> Self::Raw {
        self.as_ptr()
    }
}

impl ProbeArg for u64 {
    type Raw = u64;

    #[inline(always)]
    fn into_probe_arg(self) -> Self::Raw {
        self
    }
}

impl ProbeArg for i64 {
    type Raw = i64;

    #[inline(always)]
    fn into_probe_arg(self) -> Self::Raw {
        self
    }
}

/// Fires a probe in the `compass` provider.
///
/// Every argument is passed through [`ProbeArg`], so strings must arrive as a
/// [`ProbeStr`]. Arguments are only evaluated when a tracer is attached, exactly as
/// with the underlying `probe::probe_lazy!`, and the macro returns whether the probe
/// fired.
///
/// Call this rather than `probe::probe_lazy!` directly — `mise run probe-guard`
/// enforces that.
macro_rules! compass_probe {
    ($name:ident $(, $arg:expr)* $(,)?) => {
        ::probe::probe_lazy!(
            compass,
            $name
            $(, $crate::probe_str::ProbeArg::into_probe_arg($arg))*
        )
    };
}

pub(crate) use compass_probe;
