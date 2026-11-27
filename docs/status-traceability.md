# Status traceability

This file is the evidence table for the completion matrix in [STATUS.md](../STATUS.md).
Every entry maps one STATUS row to the tests that exercise it and the
documentation that specifies it. The file contains rows that are not complete
yet; scripts/check_status.py mandates evidence for rows whose STATUS.md state is
`complete` and validates every reference that is present.

The format is deliberately small:

1. There is one `##` section per STATUS.md section, with the same section title.
2. Each section holds a Markdown table with the columns
   `Row | Tests | Documentation`.
3. `Tests` is a semicolon-separated list of `path[::symbol]` references.
   The path is relative to the repository root. The script reads the file and
   checks that the symbol (or `mod name` for a module) occurs in it. A bare
   path is accepted when the whole file is the test suite, such as a C test
   file or a script.
4. `Documentation` is a semicolon-separated list of `docs/...md[#anchor]`
   references. The script checks the file exists and, when an anchor is
   present, recomputes the GitHub heading slug for the page and requires a
   matching heading.

The table covers every row in the current matrix: rows that are still partial
or missing are listed so they already have verified evidence when they flip to
complete. Only complete rows are mandatory today.

Maintaining it: add or update the row here whenever a STATUS row flips to
`complete`, and run `python scripts/check_status.py`.

## Language and semantics

| Row | Tests | Documentation |
| --- | --- | --- |
| Lexing, literals, escapes, comments | crates/lsaca/src/lexer.rs::mod tests; crates/lsaca/tests/e2e.rs::compiled_programs | docs/language/03-lexical-structure.md#identifiers; docs/language/03-lexical-structure.md#comments; docs/language/03-lexical-structure.md#integer-literals; docs/language/03-lexical-structure.md#string-literals; docs/language/03-lexical-structure.md#character-literals |
| Type system and casts | crates/lsaca/src/typeck/mod.rs::mod tests; crates/lsaca/tests/e2e.rs::compiled_programs | docs/language/04-types.md#the-type-table; docs/language/04-types.md#casts; docs/language/07-operators.md#casts |
| Operators and overflow | crates/lsaca/tests/e2e.rs::release_builds_wrap_overflow; crates/lsaca/tests/e2e.rs::compiled_programs; runtime/test/main.c::test_checked_math | docs/language/07-operators.md#precedence-and-associativity; docs/language/07-operators.md#overflow-and-division-by-zero |
| Statements and control flow | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/tests/robustness.rs::accepts_labelled_break_and_continue; crates/lsaca/tests/robustness.rs::rejects_break_with_a_value; crates/lsaca/tests/robustness.rs::rejects_unknown_loop_label | docs/language/08-statements-and-control-flow.md#blocks; docs/language/08-statements-and-control-flow.md#break-and-continue; docs/language/28-loop-labels.md#leaving-a-nested-loop |
| Functions and function values | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/src/codegen/tests.rs | docs/language/09-functions.md#functions; docs/language/09-functions.md#return-values; docs/language/09-functions.md#function-values |
| Structs, enums, impl, tuple and unit structs | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/src/codegen/tests.rs | docs/language/10-structs-enums-and-impl.md#structs; docs/language/10-structs-enums-and-impl.md#tuple-and-unit-structs; docs/language/10-structs-enums-and-impl.md#methods-and-associated-functions-with-impl |
| Ownership, moves, Drop | crates/lsaca/src/moveck.rs::mod tests; crates/lsaca/tests/e2e.rs::move_errors_are_reported; crates/lsaca/tests/e2e.rs::compiled_programs | docs/language/11-ownership-and-moves.md#moves; docs/language/11-ownership-and-moves.md#the-drop-trait; docs/language/11-ownership-and-moves.md#no-explicit-drop-function; docs/language/06-variables-and-mutability.md#destructuring-bindings; docs/language/deviations.md#deviations-from-rust |
| Borrow checker | crates/lsaca/src/borrow.rs::mod tests; crates/lsaca/tests/e2e.rs::borrow_errors_are_reported; crates/lsaca/tests/e2e.rs::compiled_programs | docs/language/12-references-and-borrowing.md#aliasing-rules; docs/language/12-references-and-borrowing.md#known-limitations-in-version-230 |
| References in aggregates | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/tests/robustness.rs::accepts_reference_fields_in_a_struct; crates/lsaca/tests/robustness.rs::rejects_reference_stored_in_vector | docs/language/12-references-and-borrowing.md#where-references-are-allowed; docs/language/29-lifetimes.md#reference-fields; docs/language/12-references-and-borrowing.md#known-limitations-in-version-230 |
| Lifetimes | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/tests/robustness.rs::accepts_reference_field_with_a_named_lifetime; crates/lsaca/tests/robustness.rs::rejects_reference_field_without_a_lifetime | docs/language/29-lifetimes.md#lifetimes; docs/language/29-lifetimes.md#elision-rules; docs/language/29-lifetimes.md#where-references-may-be-stored |
| Generics | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/src/typeck/mod.rs::mod tests | docs/language/17-generics.md#generics; docs/language/17-generics.md#generic-structs; docs/language/17-generics.md#what-generics-do-not-include |
| Traits | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/src/typeck/mod.rs::mod tests | docs/language/18-traits.md#declaring-a-trait; docs/language/18-traits.md#default-method-bodies; docs/language/18-traits.md#what-traits-do-not-include |
| Trait objects | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/src/codegen/tests.rs | docs/language/25-trait-objects.md#the-dyn-type; docs/language/25-trait-objects.md#object-safety-e0038; docs/language/25-trait-objects.md#known-limitations-in-220 |
| Pattern matching | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/src/typeck/mod.rs::mod tests | docs/language/13-pattern-matching.md#patterns; docs/language/13-pattern-matching.md#or-patterns; docs/language/13-pattern-matching.md#known-limitations-in-220 |
| Closures | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/src/codegen/tests.rs | docs/language/14-closures.md#capture-by-value; docs/language/14-closures.md#the-move-keyword; docs/language/14-closures.md#known-limitations-in-220 |
| Modules and visibility | crates/lsaca/tests/modules_audit.rs::audit_positive_module_programs_compile_and_run; crates/lsaca/tests/modules_audit.rs::audit_negative_module_programs_are_rejected; crates/lsaca/tests/e2e.rs::compiled_module_programs; crates/lsaca/tests/e2e.rs::module_diagnostics_are_reported | docs/language/23-modules.md#modules; docs/language/23-modules.md#visibility; docs/language/23-modules.md#known-limitations-in-220 |
| Packages | crates/lsaca/tests/e2e.rs::compiled_package_programs; crates/lsaca/tests/e2e.rs::library_packages_type_check_without_main; crates/lsaca/tests/e2e.rs::package_visibility_is_enforced; crates/lsaca-cli/tests/cli.rs::new_creates_a_runnable_binary_package | docs/language/24-packages.md#packages; docs/language/24-packages.md#the-manifest; docs/language/24-packages.md#the-dependencies-table |
| Macros and attributes | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/tests/e2e.rs::format_specifier_errors_are_reported | docs/language/15-standard-library.md#the-formatting-macros; docs/language/15-standard-library.md#assertion-and-vec-macros; docs/language/deviations.md#deviations-from-rust |
| Method call form (UFCS) | crates/lsaca/tests/e2e.rs::v15_feature_errors_are_reported; crates/lsaca/src/typeck/mod.rs::mod tests | docs/language/deviations.md#deviations-from-rust |
| Unsafe and FFI | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/src/parser.rs::mod tests | docs/language/deviations.md#deviations-from-rust; docs/language/09-functions.md#extern-functions |
| Threads, Send, Sync | crates/lsaca/tests/e2e.rs::compiled_programs; runtime/test/main.c::test_alloc | docs/language/deviations.md#deviations-from-rust; docs/language/limitations.md#not-in-the-current-release |

## Standard library

| Row | Tests | Documentation |
| --- | --- | --- |
| core (Option, Result, Iterator) | crates/lsaca/tests/e2e.rs::compiled_programs; runtime/test/main.c::test_option_result | docs/stdlib/core.md#optiont; docs/stdlib/core.md#resultt-e; docs/stdlib/core.md#iteratort |
| string, str | crates/lsaca/tests/e2e.rs::compiled_programs; runtime/test/main.c::test_strings | docs/stdlib/string.md#string-methods; docs/stdlib/string.md#str-methods; docs/stdlib/string.md#concatenation-and-parse |
| char | crates/lsaca/tests/e2e.rs::compiled_programs; runtime/test/main.c::test_strings | docs/stdlib/char.md#the-type; docs/stdlib/char.md#ascii-classification-and-conversion |
| vec | crates/lsaca/tests/e2e.rs::compiled_programs; runtime/test/main.c::test_vec_string; runtime/test/main.c::test_vec_primitives | docs/stdlib/vec.md#method-reference |
| iter | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/src/codegen/tests.rs | docs/stdlib/iter.md#adapters; docs/stdlib/iter.md#terminal-operations |
| HashMap | crates/lsaca/tests/e2e.rs::compiled_programs; runtime/test/main.c::test_map_int_basics; runtime/test/main.c::test_map_string_keys; runtime/test/main.c::test_map_string_values | docs/stdlib/collections/hash-map.md#surface; docs/stdlib/collections/hash-map.md#rules |
| HashSet, BTreeSet, BTreeMap, VecDeque, BinaryHeap | crates/lsaca/tests/e2e.rs::compiled_programs | docs/stdlib/collections/README.md#containers; docs/stdlib/collections/hash-set.md#surface; docs/stdlib/collections/btree-set.md#surface; docs/stdlib/collections/btree-map.md#surface; docs/stdlib/collections/deque.md#surface; docs/stdlib/collections/binary-heap.md#surface |
| fmt | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/tests/e2e.rs::format_specifier_errors_are_reported; runtime/test/main.c::test_formatting; runtime/test/main.c::test_format_specs | docs/stdlib/fmt.md#macros; docs/stdlib/fmt.md#format-specifiers; docs/language/15-standard-library.md#format-specifier-grammar |
| math | runtime/test/main.c::test_checked_math; crates/lsaca/tests/e2e.rs::compiled_programs | docs/stdlib/math.md#constants |
| io | crates/lsaca/tests/e2e.rs::compiled_io_programs; runtime/test/io_tests.c | docs/stdlib/io.md#text-helpers; docs/stdlib/io.md#handles |
| fs | crates/lsaca/tests/e2e.rs::compiled_io_programs; runtime/test/io_tests.c | docs/stdlib/fs.md#functions; docs/stdlib/fs.md#metadata-and-file-types |
| net | crates/lsaca/tests/e2e.rs::compiled_io_programs; runtime/test/io_tests.c | docs/stdlib/net.md#tcplistener; docs/stdlib/net.md#tcpstream |
| time | crates/lsaca/tests/e2e.rs::compiled_io_programs; runtime/test/io_tests.c | docs/stdlib/time.md#duration; docs/stdlib/time.md#instant; docs/stdlib/time.md#systemtime; docs/stdlib/time.md#sleep |
| env | crates/lsaca/tests/e2e.rs::compiled_io_programs; runtime/test/io_tests.c | docs/stdlib/env.md#env-environment-variables |
| process | crates/lsaca/tests/e2e.rs::compiled_io_programs; runtime/test/run_smoke.ps1 | docs/stdlib/process.md#process-arguments-and-exit |
| error | crates/lsaca/tests/e2e.rs::compiled_programs; crates/lsaca/tests/e2e.rs::rejects_try_operator_misuse | docs/stdlib/error.md#error-the-error-trait; docs/language/30-from-try.md#the-error-trait-and-source |

## Tooling and delivery

| Row | Tests | Documentation |
| --- | --- | --- |
| CLI (run, build, emit-c, check, fmt, new, version, help) | crates/lsaca-cli/tests/cli.rs::version_is_printed; crates/lsaca-cli/tests/cli.rs::check_accepts_a_valid_program; crates/lsaca-cli/tests/cli.rs::run_builds_and_executes_a_program; crates/lsaca-cli/tests/cli.rs::emit_c_writes_a_cmake_project | docs/language/01-getting-started.md#command-line-interface; docs/language/01-getting-started.md#exit-codes |
| Build cache | crates/lsaca-cli/tests/cli.rs::unusable_cache_directory_is_reported; crates/lsaca/src/driver.rs::mod tests | docs/language/01-getting-started.md#how-compilation-works; docs/language/01-getting-started.md#the-generated-c-project |
| Package manager | crates/lsaca-cli/tests/cli.rs::new_creates_a_runnable_binary_package; crates/lsaca-cli/tests/cli.rs::package_dependencies_are_built_together; crates/lsaca/tests/e2e.rs::compiled_package_programs | docs/language/24-packages.md#the-manifest; docs/language/24-packages.md#the-dependencies-table; docs/language/24-packages.md#creating-a-package |
| Formatter | crates/lsaca-cli/tests/cli.rs::fmt_rewrites_indentation_and_line_endings; crates/lsaca-cli/tests/cli.rs::fmt_check_reports_but_does_not_modify; crates/lsaca/src/fmt.rs::mod tests | docs/language/01-getting-started.md#formatting-source-files |
| Benchmarks | scripts/check_bench21.py; scripts/bench.py | docs/language/21-benchmarks.md#results; docs/language/21-benchmarks.md#validation; docs/language/21-benchmarks.md#reproducing |
| Documentation site | scripts/docsite.py; scripts/linkcheck.py; scripts/docs_verify.py | docs/README.md#documentation-map |
| CI | .github/workflows/ci.yml; scripts/check_generated_c.py | docs/language/01-getting-started.md#command-line-interface |
| Diagnostic code index | crates/lsaca/src/diag.rs::mod tests; crates/lsaca-cli/tests/cli.rs::check_reports_type_errors | docs/language/01-getting-started.md#diagnostics |
| Conformance traceability | scripts/docs_verify.py; crates/lsaca/tests/e2e.rs::compiled_programs | docs/README.md#documentation-map |
| Real-program CI gate | crates/lsaca/tests/e2e.rs::real_programs_match_expected_output | docs/README.md#guides-and-examples |
