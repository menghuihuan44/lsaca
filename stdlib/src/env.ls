// Environment and process information.
//
// Since 2.1.0 reading an environment variable is fallible in the Rust sense:
// `var` returns a Result whose error distinguishes a variable that is not set
// from one whose value cannot be represented as text. Values are owned String
// values; a missing variable is reported as VarError::NotPresent, never as a
// panic.

pub extern fn __env_var(name: &str) -> Option<String>;
pub extern fn __env_vars() -> Vec<String>;

/// The reason an environment variable could not be read.
pub enum VarError {
    /// The variable is not present in the environment.
    NotPresent,
    /// The variable is present but its value is not valid Unicode.
    ///
    /// The current runtime always reports environment values as UTF-8 text, so
    /// this variant is not produced yet; it exists for API compatibility.
    NotUnicode(String),
}

/// Reads an environment variable.
///
/// Returns Err(VarError::NotPresent) when the variable is not set.
pub fn var(name: &str) -> Result<String, VarError> {
    match __env_var(name) {
        Some(value) => {
            return Ok(value);
        }
        None => {
            return Err(VarError::NotPresent);
        }
    }
}

/// Every environment variable as a K=V string.
pub fn vars() -> Vec<String> {
    return __env_vars();
}
