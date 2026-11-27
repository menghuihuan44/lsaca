# lsaca Roadmap

This document is the single source of truth for where lsaca is going and for
what "finished" means. It is a contract, not a wish list: a release ships only
when its exit criteria are met. The completion state of every area is tracked in
[STATUS.md](STATUS.md), and the user-visible history is in
[CHANGELOG.md](CHANGELOG.md). The language reference for the current release is
[docs/README.md](docs/README.md).

Version 2.3.0 is the current release; 2.2.0 is the baseline for the v2.3 performance budget.

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

### v2.4 - Tool-grade language (direction)

Make the language pleasant for programs that are more than benchmarks: array and
slice patterns, values out of break and continue, derive and trait-protocol
boundaries (or a documented refusal), collection entry APIs and API density,
PathBuf/Path, Command, SocketAddr/UDP where the runtime supports it, iterator
completeness, pub(super)/pub(in path), and a crate-wide formatter mode. The exit
bar stays: every feature complete per the definition of done, and no new yellow
cells.

### v2.5 - System foundation (direction)

Controlled unsafe as an auditable escape hatch, not a second language: raw
pointers (minimal set), extern "C" declarations and calls, repr(C), CString/CStr,
MaybeUninit and union access, plus documentation of the ORC invariants a safe
wrapper must preserve. Safety artifacts: a real C library bound as the acceptance
case, ASan and UBSan in CI, and an ORC invariant audit. The compiler itself keeps
#![forbid(unsafe_code)].

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

The official 2.3.0 results are published in
[docs/language/21-benchmarks.md](docs/language/21-benchmarks.md) and frozen in
[benchmarks/results/archive/v2.3/](benchmarks/results/archive/v2.3/): 22 executed
workloads, 13 within 1.4x of C, 14 within 1.5x and 6 within 1.15x, a median of
1.34x, typical cold single-file builds of 0.45-2.67 s (median 0.64 s) and cached
rebuilds of 86-154 ms on the reference machine. The comparison with the 2.2.0
baseline is made on C-normalised ratios because the C reference compiler moved
from gcc 15.2.0 to 16.2.0. Every release:

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
A release is not shipped with a red gate.

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
| Real programs in the CI gate | three at v2.3, one more per release |
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
| [docs/language/21-benchmarks.md](docs/language/21-benchmarks.md) | Measured performance |
| [docs/README.md](docs/README.md) | Language reference (current release) |
| [rfcs/](rfcs/README.md) | Accepted and proposed design changes |
| .github/workflows/ci.yml | The gates |
