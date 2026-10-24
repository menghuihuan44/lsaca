//! The C runtime that every generated program links against.
//!
//! The runtime is written in portable C17 and lives in the repository as real
//! source files (runtime/lsaca_rt.h and runtime/lsaca_rt.c) so that it can be
//! read, reviewed, compiled and tested independently. It is embedded into the
//! compiler binary with include_str! and written out next to the generated
//! main.c when a project is emitted.
//!
//! The ABI contract between generated code and the runtime is summarised in
//! README.md ("Memory management (ORC)") and documented in detail by the
//! comments in runtime/lsaca_rt.h.

/// The contents of runtime/lsaca_rt.h.
pub const RUNTIME_H: &str = include_str!("../../../runtime/lsaca_rt.h");

/// The contents of runtime/lsaca_rt.c.
pub const RUNTIME_C: &str = include_str!("../../../runtime/lsaca_rt.c");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_is_present() {
        assert!(RUNTIME_H.contains("LSACA_RT_H"));
        assert!(RUNTIME_C.contains("lsaca_rt"));
    }
}
