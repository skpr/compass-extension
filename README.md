Compass PHP Extension
=====================

Extension for probing PHP applications.

Used by [Compass](github.com/skpr/compass).

## Probes

All probes use the USDT provider named `compass`. They are only active when an external tracer (e.g. bpftrace) is attached.

### System

| Probe | Arguments | Purpose |
|-------|-----------|---------|
| `canary` | _(none)_ | Gatekeeper probe. Every other probe only fires if this one is being actively traced, so **a consumer must attach to `canary`** or it will see nothing. Sampled once per request and latched for that request's duration, so attaching takes effect from the next request. |

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
| `cli_request_init` | `pid` (u64) - Process ID of the PHP CLI process<br>`command` (string) - Full CLI command from `argv` or `SCRIPT_NAME` | Fired during CLI request initialization. Records the PID and the full command being executed. |
| `cli_request_shutdown` | `pid` (u64) - Process ID of the PHP CLI process | Fired during CLI request shutdown. Signals the end of a CLI process execution. |
| `cli_function` | `pid` (u64) - Process ID of the PHP CLI process<br>`function_name` (string) - Fully-qualified PHP function or method name<br>`elapsed` (u64) - Wall-clock time in nanoseconds<br>`memory` (u64) - PHP memory usage in bytes | Fired on PHP function completion. Only triggers if elapsed time exceeds `compass.function_threshold`. |

### Database

Hooked at the PDO layer (`PDOStatement::execute`, `PDO::query`, `PDO::exec`), so one hook set covers
Drupal, Symfony, Laravel and WordPress. Matching is on the *declaring* class, so subclasses that do
not override the method are covered too.

SQL text is **interned**: the statement is announced once per distinct query per process on
`db_query_text`, and each execution then carries only the compact `sql_id`. Join on `sql_id`. This
keeps a 400-query page to a few kilobytes of probe traffic instead of hundreds of them.

Unlike `*_function`, these are **not** filtered by `compass.function_threshold` - an N+1 is hundreds
of individually fast queries, and thresholding would hide exactly the pattern these probes exist to
surface.

| Probe | Arguments | Purpose |
|-------|-----------|---------|
| `db_query_text` | `sql_id` (u64) - Hash identifying the statement<br>`sql` (string) - Statement text, truncated to 512 bytes | Announces the SQL behind a `sql_id`. Fired once per distinct statement per process, and again for every statement if a new tracer attaches. |
| `fpm_db_query` | `request_id` (string) - `HTTP_X_REQUEST_ID` header, or a generated UUID<br>`sql_id` (u64) - Hash identifying the statement<br>`elapsed` (u64) - Wall-clock time in nanoseconds | Fired on completion of a database query under FPM. |
| `cli_db_query` | `pid` (u64) - Process ID of the PHP CLI process<br>`sql_id` (u64) - Hash identifying the statement<br>`elapsed` (u64) - Wall-clock time in nanoseconds | Fired on completion of a database query under CLI. Covers drush, queue runners and migrations. |

`sql_id` collapses runs of digits to a single `?`, so unparameterised literals such as `WHERE nid = 1`
and `WHERE nid = 2` share an id. Quoted string literals are left intact - normalising them correctly
needs a real SQL lexer, and a wrong guess would merge unrelated statements. Drupal uses prepared
statements with named placeholders, so its SQL arrives already normalised.

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
