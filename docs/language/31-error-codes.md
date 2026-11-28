# Error codes

A diagnostic prints its severity, its code when it has one, and its message, for
example `error[E0308]: mismatched types`; a warning with a code prints
`warning[...]: message` in the same shape. A compile error exits with status 1
and a runtime panic exits with status 101, as described in
[Getting started](01-getting-started.md#exit-codes). Codes are stable across
releases and are never reused: a retired diagnostic keeps its code retired.
This page indexes every code the compiler can emit.

## How this page is organized

Codes are grouped by the phase that reports them. The "Raised in" column names
the compiler source files, relative to the repository root, where the
diagnostic is constructed; a code can be raised from more than one file. A few
codes are shared by two phases, and their meaning covers every use.

## Lexing and parsing

Errors in the token stream and the syntax tree, reported before type checking starts.

| Code | Meaning | Raised in |
| --- | --- | --- |
| `E0001` | A general-purpose error with no more specific code: an unexpected character or token, a manifest problem, a package with both entry points, a circular module file, or an unsupported unsized type. | `crates/lsaca/src/lexer.rs`, `crates/lsaca/src/parser.rs`, `crates/lsaca/src/package.rs`, `crates/lsaca/src/driver.rs`, `crates/lsaca/src/typeck/refs.rs` |
| `E0002` | An invalid integer literal: no digits after a radix prefix, a bad digit or suffix, or a value too large for any supported integer type. | `crates/lsaca/src/lexer.rs` |
| `E0003` | An invalid float literal: a missing exponent digit, a bad suffix, or a value that is not a finite `f64`. | `crates/lsaca/src/lexer.rs` |
| `E0004` | A string or character literal is not terminated, or a `match` expression does not cover every value. | `crates/lsaca/src/lexer.rs`, `crates/lsaca/src/typeck/exhaust.rs` |
| `E0005` | An invalid escape sequence in a literal, or a refutable pattern in a local `let` binding. | `crates/lsaca/src/lexer.rs`, `crates/lsaca/src/typeck/patterns.rs` |
| `E0006` | A block comment is not terminated. | `crates/lsaca/src/lexer.rs` |
| `E0007` | A character literal is empty or contains more than one codepoint. | `crates/lsaca/src/lexer.rs` |

## Names, modules and packages

Name resolution, imports, visibility, module files, and package layout.

| Code | Meaning | Raised in |
| --- | --- | --- |
| `E0252` | The same name is imported more than once. | `crates/lsaca/src/typeck/modules.rs` |
| `E0255` | An imported name conflicts with another definition of the same name. | `crates/lsaca/src/typeck/modules.rs` |
| `E0364` | A private item is re-exported with `pub use`. | `crates/lsaca/src/typeck/modules.rs` |
| `E0365` | A private module is re-exported with `pub use`. | `crates/lsaca/src/typeck/modules.rs` |
| `E0423` | A type, module, generic function, or other non-value name is used where a value is expected. | `crates/lsaca/src/typeck/exprs/paths.rs`, `crates/lsaca/src/typeck/exprs/calls.rs` |
| `E0424` | `self` is used in a method that has no `self` parameter. | `crates/lsaca/src/typeck/exprs/paths.rs` |
| `E0425` | A function or value name cannot be resolved in this scope. | `crates/lsaca/src/typeck/exprs/paths.rs`, `crates/lsaca/src/typeck/exprs/calls.rs` |
| `E0428` | A name is defined more than once in the same scope. | `crates/lsaca/src/typeck/items.rs`, `crates/lsaca/src/typeck/modules.rs`, `crates/lsaca/src/typeck/traits.rs`, `crates/lsaca/src/driver.rs` |
| `E0432` | An import cannot be resolved, or a glob import is used, which is not supported. | `crates/lsaca/src/typeck/modules.rs` |
| `E0433` | A module or a segment of a path cannot be resolved, including the path of a `pub(in …)` restriction. | `crates/lsaca/src/typeck/modules.rs`, `crates/lsaca/src/typeck/exprs/paths.rs`, `crates/lsaca/src/typeck/exprs/calls.rs` |
| `E0451` | A struct field named in a struct literal or pattern is private. | `crates/lsaca/src/typeck/exprs.rs`, `crates/lsaca/src/typeck/patterns.rs` |
| `E0583` | A module file cannot be found, read, or loaded from an in-memory source. | `crates/lsaca/src/driver.rs` |
| `E0603` | An item or module is private and cannot be accessed from here. | `crates/lsaca/src/typeck/modules.rs` |
| `E0616` | A field accessed on a value is private. | `crates/lsaca/src/typeck/exprs.rs` |
| `E0624` | An associated function or method is private. | `crates/lsaca/src/typeck/exprs/calls.rs`, `crates/lsaca/src/typeck/exprs/generic_calls.rs` |
| `E0659` | An enum variant name is ambiguous and must be qualified with the enum name. | `crates/lsaca/src/typeck/exprs/paths.rs` |
| `E0742` | A restricted visibility names a module that is not an ancestor of the item, including `pub(super)` at the crate root. | `crates/lsaca/src/typeck/modules.rs` |
| `E0761` | A module has files at both `name.ls` and `name/mod.ls`. | `crates/lsaca/src/driver.rs` |

## Types, generics and traits

Type checking, inference, generic instantiation, trait matching, and member lookup.

| Code | Meaning | Raised in |
| --- | --- | --- |
| `E0034` | A method call matches more than one applicable item in scope. | `crates/lsaca/src/typeck/traits.rs`, `crates/lsaca/src/typeck/exprs/generic_calls.rs` |
| `E0038` | A trait cannot be made into a trait object because it is not object-safe. | `crates/lsaca/src/typeck/dyn_traits.rs` |
| `E0046` | A trait implementation does not implement all required trait items. | `crates/lsaca/src/typeck/traits.rs` |
| `E0053` | A method has a signature incompatible with the trait it implements. | `crates/lsaca/src/typeck/traits.rs` |
| `E0061` | A function, method, constructor, or builtin receives the wrong number of arguments. | `crates/lsaca/src/typeck/exprs/calls.rs`, `crates/lsaca/src/typeck/iterators.rs` |
| `E0062` | A struct initializer specifies the same field more than once. | `crates/lsaca/src/typeck/exprs.rs` |
| `E0063` | A struct initializer omits a field. | `crates/lsaca/src/typeck/exprs.rs` |
| `E0072` | A recursive type contains itself by value and has infinite size. | `crates/lsaca/src/typeck/items.rs` |
| `E0106` | A reference is missing a required lifetime specifier, or a lifetime is used where only a generic argument is allowed. | `crates/lsaca/src/typeck/items.rs`, `crates/lsaca/src/typeck/generics.rs` |
| `E0107` | Wrong number of generic arguments, or generic arguments supplied where the item takes none. | `crates/lsaca/src/typeck/items.rs`, `crates/lsaca/src/typeck/generics.rs`, `crates/lsaca/src/typeck/exprs/calls.rs` |
| `E0118` | An `impl` target is not a struct or enum type defined in this program. | `crates/lsaca/src/typeck/items.rs` |
| `E0119` | Conflicting trait implementations: an explicit `Copy` or `Clone` impl, a second `Drop` impl, or another overlapping impl. | `crates/lsaca/src/typeck/items.rs`, `crates/lsaca/src/typeck/traits.rs` |
| `E0124` | A struct declares the same field name twice. | `crates/lsaca/src/typeck/items.rs` |
| `E0261` | A lifetime name is used but never declared. | `crates/lsaca/src/typeck/items.rs` |
| `E0275` | The recursion limit is reached while instantiating a generic function. | `crates/lsaca/src/typeck/generics.rs` |
| `E0277` | A use requires a trait bound or iterator capability that the type does not provide, such as `Iterator` for a `for` loop or `Hash`/`Eq` for a map key. | `crates/lsaca/src/typeck/generics.rs`, `crates/lsaca/src/typeck/dyn_traits.rs`, `crates/lsaca/src/typeck/iterators.rs`, `crates/lsaca/src/typeck/hash_map.rs` |
| `E0282` | The compiler cannot infer a type and a type annotation is needed. | `crates/lsaca/src/typeck/check.rs`, `crates/lsaca/src/typeck/generics.rs`, `crates/lsaca/src/typeck/patterns.rs` |
| `E0283` | The compiler cannot infer the generic arguments of a call and a type annotation is needed. | `crates/lsaca/src/typeck/generics.rs`, `crates/lsaca/src/typeck/exprs/generic_calls.rs` |
| `E0308` | Mismatched types, including errors in operator and cast operand types, a fixed-size array pattern whose length differs from the array, and a value on `continue`. | `crates/lsaca/src/parser.rs`, `crates/lsaca/src/typeck/check.rs`, `crates/lsaca/src/typeck/exprs/ops.rs`, `crates/lsaca/src/typeck/generics.rs`, `crates/lsaca/src/typeck/patterns.rs` |
| `E0369` | A binary operator cannot be applied to these operand types, or cannot be used as an assignment operator. | `crates/lsaca/src/typeck/exprs/ops.rs` |
| `E0392` | A lifetime parameter is declared but never used. | `crates/lsaca/src/typeck/items.rs` |
| `E0403` | A generic parameter name is already used by another generic parameter. | `crates/lsaca/src/typeck/items.rs`, `crates/lsaca/src/typeck/traits.rs` |
| `E0405` | A trait name cannot be resolved in this scope. | `crates/lsaca/src/typeck/items.rs`, `crates/lsaca/src/typeck/generics.rs`, `crates/lsaca/src/typeck/traits.rs` |
| `E0407` | A method is not a member of the trait it is implemented for. | `crates/lsaca/src/typeck/traits.rs` |
| `E0411` | `Self` is used outside an `impl` block. | `crates/lsaca/src/typeck/exprs/paths.rs`, `crates/lsaca/src/typeck/exprs.rs`, `crates/lsaca/src/typeck/patterns.rs` |
| `E0412` | A type name cannot be resolved, or a trait is used directly as a type. | `crates/lsaca/src/typeck/items.rs`, `crates/lsaca/src/typeck/exprs/paths.rs`, `crates/lsaca/src/typeck/patterns.rs` |
| `E0435` | An array length must be an integer literal or an integer constant. | `crates/lsaca/src/typeck/exprs.rs` |
| `E0532` | A constructor or pattern uses the wrong form: a struct literal, tuple constructor, or variant with the expected number of arguments was required. | `crates/lsaca/src/typeck/exprs/calls.rs`, `crates/lsaca/src/typeck/exprs/paths.rs`, `crates/lsaca/src/typeck/exprs.rs`, `crates/lsaca/src/typeck/patterns.rs` |
| `E0560` | A struct literal names a field the struct does not have. | `crates/lsaca/src/typeck/exprs.rs` |
| `E0592` | Two definitions with the same name appear in one `impl` block. | `crates/lsaca/src/typeck/items.rs` |
| `E0599` | A method, associated function, variant, or other associated item cannot be resolved or called in this form for the type. | `crates/lsaca/src/typeck/exprs/calls.rs`, `crates/lsaca/src/typeck/exprs/paths.rs`, `crates/lsaca/src/typeck/exprs/generic_calls.rs` |
| `E0600` | A unary operator (`-` or `!`) cannot be applied to this type. | `crates/lsaca/src/typeck/exprs/ops.rs` |
| `E0605` | A cast with `as` is not allowed between these types. | `crates/lsaca/src/typeck/exprs/ops.rs` |
| `E0608` | A value of this type cannot be indexed or sliced. | `crates/lsaca/src/typeck/exprs.rs`, `crates/lsaca/src/typeck/slices.rs` |
| `E0609` | A field or tuple index does not exist on the type. | `crates/lsaca/src/typeck/exprs.rs` |
| `E0614` | This type cannot be dereferenced. | `crates/lsaca/src/typeck/refs.rs` |
| `E0618` | A call expression is applied to a value that is not a function. | `crates/lsaca/src/typeck/exprs/calls.rs` |

## Ownership, moves and borrowing

Move checking and the borrow checker.

| Code | Meaning | Raised in |
| --- | --- | --- |
| `E0382` | A value is used after it was moved. | `crates/lsaca/src/moveck.rs` |
| `E0498` | A reference cannot be stored, captured, or given a `'static` lifetime in this position, or a mutable reference targets an unsupported type. | `crates/lsaca/src/typeck/refs.rs`, `crates/lsaca/src/typeck/items.rs`, `crates/lsaca/src/typeck/check.rs` |
| `E0499` | A value is borrowed as mutable more than once at a time. | `crates/lsaca/src/borrow.rs` |
| `E0502` | A value is borrowed as immutable while it is also borrowed as mutable. | `crates/lsaca/src/borrow.rs` |
| `E0505` | A value is moved while it is borrowed. | `crates/lsaca/src/borrow.rs` |
| `E0506` | A value is assigned while it is borrowed. | `crates/lsaca/src/borrow.rs` |
| `E0507` | A value is moved out of a place that cannot be moved from, such as a borrowed value, a field of a borrowed value, or a match guard. | `crates/lsaca/src/moveck.rs`, `crates/lsaca/src/typeck/exprs.rs` |
| `E0509` | A value that implements `Drop` is destructured by value in a pattern. | `crates/lsaca/src/typeck/patterns.rs` |
| `E0515` | A returned value borrows from a local, a captured value, or a parameter other than the elided lifetime source. | `crates/lsaca/src/borrow.rs`, `crates/lsaca/src/typeck/refs.rs` |
| `E0596` | A value is mutated through a shared reference, or assigned through an immutable binding or a non-place expression. | `crates/lsaca/src/typeck/exprs.rs`, `crates/lsaca/src/typeck/exprs/ops.rs`, `crates/lsaca/src/typeck/exprs/calls.rs` |
| `E0597` | A borrowed value does not live long enough. | `crates/lsaca/src/borrow.rs` |
| `E0745` | A temporary value is borrowed; bind it to a local first. | `crates/lsaca/src/typeck/refs.rs`, `crates/lsaca/src/typeck/slices.rs` |

## Unsafe code and C FFI

The `unsafe` permission boundary, raw pointers, C declarations, C-layout types and the
unions that represent C overlay storage.

| Code | Meaning | Raised in |
| --- | --- | --- |
| `E0133` | An unsafe operation is performed outside an `unsafe` block or `unsafe fn`: dereferencing or assigning through a raw pointer, calling an `extern "C"` function, `MaybeUninit::assume_init`, `CStr::from_ptr`, `CString::from_raw`, or constructing, reading or writing a union. | `crates/lsaca/src/typeck/exprs.rs` |
| `E0770` | A type in an `extern "C"` declaration is not FFI-safe: managed values such as `String`, `Vec`, `Rc`, `Box`, `Option`, `Result` and `HashMap`, tuples, enums, trait objects, slices, `str`, `CString`/`CStr`/`MaybeUninit`, or a struct without `#[repr(C)]`. | `crates/lsaca/src/typeck/items.rs` |
| `E0771` | A `repr` attribute is unsupported or misplaced: only `#[repr(C)]` is accepted, and only on a struct or (as a no-op) on a union. | `crates/lsaca/src/parser.rs` |
| `E0772` | A union field has a type that C cannot overlay: a managed type or another forbidden non-FFI-safe type. | `crates/lsaca/src/typeck/items.rs` |
| `E0773` | A union is used in a forbidden way: it has a `Drop` impl or method, it is destructured by a pattern, or it is used with generated equality or formatting. | `crates/lsaca/src/typeck/items.rs`, `crates/lsaca/src/typeck/patterns.rs` |

## Patterns and control flow

Pattern shape and bindings, labels, loop control, and match coverage.

| Code | Meaning | Raised in |
| --- | --- | --- |
| `E0023` | A tuple or tuple-struct pattern has the wrong number of elements. | `crates/lsaca/src/typeck/patterns.rs` |
| `E0025` | A struct field is bound more than once in one pattern. | `crates/lsaca/src/typeck/patterns.rs` |
| `E0026` | A struct pattern names a field the struct does not have. | `crates/lsaca/src/typeck/patterns.rs` |
| `E0027` | A struct pattern omits a field without using `..`. | `crates/lsaca/src/typeck/patterns.rs` |
| `E0268` | `break` or `continue` is used outside a loop. | `crates/lsaca/src/typeck/exprs.rs` |
| `E0408` | A variable is not bound in every alternative of an or-pattern. | `crates/lsaca/src/typeck/patterns.rs` |
| `E0416` | An identifier is bound more than once in one pattern. | `crates/lsaca/src/typeck/patterns.rs` |
| `E0426` | A label name is used but never declared. | `crates/lsaca/src/typeck/exprs.rs` |
| `E0571` | A `break` carries a value from a `while` or `for` loop, whose type is `()`. | `crates/lsaca/src/typeck/exprs.rs` |

## Standard library and const evaluation

Constant folding and the builtin calls the compiler provides for the standard library.

| Code | Meaning | Raised in |
| --- | --- | --- |
| `E0015` | A constant initializer is not a constant expression, or uses `?` inside a constant. | `crates/lsaca/src/typeck/consts.rs`, `crates/lsaca/src/typeck/exprs.rs` |
| `E0080` | Constant evaluation failed: a literal is out of range, an operation overflows, divides by zero, shifts by too much, or an array length is invalid. | `crates/lsaca/src/typeck/consts.rs`, `crates/lsaca/src/typeck/lower.rs` |
| `E0301` | A standard-library builtin requires a numeric argument but was given another type. | `crates/lsaca/src/typeck/exprs/calls.rs` |
| `E0391` | A cycle is detected while evaluating a constant. | `crates/lsaca/src/typeck/consts.rs` |

## Code generation and build

Entry-point checks and build-level constraints checked before C is emitted.

| Code | Meaning | Raised in |
| --- | --- | --- |
| `E0580` | `main` is declared with parameters or returns a non-unit type. | `crates/lsaca/src/typeck/mod.rs` |
| `E0601` | No `main` function is found. | `crates/lsaca/src/typeck/mod.rs` |

## How this page is checked

`scripts/check_error_index.py` keeps this page honest. It scans every Rust
source file under `crates/lsaca/src`, collects the quoted `"E"` code
literals, parses the Code and Meaning columns of the tables above, and fails
when a code is raised in the compiler but missing here, when a code is
documented but never raised, when a row has an empty meaning, or when a code is
listed twice. On success it prints one summary line:

```text
error index: 94 codes documented and all raised in the compiler
```

The documentation gate runs the script in CI with the link check and the prose
check. From the repository root:

```text
python scripts/check_error_index.py
```

Pass `--root DIR` to check another checkout, and `--json PATH` to write a
machine-readable report of the documented and raised codes.
