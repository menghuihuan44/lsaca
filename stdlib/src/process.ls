// Process helpers.
//
// The frozen Appendix C intrinsic name for the argument vector is __proc_args.
// The legacy __args intrinsic continues to exist for the old helper only.
//
// Since 2.1.0 the standard library also exposes `exit(code: i32)`. The frozen
// Phase D intrinsic is __proc_exit; it maps onto the runtime's ls_exit, which
// flushes the standard streams and terminates the process without running
// destructors (the language has no `!` return type, so `exit` is documented as
// never returning instead).

pub extern fn __proc_args() -> Vec<String>;
pub extern fn __proc_exit(code: i32);

/// The command line arguments of the program, excluding the executable name.
pub fn args() -> Vec<String> {
    return __proc_args();
}

/// Terminates the process with the given exit code.
///
/// The standard streams are flushed, but no destructors run afterwards; this
/// function never returns to its caller.
pub fn exit(code: i32) {
    __proc_exit(code);
}
