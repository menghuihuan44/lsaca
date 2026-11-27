# 0003: Derive, operators and formatting protocols stay closed in 2.x

* Status: accepted
* Target release: v2.4 (decision only; no implementation)
* Author: lsaca maintainers, 2026-11-27

## Summary

Rust-style protocol machinery -- `#[derive(...)]`, operator overloading via
`Add`/`Index`/`Deref`, and user-implementable `Display`, `Debug`, `Default`,
`PartialEq`, `Eq`, `Ord`, `Hash` -- is **refused for the 2.x series**. The
protocols the language already has stay compiler-defined and total: equality,
ordering, hashing and both formatting modes never call user code. This RFC
fixes the meaning of those protocols now so that a future release can add
derives or operator traits additively instead of redefining what existing
programs mean.

## Motivation

v2.4 is the tool-grade language release. Every candidate feature has to pass a
cost test: it may not introduce a second way to express the same program, and
it may not make the memory model or code generation harder to reason about.
The protocol suite fails both tests today:

* Operators are resolved by the compiler from the operand types (integer op,
  string concatenation, sequence comparison). Adding `Add` means the code
  generator must choose between an intrinsic and a trait method on every
  operator, and borrow checking must reason about two dispatch paths.
* Formatting is compiler-generated for every type that supports it. Allowing a
  user `Display` impl makes `println!` execute arbitrary user code; panics,
  recursion and ORC handle cycles inside formatting become the user's problem.
* Derives need either a proc-macro subsystem or a fixed set of compiler-blessed
  attributes. Both are a large surface that would land before the language has
  a stable trait/coherence story for implementations across crates.

Real programs already have the workaround: write the method or function
explicitly (`point.length_squared()`, `format!("...")`), or compare and hash
through the built-in operations.

## Design

### What stays compiler-defined (must not change in 2.x)

| Protocol | Meaning in 2.x |
| --- | --- |
| `==`, `!=` | Structural comparison for `Unit`, `Bool`, `Char`, integers, floats (IEEE), `String`, `Vec`, arrays, tuples, structs, enums, `Option`, `Result`; handle identity for `Rc` cells (deviation 13); never user code. |
| `<`, `<=`, `>`, `>=` | Integer/float ordering and lexicographic byte ordering of strings. No user-defined ordering. |
| Hashing | Only the built-in key types of `HashMap`/`HashSet` (`String`, `char`, `bool`, all integer widths, `isize`, `usize`); the hash function is the runtime's and is not extensible. |
| `{}` formatting | Compiler-generated for unit, bool, char, all integers, floats, `String`, and types that implement it through the built-in rules; never user code. |
| `{:?}` formatting | Compiler-generated structural debug output for every supported type; user types expose their structure, handles print as `Rc(payload)`; never user code. |

None of these protocols appears as a trait a user can implement. `#[derive]`
does not exist in the grammar (the lexer has no `#` token); `impl Display for T`
is `E0405` (unknown trait) today and stays that way.

### Boundary conditions recorded now

1. Protocol behaviour is part of the language, not of the library: a future
   release may add *new* traits, but it may not change what `a == b` means for
   a type that exists today.
2. `assert_eq!` keeps formatting both operands with `{:?}`; it never calls a
   user function.
3. `Rc` equality stays handle identity. If a future `PartialEq`-style trait
   appears, `Rc` will not implement it structurally, because comparing payloads
   of a cyclic structure does not terminate.
4. Sorting/searching helpers (`Vec::sort`, `BTreeMap`) are defined for the
   built-in key types only; generic ordering beyond them needs the same future
   trait work as operator overloading.
5. `Hash` for user structs is not provided and cannot be derived in 2.x, so a
   user struct cannot be a `HashMap` key. Wrap the key fields in a tuple of
   supported types or use a `String` key.

### What a future version must design first

* A protocol/dyn-dispatch story that does not require operator overloading to
  be useful; probably a dedicated RFC after the reference-model completion in
  v2.6.
* Derive syntax that is compiler-builtin (no proc macros) and works across
  crates without exposing implementation order to coherence.
* Operator traits only together with the ability to express the corresponding
  method call explicitly (UFCS) so that generic code has a fallback.

## Drawbacks

* No custom debug output makes diagnostics for user types less readable; the
  structural form is often noisy.
* No custom equality for structs used as set members; users must key on fields.
* Saying no now means a later release has to do the design work under more
  compatibility pressure.

## Alternatives

1. **Implement derives as compiler-blessed attributes only.** Cheapest good
   version of the feature, but it still needs the protocol story for `Hash`
   and `Ord`, and it competes for v2.4 budget with the items real programs
   need (patterns, break values, system surface). Deferred, not refused.
2. **Trait-based protocols with static dispatch.** Makes `==` and formatting
   generic, slows every formatting call and forces codegen to monomorphize
   protocol calls; rejected for 2.x.
3. **Proc macros.** Out of scope until the package/build model can carry macro
   crates; no target release.

## Compatibility and migration

No syntax or behaviour changes: every program that compiles today compiles
with the same meaning. The RFC only records that a future protocol feature is
additive; the one planned breaking door (`Rc` structural equality) is closed
by decision 3 above.

## Unresolved questions

* Whether compiler-blessed `#[derive]` lands in 2.6 together with references
  in aggregates or later.
* Whether a `Hash` protocol is needed before generic collections can accept
  user struct keys; if so it must be designed with the hashing ABI, not on top
  of the current Swiss-table runtime.
