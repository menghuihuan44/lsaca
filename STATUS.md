# lsaca STATUS

Completion matrix for the release train in [ROADMAP.md](ROADMAP.md).
Baseline: version 2.5.0 (2026-11-28). Every row marked complete must be traced to
tests and to a documentation anchor; scripts/check_status.py enforces that and is
run in CI.

## How to read this file

| State | Meaning |
| --- | --- |
| complete | Every applicable layer of the definition of done is implemented, tested and documented |
| partial | It exists and is documented, but at least one layer or API gap remains; the gap names its target release |
| missing | Designed and documented, not implemented; the target release is named |
| deliberate | It will not be implemented as designed; the reason is recorded in docs/language/deviations.md |

A row may mix states; the state column records the weakest applicable one.

## Language and semantics

| Area | State | Gap or note | Target |
| --- | --- | --- | --- |
| Lexing, literals, escapes, comments | complete | Unicode identifiers, nested block comments, all escape forms | - |
| Type system and casts | complete | No i128/u128; no cast to bool (deliberate) | - |
| Operators and overflow | complete | Debug builds panic on overflow; release builds wrap; div by zero always panics | - |
| Statements and control flow | complete | `break value` gives the loop expression that value since 2.4.0; a value in a while/for break is E0571 and `continue value` is E0308 | - |
| Functions and function values | complete | No overloading; function values are move-only and not comparable | - |
| Structs, enums, impl, tuple and unit structs | complete | impl Copy and impl Clone rejected because both are derived | - |
| Ownership, moves, Drop | complete | Partial moves since 2.3.0, all-or-nothing: a pattern that binds an owning value moves the whole aggregate and the source becomes unavailable (deviation 15); no explicit drop() | - |
| Borrow checker | partial | Conservative: whole-container Vec borrows; two mutable borrows in one expression rejected; &mut of a struct with a heap field rejected (E0498) | v2.6 track |
| References in aggregates | partial | Struct, enum and tuple-struct fields work since 2.2 (mutable-reference fields since 2.3); anonymous-tuple fields, Option, Vec, HashMap, Result, Box, arrays and closure captures reject references (E0498) | v2.6 |
| Lifetimes | partial | Lightweight only: no variance, no outlives relations, no named-lifetime equality; a 'static field lifetime is rejected | v2.6 |
| Generics | partial | Monomorphized and late-checked; no where clauses, associated types, const generics or default parameters; unused parameters accepted; 128-instantiation depth limit | later releases |
| Traits | partial | No associated functions, associated types, associated constants, supertraits or generic methods; default bodies since 2.0; protocol boundaries fixed by RFC 0003 | later releases |
| Trait objects | complete | Only behind Box, Rc, & or &mut; no upcasting; no equality or ordering (E0369) | - |
| Pattern matching | complete | Array, slice and @ patterns since 2.4.0, with Rust-compatible slice exhaustiveness; no ref patterns; integer range patterns only | - |
| Closures | complete | Capture by value; move accepted but has no effect; references cannot be captured; no recursive closures | - |
| Modules and visibility | complete | pub(super), pub(in path) and module aliases since 2.4.0; glob imports rejected | - |
| Packages | complete | Path dependencies only; no registry, git, workspaces, features or lockfile | - |
| Macros and attributes | deliberate | Builtin formatting/assertion macros and vec! only; RFC 0003 fixes the boundary: no macro_rules!, derive or attributes in 2.x | - |
| Method call form (UFCS) | deliberate | Type::method(&value) is rejected with E0599 and a call-it-on-a-value note; the refusal was confirmed in RFC 0003 | - |
| Unsafe and FFI | complete | Controlled unsafe, raw pointers, extern "C" with FFI-safe checking, repr(C) structs, unions and CString/CStr/MaybeUninit since 2.5.0; no function pointers, variadics, managed types across FFI, #[link] or repr(packed/align/transparent) | - |
| Threads, Send, Sync | missing | Single-threaded; Rc is not atomic | v3.0 |

## Standard library

| Module | State | Gap or note | Target |
| --- | --- | --- | --- |
| core (Option, Result, Iterator) | complete | The Rust combinators listed in deviation 12 are present since 2.3.0; no blanket From<T> for T, no Into, no Try | - |
| string, str | partial | parse::<T> accepts only i64 and f64; no String::from_utf8 family | later release |
| char | complete | Unicode 16.0.0 classification and case conversion since 2.3.0; to_uppercase/to_lowercase return one char (closest scalar, deviation 20) | - |
| vec | complete | Borrowed iteration since 2.3.0: iter() yields &T, iter_mut(), into_iter(), and for over &Vec/&mut Vec/&[T] | - |
| iter | complete | rev and flat_map since 2.3.0; no size_hint (deliberate) | - |
| HashMap | complete | Borrowed iteration, keys/values, HashMap::from and HashSet::from since 2.3.0; get_or_insert and get_or_insert_with since 2.4.0; the borrowed Entry value stays deferred to v2.6 (RFC 0002); keys limited to integers, bool, char and String | v2.6 for entry |
| HashSet, BTreeSet, BTreeMap, VecDeque, BinaryHeap | complete | Surface extended in 2.4.0 (from/to_vec, get_mut, first_key/last_key, extend, with_capacity, front/back) | - |
| fmt | partial | No named or indexed arguments, no integer bases; precision and flags have documented limits | later release |
| math | complete | Float constants and free functions | - |
| io | complete | Read/Write/Seek/BufRead, BufReader/BufWriter and text helpers since 2.3.0; File implements all four traits; no async or non-blocking I/O | - |
| fs | complete | Path/PathBuf and directory iteration since 2.3.0; no recursive walker | - |
| net | complete | TCP and UDP since 2.4.0, including SocketAddr; no TLS or DNS | - |
| time | partial | Clocks, Duration, Instant and SystemTime; no time-zone database or date formatting | later release |
| env | complete | Variables and arguments | - |
| process | complete | args, exit and Command since 2.4.0 (arg/args/current_dir/status/output, ExitStatus, Output) | - |
| error | complete | Error trait with source; io and parse errors implement it | - |

## Tooling and delivery

| Area | State | Gap or note | Target |
| --- | --- | --- | --- |
| CLI (run, build, emit-c, check, fmt, new, version, help) | complete | Direct backend default; --cmake opt-in | - |
| Build cache | complete | Content hash plus cached runtime objects; cache-root probing | - |
| Package manager | complete | lsaca.toml subset, path dependencies, lsaca new | - |
| Formatter | complete | Crate mode since 2.4.0 (every module of a package, --check, --stdout) | - |
| Benchmarks | complete | 22 executed and 3 compile-only workloads, oracle-checked, results archived; speed guard in CI | - |
| Documentation site | complete | Pages generated from docs/ by scripts/docsite.py | - |
| CI | complete | Formatting, clippy, tests, docs, runtime smoke, generated C, sanitizers, site, docs drift, status traceability, real programs, benchmark guard | - |
| Diagnostic code index | complete | All diagnostic codes indexed in docs/language/31-error-codes.md and checked by scripts/check_error_index.py | - |
| Conformance traceability | complete | Every complete row is traced to tests and a docs anchor; checked by scripts/check_status.py | - |
| Real-program CI gate | complete | examples/programs/json.ls, markdown.ls, lisp.ls and wordstats.ls run in CI with golden output and a C comparison | - |

## Test baseline at 2.5.0

* 1,035 library unit tests, 22 end-to-end tests, 2 module audits, 121 robustness
  tests, 1 v2.3 feature test, 1 v2.4 feature test, 1 v2.5 feature test and 28 CLI
  tests, 1,211 in the workspace;
* the C runtime smoke suite, 121 checks;
* the FFI acceptance suite in crates/lsaca/tests/programs/cffi/ (6 positive
  programs with golden output, 11 negative programs), also run under
  AddressSanitizer and UndefinedBehaviorSanitizer;
* the generated-C checker and the documented-example runner are rerun at every
  release and reported in the release notes;
* 22 executed benchmark workloads plus 3 compile-only workloads; the official
  results are archived in benchmarks/results/archive/v2.5/.

## v2.5 closure checklist

- [x] `unsafe { ... }` blocks and `unsafe fn`, with E0133 on every unsafe operation
- [x] raw pointers `*const T`/`*mut T`: creation, operations, casts and the FFI accessors
- [x] `extern "C"` declarations, FFI-safe checking (E0770) and the asm-label symbol binding
- [x] `#[repr(C)]` structs, with E0771 elsewhere
- [x] `union` declarations, with E0772 and E0773
- [x] `CString`, `CStr` and `MaybeUninit` in the standard library
- [x] the safety contract, the ORC invariants and the `ptr`/`cstring`/`cstr`/`mem` pages
- [x] the libc/libm acceptance suite and the negative suite, in e2e, generated C and the sanitizer job
- [x] all gates green, benchmarks inside the 2.4.0 budget and 2.5.0 released
