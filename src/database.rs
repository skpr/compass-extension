use crate::canary;
use crate::cli::is_cli;
use crate::function_observer::take_elapsed;
use crate::util::{bytes_to_cstring, get_pid, get_request_id, get_request_server};
use phper::strings::ZStr;
use phper::{sys, values::ExecuteData};
use probe::probe_lazy;
use rustc_hash::{FxHashSet, FxHasher};
use std::cell::RefCell;
use std::hash::Hasher;

// Longest SQL prefix carried on db_query_text. Consumers read string arguments
// with bpf_probe_read_user_str and cap what they copy anyway (bpftrace's default
// BPFTRACE_STRLEN is 64 bytes), so a bound here costs nothing real and stops a
// multi-kilobyte statement dominating the ring buffer.
const MAX_SQL_TEXT: usize = 512;

// Upper bound on interned ids per process. A worker that genuinely sees more
// distinct statements than this is better served by re-announcing them than by
// growing a set without bound for its whole lifetime.
const MAX_INTERNED: usize = 4096;

thread_local! {
    // sql_ids whose text has already been emitted, so the text travels once per
    // process rather than on every execution.
    static SEEN_SQL: RefCell<FxHashSet<u64>> = RefCell::new(FxHashSet::default());
}

// Forgets every interned id so their text is announced again.
//
// Called when a tracer attaches: it was not listening when the ids were first
// announced, so without this it would receive db_query ids it cannot resolve.
pub fn forget_interned_sql() {
    SEEN_SQL.with(|seen| {
        if let Ok(mut seen) = seen.try_borrow_mut() {
            seen.clear();
        }
    });
}

// Hashes SQL into a stable grouping key.
//
// Runs of digits collapse to a single '?' so unparameterised literals such as
// "WHERE nid = 1" and "WHERE nid = 2" share an id, which is what makes N+1
// patterns visible as one group. Drupal uses prepared statements with named
// placeholders, so its SQL arrives already normalised; this mainly helps direct
// PDO::query/exec callers. Quoted string literals are deliberately left alone -
// stripping them correctly needs a real lexer, and a wrong guess would merge
// unrelated statements.
fn sql_id(sql: &[u8]) -> u64 {
    let mut hasher = FxHasher::default();
    let mut prev_digit = false;

    for &byte in sql {
        let digit = byte.is_ascii_digit();
        if digit {
            if !prev_digit {
                hasher.write_u8(b'?');
            }
        } else {
            hasher.write_u8(byte);
        }
        prev_digit = digit;
    }

    hasher.finish()
}

// True if this id's text still needs announcing.
fn needs_text(id: u64) -> bool {
    SEEN_SQL.with(|seen| match seen.try_borrow() {
        Ok(seen) => !seen.contains(&id),
        // Treat a failed borrow as already announced rather than re-emitting.
        Err(_) => false,
    })
}

fn mark_text_sent(id: u64) {
    SEEN_SQL.with(|seen| {
        if let Ok(mut seen) = seen.try_borrow_mut() {
            if seen.len() >= MAX_INTERNED {
                seen.clear();
            }
            seen.insert(id);
        }
    });
}

// Emits the probes for one completed statement.
fn emit(sql: &ZStr, elapsed: u64) {
    let bytes = sql.to_bytes();
    let id = sql_id(bytes);

    if needs_text(id) {
        let prefix = &bytes[..bytes.len().min(MAX_SQL_TEXT)];
        let text = bytes_to_cstring(prefix);

        // Only record the id as announced once the probe actually fired. If
        // nobody is attached to db_query_text yet, leave it unmarked so the text
        // goes out when someone is.
        if probe_lazy!(compass, db_query_text, id, text.as_ptr()) {
            mark_text_sent(id);
        }
    }

    // Deliberately not filtered by compass.function_threshold. An N+1 is hundreds
    // of individually fast queries, so thresholding would hide the exact pattern
    // this probe exists to surface. Each event is a few registers wide.
    if is_cli() {
        probe_lazy!(compass, cli_db_query, get_pid(), id, elapsed);
        return;
    }

    let Ok(server) = get_request_server() else {
        return;
    };

    let request_id = get_request_id(server);

    probe_lazy!(compass, fpm_db_query, request_id.as_ptr(), id, elapsed);
}

// PDOStatement::execute - the SQL is on $this->queryString, not in the arguments.
pub unsafe extern "C" fn statement_execute_observer_end(
    execute_data: *mut sys::zend_execute_data,
    _return_value: *mut sys::zval,
) {
    if !canary::is_traced() {
        return;
    }

    let Some(elapsed) = take_elapsed(execute_data) else {
        return;
    };

    let Some(data) = (unsafe { ExecuteData::try_from_mut_ptr(execute_data) }) else {
        return;
    };

    let Some(this) = data.get_this() else {
        return;
    };

    if let Some(sql) = this.get_property("queryString").as_z_str() {
        emit(sql, elapsed);
    }
}

// PDO::query / PDO::exec - the SQL is parameter 0.
pub unsafe extern "C" fn query_arg0_observer_end(
    execute_data: *mut sys::zend_execute_data,
    _return_value: *mut sys::zval,
) {
    if !canary::is_traced() {
        return;
    }

    let Some(elapsed) = take_elapsed(execute_data) else {
        return;
    };

    let Some(data) = (unsafe { ExecuteData::try_from_mut_ptr(execute_data) }) else {
        return;
    };

    // get_parameter does no bounds checking, and the end handler still runs when
    // a required argument was missing, so check the count first.
    if data.num_args() == 0 {
        return;
    }

    if let Some(sql) = data.get_parameter(0).as_z_str() {
        emit(sql, elapsed);
    }
}
