// Environment and process information.
//
// Values are owned String values; a missing variable is reported as None,
// never as an error or a panic.

pub extern fn __env_var(name: String) -> Option<String>;
pub extern fn __env_vars() -> Vec<String>;

/// Reads an environment variable, or None when it is not set.
pub fn var(name: String) -> Option<String> {
    return __env_var(name);
}

/// Every environment variable as a K=V string.
pub fn vars() -> Vec<String> {
    return __env_vars();
}
