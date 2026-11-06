# Modules

Version 1.7.0 adds modules, so a program no longer has to be a single file. A
module is a named scope that holds items: functions, structs, enums, impls,
traits, constants, type aliases, and further modules. The root source file of a
crate is its **root module** (the crate root), and every other module lives
either inline in a `mod … { … }` block or in its own file. Modules exist only at
compile time: the compiler resolves every path and flattens the whole crate graph
into one program before generating C, so modules cost nothing at run time (see
[How modules are compiled](#how-modules-are-compiled)).

A crate is the unit of compilation; a package is one or more crates with
dependencies. See [Packages](24-packages.md) for the package layer.

## Inline modules

An inline module is a `mod` declaration followed by a body of items:

```ls
mod geometry {
    pub fn area(width: f64, height: f64) -> f64 {
        width * height
    }

    pub mod solid {
        pub fn volume(width: f64, height: f64, depth: f64) -> f64 {
            width * height * depth
        }
    }
}

fn main() {
    println!("{}", geometry::area(2.0, 3.0));
    println!("{}", geometry::solid::volume(2.0, 3.0, 4.0));
}
```

Output:

```text
6
24
```

Items in a module are reached through a path: `geometry::area`, and
`geometry::solid::volume` for the nested module. Inside `geometry` you would write
`solid::volume` to reach the nested item; `main` is outside both modules and
therefore names them from the crate root.

A module is private by default, together with everything in it. The `pub` keyword
makes an item visible to the modules and crates that can see the containing
module; visibility is described under [Visibility](#visibility).

## External modules and files

A module can also be declared without a body: `mod math;` (a semicolon instead of
braces). That is an **external module**: the compiler looks for its file on disk.
The search follows the same rule as Rust:

| The parent module's directory is `D` | The compiler looks for |
| --- | --- |
| | `D/math.ls` first, then `D/math/mod.ls` |

If exactly one of the two files exists it is loaded. If both exist the
declaration is an error (`E0761`: the file is found at both candidates), and if
neither exists the declaration is an error (`E0583`: file not found for module,
with a note naming the two expected paths).

The root source file of a crate is the crate root module, and its directory is
the directory of the root module. For a program run as `lsaca run main.ls` or
`lsaca run src/main.ls`, an external `mod math;` in the root file therefore looks
for `math.ls` or `math/mod.ls` next to the root file. Every loaded module file is
itself a module and may declare further external modules; the rule is applied
recursively:

```ls,tree
--- main.ls
mod math;

fn main() {
    println!("{}", math::tables::triple(14));
    println!("{}", math::add(40, 2));
}
--- math.ls
pub mod tables;

pub const OFFSET: i64 = 0;

pub fn add(a: i64, b: i64) -> i64 {
    a + b + OFFSET
}
--- math/tables.ls
pub fn triple(value: i64) -> i64 {
    value * 3
}
```

Output:

```text
42
42
```

Here `main.ls` declares `mod math;`, so `math.ls` is loaded; that file declares
`pub mod tables;`, and because `math` is a file-style module its module directory
is `math/`, so `math/tables.ls` is loaded. A module declared inside an inline
module follows the inline rule: the module directory is the parent directory plus
the module name.

The child declaration is `pub mod tables;`, not `mod tables;`: the root module
names `math::tables::triple`, and a path that reaches into a module requires the
module to be visible from the naming module. A private `mod tables;` would make
that path `E0603` (“module `tables` is private”) while `math::add` still works.

The next example mixes both forms: `greet/mod.ls` is found through the
`name/mod.ls` candidate, and its own directory is `greet/`; the inline module
`inner` then contributes the directory `greet/inner/`.

```ls,tree
--- main.ls
mod greet;

fn main() {
    println!("{}", greet::message());
    println!("{}", greet::detail::count());
    println!("{}", greet::inner::extra::suffix());
}
--- greet/mod.ls
pub mod detail;

pub mod inner {
    pub mod extra;
}

pub fn message() -> String {
    format!("hello {}", detail::count())
}
--- greet/detail.ls
pub fn count() -> i64 {
    2
}
--- greet/inner/extra.ls
pub fn suffix() -> String {
    String::from("!")
}
```

Output:

```text
hello 2
2
!
```

The rules in one place:

* the directory of the crate root module is the directory of the root file;
* a module loaded from `D/name.ls` has module directory `D/name/`;
* a module loaded from `D/name/mod.ls` has module directory `D/name/`;
* an inline module inside a module with directory `D` has module directory
  `D/name/`.

Only `.ls` files are read: there is no directory scanning, no `include` and no
manifest entry that lists module files. Converting a module to a directory
module is therefore a matter of moving its file: `math.ls` and `math/mod.ls` are
interchangeable as far as path resolution is concerned.

Module loading happens before type checking, and its errors are reported on the
`mod` declaration. Besides `E0583` and `E0761`, loading reports:

* `E0001` for circular module files, with the chain of files in the message;
* `E0583` when a module file cannot be read or is not valid UTF-8;
* `E0428` when the same module name is declared twice in one parent module;
* `E0001` when module files nest deeper than 256 levels.

A module file whose own items fail to compile produces ordinary type errors with
positions in that file; see [Diagnostics across files](#diagnostics-across-files).

## Paths

A path is a sequence of names separated by `::`. In expressions, types, patterns
and `use` declarations the first segment is resolved in this order:

| First segment | Meaning |
| --- | --- |
| `crate` | The crate root module of the current crate |
| `self` | The current module |
| `super` | The parent module (`super::super` and so on are allowed; `super` at the crate root is an error) |
| an item of the current module | A child module or any other item declared in the current module |
| a single-segment import alias | A name introduced by `use … as name;` in the current module |
| a dependency package name | The crate root module of that dependency crate; only its `pub` items are reachable |
| a prelude name | A builtin type, function or macro |

The remaining segments are looked up inside the module named by the previous
segment, so `crate::math::add` means “the item `add` in module `math` of the
current crate”. Modules do not inherit the names of their parents: a function in
the parent module is not visible by bare name inside a child module.

```ls
mod shapes {
    pub struct Rectangle {
        pub width: f64,
        pub height: f64,
    }

    pub fn area(rectangle: &Rectangle) -> f64 {
        rectangle.width * rectangle.height
    }
}

fn main() {
    let rectangle = shapes::Rectangle { width: 2.0, height: 3.0 };
    println!("{}", shapes::area(&rectangle));
}
```

Output:

```text
6
```

The special segments work from any module:

```ls
mod a {
    pub fn one() -> i64 {
        1
    }
}

mod b {
    pub fn two() -> i64 {
        super::a::one() + crate::a::one() + self::three()
    }

    fn three() -> i64 {
        3
    }
}

fn main() {
    println!("{}", b::two());
}
```

Output:

```text
5
```

A name that resolves to no item is an error: an unresolved function or variable
is `E0425` (“cannot find function … in this scope” or “cannot find value … in
this scope”), an unresolved type is `E0412`, and a path whose first module cannot
be resolved is `E0433`. The following program calls a function of the parent
module without a path, which is a plain unresolved name:

```ls,compile-fail
mod outer {
    pub fn helper() -> i64 {
        1
    }

    pub mod inner {
        pub fn call() -> i64 {
            helper()
        }
    }
}

fn main() {
    println!("{}", outer::inner::call());
}
```

Error:

```text
error[E0425]: cannot find function helper in this scope
  --> example-06.ls:8:13
   |
 8 |             helper()
   |             ^^^^^^^^

lsaca: example-06.ls: could not compile example-06.ls due to previous error(s)
```

An unresolved module path is `E0433` (“failed to resolve module …”), while a
single unresolved name is `E0425` for values and `E0412` for types. The
[diagnostics table](#diagnostics-across-files) lists every code this chapter
introduces.

## Imports with use

`use` binds a name from another module into the current module. A declaration is
the `use` keyword, a use tree and a semicolon. The first segment of a use path is
resolved like any other path (see [Paths](#paths)), so it can name the crate root
(`crate`), this module (`self`), the parent (`super`), an item of the current
module, an existing import alias, or a dependency package. A bare
`use math::add;` is not accepted, because `math` would be a child module of the
current module rather than a root of the crate. The leaf of the tree binds one
name, either the last segment or an explicit alias:

```ls,tree
--- main.ls
mod math;

use crate::math::{add, mul as times};

fn main() {
    println!("{}", add(2, 3));
    println!("{}", times(4, 5));
}
--- math.ls
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

pub fn mul(a: i64, b: i64) -> i64 {
    a * b
}
```

Output:

```text
5
20
```

The full set of trees is:

```ebnf
useTree    = useSegment ( "::" useTree )? | useSegment "as" IDENT | "{" useTree ( "," useTree )* ","? "}" | "*" ;
useSegment = IDENT | "crate" | "self" | "super" ;
```

Groups nest and accept a trailing comma. A glob (`use crate::math::*;`) parses
but the resolver rejects it; version 2.1.0 has no glob imports. An import is
local to its module: importing a name in the root module does not make it
visible in a child module.

`pub use` re-exports the imported name with the visibility of the `use`
declaration, which is how a module can present a shorter public path to an item
that lives deeper in the tree:

```ls,tree
--- main.ls
mod shapes;

use crate::shapes::area;

fn main() {
    println!("{}", area(2.0, 3.0));
}
--- shapes.ls
mod rect;

pub use crate::shapes::rect::area;
--- shapes/rect.ls
pub fn area(width: f64, height: f64) -> f64 {
    width * height
}
```

Output:

```text
6
```

A `use` declaration may only narrow visibility, never widen it. Re-exporting a
private item from a publicly reachable module is `E0364`; re-exporting a private
module from the same position is `E0365`. When the module that contains the
`pub use` is itself private, the re-export is only as visible as that module,
which is exactly the facade pattern above: it is reachable through the crate,
but not from outside it.

```ls,compile-fail
mod inner {
    fn hidden() -> i64 {
        1
    }
}

pub use crate::inner::hidden;

fn main() {
    println!("{}", hidden());
}
```

Error:

```text
error[E0364]: function 'hidden' is private, and cannot be re-exported
  --> example-09.ls:7:23
   |
 7 | pub use crate::inner::hidden;
   |                       ^^^^^^

error[E0425]: cannot find function hidden in this scope
   --> example-09.ls:10:20
    |
 10 |     println!("{}", hidden());
    |                    ^^^^^^^^

lsaca: example-09.ls: could not compile example-09.ls due to previous error(s)
```

When the re-export really is wider than the module it names, the compiler
rejects it with `E0365`:

```ls,tree,compile-fail
--- main.ls
pub mod outer;

use outer::exposed;

fn main() {
    println!("{}", exposed::answer());
}
--- outer.ls
mod inner;

pub use inner as exposed;
--- outer/inner.ls
pub fn answer() -> i64 {
    7
}
```

Error:

```text
error[E0432]: unresolved import 'outer::exposed'
  --> main.ls:3:12
   |
 3 | use outer::exposed;
   |            ^^^^^^^

error[E0412]: cannot find type exposed in this scope
  --> main.ls:6:20
   |
 6 |     println!("{}", exposed::answer());
   |                    ^^^^^^^^^^^^^^^^^

error[E0365]: module 'inner' is private, and cannot be re-exported
  --> outer.ls:3:9
   |
 3 | pub use inner as exposed;
   |         ^^^^^

lsaca: main.ls: could not compile main.ls due to previous error(s)
```

### Import conflicts

Two imports binding the same name in one module are an error (`E0252`), and an
import that collides with an item declared in the same module is also an error
(`E0255`). Both diagnostics add a note pointing at the first definition.

```ls,tree,compile-fail
--- main.ls
mod math;

use crate::math::add;
use crate::math::add;

fn main() {
    println!("{}", add(2, 3));
}
--- math.ls
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}
```

Error:

```text
error[E0252]: the name 'add' is imported multiple times
  --> main.ls:4:18
   |
 4 | use crate::math::add;
   |                  ^^^
   = note: main.ls:3:18: first import of 'add' here

lsaca: main.ls: could not compile main.ls due to previous error(s)
```

```ls,compile-fail
mod math {
    pub fn add(a: i64, b: i64) -> i64 {
        a + b
    }
}

use crate::math::add;

fn add(a: i64, b: i64) -> i64 {
    a - b
}

fn main() {
    println!("{}", add(4, 2));
}
```

Error:

```text
error[E0255]: the name 'add' is defined multiple times
  --> example-12.ls:7:18
   |
 7 | use crate::math::add;
   |                  ^^^
   = note: example-12.ls:9:4: previous definition of 'add' here

lsaca: example-12.ls: could not compile example-12.ls due to previous error(s)
```

A glob import is rejected by the resolver:

```ls,tree,compile-fail
--- main.ls
mod math;

use crate::math::*;

fn main() {
    println!("{}", add(2, 3));
}
--- math.ls
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}
```

Error:

```text
error[E0432]: glob imports are not supported
  --> main.ls:3:18
   |
 3 | use crate::math::*;
   |                  ^

error[E0425]: cannot find function add in this scope
  --> main.ls:6:20
   |
 6 |     println!("{}", add(2, 3));
   |                    ^^^^^^^^^

lsaca: main.ls: could not compile main.ls due to previous error(s)
```

## Visibility

Every named item can carry a visibility. Version 2.1.0 has three levels:

| Written | Visible from |
| --- | --- |
| nothing (private) | The defining module and its descendants |
| `pub(crate)` | Every module of the same crate, never another crate |
| `pub` | Everywhere, including dependency crates |

The visibility of a path's first segment matters: resolving `a::b` requires `a`
to be visible from the module doing the lookup, and then `b` must be visible from
the same place. Private items are still reachable downwards: a module can use
the private items of its own descendants.

`pub` and `pub(crate)` are accepted before any item: `fn`, `struct`, `enum`,
`trait`, `const`, `type`, `mod` and the methods and associated functions inside
`impl` blocks. Other restricted forms are not implemented: `pub(super)` and
`pub(in path)` are rejected by the parser. Struct fields have their own
visibility and default to private, so an item can be visible without its fields
being visible; an enum variant follows the visibility of its enum, and the fields
of tuple variants are as public as the enum.

A private field cannot be read from another module:

```ls,tree,compile-fail
--- main.ls
mod counter;

use crate::counter::make;

fn main() {
    let counter = make();
    println!("{}", counter.value);
}
--- counter.ls
pub struct Counter {
    value: i64,
}

pub fn make() -> Counter {
    Counter { value: 1 }
}
```

Error:

```text
error[E0616]: field value of struct Counter is private
  --> main.ls:7:28
   |
 7 |     println!("{}", counter.value);
   |                            ^^^^^

lsaca: main.ls: could not compile main.ls due to previous error(s)
```

A struct literal that names a private field from another module is a different
error:

```ls,tree,compile-fail
--- main.ls
mod counter;

use crate::counter::Counter;

fn main() {
    let counter = Counter { value: 1 };
    println!("{}", counter.value());
}
--- counter.ls
pub struct Counter {
    value: i64,
}

impl Counter {
    pub fn value(&self) -> i64 {
        self.value
    }
}
```

Error:

```text
error[E0451]: field value of struct Counter is private
  --> main.ls:6:29
   |
 6 |     let counter = Counter { value: 1 };
   |                             ^^^^^

lsaca: main.ls: could not compile main.ls due to previous error(s)
```

A private module cannot be named from outside its parent:

```ls,tree,compile-fail
--- main.ls
mod outer;

use crate::outer::inner;

fn main() {
    println!("{}", inner::answer());
}
--- outer.ls
mod inner;
--- outer/inner.ls
pub fn answer() -> i64 {
    42
}
```

Error:

```text
error[E0603]: module 'inner' is private
  --> main.ls:3:19
   |
 3 | use crate::outer::inner;
   |                   ^^^^^

error[E0412]: cannot find type inner in this scope
  --> main.ls:6:20
   |
 6 |     println!("{}", inner::answer());
   |                    ^^^^^^^^^^^^^^^

lsaca: main.ls: could not compile main.ls due to previous error(s)
```

`pub(crate)` makes an item visible across modules of the same crate but not to
dependencies:

```ls
pub mod outer {
    pub(crate) fn shared() -> i64 {
        7
    }
}

mod user {
    pub fn call() -> i64 {
        crate::outer::shared()
    }
}

fn main() {
    println!("{}", user::call());
}
```

Output:

```text
7
```

Across crates the difference between `pub` and `pub(crate)` is visible. This
package has a dependency `util` whose `internal` module is `pub(crate)`, so the
root package cannot name it:

```ls,tree,package=app,compile-fail
--- app/lsaca.toml
[package]
name = "app"
version = "0.1.0"

[dependencies]
util = { path = "../util" }
--- app/src/main.ls
use util::internal;

fn main() {
    println!("{}", internal::value());
}
--- util/lsaca.toml
[package]
name = "util"
version = "0.1.0"
--- util/src/lib.ls
pub mod api;
pub(crate) mod internal;
--- util/src/api.ls
pub fn value() -> i64 {
    1
}
--- util/src/internal.ls
pub fn value() -> i64 {
    2
}
```

Error:

```text
error[E0603]: module 'internal' is private
  --> src/main.ls:1:11
   |
 1 | use util::internal;
   |           ^^^^^^^^

error[E0412]: cannot find type internal in this scope
  --> src/main.ls:4:20
   |
 4 |     println!("{}", internal::value());
   |                    ^^^^^^^^^^^^^^^^^

lsaca: package 'app': could not compile package 'app' due to previous error(s)
```

## Diagnostics across files

Modules change where diagnostics point, not how they are rendered. Every span in
the compiler is an offset into one of the source files, and the renderer resolves
it to that file, so the `-->` line names the file that contains the error and the
source excerpt comes from there. A note can point into a different file (a
previous definition, the first import, the first declaration of a module), and
the summary line at the end always names the root file of the compilation:

```ls,tree,compile-fail
--- main.ls
mod a;

fn main() {
    println!("{}", a::call());
}
--- a.ls
mod missing;

pub fn call() -> i64 {
    missing::value()
}
```

Error:

```text
error[E0583]: file not found for module `missing`
  --> a.ls:1:5
   |
 1 | mod missing;
   |     ^^^^^^^
   = note: expected one of: a/missing.ls, a/missing/mod.ls

lsaca: main.ls: could not compile main.ls due to previous error(s)
```

The codes introduced by this chapter:

| Code | Meaning |
| --- | --- |
| `E0583` | A module file was not found, could not be read, or is not valid UTF-8 |
| `E0761` | Both `name.ls` and `name/mod.ls` exist for one module |
| `E0428` | The same name is defined multiple times in one module or crate |
| `E0001` | General misuse: circular module files, nesting beyond 256 levels, rejected visibility forms, package errors |
| `E0432` | An unresolved import path, including glob imports |
| `E0433` | A multi-segment path whose first module cannot be resolved, including `super` at the crate root |
| `E0425` | An unresolved value or function name |
| `E0412` | An unresolved type name |
| `E0252` | A name imported more than once in one module |
| `E0255` | An import collides with an item defined in the same module |
| `E0603` | A module or item is private at the point of use |
| `E0624` | A method or associated function is private at the point of use |
| `E0616` | A struct field is private at the point of use |
| `E0451` | A struct literal or pattern names a private field from outside its module |
| `E0364` | A `pub use` re-exports a private item |
| `E0365` | A `pub use` re-exports a private module |

## How modules are compiled

Module boundaries do not survive compilation. The loader expands every external
`mod` into an inline module, the type checker resolves the whole graph and flattens
it into one monomorphic program, and the code generator emits one C program
containing every function, type and instantiation from every crate. The
flattening order is deterministic: a module's own items in declaration order,
with each child module processed where it is declared.

Symbols from different modules cannot collide in C, so the names are
module-qualified:

* the root module of the root crate keeps its pre-1.7 names, which keeps the
  generated C of single-file programs unchanged: `fn main` is still
  `lsaca_fn_main` and a struct `Rect` is still `Rect`;
* a nested module chain is joined with underscores and placed in front of
  `lsaca_fn_`: a function `area` in module `geometry` becomes
  `geometry_lsaca_fn_area`, and `geometry::solid::volume` becomes
  `geometry_solid_lsaca_fn_volume`;
* a dependency crate always contributes its crate name, for example
  `pub fn triple` in the `util` package becomes `util_lsaca_fn_triple`;
* a type declared in a nested module is prefixed with `Ls` and the module chain:
  `Rect` in module `shapes` becomes `Lsshapes_Rect`, while types of the root
  module keep their plain names;
* methods carry the module chain and the type name: `Rect::area` in module
  `shapes` becomes `shapes_lsaca_fn_Rect_area`;
* constants are folded into their use sites, so they have no C symbol of their
  own.

The names are sanitized to `[A-Za-z0-9_]` and never depend on hash-map order, so
the generated C is deterministic for a given source tree. There is no separate
compilation: the whole crate graph is checked and emitted as one program on every
build. The build cache compensates: its key hashes the complete generated C, the
runtime sources, the compiler path and the build mode, so an unchanged program is
reused as a whole and editing any file in the graph rebuilds it.

## Known limitations in 2.1.0

* Glob imports (`use path::*;`) parse but are rejected by the resolver.
* Only private, `pub(crate)` and `pub` exist: `pub(super)` and `pub(in path)` are
  rejected by the parser.
* A module file is found only by the `name.ls`/`name/mod.ls` rule: there are no
  path attributes, no conditional compilation and no directory scanning.
* Circular module files are rejected; the tree of `mod` declarations must be
  acyclic.
* Module files may nest at most 256 levels deep, and each file must be valid
  UTF-8.
* Imports are per module: there are no implicit imports of a parent module, and
  `pub use` re-exports are the only way to make one module's names reachable
  under another module's path.
* There is no separate compilation or per-module incremental artifact: building
  any change re-checks and re-emits the whole crate graph.
* A `fn main` is the entry point only in the root module of the root crate of a
  binary package; `main` in a nested module is an ordinary function.

## Summary

* A crate starts at a root file; `mod name { … }` is an inline module and
  `mod name;` loads `name.ls` or `name/mod.ls` from the parent module's directory.
* Paths start with `crate`, `self`, `super`, an item of the current module, an
  import alias, a dependency package name, or a prelude name.
* `use` imports one name per leaf, with groups, aliases and `pub use` re-exports;
  glob imports are rejected.
* Visibility is private, `pub(crate)` or `pub`; fields have their own visibility.
* Diagnostics render with real file names and positions across the whole crate
  graph.
* Modules are erased before code generation: one program, one C project, no run
  time cost.

## What next

* Packages, manifests and path dependencies: [Packages](24-packages.md).
* The structure of a crate and its item kinds: [Program structure](02-program-structure.md).
* The memory model with ownership and ORC: [Memory model](05-memory-model.md).
* What is planned next: [Roadmap](16-roadmap.md).
