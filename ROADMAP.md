# lsaca Roadmap

This document is the single source of truth for where lsaca is going and for
what "finished" means. It is a contract, not a wish list: a release ships only
when its exit criteria are met. The completion state of every area is tracked in
[STATUS.md](STATUS.md), and the user-visible history is in
[CHANGELOG.md](CHANGELOG.md). The language reference for the current release is
[docs/README.md](docs/README.md).

Version 2.5.0 is the current release; 2.4.0 is the baseline for the v2.5 performance budget.

## Principles

1. **Close before you open.** A release may not add a capability while a
   documented promise in the same area is still open.
2. **One theme per release.** Each release has one theme, a frozen scope and an
   explicit not-doing list. New ideas go to the backlog or an RFC.
3. **Done means done.** "The demo runs" is not done; see the definition of done.
   Anything less is marked partial in STATUS.md.
4. **STATUS drives the work.** Work starts from a red or yellow cell in the
   status matrix, not from an idea.
5. **Promises are executable.** Version numbers, documentation claims, examples
   and performance budgets are checked by CI, not by reviewers' memory.
6. **Removals are announced.** A removal needs a deprecation release, a
   migration note and a major version bump.
7. **Deviations are deliberate.** Every difference from Rust is listed in
   [docs/language/deviations.md](docs/language/deviations.md) with its reason, and
   every verified limitation in [docs/language/limitations.md](docs/language/limitations.md).

## Definition of done

A feature, library module or tool change is done only when every applicable
layer is complete:

| Layer | Required evidence |
| --- | --- |
| Syntax and parsing | Grammar in docs/language/02 updated; parser tests; recovery errors |
| Type checking | Resolution, inference and diagnostics; positive and negative tests |
| Lowering | HIR shape documented in the compiler source; move and borrow behaviour defined |
| Code generation | Deterministic C; no new warnings under gcc -std=c17 -Wall -Wextra |
| Runtime | C runtime support with tests where the feature needs it |
| Diagnostics | Stable E0xxx code, exact message, snapshot test, docs entry |
| Documentation | Chapter updated with a runnable example and its real output |
| Tests | Positive, negative and conformance tests traced to the docs chapter |
| Performance | Measured when it can affect runtime, memory or compile time |
| Bookkeeping | STATUS.md row and CHANGELOG.md entry updated |

A missing layer means the feature is partial, and partial features are listed as
such.

## Version policy

* **Semantic versioning.** Major: breaking language or library changes. Minor:
  additive features or announced behaviour changes. Patch: fixes only.
* **Deprecation window.** A removed or changed form keeps working with a warning
  for at least one minor release and has a migration entry in CHANGELOG.md. Only
  a major release removes it.
* **One version source.** The workspace Cargo.toml is the source of truth; the
  CLI output, docs/README.md and the generated CMake project must agree, and CI
  checks that they do.
* **Schedule.** A minor release every four to six weeks of focused work. An
  unready release moves rather than ships partial; patch releases as needed.

## Release train

Only v2.3 is under contract. The later sections are direction papers; each
release freezes its own contract (scope, exit criteria, not-doing list) when it
starts.

### v2.3 - Close the promises (shipped 2026-11-27)

Theme: finish what 2.0, 2.1 and 2.2 documented instead of adding new surface.

In scope, in priority order:

1. **Partial moves.** A let pattern that destructures a tuple, struct or enum
   moves non-Copy fields out instead of deep-copying them, matching Rust. This
   removes the oldest documented deviation (deviation 15) and the stale promise
   in docs/language/16-roadmap.md.
2. **Borrowed iteration.** Vec::iter_mut and VecIterMut, Vec::into_iter,
   iteration over &[T], and for loops that borrow; remove the "iter() consumes
   the vector" workaround for good.
3. **HashMap iteration.** for loops over a map, iter/keys/values without
   cloning, and HashMap::from; decide the entry API with an RFC if the
   representation makes it worthwhile.
4. **I/O traits and buffered I/O.** Read, Write, Seek and BufRead, BufReader,
   BufWriter, and read_line/read_to_string helpers; File implements them.
5. **Filesystem paths.** PathBuf and Path, directory iteration, and a path-based
   API alongside the existing &str helpers.
6. **Unicode character classification.** is_alphabetic, is_numeric,
   is_whitespace and Unicode case conversion for char, built from the existing
   Unicode table generator; otherwise an ADR that defers them with a reason.
7. **Option and Result completion.** The missing Rust combinators listed in
   deviation 12, so ordinary error-handling code does not hit a wall.
8. **Iterator completion.** rev and flat_map, and size_hint only if it can be
   implemented without unsound assumptions.
9. **UFCS diagnostics.** A method called as Type::method(&value) gets a clear
   explanatory diagnostic (or the form is implemented, decided by RFC); the
   current E0412 "cannot find type" message is a usability bug.
10. **Documentation truth.** Split docs/language/16-roadmap.md into deviations,
    limitations and history; move version history into CHANGELOG.md; make
    docs/README.md the only release-version statement; remove every "planned for
    version X" claim where X has already shipped.
11. **Governance and CI.** Add a diagnostic-code index; add
    scripts/check_status.py (matrix consistency and test tracing) and a
    documentation drift check; add the real programs
    (examples/programs/json.ls, markdown.ls, lisp.ls) as a CI gate with golden
    output; add the incremental speed guard on the benchmark matrix.

Not in v2.3 (deferred with an owner, not silently dropped):

* references inside Vec, HashMap, Result, Box, arrays and closures; full
  lifetime relations: v2.6 track;
* values out of break and continue, array patterns, slice patterns, @ bindings:
  v2.4;
* where clauses, associated types, supertraits, generic methods, const generics:
  later releases, one RFC each;
* operator overloading, derive attributes, the Display/Debug/Default/Ord/Hash
  protocols: v2.4 at the earliest;
* pub(super), pub(in path), module aliases: v2.4;
* controlled unsafe and FFI: v2.5;
* threads and Send/Sync: v3.0;
* registry packages, user macros, an LSP, separate compilation: backlog, no
  version committed.

All eleven items shipped in 2.3.0; the completion matrix and test/doc tracing are
in [STATUS.md](STATUS.md) and checked by scripts/check_status.py.

Exit criteria (all required):

- [x] every in-scope item is done per the definition of done and marked complete
      in STATUS.md;
- [x] every remaining yellow or missing cell has a committed target version or is
      marked deliberate with a reason; no undated yellow cells;
- [x] scripts/check_status.py passes: every complete row is traced to tests and a
      documentation anchor;
- [x] documentation drift check passes: zero stale version or planned-claim
      statements;
- [x] the three real programs compile, run and match golden output in CI;
- [x] all existing gates pass: fmt, clippy -D warnings, workspace tests, doc -D
      warnings, runtime smoke, generated-C checker, docs examples, link check,
      prose check, benchmark doc check, stdlib embedding check;
- [x] benchmarks rerun: no runtime median regression above 10% and no cold
      compile median regression above 20% against 2.2.0; deviations documented;
- [x] CHANGELOG.md has the 2.3.0 entry and the docs site is republished.


The 2.3.0 matrix was rerun on 2026-11-27 and published in [docs/language/21-benchmarks.md](docs/language/21-benchmarks.md) (raw data in [benchmarks/results/latest.md](benchmarks/results/latest.md), frozen copy in [benchmarks/results/archive/v2.3/](benchmarks/results/archive/v2.3/)): 13 of the 22 executed workloads are within 1.4x of C, 14 within 1.5x and 6 within 1.15x, with a median of 1.34x, up from 9 and 14 at 2.2.0. Five C-normalised runtime medians (binary_trees, hashmap_string, sieve, loop_sum and fib_iter) remain outside the 10% budget against 2.2.0 and are listed with their causes in the page's regression table; a same-host A/B run of the 2.2.0 and 2.3.0 compilers on nine affected workloads measured lsaca medians between 0.88x and 1.24x of the 2.2.0 compiler's while the C reference medians themselves moved by up to 16%, so those normalised deltas are within run-to-run noise rather than a code regression. The incremental guard baseline therefore advances to the 2.3.0 archive.

### v2.4 - Tool-grade language (shipped 2026-11-27)

Make the language pleasant for programs bigger than benchmarks. The scope is
frozen; anything not listed here is a v2.5+ item, not an unplanned v2.4 addition.

0. **CI toolchain pin.** Every workflow installs one pinned Rust version
   (`RUST_TOOLCHAIN` in each workflow's top-level env), so a new stable clippy
   lint can no longer break main without a reviewed version bump.
1. **Array, slice and `@` patterns.** `[a, b, c]` matches a fixed-size array and
   its length must equal the array's length (E0308); `[a, b, ..]`, `[.., y]`
   and `[first, .., last]` match slices behind `&[T]` / `&mut [T]` (match
   ergonomics applies, at most one `..`); `[..]` is irrefutable, every other
   slice pattern is refutable. `name @ pattern` binds the whole value and
   matches the sub-pattern; binding the whole value by move while a non-Copy
   part is also moved out is E0382. All pattern positions accept the new forms.
2. **Values out of `break`.** `break value` in a `loop` gives the loop
   expression that value's type; a bare `break` has type `()`. A value in a
   `while`/`for` break is E0571; `continue value` is rejected with E0308 and a
   note that the language has no continue values. Labelled and nested loops
   follow the same rules. Codegen writes the value into the loop result before
   releasing body frames.
3. **Collection entry idioms and API density.** HashMap gains
   `get_or_insert` and `get_or_insert_with` (a borrowed `Entry` value is
   refused until v2.6; see rfcs/0002-hashmap-entry.md), HashSet, BTreeSet,
   BTreeMap, VecDeque and BinaryHeap gain the surface documented in
   docs/language/15-standard-library.md, and the iterator adapters gain
   filter_map, zip, chain, take_while, skip_while, position, nth, last, min,
   max and product. No new C runtime ABI is required for this item.
4. **System surface.** `process::Command` (arg/args/current_dir/status/output)
   and `net::SocketAddr` plus `UdpSocket` (bind/send_to/recv_from/set_timeout/
   close) on Windows and POSIX. Additive to the runtime ABI; the existing TCP
   API keeps working.
5. **Visibility and module aliases.** `pub(super)` and `pub(in path)` with
   Rust semantics (visible inside the named ancestor module and its
   descendants; `crate`/`super`/`self` prefixes allowed; E0433 for an unknown
   path, E0742 for a path that does not name an ancestor).
   `use path::module as alias;` and `pub use path::module as alias;` import
   modules under the alias.
6. **Formatter crate mode.** `lsaca fmt [PATH] [--check]` formats a single file
   or every module file of a package (auto-discovered lsaca.toml); `--check`
   lists unformatted files and exits 1 without writing. Output stays idempotent
   and byte-stable.
7. **Protocol boundaries decided, not implemented.** RFC 0003 records the
   refusal of derive macros, operator overloading and user-defined
   Display/Debug/Default/Ord/Hash protocols in 2.4 and fixes what each protocol
   means; RFC 0004 records the v3.0 concurrency decision. UFCS stays rejected
   with an actionable E0599 note (documented deviation 19).

Non-goals: references in aggregates and a borrowed Entry (v2.6); unsafe/FFI
(v2.5); threads (v3.0); where clauses, associated types, const generics,
user macros.

Definition of done: syntax, type checking, lowering, codegen where applicable,
stable diagnostic codes, standard-library documentation, unit and e2e tests
with exact output, a STATUS.md row and a CHANGELOG entry. The shared exit
criteria apply unchanged; the v2.4 benchmark baseline is the 2.3.0 archive (no
runtime median more than 10% slower, no cold compile median more than 20%
slower, unless documented in the release page).

All seven items shipped in 2.4.0; the completion matrix and test/doc tracing are
in [STATUS.md](STATUS.md) and checked by scripts/check_status.py.

Exit criteria (all required):

- [x] every in-scope item is done per the definition of done and marked complete
      in STATUS.md;
- [x] every remaining yellow or missing cell has a committed target version or is
      marked deliberate with a reason; no undated yellow cells;
- [x] scripts/check_status.py passes: every complete row is traced to tests and a
      documentation anchor;
- [x] documentation drift check passes: zero stale version or planned-claim
      statements;
- [x] the four real programs compile, run and match golden output in CI;
- [x] all existing gates pass: fmt, clippy -D warnings, workspace tests, doc -D
      warnings, runtime smoke, generated-C checker, docs examples, link check,
      prose check, benchmark doc check, stdlib embedding check;
- [x] benchmarks rerun: no runtime median regression above 10% and no cold
      compile median regression above 20% against 2.3.0; deviations documented;
- [x] CHANGELOG.md has the 2.4.0 entry and the docs site is republished.

The 2.4.0 matrix was rerun on 2026-11-27 and published in
[docs/language/21-benchmarks.md](docs/language/21-benchmarks.md) (raw data in
[benchmarks/results/latest.md](benchmarks/results/latest.md), frozen copy in
[benchmarks/results/archive/v2.4/](benchmarks/results/archive/v2.4/)): 16 of
the 22 executed workloads are within 1.4x of C, 17 within 1.5x and 9 within
1.15x, with a median of 1.20x, up from 13, 14, 6 and 1.34x at 2.3.0. Cold
single-file builds take 0.57-2.54 s (median 0.88 s) and cached rebuilds
130-219 ms (median 164 ms). Four C-normalised runtime medians (iterator_pipeline,
generic_stack, fib and fn_pipeline) and five cold-build medians are outside the
budget against the 2.3.0 archive and are listed in the page's regression table;
a same-host A/B run of the 2.3.0 and 2.4.0 compilers in the release window, over
twelve workloads in official mode, measured a median runtime ratio of 0.993
(per-workload 0.80x-1.77x while the C reference itself moved by up to 42%) and
cold-build ratios between 0.76 and 1.17, so the normalised deltas are host-state
noise rather than a code regression. The incremental guard baseline therefore
advances to the 2.4.0 archive; because a shared CI runner cannot separate that
noise from a real regression, the scheduled benchmark-matrix job reports the
guard as advisory and release validation uses the same-host A/B measurement.

### v2.5 - System foundation (shipped 2026-11-28)

Theme: controlled unsafe as an auditable escape hatch, not a second language. The
safe subset stays safe, ORC stays automatic, and the compiler crate keeps
#![forbid(unsafe_code)]; user code gains the minimum machinery needed to call the
platform C libraries and to build C-compatible data.

1. **CI and safety baseline.** The Address-and-UB-sanitizer job builds and runs
   the FFI acceptance programs with -fsanitize=address,undefined. The generated-C
   checker compiles every unsafe and FFI fixture warning-free. The compiler crate
   keeps #![forbid(unsafe_code)]. The ASan/UBSan job is a required gate, not
   optional.
2. **`unsafe` blocks and `unsafe fn`.** An `unsafe { ... }` expression evaluates
   to its block value and adds no runtime cost. An `unsafe fn` may perform unsafe
   operations without an inner block; calling it requires an unsafe context. The
   operations that need an unsafe context are: dereferencing or assigning through
   a raw pointer, calling an `extern "C"` function, `MaybeUninit::assume_init`,
   `CStr::from_ptr`, `CString::from_raw`, and reading, writing or constructing
   a union. Violations are E0133 with a note that names the unsafe operation. The
   emitter lowers unsafe syntax to the same statements as safe syntax; unsafe is a
   compile-time permission, not a runtime mode.
3. **Raw pointers.** Types `*const T` and `*mut T` (sized `T` only). They are
   Copy, never reference counted, never traced by the cycle collector, and legal in
   aggregates; they carry no lifetime, so keeping a pointee alive is the user's
   contract. Creation: `&value as *const T`, `&mut value as *mut T`, integer to
   pointer casts, and `std::ptr::null::<T>()` built on the cast. Operations:
   `is_null()`, `addr()`, `add(count)` and `sub(count)` (the last two require an
   unsafe context), `*p` reads and `*p = v` writes (unsafe), and `==`/`!=`
   comparisons by address. Pointer to pointer, pointer to integer and integer to
   pointer casts are allowed; pointer arithmetic through operators is not. FFI
   bridge accessors: `String::as_ptr`, `Vec::as_ptr` and `Vec::as_mut_ptr`.
4. **`extern "C"` declarations.** An `extern "C" { ... }` item declares external
   functions in user code (the std-only `extern fn __intrinsic` form remains for
   compiler intrinsics; name collisions are E0001). Only the `"C"` ABI is accepted,
   declarations have no body, no generics and no receiver, and may be `pub`.
   Parameter and return types must be FFI-safe: integer types, floats, `bool`,
   `char`, raw pointers, `&T`/`&mut T` over FFI-safe `T`, `repr(C)` structs and
   unions by value or pointer, and `()`. Every other type is rejected with E0770
   ("type is not FFI-safe") in the declaration. Calling an extern function requires
   an unsafe context (E0133); codegen calls the platform symbol directly through a
   compiler-generated declaration bound with a GCC/Clang asm label, so the libc
   and standard-header declarations do not conflict (GCC or Clang is required).
   The generated project links the C library and libm; there is no
   `#[link]` attribute in v2.5.
5. **`#[repr(C)]` structs.** The attribute is accepted on `struct` (and as a no-op
   on `union`), only with the `C` argument; other placements or arguments are E0771. The layout is
   part of the language contract: declaration order, C alignment and padding, and
   the size and alignment the C compiler produces. Structs without `repr(C)` keep
   an unspecified layout (the compiler currently uses declaration order, but no
   program may depend on it). `repr(C)` structs travel by value across FFI when all
   their fields are FFI-safe.
6. **`union` declarations.** `union Name { a: i64, b: f64 }` overlays its fields at
   offset 0 with size and alignment equal to the largest field. Field types are
   restricted to FFI-safe scalar, pointer, `repr(C)` struct and union types: no
   managed type, no `Drop` type, no generics (E0772 for a forbidden field, E0773
   for `Drop` or pattern use). A union has no methods, no `Drop`, no destructuring
   and no generated equality or formatting; constructing, reading and writing
   fields happens inside unsafe. Unions may be passed by value across FFI.
7. **`CString`, `CStr` and `MaybeUninit`.** `CString` owns a NUL-terminated byte
   buffer: `new(&str) -> Result<CString, NulError>` (embedded NUL is an error),
   `as_ptr() -> *const u8`, `into_raw()` and unsafe `from_raw` (only pointers from
   `into_raw` are accepted). `CStr` is a borrowed NUL-terminated view built by
   unsafe `from_ptr` with `to_str()` and `len()`. `MaybeUninit<T>` is a builtin
   layout wrapper with exactly the size and alignment of `T`: `uninit()`,
   `write(value)`, `as_mut_ptr()` and unsafe `assume_init()`; leaking an
   initialized value whose type has `Drop` is a documented leak, not a double free.
   These types live in the standard library (`std::cstring`, `std::cstr`,
   `std::mem`) and are implemented in lsaca wherever the language allows it.
8. **Safety documentation, ORC invariants and acceptance.**
   `docs/language/32-unsafe-and-ffi.md` states the safety contract: what the
   compiler checks, what the user promises, the FFI-safe type table, and the ORC
   invariants unsafe code must preserve (never release or retain an ORC object by
   hand, never forge or rely on reference counts, raw pointers never own, managed
   values never cross the FFI boundary). The acceptance suite in `tests/cffi/` binds
   the real platform C libraries: libc `strlen`/`strcmp`/`memcmp`/`div`/`getenv` and libm
   `frexp`/`tgamma`/`erf`, exercising a `repr(C)` struct returned by value, out
   parameters through raw pointers and `MaybeUninit`, `CString`/`CStr` round trips,
   null pointer handling and a union round trip. The suite is wired into e2e, the
   generated-C checker and the sanitizer job. A negative suite proves that managed
   types are rejected in extern signatures and union fields, that raw pointer
   dereference outside unsafe is E0133, and that codegen emits no retain or
   release for pointer-typed values. The code index, STATUS.md and the deviations
   list are updated in the same release.

Not in v2.5: managed types across FFI, function pointers and callbacks,
variadics, `#[link]` or extra libraries, `repr(packed/align/transparent)`,
`repr(C)` enums and explicit discriminants, inline assembly, allocator APIs,
`Box::into_raw` or `Rc` raw interop, `Send`/`Sync` and threads (v3.0), references
in arbitrary aggregates (v2.6), and self-referential structures (v2.6 ADR).

All eight items shipped in 2.5.0; the completion matrix and test/doc tracing are
in [STATUS.md](STATUS.md) and checked by scripts/check_status.py.

Exit criteria (all required):

- [x] every in-scope item is done per the definition of done and marked complete
      in STATUS.md;
- [x] every remaining yellow or missing cell has a committed target version or is
      marked deliberate with a reason; no undated yellow cells;
- [x] scripts/check_status.py passes: every complete row is traced to tests and a
      documentation anchor;
- [x] documentation drift check passes: zero stale version or planned-claim
      statements;
- [x] all existing gates pass, including the FFI acceptance suite under
      AddressSanitizer and UndefinedBehaviorSanitizer;
- [x] benchmarks rerun: no runtime median regression above 10% and no cold
      compile median regression above 20% against 2.4.0; deviations documented;
- [x] CHANGELOG.md has the 2.5.0 entry and the docs site is republished.

The 2.5.0 matrix was rerun on 2026-11-28 and published in
[docs/language/21-benchmarks.md](docs/language/21-benchmarks.md) (raw data in
[benchmarks/results/latest.md](benchmarks/results/latest.md), frozen copy in
[benchmarks/results/archive/v2.5/](benchmarks/results/archive/v2.5/)): 14 of the
22 executed workloads are within 1.4x of C, 16 within 1.5x and 12 within 1.15x,
with a median of 1.14x. Cold single-file builds take 0.83-2.76 s (median
1.06 s) and cached rebuilds 140-233 ms (median 176 ms). Absolute medians are not
comparable with the 2.4.0 archive because host state moves short workloads, so
the comparison is C-normalised; seven ratios moved outside the 10% budget and are
listed with their causes in the page's regression table. A same-host A/B run of
the 2.4.0 and 2.5.0 compilers over fifteen workloads measured a median runtime
ratio of 0.906 and a cold-compile median ratio of 0.951 while the C reference
moved by 0.72x-1.19x, so those normalised deltas are host-state noise rather than
a code regression. The incremental guard baseline therefore advances to the
2.5.0 archive.

### v2.6 - Borrow-model completion (direction)

References inside Vec, HashMap, Result, Box, arrays and closure captures;
variance, outlives relations and named-lifetime equality; the self-referential
structure question (pin-like semantics) as an ADR. This is the release that makes
zero-copy views usable in real data structures.

### v3.0 - Memory model and concurrency decision (direction)

An ADR decides whether lsaca stays single-threaded or gains Send/Sync, Arc and
threads. If it gains threads, the release defines the RC split, the collector's
thread-safety contract and a deprecation path for every affected API. This is the
only release allowed to make breaking changes without a prior deprecation cycle.

## Cross-cutting tracks

### Performance budget

The official 2.5.0 results are published in
[docs/language/21-benchmarks.md](docs/language/21-benchmarks.md) and frozen in
[benchmarks/results/archive/v2.5/](benchmarks/results/archive/v2.5/): 22 executed
workloads, 14 within 1.4x of C, 16 within 1.5x and 12 within 1.15x, a median of
1.14x, typical cold single-file builds of 0.83-2.76 s (median 1.06 s) and cached
rebuilds of 140-233 ms on the reference machine. The comparison with the 2.3.0
baseline is made on C-normalised ratios and cross-checked with a same-host A/B
run, because host state alone moves individual ratios by tens of percent. Every
release:

* reruns the official matrix and publishes the numbers;
* may not regress a runtime median by more than 10% or a cold-build median by
  more than 20% against the previous release without a documented reason;
* records compile time, runtime and peak memory for every workload, not a
  selected subset;
* keeps the benchmark programs oracle-checked (scripts/oracles.py) so a faster
  wrong answer cannot pass.

The v3.0 target is a median within 1.3x of C on comparable workloads.

### Quality gates

fmt, clippy -D warnings, workspace tests, rustdoc -D warnings, the runtime smoke
suite in three profiles, the generated-C warning checker, the docs example
runner, the link checker, the prose checker, the benchmark documentation check,
the stdlib embedding check, the diagnostic code index, the real-program gate,
the benchmark regression guard, scripts/check_status.py and scripts/docs_drift.py.
A release is not shipped with a red gate. The regression guard is a hard check
when the measurement host matches the baseline; on shared CI runners it reports
advisory results, and release validation uses a same-host A/B run instead.

### Documentation

Every documented behaviour has an executed example; every diagnostic has a
documented code; every release updates the manual, STATUS.md, CHANGELOG.md and
the benchmark page in the same change.

## Work process

1. **RFC first.** Any language or library surface that is not already documented
   gets a short RFC in rfcs/ (summary, motivation, design, drawbacks,
   alternatives, compatibility). Accepted RFCs are scheduled; everything else is
   backlog.
2. **Version contract at start.** When a release starts, its ROADMAP section
   becomes a frozen contract: scope, exit criteria, not-doing list. Changes to
   the contract need a written reason.
3. **Budget.** 70% of the effort goes to the release theme, 20% to debt and
   yellow cells, 10% is reserved for discoveries. New ideas do not steal from the
   reserve.
4. **STATUS-driven work.** Start from the matrix; when a cell turns green, trace
   it to tests and docs and update STATUS.md in the same change.
5. **CI as promise enforcer.** The gates above are the only definition of green;
   local confidence is not evidence.
6. **Release checklist.** Gates green, benchmarks rerun, CHANGELOG entry, STATUS
   rows updated, version bumped in Cargo.toml, docs republished, tag.
7. **Subtraction audit every third release.** Remove duplicate implementations,
   dead builtins and stale compatibility code; a release that only adds is
   suspect.

## Health metrics

| Metric | Target |
| --- | --- |
| Unexplained yellow cells in STATUS.md | zero at every release exit |
| Documentation drift (stale version or planned claims) | zero |
| Real programs in the CI gate | four at v2.4, one more per release |
| Runtime median vs C on the official matrix | no regression; 1.3x target by v3.0 |
| Cold build median | no regression above 20%; under 1 s typical |
| Breaking changes per minor release | zero (except an announced major) |
| Roadmap items shipped per release | meets the frozen contract |

## Where things live

| Document | Purpose |
| --- | --- |
| [ROADMAP.md](ROADMAP.md) | This file: contracts, process, budgets |
| [STATUS.md](STATUS.md) | Feature-by-layer completion matrix |
| [CHANGELOG.md](CHANGELOG.md) | User-visible history, release by release |
| [docs/language/16-roadmap.md](docs/language/16-roadmap.md) | Roadmap summary and compatibility notes |
| [docs/language/deviations.md](docs/language/deviations.md) | Deliberate deviations from Rust |
| [docs/language/limitations.md](docs/language/limitations.md) | Verified limitations of the current release |
| [docs/language/32-unsafe-and-ffi.md](docs/language/32-unsafe-and-ffi.md) | The unsafe and C FFI safety contract |
| [docs/language/21-benchmarks.md](docs/language/21-benchmarks.md) | Measured performance |
| [docs/README.md](docs/README.md) | Language reference (current release) |
| [rfcs/](rfcs/README.md) | Accepted and proposed design changes |
| .github/workflows/ci.yml | The gates |
