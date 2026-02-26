Compass PHP Extension
=====================

Extension for probing PHP applications.

Used by [Compass](github.com/skpr/compass).

## Probes

### Core

These probes can be used for any PHP application.

* **canary** - Uses to implement zero cost probing. All the probes below will only work if this probe is also being probed.
* **request_init** - Triggered on request initialisation and records the request ID, URI and method.
* **request_shutdown** - Triggered on request shutdown. Handy for rollup of traces.
* **php_function** - Triggered on PHP function completion and records the request ID, function name and elapsed time.

## Drupal

These probes are specific to Drupal applications.

* **drupal_cacheablemetadata_createfromobject** - Used to debug CacheableMetadata for a object. Records request ID, object type, caller, max age, cache tags and contexts.
* **drupal_cacheablemetadata_createfromrenderarray** - Used to debug CacheableMetadata for an array. Records request ID, caller, max age, cache tags and contexts.

```bash
$ readelf -n compass.so

Displaying notes found in: .note.gnu.build-id
  Owner                Data size        Description
  GNU                  0x00000014       NT_GNU_BUILD_ID (unique build ID bitstring)
    Build ID: 639122726ca0c31dff403ad24f058bf5f1df487e

Displaying notes found in: .note.stapsdt
  Owner                Data size        Description
  stapsdt              0x0000007a       NT_STAPSDT (SystemTap probe descriptors)
    Provider: compass
    Name: drupal_cacheablemetadata_createfromobject
    Location: 0x00000000000114bd, Base: 0x0000000000065030, Semaphore: 0x0000000000075ad8
    Arguments: -8@%rcx -8@%rdx -8@%rdi -8@%rsi -8@%r14 -8@%rax
  stapsdt              0x00000077       NT_STAPSDT (SystemTap probe descriptors)
    Provider: compass
    Name: drupal_cacheablemetadata_createfromrenderarray
    Location: 0x000000000001188e, Base: 0x0000000000065030, Semaphore: 0x0000000000075ada
    Arguments: -8@%rcx -8@%rdx -8@%rsi -8@%rbx -8@%rax
  stapsdt              0x00000045       NT_STAPSDT (SystemTap probe descriptors)
    Provider: compass
    Name: php_function
    Location: 0x0000000000011ac0, Base: 0x0000000000065030, Semaphore: 0x0000000000075adc
    Arguments: -8@%r14 -8@%rax -8@%r15
  stapsdt              0x00000045       NT_STAPSDT (SystemTap probe descriptors)
    Provider: compass
    Name: request_init
    Location: 0x0000000000011f19, Base: 0x0000000000065030, Semaphore: 0x0000000000075ae0
    Arguments: -8@%rcx -8@%r15 -8@%rax
  stapsdt              0x00000039       NT_STAPSDT (SystemTap probe descriptors)
    Provider: compass
    Name: request_shutdown
    Location: 0x000000000001221e, Base: 0x0000000000065030, Semaphore: 0x0000000000075ae2
    Arguments: -8@%rax
  stapsdt              0x00000028       NT_STAPSDT (SystemTap probe descriptors)
    Provider: compass
    Name: canary
    Location: 0x000000000001257b, Base: 0x0000000000065030, Semaphore: 0x0000000000075ade
    Arguments:
```

## INI Configuration

* **compass.enabled** - Enables the PHP extension
* **compass.function_threshold** - Filters function calls greater than configured value. Time is in nanoseconds.
