# Packages

Version 1.7.0 adds a small package manager on top of the module system from
the [Modules](23-modules.md) chapter. A **package** is a directory with a
manifest called `lsaca.toml` and a `src/` directory. Packages can depend on
other packages by path, and the compiler builds the whole dependency graph
into one native program.

This chapter covers the package layout, the manifest format, path
dependencies, the `lsaca new` command, and the package mode of the existing
commands. Modules themselves are described in [Modules](23-modules.md).

## What a package is

A package is a directory containing:

- `lsaca.toml` — the manifest, at the package root;
- `src/main.ls` — the root module of a **binary package**, or
- `src/lib.ls` — the root module of a **library package**.

Module files live under `src/` as described in
[Modules](23-modules.md#external-modules-and-files), so a package like this

```text
demo/
├── lsaca.toml
└── src/
    ├── main.ls
    └── math.ls
```

is one crate whose root module is `src/main.ls`. The module `math` declared
with `mod math;` inside `src/main.ls` is loaded from `src/math.ls` or
`src/math/mod.ls`, exactly as described in the module chapter.

Each package is one crate. `crate::` inside a package refers to that
package's own root module, never to a dependency.

- A **binary package** has `src/main.ls` and can be run with `lsaca run`.
- A **library package** has `src/lib.ls` and is meant to be used as a
  dependency. Only `lsaca check` works on a library package; `run`, `build`
  and `emit-c` need a binary package.

A `src/main.ls` must declare a `fn main` to be a runnable binary. If it does
not, `check` fails with `E0601` ("main function not found") pointing at the
manifest, and `run`, `build` and `emit-c` report that the package has a
`src/main.ls` but no `fn main()` and suggest adding one or moving the crate
root to `src/lib.ls`. A library package (`src/lib.ls`) has no entry point and
is checked normally.

A package has exactly one root module. A package that contains both
`src/main.ls` and `src/lib.ls` is rejected:

```text
lsaca: error[E0001]: package 'both' has both src/main.ls and src/lib.ls
  --> lsaca.toml:1:1
   |
 1 | [package]
   | ^
   = note: a package must be a binary (src/main.ls) or a library (src/lib.ls), not both
```

## The manifest

`lsaca.toml` uses a strict, line-oriented subset of TOML. This is the
manifest of a package that depends on a sibling directory:

```text
# demo/lsaca.toml
[package]
name = "demo"
version = "0.1.0"

[dependencies]
util = { path = "../util" }
```

The supported syntax is:

- one `key = value` pair per line;
- `#` starts a comment, either at the beginning of a line or after a value;
- blank lines are ignored (a lone `\r` from Windows line endings is fine);
- only two tables: `[package]` and `[dependencies]`;
- string values are double-quoted and support the escapes `\"`, `\\`,
  `\n` and `\t`.

### The [package] table

| Key | Required | Meaning |
| --- | --- | --- |
| `name` | yes | Package name; also the crate name used in `use` paths. |
| `version` | no (default `"0.1.0"`) | Metadata only; see below. |

A package name must be a valid lsaca identifier written in lower case: it
starts with a lowercase ASCII letter or `_`, and contains only lowercase
ASCII letters, digits and `_`. Keywords cannot be used as package names.

A version must be three dot-separated integers, for example `1.0.0` or
`0.1.12`. The version is **metadata**: dependency resolution is purely by
path, so changing a version never changes which code is compiled.

### The [dependencies] table

Every key in `[dependencies]` is a dependency name, and the value must be
exactly the inline table `{ path = "..." }`:

- the key follows the same identifier rules as a package name and cannot be a
  keyword;
- `path` is a filesystem path to the dependency's directory, resolved
  relative to the directory that contains the manifest;
- the dependency directory must exist, must contain its own `lsaca.toml`,
  and must be a library package (it must contain `src/lib.ls`).

The dependency **key** and the dependency's own package `name` are separate
things at the moment: the crate you import is always named by the
dependency's manifest `name`, not by the key in your `[dependencies]`
table. Keep the two the same to avoid confusion.

Only path dependencies are supported. There is no registry, no version
selection, no git dependency, no lockfile and no workspace file; the
dependency graph is recomputed from the manifests on disk every time.

### Manifest diagnostics

Every manifest and resolution problem is reported as `error[E0001]` with a
span pointing into the manifest at the offending line and column. A few
examples of the messages:

| Problem | Message |
| --- | --- |
| `[package]` table missing | `missing \`[package]\` table in the manifest` |
| `name` key missing | `missing \`name\` in the \`[package]\` table` |
| Bad version | `invalid version \`1.0\`: expected three dot-separated integers such as \`0.1.0\`` |
| Unknown section | `unknown section \`[build]\`; expected \`[package]\` or \`[dependencies]\`` |
| Table declared twice | `the table \`[package]\` is declared more than once` |
| Unknown key | `unknown key \`authors\` in \`[package]\`; expected \`name\` or \`version\`` |
| Single-quoted string | `single-quoted strings are not supported; use double quotes` |
| Malformed dependency | `malformed dependency: expected \`path\` in the inline table` |
| Dependency path missing | `dependency \`util\`: path \`../util\` does not exist` |
| No manifest in dependency | `dependency \`util\`: \`../util\` does not contain a \`lsaca.toml\` manifest` |
| Dependency is not a library | `dependency \`util\` is not a library: \`../util\` does not contain \`src/lib.ls\`` |
| Self dependency | `package \`demo\` cannot depend on itself` |
| Cycle | `cyclic package dependency: a -> b -> a` |
| One name, two paths | `the package name \`util\` is provided by two different paths: ...` |

The last two groups are resolution errors: they are reported before any
`.ls` file is read.

For example, this manifest names a section that does not exist:

```ls,tree,package=broken,compile-fail
--- broken/lsaca.toml
[package]
name = "broken"

[build]
opt-level = "3"
--- broken/src/main.ls
fn main() {
    println!("never checked");
}
```

Error:

```text
lsaca: error[E0001]: unknown section `[build]`; expected `[package]` or `[dependencies]`
  --> lsaca.toml:4:2
   |
 4 | [build]
   |  ^^^^^
```

## Dependency graph

Starting from the root package's manifest, the compiler resolves every path
dependency, and every dependency's own dependencies, into a graph:

- the graph is visited depth first, dependencies before the packages that
  need them, so it has a deterministic order and the root package comes last;
- the same directory reached through two different paths is loaded once (a
  diamond dependency is not an error);
- two different directories that declare the same package name are an error;
- cycles are an error, and the message shows the cycle chain;
- a package cannot depend on itself.

Dependencies are resolved before sources are loaded. A missing dependency
directory, a dependency without a manifest or a dependency that is not a
library is reported as a manifest error, not as a compile error in the
dependent package.

## Using a dependency

Every package in the graph can be named from every module of every crate, as
if it were a module at the crate root. The name is the dependency's package
`name`:

```ls,tree,package=app
--- app/lsaca.toml
[package]
name = "app"
version = "1.0.0"

[dependencies]
util = { path = "../util" }
--- app/src/main.ls
use util::text::excited;
use util::triple;

fn main() {
    println!("triple: {}", triple(14));
    let word = "done";
    println!("{}", excited(word));
}
--- util/lsaca.toml
[package]
name = "util"

[dependencies]
--- util/src/lib.ls
pub mod text;

pub fn triple(value: i64) -> i64 {
    value * 3
}
--- util/src/text.ls
pub fn excited(text: String) -> String {
    format!("{}!", text)
}
```

Output:

```text
triple: 42
done!
```

The root package can import anything public from any package in the graph, not
just its direct dependencies. A dependency cannot import the root package:
dependencies only ever see their own dependencies.

The visibility rules from [Modules](23-modules.md#visibility) apply across
crates too: a `pub` item in a dependency is visible, `pub(crate)` is not, and
a private item is only visible inside its own crate. A library package
typically re-exports what it wants to offer with `pub use`; see
[Modules](23-modules.md#imports-with-use).

## Creating a package

`lsaca new <DIR>` creates a binary package in a new directory:

```console
$ lsaca new demo
lsaca: created binary package 'demo' in demo
lsaca: next: cd demo && lsaca run
```

The directory `demo` then contains:

```text
demo/
├── lsaca.toml
└── src/
    └── main.ls
```

with this manifest and this program:

```text
[package]
name = "demo"
version = "0.1.0"
```

```text
fn main() {
    println!("Hello, world!");
}
```

Rules for `lsaca new`:

- the package name is derived from the final path component: it is lowercased,
  any character that is not an ASCII letter, digit or `_` becomes `_`, a
  leading digit gets a `_` prefix, and a keyword gets a `_` suffix — so
  `lsaca new My-Pkg` creates the package `my_pkg`;
- parent directories are created as needed;
- `lsaca new` refuses an existing non-empty directory, and refuses a path that
  exists as a file;
- it takes exactly one directory argument and has no other options.

## Package mode for run, build, check and emit-c

When no `FILE.ls` argument is given, the four main commands operate on a
package. The compiler walks up from the current directory to the nearest
`lsaca.toml`, so they work from any subdirectory of a package:

```ls,tree,package=app,check
--- app/lsaca.toml
[package]
name = "app"
version = "1.0.0"

[dependencies]
util = { path = "../util" }
--- app/src/main.ls
use util::text::excited;
use util::triple;

fn main() {
    println!("triple: {}", triple(14));
    let word = "done";
    println!("{}", excited(word));
}
--- util/lsaca.toml
[package]
name = "util"

[dependencies]
--- util/src/lib.ls
pub mod text;

pub fn triple(value: i64) -> i64 {
    value * 3
}
--- util/src/text.ls
pub fn excited(text: String) -> String {
    format!("{}!", text)
}
```

Output:

```text
lsaca: package 'app': ok
```

`lsaca check` type-checks the package and its dependencies together and
prints a single confirmation line.

A package with no manifest anywhere up the directory tree is a usage error:

```console
$ lsaca check
lsaca: no lsaca.toml found in this directory or any parent directory
lsaca: run 'lsaca help' for usage
```

When a `FILE.ls` argument **is** given, the commands keep their single-file
behaviour and the manifest is not consulted.

| Command | Binary package | Library package |
| --- | --- | --- |
| `lsaca run` | builds and runs the program | error: needs `src/main.ls` |
| `lsaca build` | builds a native executable | error: needs `src/main.ls` |
| `lsaca check` | type-checks the graph | type-checks the graph |
| `lsaca emit-c` | writes the C project for the graph | error: needs `src/main.ls` (only `lsaca check` works) |

The error names `src/main.ls` when the file is missing. When `src/main.ls`
exists but declares no entry point, `run`, `build` and `emit-c` say so
directly, and `check` reports `E0601` at the manifest.

The options `-r`/`--release`, `-o`/`--out-dir DIR`, `-v`/`--verbose`,
`--keep` and `--cmake` keep their meaning in package mode. `lsaca run`
forwards arguments after `--` to the program. Exit codes are unchanged:
0 success, 1 compile or build error, 2 usage error, 101 a panic inside the
compiled program.

`lsaca build` names the executable after the package (`demo.exe` on Windows,
`demo` elsewhere) and puts it in the output directory, `-o` or the current
directory. `lsaca emit-c` defaults to a directory named after the package with
a `_c` suffix (`demo_c`), containing one C project for the entire dependency
graph.

The build cache is shared with single-file builds. Because the whole graph is
re-checked as one program, there is no separate per-package artifact: changing
any file in the graph re-checks the graph and rebuilds the executable.

## How a package is compiled

All packages in the graph are parsed, checked and lowered into one
monomorphized program: modules do not exist at run time, and neither do
package boundaries. Type and function names are mangled with their crate and
module qualifiers, as described in [Modules](23-modules.md#how-modules-are-compiled),
so a function `pub fn triple` in the package `util` is emitted as
`util_lsaca_fn_triple` in the generated C.

The generated C program has one entry point, the `fn main` of the root
package's root module. A library package contributes no entry point.

## Known limitations in 1.7.0

- Only path dependencies; no registry, git dependencies, version selection,
  lockfile or workspaces.
- The manifest is a small subset of TOML: no arrays, numbers, booleans,
  multi-line strings, dotted keys, inline tables other than a dependency, or
  other tables and keys.
- A dependency key does not have to match the dependency's own package name;
  imports always use the package name. Keeping them the same is recommended.
- A package may not contain both `src/main.ls` and `src/lib.ls`: the
  compiler rejects it with `E0001`. Pick one root file.
- No `[lib]` or `[bin]` sections, custom target names or paths, features,
  profiles or build scripts.
- A library-only package cannot be run, built or emitted as C by the CLI; it can
  only be checked, and it can be used as a dependency. A package whose
  `src/main.ls` declares no `fn main` is not a library: `check` reports `E0601`
  and `run`, `build` and `emit-c` report the missing entry point.
- Dependency cycles are rejected, including cycles that go through a
  dev-dependency-like structure.
- There is no separate compilation and no per-crate incremental cache: every
  build checks the entire graph again.
- Package versions are metadata only and never affect resolution.

## Summary

- A package is a directory with `lsaca.toml` and `src/main.ls` (binary) or
  `src/lib.ls` (library); one package is one crate.
- `[package]` holds `name` (required) and `version` (optional metadata);
  `[dependencies]` maps names to `{ path = "..." }` entries.
- Dependencies are resolved by path before any source is read; missing
  manifests, non-libraries, self dependencies, cycles and duplicate names are
  `E0001` errors.
- Every package in the graph is importable by its package name from every
  module of every crate; visibility applies across crates.
- With no `FILE.ls` argument, `run`, `build`, `check` and `emit-c` work on
  the nearest package found walking up from the current directory.
- `lsaca new <DIR>` creates a binary package skeleton.
- The whole graph becomes one monomorphized program and one C project.

## What next

- [Modules](23-modules.md) — modules, paths, `use` and visibility.
- [Getting started](01-getting-started.md#multi-file-programs-and-packages) —
  a quick start for multi-file and package projects.
- [Program structure](02-program-structure.md) — the item kinds and the
  grammar.
- [Roadmap and deviations from Rust](16-roadmap.md) — what comes after 1.7.0.
