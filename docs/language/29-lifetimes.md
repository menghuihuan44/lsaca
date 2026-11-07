# Lifetimes

Version 2.2 adds lightweight lifetimes for storing references in user-defined
aggregate types. Before 2.2 a reference could only appear in a function
signature; now a struct, enum, tuple struct or `Option` field may hold `&'a T`
or `&'a mut T` as long as the type names the lifetime. This is not the full
Rust lifetime system: there are no lifetime bounds, outlives requirements,
variance checks or general inference, and all lifetime information is erased
after type checking. The feature exists so a view type can bundle data with a
borrow of that data without copying it.

## Reference fields

A type declares its lifetime parameters in angle brackets, and every reference
field must use a declared lifetime:

```rust
struct View<'a> {
    data: &'a [i64],
}

struct Names<'a> {
    first: &'a str,
    second: &'a str,
}

enum Token<'a> {
    Word(&'a str),
    Number(i64),
}

struct Slice<'a, T> {
    items: &'a [T],
}
```

A reference field that omits its lifetime is rejected with
`error[E0106]: missing lifetime specifier`; there is no implicit lifetime for
fields. A type path may carry lifetime arguments, as in `View<'a>`, and a
lifetime argument that was not declared on the item is reported as
`error[E0261]: undeclared lifetime`.

The values are constructed with ordinary struct syntax, borrowing data that
outlives the aggregate:

```rust
fn main() {
    let values = vec![1i64, 2, 3];
    let view = View { data: &values };
    println!("{}", view.data[1]);
}
```

Reading through a reference field dereferences it automatically. A `&'a mut T`
field gives the aggregate mutable access to the borrowed data, subject to the
same single-mutable-borrow rule as any other `&mut`.

## impl blocks and methods

An `impl` block repeats the lifetime parameters of the type:

```rust
impl<'a> View<'a> {
    fn len(&self) -> usize {
        return self.data.len();
    }

    fn get(&self, index: usize) -> i64 {
        return self.data[index];
    }
}
```

Methods borrow `self`, so the lifetimes of their arguments follow the ordinary
elision rules described below.

## Returning and passing aggregates with references

A function that returns an aggregate containing a reference must tie that
reference to one of its inputs:

```rust
fn make<'a>(values: &'a [i64]) -> View<'a> {
    return View { data: values };
}

fn first<'a>(values: &'a [i64]) -> &'a i64 {
    return &values[0];
}
```

When an aggregate containing a reference is returned, the borrow is attributed
to the receiver or to the single elided input, in the same conservative spirit
as returning a plain reference. Returning an aggregate that borrows a local
value is rejected, just like returning a reference to a local.

## Elision rules

Lifetimes are usually elided. In function and method signatures an elided
`&T` is a fresh input lifetime, and a single input lifetime is used for the
output, exactly as before 2.2; `fn first(values: &[i64]) -> &i64` is
therefore the elided form of the signature above. Type paths may omit
lifetime arguments, so `View` written in a position where the lifetime is
already known means the same as `View<'_>`. There is no inference for
lifetime parameters that appear only in a return type: two elided input
references with one borrowed return still produce
`error[E0106]: missing lifetime specifier`.

`'static` is **not** a supported field lifetime in 2.2.0. Every reference
field must name a lifetime parameter that the type declares; a field such as
`text: &'static str` is rejected with
`error[E0498]: the 'static lifetime is not supported yet` and the note *use a
named lifetime parameter declared by the type*. Write
`struct Name<'a> { text: &'a str }` instead. An explicit `'static` is accepted
in a function signature, but a named parameter is the supported form there as
well.

## Where references may be stored

References may be stored in these positions:

* fields of user-defined structs, including tuple structs;
* fields of user-defined enums, including variant payloads;
* fields whose type is `Option<&'a T>` or `Option<&'a mut T>`.

References are still rejected everywhere else, with
`error[E0498]: references cannot be stored in this type`: type arguments
such as `Vec<&'a T>`, `Result<&'a T, E>`, `HashMap<&'a K, V>`, `Rc<&'a T>`,
`Box<&'a T>`, array element types such as `[&'a T; N]`, and values captured
by closures. A reference in an aggregate still cannot be widened into one of
those containers.

## How much checking is done

The borrow checker tracks an aggregate containing references like any other
value: the borrow lives as long as the aggregate, copying or moving the
aggregate carries the borrow, and dropping or overwriting the aggregate ends
it. The conservative checks that reject returning a reference to a local
`error[E0515]` and assigning a temporary borrow `error[E0597]` are
extended to aggregates. Beyond that, checking is intentionally simple: named
lifetimes are not compared for equality or subtyping, there are no outlives
bounds, and variance is not checked, so some programs Rust accepts are rejected
and lifetime mistakes are not all caught at compile time.

## Known limitations in 2.2.0

* Lifetime support is limited to reference fields in user structs, tuple
  structs, enums and `Option`. References in `Vec`, `Result`, `HashMap`,
  `Rc`, `Box`, arrays and closures remain rejected with `E0498`.
* There are no lifetime bounds (`'a: 'b`), no outlives requirements, no
  higher-ranked lifetimes, no variance checks and no named-lifetime
  equivalence checks.
* Lifetime inference is limited to the elision rules above. Reference fields
  must name a lifetime parameter; elided reference fields are rejected, and a
  `'static` reference field is rejected with `E0498` ("the 'static lifetime is
  not supported yet").
* Lifetime information is erased after type checking: lifetimes are not part
  of the generated C types and cannot be inspected at run time.
* References into a local value still cannot be returned. Code that would
  need full Rust lifetime analysis must restructure the borrow instead.

## Summary

* A struct, enum, tuple struct or `Option` field may hold `&'a T` or
  `&'a mut T`; the item must declare `<'a>` and the field must name it.
* `impl<'a> View<'a>` blocks and functions returning such aggregates use the
  same lifetime parameters.
* Elision covers function and method signatures and struct paths; `'static` is
  not accepted as a field lifetime, so name a declared parameter.
* References remain unstoreable in `Vec`, `Result`, `HashMap`, `Rc`, `Box`,
  arrays and closures (`E0498`).
* Checking is conservative and lifetimes are erased before code generation;
  `E0106`, `E0261`, `E0498`, `E0515` and `E0597` are the relevant diagnostics.

## What next

* [References and borrowing](12-references-and-borrowing.md) covers the rules
  for `&` and `&mut` outside aggregates.
* [Structs, enums and impl](10-structs-enums-and-impl.md) covers the types that
  can now hold references.
* [Arrays](27-arrays.md) covers fixed-size values, which cannot hold references.
* [Trait objects](25-trait-objects.md) covers fat pointers, another form of
  borrowed data.
