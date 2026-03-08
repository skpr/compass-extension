use crate::util::get_sapi_module_name;

use once_cell::sync::Lazy;

static IS_CLI: Lazy<bool> = Lazy::new(|| get_sapi_module_name().to_bytes() == b"cli");

#[inline]
pub fn is_cli() -> bool {
    *IS_CLI
}
