# The lsaca language manual

Version 1.0. This document is the authoritative manual for the language: syntax,
semantics, memory model, standard library and the deliberate deviations from
Rust. The compiler is written in Rust and emits C17; this document describes
only what the user of the language sees. See [README.md](../README.md) for
building the compiler, the command line and the compiler architecture.

## Hello, world

    fn main() {
        println!("hello world!");
    }

Compile and run with:

    lsaca run hello.ls

## Program structure

A compilation unit is a single .ls file (no modules, no imports in version 1).
It contains items:

* fn declarations (free functions and associated functions),
* struct declarations,
* enum declarations,
* impl blocks (inherent methods and associated functions),
* const declarations,
* type aliases.

Items may appear at the top level and nested inside blocks; nested items are
hoisted and visible in the whole enclosing body (like Rust's item scoping).
The entry point is a function named main with the signature fn main() (no
parameters, unit return type). The compiler reports an error when main is
missing or has a different signature.

```
const LIMIT: i64 = 10;

struct Point { x: f64, y: f64 }

enum Shape {
    Circle(f64),
    Rect(f64, f64),
    Empty,
}

impl Point {
    fn new(x: f64, y: f64) -> Point {
        Point { x: x, y: y }
    }

    fn len_squared(&self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    fn origin() -> Point {
        Point { x: 0.0, y: 0.0 }
    }
}

type Real = f64;

fn main() {
    let p = Point::new(3.0, 4.0);
    println!("{}", p.len_squared());
}
```

## Lexical structure

* Source is UTF-8. Identifiers are ASCII letters, digits and underscores, and
  may contain non-ASCII letters; an identifier cannot start with a digit.
* Comments: // line comments and /* ... */ block comments that nest.
* Keywords: fn let mut const struct enum impl type if else match while loop
  for in return break continue true false as self Self pub move.
* Whitespace is insignificant apart from separating tokens.

### Integer literals

Decimal (123), hexadecimal (0xFF), octal (0o77), binary (0b1010); underscores
may separate digits (1_000_000). Literals without a type context have type i64
(or another integer type when the context requires it, e.g. a comparison with an
i32 value). There are no literal suffixes; use a type annotation or a cast to
choose a different integer type. A literal that does not fit the inferred type is
a compile-time error.

### Float literals

1.5, 2.0, 1e10, 1.5e-3, 2.0E+4. A literal containing a dot or an exponent is a
float; the default type is f64. There are no suffix forms, no NaN or infinity
literals (use the math functions).

### String literals

Double quoted, UTF-8, with Rust escapes: \n \t \r \\ \" \' \0 \xNN (ASCII
only) \u{HEX}.
Strings are owned, immutable, reference-counted values of type String.

### Boolean and unit literals

true, false, and the unit value () (only usable where a unit-typed expression is
expected, see Types).

## Types

| Type | Description |
| --- | --- |
| () | unit; only function return / discarded value, never stored |
| bool | true or false |
| i8 i16 i32 i64 | signed integers (two's complement) |
| u8 u16 u32 u64 | unsigned integers |
| f32 f64 | IEEE-754 binary floating point |
| String | owned, growable UTF-8 string (reference counted, COW) |
| Vec<T> | growable array (reference counted, COW) |
| Option<T> | Some(value) or None |
| Result<T, E> | Ok(value) or Err(error) |
| Rc<T> | shared, mutable heap cell; handles share one payload (see Rc<T>) |
| (T1, T2, ...) | tuples, including the unit tuple () |
| fn(A, B) -> R | function values and closures |
| struct types | user defined named-field structs |
| enum types | user defined enums, unit and tuple variants |
| type aliases | alternative names for any type |

Inside an impl block the type name Self refers to the implemented struct or
enum type (for example fn new() -> Self); outside an impl block it is an error.

There are no references (&T, &mut T), no raw pointers, no usize, no arrays (use
Vec), no borrow checker. Shared mutable indirection is provided only by the
builtin Rc<T> cell type (see Rc<T>). Lengths and indices are i64. Strings are
always valid UTF-8 and can be indexed only through methods that cut at
character boundaries (see the standard library).

### Type inference

Types are local: they are inferred from how a value is used inside a function,
with unification in the style of Rust. Integer literals start unconstrained and
default to i64; float literals default to f64. A constructor that needs a type
parameter (Vec::new(), None, String::from with an ambiguous result) requires
context or an explicit turbofish:

    let empty: Vec<i64> = Vec::new();
    let also = Vec::<i64>::new();
    let none: Option<String> = None;
    let nothing: Rc<Option<String>> = Rc::new(None);

There are no implicit numeric conversions. Use as:

    let a: i64 = 1;
    let b: i32 = a as i32;      // truncating cast
    let c: f64 = b as f64;
    let d: u8 = 300;            // rejected: 300 does not fit in u8
    let e: u8 = 300 as u8;      // allowed: runtime cast truncates like Rust

## Memory model: ORC

lsaca manages memory with ORC - ownership and reference counting, in the spirit
of Nim's ARC/ORC - not a tracing garbage collector. ORC has two parts:
automatic reference counting (ARC), which reclaims acyclic data
deterministically, and a trial-deletion cycle collector, which reclaims cyclic
garbage. The rules are simple:

1. Owned values (String, Vec, Option/Result with owned payloads, tuples, structs
   and enums that contain owned values, function values and closures) are
   reference counted. Scalars (integers, floats, bools) are plain values. An
   Rc<T> cell is reference counted as well: an Rc<T> value is a handle whose
   copies share one cell (see Rc<T>).
2. Assigning, passing as an argument, returning, capturing (in a closure) and
   storing into a container copies the handle and increments the reference
   count. Copying an Rc<T> handle makes another handle to the same cell rather
   than copying the payload. The language has no compile-time move errors: a
   value can be used after being assigned or passed. This is a deliberate
   deviation from Rust (see Deviations).
3. Function parameters are borrowed: the callee never releases them. The caller
   owns the argument expressions and releases them at the end of the enclosing
   statement. Returning a value transfers ownership to the caller.
4. Locals are released at the end of the block they are declared in, in reverse
   declaration order, including on early exit (return, break, continue).
5. Mutation is copy-on-write: a String or Vec that is mutated while its handle
   is shared clones its buffer first; unshared handles are mutated in place, so
   the common case is O(1). Consequently two variables that were assigned from
   the same Vec never observe each other's mutations. Rc<T> is not
   copy-on-write: it is a shared, mutable cell by design, and a change made
   through one handle is visible through every handle to the same cell.
6. Structs, enums and tuples have value semantics: assigning them copies the
   value (retaining owned fields), mutating a copy does not affect the original.
   An Rc<T> field is copied as a handle, so both copies share the cell.
7. Closures capture by value (handles are retained); the environment is
   reference counted, and capturing an Rc<T> handle shares the cell. move
   closures behave identically in version 1.
8. Reference cycles are possible, but only through Rc<T>. The language still
   has no general references, no raw pointers and no global mutable state, so an
   owned value cannot point back at a value that contains it. Rc<T> is the only
   source of shared aliasing and indirection, so a cycle needs at least one
   Rc<T> handle on the path from an object back to itself (for example a struct
   with a next: Option<Rc<Node>> field). String, Vec, Option/Result and plain
   struct fields are owned values: they can hold Rc<T> handles, but without one
   they can never close a cycle.
9. Cyclic garbage is reclaimed by a trial-deletion cycle collector (the
   Bacon-Rajan mark-gray / scan / collect-white algorithm) that runs over the
   reference-counted objects. It is triggered automatically once enough
   candidate objects accumulate; the threshold is an internal implementation
   detail and is not user-visible. A cyclic structure becomes reclaimable as
   soon as no handles outside the cycle remain, without any action by the
   program. Reachable cycles are never collected: anything still reachable from
   a live binding, or from data that binding owns, stays alive. There is no
   user-facing collect() function and no weak reference type in version 1.
10. The reference count can be observed with Rc<T>::strong_count (see Rc<T>).
    Leak checking is opt-in: the Debug profile of the generated CMake project
    compiles the runtime with -DLSACA_DEBUG. Set the LSACA_DEBUG_RC=1
    environment variable when running the compiler, or pass
    -DLSACA_DEBUG_RC=ON when configuring an emitted project, to enable tracking
    in a release build too (the generated CMakeLists.txt exposes the option
    LSACA_DEBUG_RC). With either define the
    runtime tracks reference-counted allocations, runs a final collection
    during process shutdown, and then prints a report to stderr:
        lsaca: warning: N reference-counted allocation(s) leaked
    followed by up to a few "lsaca:   leaked allocation of S bytes" lines.
    After the final collection the report only counts allocations that are
    genuinely leaked, for example acyclic data that the compiler failed to
    release, so it indicates a compiler/runtime bug, not a user bug, unless the
    program panicked or called exit (those paths skip the shutdown report).
11. Panics and exit terminate the process; the operating system reclaims memory,
    so no unwinding is required.

## Variables and mutability

    let x = 1;            // immutable binding, type inferred i64
    let mut y: i32 = 2;   // mutable binding, explicit type
    y = y + 1;

Bindings are immutable by default; assignment to an immutable place is a
compile-time error. Shadowing is allowed. Reading a local before its
declaration is an error. Every let declaration requires an initializer, so a
variable is always initialized before it is used.

## Operators

Precedence, from loosest to tightest:

| Operators | Associativity |
| --- | --- |
| = += -= *= /= %= &= |= ^= <<= >>= (assignment) | right |
| || | left |
| && | left |
| == != < <= > >= | left |
| \| (bitwise or) | left |
| ^ (bitwise xor) | left |
| & (bitwise and) | left |
| << >> (shifts) | left |
| + - | left |
| * / % | left |
| as | left |
| unary - ! | prefix |
| call, method call, field access, index | postfix |

Semantics:

* Arithmetic requires both operands to have the same type. String supports + +
  (concatenation), == !=, and ordering comparisons (byte-wise lexicographic).
* Integer arithmetic follows Rust: in debug builds overflow panics, in release
  builds it wraps; division and remainder by zero always panic; MIN / -1 panics
  in debug builds and wraps in release builds. Shifts panic when the shift
  amount is negative or >= the bit width.
* Float arithmetic is IEEE-754; floats are Display-formatted with the shortest
  representation that round-trips.
* Bool uses && || ! == !=; no truthiness conversions exist.
* Bitwise operators require integer types; shifts require integer types.
* as casts: integer <-> integer (truncating / sign-extending like Rust),
  integer <-> float (round-to-nearest; float -> int saturates, NaN -> 0),
  bool -> integer (0 or 1), integer -> bool is not allowed. Casts between
  strings and numbers are not implicit; use the parse methods.
* Comparison of Option/Result/tuples/structs/enums is not supported (use match,
  or compare the extracted payloads).

## Statements and control flow

A block { ... } contains statements and may end with a tail expression whose
value is the block's value; a block without a tail expression has type ().
Semicolons separate statements; a trailing semicolon discards a value. An
expression statement whose expression is block-like (if, match, loop, while,
for or a bare block) may omit the semicolon.

    let x = {
        let y = 2;
        y * 2
    };                      // x == 4

* if condition { ... } else { ... } is an expression; the condition must be
  bool; both branches must have compatible types when used as a value. Else
  branches may chain through else if and else if let.
* match value { pattern => expr, ... } is an expression (see Pattern matching).
* loop { ... } repeats forever; break exits. loop, while and for expressions
  have type (); break cannot carry a value.
* while condition { ... } repeats while the condition is true.
* for i in start..end { ... } and start..=end iterate i64 ranges (start <= end
  required at runtime; an empty range is fine). The loop variable is immutable.
  Range expressions are only valid in the header of a for loop; .. anywhere
  else is a syntax error.
* for x in vec { ... } iterates over the elements of a Vec by value (each
  element is a copy; mutating x is not possible without let mut and does not
  modify the vector).
* if let pattern = expr { ... } else { ... } and while let pattern = expr { ... }
  combine matching and control flow (irrefutable patterns are allowed too).
  Like if, an else branch may be another if or if let.
* break and continue are only valid inside loop/while/for.
* return [expr]; returns from the enclosing function.
  Tail expression: the last expression of a function body without a semicolon
  is the return value.

## Functions

    fn add(a: i64, b: i64) -> i64 {
        a + b
    }

    fn log(msg: String) {
        println!("{}", msg);
    }

* Every parameter needs a type and a simple name (destructuring patterns are
  not allowed in parameter position); the return type defaults to ().
* Recursion and mutual recursion are fully supported: a function can call
  itself and any other function by name.
* Functions are values: the name of a function can be used where a fn type is
  expected (fn(i64, i64) -> i64). Such values are reference counted and can be
  stored in structs, enums, Vecs and Options. Function values cannot be
  compared with == or ordered.
* There is no function overloading, no default or variadic parameters. Use
  Option/Result or structs for optional and named data.

## Structs, enums and impl blocks

    struct Point { x: f64, y: f64 }

    enum Shape {
        Circle(f64),
        Rect(f64, f64),
        Empty,
    }

    impl Point {
        fn new(x: f64, y: f64) -> Point { Point { x: x, y: y } }

        fn len_squared(&self) -> f64 { self.x * self.x + self.y * self.y }

        fn translate(&mut self, dx: f64, dy: f64) {
            self.x = self.x + dx;
            self.y = self.y + dy;
        }

        fn origin() -> Point { Point { x: 0.0, y: 0.0 } }
    }

* Structs have named fields; there is no tuple struct and no struct update
  syntax (construct a new value explicitly). Struct literals name their fields
  (Point { x: 1.0, y: 2.0 }); a field bound to a local of the same name can use
  the shorthand Point { x, y }, and S {} is the literal of a fieldless struct.
* Enums have unit variants (Empty) and tuple variants with any number of
  payload fields (Circle(f64), Rect(f64, f64)). Struct and enum bodies may be
  empty.
* impl blocks define inherent methods on a struct or enum. A method takes
  self, &self or &mut self as its first parameter (the receiver must come
  first, there is at most one, and mut self is not supported); methods without
  a receiver are associated functions called as Type::name(...).
* Mutating methods require the receiver to be reachable through a mut place;
  receivers are checked like any other place. The one exception is Rc<T>::set,
  which mutates the shared cell through an immutable handle (see Rc<T>).
* Fields are private to the compilation unit (there is no module system in
  version 1; pub is accepted and ignored).
* Structs and enums have value semantics and are copied on assignment (owned
  fields are retained). An Rc<T> field is copied as a handle, so both copies
  share the cell.

## Pattern matching

    match shape {
        Shape::Circle(r) => println!("circle {}", r),
        Shape::Rect(w, h) => println!("rect {} x {}", w, h),
        Shape::Empty => println!("empty"),
    }

Patterns:

* _ wildcard; identifier bindings (mut allowed): let (a, b) = pair;
* literals: true, false, integers, floats, strings; a minus sign may precede
  an integer or float literal (-1, -1.5), as in Rust;
* tuples: (a, b, c); struct patterns: Point { x, y }; with .. rest:
  Point { x, .. };
* enum variants: Shape::Circle(r), Option::Some(v) (usually written Some(v)),
  Result::Ok(v) (usually Ok(v)), None, Err(e);
* or-patterns: 1 | 2 | 3;
* guards: pattern if condition => expr (the guard is evaluated after the
  pattern binds; bindings are visible in the guard).

Matching rules:

* A match must be exhaustive: every value the scrutinee can take must be
  covered. The compiler checks bool, enum, Option, Result and tuple patterns,
  integer comparisons with literal ranges and string literals (wildcard
  required for strings unless every possible value is literally covered).
* Repeated or unreachable arms produce warnings.
* let, for and closure parameter patterns must be irrefutable (wildcards,
  bindings, single-variant destructuring). Refutable patterns are allowed in
  match arms, if let and while let.
* Bindings introduced by a pattern are immutable unless declared mut; they
  shadow outer names. The same name may not be bound twice in one pattern.
* Patterns do not support @ bindings, reference patterns, slice patterns or
  range patterns.

## Closures

    let add = |a: i64, b: i64| a + b;
    let scale = move |x: f64| x * factor;

* Closure syntax: |params| expr; || expr is the closure with no parameters.
  Parameters may carry optional types and an optional -> ReturnType may appear
  before the body. The body is a single expression; use { ... } for a block.
* Closures capture the variables they use by value: each captured value is
  copied (handles are retained) into a reference counted environment. A
  variable captured by a closure and mutated inside it is captured as a copy
  in version 1, so mutations inside the closure do not affect the outer
  variable (documented deviation from Rust).
* move closures are accepted and behave like ordinary closures in version 1.
* A closure can be called directly where its type is statically known, or
  stored in a variable/struct field/Vec whose type is a fn type:
  fn(i64, i64) -> i64. It cannot be compared, and different closures have
  different types (they are converted to the fn type when stored in one).
* Closures cannot be recursive directly (there is no self reference); use a
  named function instead.
* Capturing a mutable variable inside a closure and mutating it from outside is
  not supported (there are no references). Capture an Rc<T> handle instead when
  a closure and the surrounding code must share mutable state: the closure
  copies the handle, and both sides see set() calls through the same cell.

## Macros

Macros are built into the language; there is no user-defined macro system.
Ordinary macros take their arguments in parentheses; the vec! forms also accept
brackets.

* print!(format), println!(format), eprint!/eprintln! - write to stdout/stderr.
* format!(format) -> String.
* panic!(message) - abort with a panic message; panic!() is allowed.
* assert!(condition) and assert_eq!(a, b) - panic when the assertion fails.
* vec![a, b, c] and vec![value; count] - construct a Vec; the parentheses
  forms vec!(a, b, c) and vec!(value; count) are equivalent.

Format strings: {}, {:?} (debug representation: strings are quoted and escaped,
composites are printed structurally), and {{ / }} for literal braces. Each {}
consumes one argument, in order; the number of arguments must match. Format
specifiers with width/precision are not supported in version 1.

    println!("{} is {} years old", name, age);
    println!("{:?}", errors);       // [] or ["a", "b"]
    println!("{{literal braces}}");

## Standard library

No modules in version 1: the standard library is available without import.

### String

    String::new() -> String
    String::from(value)         // String::from("lit") and String::from(other_string)
    len(&self) -> i64                    // number of UTF-8 bytes
    is_empty(&self) -> bool
    push_str(&mut self, s: String)
    clear(&mut self)
    contains(&self, needle: String) -> bool
    starts_with(&self, prefix: String) -> bool
    ends_with(&self, suffix: String) -> bool
    find(&self, needle: String) -> Option<i64>       // byte index
    replace(&self, from: String, to: String) -> String
    to_uppercase(&self) -> String        // ASCII only in version 1
    to_lowercase(&self) -> String        // ASCII only in version 1
    trim(&self) -> String
    split(&self, sep: String) -> Vec<String>
    substring(&self, start: i64, len: i64) -> String  // byte offsets, panics when out of bounds
    repeat(&self, count: i64) -> String
    parse_i64(&self) -> Result<i64, String>
    parse_f64(&self) -> Result<f64, String>

String concatenation with + is available (String + String -> String).

### Vec<T>

    Vec::new() -> Vec<T>
    Vec::with_capacity(capacity: i64) -> Vec<T>
    len(&self) -> i64
    is_empty(&self) -> bool
    push(&mut self, value: T)
    pop(&mut self) -> Option<T>
    insert(&mut self, index: i64, value: T)      // panics when index > len
    remove(&mut self, index: i64) -> T            // panics when out of bounds
    clear(&mut self)
    first(&self) -> Option<T>
    last(&self) -> Option<T>
    get(&self, index: i64) -> Option<T>
    extend(&mut self, other: Vec<T>)
    reverse(&mut self)

Indexing v[i] is bounds checked and panics with a clear message when the index
is negative or >= len; indexing produces a value copy (assign back to modify).

### Option<T>

    Some(value), None
    is_some(&self) -> bool
    is_none(&self) -> bool
    unwrap(self) -> T                 // panics on None
    unwrap_or(self, default: T) -> T
    expect(self, message: String) -> T  // panics with message on None

### Result<T, E>

    Ok(value), Err(error)
    is_ok(&self) -> bool
    is_err(&self) -> bool
    unwrap(self) -> T
    unwrap_or(self, default: T) -> T
    expect(self, message: String) -> T
    ok(self) -> Option<T>
    err(self) -> Option<E>

### Rc<T>

Rc<T> is a builtin shared, mutable, heap-allocated cell holding one value of
type T. It is the only way to write shared mutable state or cyclic data in
version 1. An Rc<T> value is a handle to the cell; handles are reference
counted, so copying an Rc<T> is cheap and every copy refers to the same cell.
Rc::new(value) allocates the cell with a reference count of 1 and stores a copy
of value. When the payload type cannot be inferred from context, annotate the
binding (let n: Rc<i64> = Rc::new(0);) or use Rc::<i64>::new(0).

    Rc::new(value: T) -> Rc<T>
    get(self) -> T                    // owned copy of the payload
    set(self, value: T)               // replaces the payload
    strong_count(self) -> i64         // number of live handles to the cell

* get() returns an owned copy of the payload. The copy follows the normal value
  semantics: owned parts are retained and the copy is independent of the cell,
  so mutating the copy does not change the cell. Each .get() call takes a copy,
  so an expression that chains .get() walks copied values rather than the live
  graph and finishes after a finite number of steps.
* set(value) releases the value currently in the cell and stores a copy of
  value. Every handle to the same cell observes the new payload immediately.
* strong_count() returns the cell's current reference count: the number of live
  Rc<T> handles that refer to it. The call itself neither retains nor releases
  the handle, and a freshly created cell reports 1. A count above 1 means the
  cell is shared; a cycle keeps the count above zero after all outside handles
  are released.
* get() and set() both work on an immutable binding: the cell itself is the
  shared, mutable object, so no let mut binding and no &mut receiver are
  needed; strong_count() is read-only. All three take the handle by value (a
  handle produced by an expression is a temporary released at the end of the
  statement).
* T may be any value type: integers, floats, bool, String, Vec<T>,
  Option/Result, tuples, structs, enums and fn types. Rc<T> can appear in struct
  fields, inside Vec/Option/Result and tuples, and in closures.
* A type cannot contain itself by value, so recursive types use Rc<T> (or
  Vec<T>) as indirection; the examples below show the usual pattern.
* Two handles compare equal with == (and unequal with !=) when they refer to
  the same cell: Rc<T> equality is handle identity and never compares payloads.
  This differs from Rust, where Rc<T> equality compares the payloads.
* `{}` and `{:?}` format a handle as Rc(payload), like Rust. Formatting a
  handle that takes part in a cycle never terminates, so print a copied payload
  instead: println!("{}", cell.get()). The same rule applies to assert_eq! and
  to any other macro that formats a handle.
* The idiomatic way to print a cell is to print its payload:
  println!("{}", cell.get()).

Sharing - two handles alias one cell:

    let counter = Rc::new(0);
    let copy = counter;
    copy.set(copy.get() + 1);
    println!("{}", counter.get());   // prints 1

A cyclic structure - the payload of a is a Node whose next field points back at
the cell that holds it:

    struct Node {
        value: i64,
        next: Option<Rc<Node>>,
    }

    fn main() {
        let a = Rc::new(Node { value: 1, next: None });
        let node = Node { value: 1, next: Some(a) };
        a.set(node);
        println!("{}", a.get().next.unwrap().get().value);   // prints 1
    }

The last line walks one hop out of the cell: a.get() copies the Node payload,
next is that copy's Option<Rc<Node>>, unwrap() yields the captured handle and
get() copies the Node again. The traversal terminates because .get() copies; it
never follows the live graph in place. When main returns, the local handles are
released and no handle outside the cycle remains, so the collector reclaims the
cell without any action by the program.

Cycles through containers look the same - any owned container can hold the
handles that close the cycle:

    struct Tree {
        value: i64,
        children: Vec<Rc<Tree>>,
    }

    fn main() {
        let root = Rc::new(Tree { value: 1, children: Vec::new() });
        let child = Rc::new(Tree { value: 2, children: vec![root] });
        root.set(Tree { value: 1, children: vec![child] });
        println!("{}", root.get().children.len());   // prints 1
    }

Only Rc<T> introduces shared aliasing and indirection. String, Vec, Option,
Result and plain struct fields are owned values: they can store Rc<T> handles,
but without at least one Rc<T> on the path back to an object they cannot close
a cycle. Reachable cycles are ordinary live data and are never collected; a
cycle becomes garbage only once no handles outside it remain.

### Free functions

    read_line() -> Option<String>          // stdin line without the trailing newline; None on EOF
    args() -> Vec<String>                  // program arguments, excluding argv[0]
    exit(code: i32)                        // terminate the process immediately
    read_file(path: String) -> Result<String, String>
    write_file(path: String, contents: String) -> Result<(), String>

    sqrt(x: f64) -> f64        abs(x: i64/u64/f64) -> same type
    min(a, b) / max(a, b)      pow(base: f64, exp: f64) -> f64
    floor(x: f64) -> f64       ceil(x: f64) -> f64       round(x: f64) -> f64
    sin(x) / cos(x) / tan(x) / ln(x) / log10(x) / exp(x) -> f64

Integer and float variants of abs/min/max are selected by argument type; there
is no overloading mechanism in the language, this is provided by the compiler
for these builtins only.

## Panics and exit codes

A panic prints

    thread 'main' panicked at FILE:LINE:COL:
    MESSAGE

to stderr and exits with status 101. Panics are produced by: unwrap/expect on
None/Err, integer division/remainder by zero, integer overflow in debug builds,
index/substring out of bounds, failed assertions, and explicit panic!.

The compiler CLI exits with 0 on success, 1 when the program has errors and 2
for command line usage errors. A compiled program's exit status is its own; a
panic is 101.

## Grammar (EBNF, version 1)

    program      = item* EOF ;
    item         = fnItem | structItem | enumItem | implItem | constItem | typeItem ;
    fnItem       = [ "pub" ] "fn" IDENT "(" params? ")" ( "->" type )? block ;
    params       = param ( "," param )* ","? ;
    param        = "self" | "&" "self" | "&" "mut" "self" | IDENT ":" type ;
    structItem   = [ "pub" ] "struct" IDENT "{" fieldDefs? "}" ;
    fieldDefs    = fieldDef ( "," fieldDef )* ","? ;
    fieldDef     = IDENT ":" type ;
    enumItem     = [ "pub" ] "enum" IDENT "{" variants? "}" ;
    variants     = variant ( "," variant )* ","? ;
    variant      = IDENT ( "(" typeArgs? ")" )? ;
    implItem     = [ "pub" ] "impl" type "{" fnItem* "}" ;
    constItem    = [ "pub" ] "const" IDENT ":" type "=" expr ";" ;
    typeItem     = [ "pub" ] "type" IDENT "=" type ";" ;

    block        = "{" stmt* tail? "}" ;
    stmt         = letStmt | item | exprStmt | blockLikeStmt | ";" ;
    letStmt      = "let" pattern ( ":" type )? "=" expr ";" ;
    exprStmt     = expr ";" ;
    blockLikeStmt= block | ifExpr | matchExpr | loopExpr | whileExpr | forExpr ;
    tail         = expr ;

    expr         = assignExpr ;
    assignExpr   = orExpr ( assignOp assignExpr )? ;
    assignOp     = "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" ;
    orExpr       = andExpr ( "||" andExpr )* ;
    andExpr      = cmpExpr ( "&&" cmpExpr )* ;
    cmpExpr      = bitorExpr ( ( "==" | "!=" | "<" | "<=" | ">" | ">=" ) bitorExpr )* ;
    bitorExpr    = bitxorExpr ( "|" bitxorExpr )* ;
    bitxorExpr   = bitandExpr ( "^" bitandExpr )* ;
    bitandExpr   = shiftExpr ( "&" shiftExpr )* ;
    shiftExpr    = addExpr ( ( "<<" | ">>" ) addExpr )* ;
    addExpr      = mulExpr ( ( "+" | "-" ) mulExpr )* ;
    mulExpr      = castExpr ( ( "*" | "/" | "%" ) castExpr )* ;
    castExpr     = unaryExpr ( "as" type )* ;
    unaryExpr    = ( "-" | "!" ) unaryExpr | postfixExpr ;
    postfixExpr  = primary ( call | methodCall | "." IDENT | "." INT | "[" expr "]" )* ;
    call         = "(" args? ")" ;
    methodCall   = "." IDENT ( "::" "<" typeArgs ">" )? "(" args? ")" ;
    args         = expr ( "," expr )* ","? ;
    primary      = INT | FLOAT | STRING | "true" | "false"
                 | "(" ")" | "(" expr ")" | tupleExpr
                 | path | structLiteral | vecExpr | macroCall
                 | ifExpr | matchExpr | loopExpr | whileExpr | forExpr | block
                 | closure | "break" expr? | "continue" | "return" expr? ;
    path         = pathIdent ( "::" pathIdent )* ( "::" "<" typeArgs ">" )? ;
    pathIdent    = IDENT | "self" | "Self" ;
    tupleExpr    = "(" expr "," ")" | "(" expr ( "," expr )+ ","? ")" ;
    structLiteral= path "{" ( fieldInit ( "," fieldInit )* ","? )? "}" ;
    fieldInit    = IDENT ":" expr | IDENT ;
    vecExpr      = "vec" "!" ( "[" vecItems? "]" | "(" vecItems? ")" ) ;
    vecItems     = expr ( "," expr )* ","? | expr ";" expr ;
    macroCall    = IDENT "!" "(" args? ")" ;
    ifExpr       = "if" ( "let" pattern "=" expr | expr ) block ( "else" ( ifExpr | block ) )? ;
    matchExpr    = "match" expr "{" matchArm* "}" ;
    matchArm     = pattern ( "if" expr )? "=>" expr ","? ;
    loopExpr     = "loop" block ;
    whileExpr    = "while" ( "let" pattern "=" expr | expr ) block ;
    forExpr      = "for" pattern "in" ( expr ( ".." | "..=" ) expr | expr ) block ;
    closure      = "move"? ( "||" | "|" closureParams? "|" ) ( "->" type )? expr ;
    closureParams= closureParam ( "," closureParam )* ","? ;
    closureParam = primaryPattern ( ":" type )? ;

    pattern      = orPattern ;
    orPattern    = primaryPattern ( "|" primaryPattern )* ;
    primaryPattern = "_" | "mut"? IDENT
                 | "true" | "false" | INT | FLOAT | STRING | "-" ( INT | FLOAT )
                 | "(" ")" | "(" pattern ")" | tuplePattern
                 | path ( "(" patternList? ")" )?
                 | path "{" ( fieldPat ( "," fieldPat )* ( "," ".." )? ","? | ".." )? "}" ;
    patternList  = pattern ( "," pattern )* ","? ;
    tuplePattern = "(" pattern "," ")" | "(" pattern ( "," pattern )+ ","? ")" ;
    fieldPat     = IDENT ( ":" pattern )? ;

    type         = pathType | tupleType | fnType ;
    pathType     = typeIdent ( "::" typeIdent )* ( "::" "<" typeArgs ">" | "<" typeArgs ">" )? ;
    typeIdent    = IDENT | "Self" ;
    tupleType    = "(" ")" | "(" type ")" | "(" type "," ")" | "(" type ( "," type )* ","? ")" ;
    fnType       = "fn" "(" typeArgs? ")" ( "->" type )? ;
    typeArgs     = type ( "," type )* ","? ;

## Deliberate deviations from Rust

lsaca borrows Rust's syntax and most of its semantics, but it is not Rust:

1. Memory is ORC (automatic reference counting plus a trial-deletion cycle
   collector), not ownership with borrow checking. There are no move errors, no
   lifetimes, no &T/&mut T references, no borrow checks. Copies of owned values
   cost a reference count increment unless uniquely owned mutation is possible
   (COW). Shared mutable state is written with the builtin Rc<T> cell, where
   Rust needs Rc<RefCell<T>> or Cell<T>; reference cycles are allowed and
   collected automatically instead of leaking.
2. No modules, imports, traits, generics on user items, macros with user
   definitions, unsafe, extern, async, or const generics in version 1.
3. Closures capture by value (and move closures are accepted). Captures are
   copies, so mutating a captured variable does not affect the outer variable.
4. No usize: all lengths and indices are i64.
5. No char type and no byte strings; strings are UTF-8 and methods use byte
   offsets, matching String's len() being a byte count.
6. No arrays or slices: use Vec<T>.
7. No ref/ref mut patterns, @ bindings, slice patterns or range patterns.
8. break/continue cannot carry values, so loop, while and for expressions have
   type (); there is no loop-with-break-value in version 1.
9. Integer literal suffixes are not supported (use annotations/casts).
10. Strings compare lexicographically with < <= > >=, which Rust's String does
    not (it compares only by == / != via PartialOrd for str; lsaca supports
    ordering by byte value to make sorting possible).
11. The standard library is a smaller, built-in subset (see above). There is no
    formatter API besides the format macros.
12. There is no REPL: lsaca is a compiled language.
13. Rc<T> handles compare with == by identity (same cell), not by payload.
    Rust compares Rc<T> payloads; identity is used here so that comparing the
    handles of a cycle terminates.

## Not in version 1 (roadmap)

Modules and imports, user generics/traits, a Box type, reference types, arrays
and slices, string formatting widths and precision, user-defined macros,
iterators and protocol traits, multi-threading, a package manager, incremental
compilation, and a language server.

