Compass PHP Extension
=====================

Extension for probing PHP applications.

Used by [Compass](github.com/skpr/compass).

## Probes

### FPM

These probes are triggered when PHP is running under the FPM SAPI.

* **canary** - Used to implement zero cost probing. All the probes below will only work if this probe is also being probed.
* **request_init** - Triggered on request initialisation and records the request ID, URI and method.
* **request_shutdown** - Triggered on request shutdown. Handy for rollup of traces.
* **fpm_function** - Triggered on PHP function completion and records the request ID, function name, elapsed time and memory usage.

### CLI

These probes are triggered when PHP is running under the CLI SAPI. They are grouped by PID.

* **cli_init** - Triggered on request initialisation and records the PID and command.
* **cli_shutdown** - Triggered on request shutdown and records the PID.
* **cli_function** - Triggered on PHP function completion and records the PID, function name, elapsed time and memory usage.

### Drupal

These probes are specific to Drupal applications (FPM only).

* **drupal_cacheablemetadata_createfromobject** - Used to debug CacheableMetadata for a object. Records request ID, object type, caller, max age, cache tags and contexts.
* **drupal_cacheablemetadata_createfromrenderarray** - Used to debug CacheableMetadata for an array. Records request ID, caller, max age, cache tags and contexts.

## INI Configuration

* **compass.enabled** - Enables the PHP extension
* **compass.function_threshold** - Filters function calls greater than configured value. Time is in nanoseconds.
