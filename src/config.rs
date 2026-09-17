//! Configuration the extension re-reads from a file while PHP keeps running.
//!
//! `compass.enabled` stays an INI setting: it decides whether the observer is installed
//! at all, and that happens once at module init. Everything here is a live setting, read
//! from a YAML file so it can be changed on a running fleet — a Kubernetes ConfigMap, a
//! config management run — without restarting PHP-FPM.
//!
//! The settings are held in plain atomics rather than in a thread-local, which is where
//! the canary keeps its cache. The two are caching different things: the canary caches
//! the result of asking whether *this* process is traced, while every thread here wants
//! the same answer out of the same file, so a relaxed load is both cheaper and simpler
//! than a copy per thread. Cheaper because this is read on every observed function
//! return, and reaching a thread-local from a dlopen'd cdylib costs a `__tls_get_addr`
//! call — the same cost `clock` exists to keep off this path. A relaxed load compiles to
//! an ordinary load on every architecture PHP runs on, and nothing here takes a lock
//! except the once-per-interval reload, which uses `try_lock` so it cannot block a
//! request either.

use crate::clock;
use anyhow::Context;
use once_cell::sync::Lazy;
use phper::ini::ini_get;
use serde::Deserialize;
use std::ffi::CStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::time::{Duration, SystemTime};
use tracing::warn;

pub const INI_CONFIG: &str = "compass.config_file";

// Where the live config is read from unless `compass.config_file` says otherwise. Its own
// directory rather than a bare file, so a whole ConfigMap can be mounted over it.
pub const DEFAULT_CONFIG_FILE: &str = "/etc/compass/config.yml";

// How long the settings are reused before the file is checked again. A change therefore
// takes effect within this long, which also bounds how often the check costs a syscall.
const RELOAD_INTERVAL: Duration = Duration::from_secs(5);

/// The settings that can be changed without restarting PHP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
// `default` and not `deny_unknown_fields`: a file that sets one key keeps the shipped
// value for the rest, and a file written for a newer version of the extension still
// loads rather than being rejected wholesale.
#[serde(default)]
pub struct Config {
    /// Only function calls that take longer than this, in nanoseconds, are reported.
    pub function_threshold: u64,

    /// Stops all probing while it is set, leaving the extension loaded and the file
    /// still watched, so it can be turned back on the same way it was turned off.
    pub suspend: bool,
}

impl Config {
    // An associated const rather than only a `Default` impl, so the thread-local below
    // can be initialised in a const context.
    const DEFAULT: Self = Self {
        function_threshold: 1_000_000,
        suspend: false,
    };
}

impl Default for Config {
    fn default() -> Self {
        Self::DEFAULT
    }
}

// What the file looked like at the last check. Compared to decide whether it is worth
// opening and parsing again; size as well as mtime because a filesystem with coarse
// timestamps can report the same mtime for two edits a moment apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
}

// The threshold the function-return path compares against, which is the one thing that
// path needs. A suspended process holds a threshold nothing can exceed, so suspending
// costs that path nothing beyond the load it already does.
static EFFECTIVE_THRESHOLD: AtomicU64 = AtomicU64::new(Config::DEFAULT.function_threshold);

// Read once per request, and once per function the first time a request calls it.
static SUSPEND: AtomicBool = AtomicBool::new(Config::DEFAULT.suspend);

// `clock::raw()` at the last check, or 0 before the first one.
static CHECKED_AT: AtomicU64 = AtomicU64::new(0);

// Only touched by a reload, so contention on it is once per interval at worst.
static STAMP: Mutex<Option<Stamp>> = Mutex::new(None);

// Resolved once per process: the INI entry is read at startup and cannot usefully change
// afterwards, since it names the file that everything else is read from. An empty value
// turns the file off altogether, leaving the extension on its built-in defaults.
static CONFIG_FILE: Lazy<Option<PathBuf>> = Lazy::new(|| {
    let path = ini_get::<Option<&CStr>>(INI_CONFIG)?.to_str().ok()?.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
});

/// Reads the file once up front, from module init.
///
/// For the same reason the clock is built there: it takes the first read out of whichever
/// request would otherwise have paid for it, and FPM's workers inherit the result across
/// the fork rather than each doing their own on their first observed call.
pub fn init() {
    reload(clock::raw());
}

/// The elapsed time a function has to beat to be worth reporting, in nanoseconds.
///
/// `now` is a `clock::raw()` reading. The function-return path already holds one, and
/// asking it for a second timestamp is the sort of cost `clock` exists to avoid.
#[inline]
pub fn function_threshold_at(now: u64) -> u64 {
    refresh_if_due(now);
    EFFECTIVE_THRESHOLD.load(Relaxed)
}

/// Whether probing is suspended, for the paths that run once per request or per function
/// rather than per function return.
#[inline]
pub fn is_suspended() -> bool {
    refresh_if_due(clock::raw());
    SUSPEND.load(Relaxed)
}

#[inline]
fn refresh_if_due(now: u64) {
    let checked_at = CHECKED_AT.load(Relaxed);

    if checked_at != 0 && clock::delta_nanos(checked_at, now) < RELOAD_INTERVAL.as_nanos() as u64 {
        return;
    }

    reload(now);
}

// Deliberately kept out of line: this runs once per interval, and the caller it is
// reached from runs on every observed function return.
#[cold]
#[inline(never)]
fn reload(now: u64) {
    CHECKED_AT.store(now, Relaxed);

    let Some(path) = CONFIG_FILE.as_deref() else {
        return;
    };

    // Another thread is already reloading, so this one carries on with what it has. Only
    // a ZTS build ever gets here, and the answer it wants is the one that thread is in
    // the middle of working out.
    let Ok(mut last) = STAMP.try_lock() else {
        return;
    };

    let stamp = fs::metadata(path).ok().map(|meta| Stamp {
        modified: meta.modified().ok(),
        len: meta.len(),
    });

    if stamp == *last {
        return;
    }

    // Recorded before the read, and whether or not the read works: a file that cannot be
    // parsed is not worth re-reading, or warning about again, until it changes.
    *last = stamp;

    // No file, or one that cannot be stat'd. Not an error: the file is optional, and the
    // defaults are what the extension shipped with.
    if stamp.is_none() {
        store(Config::DEFAULT);
        return;
    }

    match read(path) {
        Ok(config) => store(config),
        // The previous settings are kept rather than reverting to the defaults. A writer
        // that truncates and rewrites in place can be caught mid-write, and quietly
        // widening the threshold on a fleet is worse than running on yesterday's file.
        Err(err) => warn!("keeping the previous compass config: {err:#}"),
    }
}

fn store(config: Config) {
    SUSPEND.store(config.suspend, Relaxed);
    EFFECTIVE_THRESHOLD.store(
        if config.suspend {
            // Nothing can be slower than this, so the function-return path reports
            // nothing without having to ask a second question.
            u64::MAX
        } else {
            config.function_threshold
        },
        Relaxed,
    );
}

fn read(path: &Path) -> anyhow::Result<Config> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("unable to read {}", path.display()))?;

    parse(&raw).with_context(|| format!("unable to parse {}", path.display()))
}

fn parse(raw: &str) -> anyhow::Result<Config> {
    // Through `Option` so that an empty file, or one holding nothing but a document
    // marker or comments, reads as "no settings" rather than as a type error.
    Ok(serde_yaml_ng::from_str::<Option<Config>>(raw)?.unwrap_or_default())
}
