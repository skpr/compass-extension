use phper::functions::ZFunc;

// Specialised instrumentation for a function, decided once per zend_function.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Target {
    // No specialised probe - generic function timing only.
    Generic,
    DrupalCacheableMetadataFromObject,
    DrupalCacheableMetadataFromRenderArray,
    // Prepared statement execution; the SQL is on $this->queryString.
    DbStatementExecute,
    // Direct query/exec; the SQL is parameter 0.
    DbQueryArg0,
}

// Classifies a function by its bare name first, then its declaring class.
//
// The ordering is what keeps this cheap. observer_instrument runs for every
// function PHP executes, and get_function_or_method_name() allocates a fresh
// "Class::method" zend_string every time it is asked - thousands of allocations
// per request for a handful of matches. get_function_name() instead borrows the
// interned name, so the overwhelming majority of functions fail the bare-name
// check and never reach get_class() or allocate anything.
//
// Matching on the *declaring* scope also means subclasses are covered for free:
// a class extending PDOStatement without overriding execute() still reports
// PDOStatement as the scope.
pub fn classify(func: &ZFunc) -> Target {
    let Some(name) = func.get_function_name() else {
        return Target::Generic;
    };

    let candidate = match name.to_bytes() {
        b"createFromObject" => Target::DrupalCacheableMetadataFromObject,
        b"createFromRenderArray" => Target::DrupalCacheableMetadataFromRenderArray,
        b"execute" => Target::DbStatementExecute,
        b"query" | b"exec" => Target::DbQueryArg0,
        _ => return Target::Generic,
    };

    let Some(scope) = func.get_class() else {
        return Target::Generic;
    };

    let expected: &[u8] = match candidate {
        Target::DrupalCacheableMetadataFromObject
        | Target::DrupalCacheableMetadataFromRenderArray => {
            b"Drupal\\Core\\Cache\\CacheableMetadata"
        }
        Target::DbStatementExecute => b"PDOStatement",
        Target::DbQueryArg0 => b"PDO",
        Target::Generic => return Target::Generic,
    };

    if scope.get_name().to_bytes() == expected {
        candidate
    } else {
        Target::Generic
    }
}
