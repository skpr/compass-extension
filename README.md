Compass PHP Extension
=====================

Extension for probing PHP applications.

Used by [Compass](github.com/skpr/compass).

## Probes

All probes use the USDT provider named `compass`. They are only active when an external tracer (e.g. bpftrace) is attached.

### System

| Probe | Arguments | Purpose |
|-------|-----------|---------|
| `canary` | _(none)_ | Zero-cost gatekeeper probe. All other probes only fire if this probe is being actively traced. Result is cached for 1 second. |

### FPM

These probes are triggered when PHP is running under the FPM SAPI.

| Probe | Arguments | Purpose |
|-------|-----------|---------|
| `fpm_request_init` | `request_id` (string) - `HTTP_X_REQUEST_ID` header or `"UNKNOWN"`<br>`uri` (string) - Request URI from `REQUEST_URI`, `PHP_SELF`, `SCRIPT_NAME`, or `"/unknown"`<br>`method` (string) - HTTP method from `REQUEST_METHOD` or `"UNKNOWN"` | Fired during request initialization. Records the identity and nature of the incoming HTTP request. |
| `fpm_request_shutdown` | `request_id` (string) - `HTTP_X_REQUEST_ID` header or `"UNKNOWN"` | Fired during request shutdown. Useful for rollup/finalization of traces for a given request. |
| `fpm_function` | `request_id` (string) - `HTTP_X_REQUEST_ID` header or `"UNKNOWN"`<br>`function_name` (string) - Fully-qualified PHP function or method name<br>`elapsed` (u64) - Wall-clock time in nanoseconds<br>`memory` (u64) - PHP memory usage in bytes | Fired on PHP function completion. Only triggers if elapsed time exceeds `compass.function_threshold`. |

### CLI

These probes are triggered when PHP is running under the CLI SAPI. They are grouped by PID.

| Probe | Arguments | Purpose |
|-------|-----------|---------|
| `cli_init` | `pid` (u64) - Process ID of the PHP CLI process<br>`command` (string) - Full CLI command from `argv` or `SCRIPT_NAME` | Fired during CLI request initialization. Records the PID and the full command being executed. |
| `cli_request_shutdown` | `pid` (u64) - Process ID of the PHP CLI process | Fired during CLI request shutdown. Signals the end of a CLI process execution. |
| `cli_request_function` | `pid` (u64) - Process ID of the PHP CLI process<br>`function_name` (string) - Fully-qualified PHP function or method name<br>`elapsed` (u64) - Wall-clock time in nanoseconds<br>`memory` (u64) - PHP memory usage in bytes | Fired on PHP function completion. Only triggers if elapsed time exceeds `compass.function_threshold`. |

### Drupal

These probes are specific to Drupal applications (FPM only).

| Probe | Arguments | Purpose |
|-------|-----------|---------|
| `drupal_cacheablemetadata_createfromobject` | `request_id` (string) - `HTTP_X_REQUEST_ID` header or `"UNKNOWN"`<br>`caller` (string) - Fully-qualified name of the calling function<br>`cache_max_age` (i64) - `cacheMaxAge` property, defaults to `-1`<br>`arg_type` (string) - Class name or type of the first argument<br>`cache_tags` (string) - Space-delimited cache tags<br>`cache_contexts` (string) - Space-delimited cache contexts | Fires at the end of `CacheableMetadata::createFromObject`. Captures full cacheability metadata for diagnosing unexpected cache behavior. |
| `drupal_cacheablemetadata_createfromrenderarray` | `request_id` (string) - `HTTP_X_REQUEST_ID` header or `"UNKNOWN"`<br>`caller` (string) - Fully-qualified name of the calling function<br>`cache_max_age` (i64) - `cacheMaxAge` property, defaults to `-1`<br>`cache_tags` (string) - Space-delimited cache tags<br>`cache_contexts` (string) - Space-delimited cache contexts | Fires at the end of `CacheableMetadata::createFromRenderArray`. Same as the object probe but without `arg_type` since the input is always an array. |

## INI Configuration

| Directive | Default | Description |
|-----------|---------|-------------|
| `compass.enabled` | `false` | Master switch to enable/disable the extension. |
| `compass.function_threshold` | `1000000` (1ms) | Only function calls exceeding this elapsed time (in nanoseconds) trigger `fpm_function` / `cli_function` probes. |
