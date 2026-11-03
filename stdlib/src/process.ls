// Command line arguments.
//
// The frozen Appendix C intrinsic name is __proc_args. The legacy __args
// intrinsic continues to exist for the old helper only.

pub extern fn __proc_args() -> Vec<String>;

/// The command line arguments of the program, excluding the executable name.
pub fn args() -> Vec<String> {
    return __proc_args();
}
