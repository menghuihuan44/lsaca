//! The C runtime that every generated program links against.
//!
//! The runtime is written in portable C17 and lives in the repository as real
//! source files (runtime/*.h and runtime/*.c) so that it can be read, reviewed,
//! compiled and tested independently. It is embedded into the compiler binary
//! with include_str! and written out next to the generated main.c when a
//! project is emitted.
//!
//! The runtime is split into four translation units so that the build back end
//! can compile them in parallel:
//!
//! * `lsaca_rt.c` — allocations/ORC collector, lifetime, panics, checked
//!   arithmetic, process/IO, and math (core).
//! * `lsaca_rt_string.c` — `LsString` objects, search/transform, split, parse.
//! * `lsaca_rt_vec.c` — generic vector primitives, `LsVec_String`.
//! * `lsaca_rt_fmt.c` — format builder, float formatting, output/printing.
//!
//! lsaca_rt_internal.h is the private contract shared by those translation
//! units; generated code only ever includes the public lsaca_rt.h.
//!
//! The ABI contract between generated code and the runtime is summarised in
//! README.md ("Memory management (ORC)") and documented in detail by the
//! comments in runtime/lsaca_rt.h.

/// The contents of runtime/lsaca_rt.h (the public generated-code ABI header).
pub const RUNTIME_H: &str = include_str!("../../../runtime/lsaca_rt.h");

/// The contents of runtime/lsaca_rt.c (the primary runtime translation unit).
pub const RUNTIME_C: &str = include_str!("../../../runtime/lsaca_rt.c");

/// The contents of runtime/lsaca_rt_internal.h (private cross-unit contract).
pub const RUNTIME_INTERNAL_H: &str = include_str!("../../../runtime/lsaca_rt_internal.h");

/// The contents of runtime/lsaca_rt_string.c.
pub const RUNTIME_STRING_C: &str = include_str!("../../../runtime/lsaca_rt_string.c");

/// The contents of runtime/lsaca_rt_vec.c.
pub const RUNTIME_VEC_C: &str = include_str!("../../../runtime/lsaca_rt_vec.c");

/// The contents of runtime/lsaca_rt_fmt.c.
pub const RUNTIME_FMT_C: &str = include_str!("../../../runtime/lsaca_rt_fmt.c");

/// Every runtime file name, in the order the compiler must write them out.
///
/// Headers come first, then the translation units. Every file ending in ".c"
/// is compiled as its own translation unit and linked with the generated
/// program; the two headers are included by those translation units.
pub const RUNTIME_FILES: [&str; 6] = [
    "lsaca_rt.h",
    "lsaca_rt_internal.h",
    "lsaca_rt.c",
    "lsaca_rt_string.c",
    "lsaca_rt_vec.c",
    "lsaca_rt_fmt.c",
];

/// The runtime source files, as (file name, contents) pairs.
///
/// The compiler writes every entry next to the generated main.c. Every file
/// ending in .c is compiled as its own translation unit (in parallel by the
/// direct build back end) and linked together with the user program.
pub fn sources() -> &'static [(&'static str, &'static str)] {
    &[
        ("lsaca_rt.h", RUNTIME_H),
        ("lsaca_rt_internal.h", RUNTIME_INTERNAL_H),
        ("lsaca_rt.c", RUNTIME_C),
        ("lsaca_rt_string.c", RUNTIME_STRING_C),
        ("lsaca_rt_vec.c", RUNTIME_VEC_C),
        ("lsaca_rt_fmt.c", RUNTIME_FMT_C),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_is_present() {
        assert!(RUNTIME_H.contains("LSACA_RT_H"));
        assert!(RUNTIME_C.contains("lsaca_rt"));
    }

    #[test]
    fn sources_lists_every_runtime_file_once() {
        let sources = sources();
        assert!(sources.len() >= 2);
        assert_eq!(sources[0].0, "lsaca_rt.h");
        assert!(sources.iter().any(|(name, _)| *name == "lsaca_rt.c"));
        let mut names: Vec<&str> = sources.iter().map(|(name, _)| *name).collect();
        assert!(names.iter().all(|name| !name.is_empty()));
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), sources.len(), "duplicate runtime file name");
        assert!(sources.iter().all(|(_, contents)| !contents.is_empty()));
    }

    #[test]
    fn sources_lists_all_expected_runtime_files() {
        let sources = sources();
        assert_eq!(sources.len(), RUNTIME_FILES.len());
        for (index, expected) in RUNTIME_FILES.iter().enumerate() {
            let (name, contents) = sources[index];
            assert_eq!(name, *expected);
            assert!(!contents.is_empty(), "{} is empty", expected);
        }
    }
}
