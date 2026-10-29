# Program structure

This page describes the largest unit the compiler sees: a whole program. lsaca
has no modules and no imports, so a program is exactly one source file, and the
structure of that file is the structure of the program.

## Compilation units

A compilation unit is one UTF-8 file with the extension `.ls`. Everything the
program needs must be in that file: there is no `mod`, no `use`, no include
mechanism and no way to compile two files together. This keeps the language
small and makes programs easy to move around, but it also means the standard
library is the only library: the built-in types and free functions listed in
[Standard library](15-standard-library.md) are the entire prelude.

Whitespace is insignificant outside literals, and the file is processed from
beginning to end by the parser. An empty file is a syntactically valid
compilation unit, but the type checker rejects it because it has no `main`.

## The seven item kinds

At the top level a file contains a sequence of **items**. There are exactly seven
kinds:

| Item | Keyword | Purpose |
| --- | --- | --- |
| Function | `fn` | A named function, including `main`. |
| Struct | `struct` | A record with named fields. |
| Enum | `enum` | A tagged union of unit or tuple variants. |
| Impl | `impl` | Methods and associated functions for a struct or enum type. |
| Constant | `const` | A compile-time constant with a required type annotation. |
| Trait | `trait` | A trait declaration: method signatures with receivers only; implemented for a user struct or enum with `impl Trait for Type`. Added in 1.5.0. |
| Type alias | `type` | A new name for an existing type. |

Items may also appear **inside a block** — inside a function body, an `if`
branch, a loop body, and so on. Such nested items are scoped to the block that
contains them; see [Nested items and hoisting](#nested-items-and-hoisting).

Anything else at the top level is a syntax error: expressions, `let` statements,
`if`, `match`, `return` and so on only exist inside function bodies. There is no
top-level executable code and no initializer hook.

`pub` is accepted on top-level items, struct fields, enum variants and trait
methods, and ignored: there is no visibility system and no modules, so
everything in the file is visible everywhere in the file.

## Order does not matter

Items are collected before any body is checked, so a function can call another
function defined further down the file, a struct can contain a type defined
later, and an `impl` block can appear before or after the `struct` it belongs to.
The only ordering rule is the obvious one: the program's entry point must exist
somewhere in the file.

```ls
struct Point {
    x: i64,
    y: i64,
}

impl Point {
    fn new(x: i64, y: i64) -> Point {
        Point { x, y }
    }

    fn length_squared(&self) -> i64 {
        self.x * self.x + self.y * self.y
    }
}

trait Doubled {
    fn doubled(&self) -> i64;
}

impl Doubled for Point {
    fn doubled(&self) -> i64 {
        self.x * 2
    }
}

type Meters = i64;

const SCALE: i64 = 2;

enum Shape {
    Circle(f64),
    Square(f64),
}

fn area(shape: Shape) -> f64 {
    match shape {
        Shape::Circle(r) => 3.141592653589793 * r * r,
        Shape::Square(side) => side * side,
    }
}

fn main() {
    let p = Point::new(3, 4);
    println!("length squared = {}", p.length_squared());
    println!("scale = {}", SCALE);
    println!("scaled length squared = {}", p.length_squared() * SCALE);
    println!("doubled = {}", p.doubled());
    println!("area(circle 2) = {:.3}", area(Shape::Circle(2.0)));
    let height: Meters = 12;
    println!("height = {} m", height);
}
```

Output:
```text
length squared = 25
scale = 2
scaled length squared = 50
doubled = 6
area(circle 2) = 12.566
height = 12 m
```

The example uses all seven item kinds, and the `impl` block appears before the
`Point` definition is complete textually — item order does not matter. Note the
restriction on constants: the type of a `const` must be a scalar type or
`String`, and the initializer must be a constant expression. A struct-typed
constant is rejected with `error[E0001]: constants of type Point are not
supported; constant types must be scalars or String`, and a `const` initialized
with a function call is rejected with `E0015` (see [Variables and
mutability](06-variables-and-mutability.md)).

## The entry point: `main`

A program must define exactly one `fn main`. The type checker enforces its
shape:

* it must exist (error `E0601` otherwise);
* it must take no parameters (error `E0580`);
* it must not return a value — its return type must be omitted or written as `()`
  (error `E0580`).

The body is an ordinary block; the process exit status is `0` when `main`
returns, `101` if the program panics, and whatever `exit(n)` was given if the
program calls `exit` explicitly.

These three programs are rejected:

```ls,compile-fail
fn helper() -> i64 {
    42
}
```

Error:
```text
error[E0601]: main function not found
  --> example-02.ls:1:1
   |
 1 | fn helper() -> i64 {
   | ^
   = note: the program must define fn main()

lsaca: example-02.ls: could not compile example-02.ls due to previous error(s)
```

```ls,compile-fail
fn main(argc: i64) {
    println!("{}", argc);
}
```

Error:
```text
error[E0580]: main function must not take parameters, but it declares 1
  --> example-03.ls:1:4
   |
 1 | fn main(argc: i64) {
   |    ^^^^

lsaca: example-03.ls: could not compile example-03.ls due to previous error(s)
```

```ls,compile-fail
fn main() -> i64 {
    0
}
```

Error:
```text
error[E0580]: main function must return (), but it returns i64
  --> example-04.ls:1:4
   |
 1 | fn main() -> i64 {
   |    ^^^^

lsaca: example-04.ls: could not compile example-04.ls due to previous error(s)
```

Functions other than `main` may of course take parameters and return values;
see [Functions](09-functions.md).

## Nested items and hoisting

An item may be declared inside a block. The declaration creates a name that is
visible in the whole enclosing block, not only after the declaration — items are
hoisted the same way they are at the top level. This is useful for keeping a
helper next to its only caller without polluting the file scope, and for
declaring block-local structs and constants.

```ls
fn main() {
    // The nested function is visible before its declaration.
    println!("twice(21) = {}", twice(21));

    fn twice(x: i64) -> i64 {
        x * 2
    }

    let limit: i64 = 3;
    println!("limit = {}", limit);

    {
        // A nested item is scoped to its block and can shadow an outer name.
        const LIMIT: i64 = 10;
        println!("inner LIMIT = {}", LIMIT);
    }

    println!("outer limit = {}", limit);
    println!("sum(1..=4) = {}", sum_range(1, 4));

    fn sum_range(lo: i64, hi: i64) -> i64 {
        let mut total = 0;
        for i in lo..=hi {
            total += i;
        }
        total
    }
}
```

Output:
```text
twice(21) = 42
limit = 3
inner LIMIT = 10
outer limit = 3
sum(1..=4) = 10
```

Nested items cannot see the local variables of the block that contains them:
a nested function is a normal function, so it only has its parameters and its
own locals. If it needs a value, pass it as a parameter.

## Names

* Two items of any kind cannot share a name in the same scope: a `fn` and a
  `struct` called `Point` in the same file is error `E0428`. This also applies to
  two functions with the same name and to two constants with the same name.
* Fields within a struct, variants within an enum, and the parameters of one
  function must also be distinct; a repeated struct field is error `E0124`.
* A block may shadow a name from an enclosing scope (for example a nested `const
  LIMIT` above, or a `let` that reuses a variable name). Shadowing is lexical
  and scope-based.
* There is no overloading: a function name identifies exactly one function, and
  method names live in the namespace of their type.

Duplicate top-level definitions are rejected:

```ls,compile-fail
fn main() {
    println!("{}", f());
}

fn f() -> i64 {
    1
}

fn f() -> i64 {
    2
}
```

Error:
```text
error[E0428]: the name 'f' is defined multiple times
  --> example-06.ls:9:4
   |
 9 | fn f() -> i64 {
   |    ^
   = note: example-06.ls:5:4: previous definition of 'f' here

lsaca: example-06.ls: could not compile example-06.ls due to previous error(s)
```

## Statements do not exist at the top level

A top-level file is a sequence of items only. A `let`, a call, or any other
statement outside a function body is a parse error pointing at the offending
token:

```ls,compile-fail
let answer = 42;

fn main() {
    println!("{}", answer);
}
```

Error:
```text
error: expected an item (fn, struct, enum, impl, const, type or trait), found let
  --> example-07.ls:1:1
   |
 1 | let answer = 42;
   | ^^^

lsaca: example-07.ls: could not compile example-07.ls due to previous error(s)
```

## `pub` and visibility

`pub` is parsed on top-level items and does nothing: with a single compilation
unit there is nothing to export. The following is accepted and behaves exactly
like the same program without the `pub` keywords:

```ls
pub struct Counter {
    value: i64,
}

pub const STEP: i64 = 5;

pub fn bump(counter: Counter) -> Counter {
    Counter { value: counter.value + STEP }
}

pub fn main() {
    let c = Counter { value: 0 };
    println!("{}", bump(c).value);
}
```

Output:
```text
5
```

`pub` is also accepted, and ignored, on struct fields and enum variants;
neither has visibility in lsaca. Writing the keyword changes nothing:

```ls
pub struct Point {
    pub x: i64,
    pub y: i64,
}

pub enum Shape {
    Empty,
    pub Circle(f64),
}

fn main() {
    let point = Point { x: 1, y: 2 };
    println!("{} {}", point.x, point.y);
    match Shape::Circle(2.0) {
        Shape::Empty => println!("empty"),
        Shape::Circle(radius) => println!("circle {}", radius),
    }
}
```

Output:
```text
1 2
circle 2
```

## The complete grammar

The grammar below is the authoritative EBNF of the 1.5.0 parser. Optional parts
are written with `[...]`, repetitions with `*` and `+`, and alternatives with
`|`. It is reproduced here for reference; the individual pages explain each
construct in detail.

```ebnf
program      = item* EOF ;
item         = fnItem | structItem | enumItem | implItem | traitItem | constItem | typeItem ;
fnItem       = [ "pub" ] "fn" IDENT genericParams? "(" params? ")" ( "->" type )? block ;
genericParams= "<" genericParam ( "," genericParam )* ","? ">" ;
genericParam = IDENT ( ":" bound ( "+" bound )* )? ;
bound        = callableBound | pathType ;
callableBound= "Fn" "(" typeArgs? ")" ( "->" type )? ;
params       = param ( "," param )* ","? ;
param        = "self" | "&" "self" | "&" "mut" "self" | IDENT ":" type ;
structItem   = [ "pub" ] "struct" IDENT genericParams? "{" fieldDefs? "}" ;
fieldDefs    = fieldDef ( "," fieldDef )* ","? ;
fieldDef     = [ "pub" ] IDENT ":" type ;
enumItem     = [ "pub" ] "enum" IDENT genericParams? "{" variants? "}" ;
variants     = variant ( "," variant )* ","? ;
variant      = [ "pub" ] IDENT ( "(" typeArgs? ")" )? ;
implItem     = [ "pub" ] "impl" genericParams? type ( "for" type )? "{" fnItem* "}" ;
traitItem    = [ "pub" ] "trait" IDENT genericParams? "{" traitFnItem* "}" ;
traitFnItem  = [ "pub" ] "fn" IDENT "(" params? ")" ( "->" type )? ";" ;
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
unaryExpr    = ( "-" | "!" | "*" | "&" [ "mut" ] ) unaryExpr | postfixExpr ;
postfixExpr  = primary ( call | methodCall | "." IDENT | "." INT | "[" expr "]" | "?" )* ;
call         = "(" args? ")" ;
methodCall   = "." IDENT ( "::" "<" typeArgs ">" )? "(" args? ")" ;
args         = expr ( "," expr )* ","? ;
primary      = INT | FLOAT | STRING | "true" | "false"
             | "(" ")" | "(" expr ")" | tupleExpr
             | path | structLiteral | vecExpr | macroCall
             | ifExpr | matchExpr | loopExpr | whileExpr | forExpr | block
             | closure | "break" expr? | "continue" | "return" expr? ;
path         = pathIdent ( ( "::" pathIdent ) | ( "::" "<" typeArgs ">" ) )* ;
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

type         = refType | pathType | tupleType | fnType ;
refType      = "&" [ "mut" ] type ;
pathType     = typeIdent ( "::" typeIdent )* ( "::" "<" typeArgs ">" | "<" typeArgs ">" )? ;
typeIdent    = IDENT | "Self" ;
tupleType    = "(" ")" | "(" type ")" | "(" type "," ")" | "(" type ( "," type )* ","? ")" ;
fnType       = "fn" "(" typeArgs? ")" ( "->" type )? ;
typeArgs     = type ( "," type )* ","? ;
```

A few productions deserve a warning because the parser is more permissive than
the type checker: `methodCall` parses `::<...>` (turbofish) after the method
name, but the type checker rejects generic methods (traits have no generic
methods in 1.5.0); and `break expr` / `return expr` parse even where a value
cannot be carried.

Or-patterns (`orPattern`) and match guards (the optional `if` in `matchArm`) are
fully implemented since 1.1.0: every alternative of an or-pattern is tested, and a
guard filters the arm. Together they allow compact classification:

```ls
fn classify(n: i64) -> String {
    match n {
        0 | 1 => String::from("small"),
        n if n > 10 => String::from("big"),
        _ => String::from("other"),
    }
}

fn main() {
    println!("{} {} {}", classify(1), classify(11), classify(5));
}
```

Output:
```text
small big other
```

## Syntax added in 1.5

The grammar above already contains the productions added for generics and
traits. This section collects those forms, the turbofish, and the
syntax restrictions that the parser and the item collector enforce.

### Generic parameter lists and bounds

A generic item writes its parameters in angle brackets after its name:
`struct Pair<T>`, `enum Tree<T>`, `fn largest<T>(…)`, `impl<T> Pair<T>`,
`trait Iterator<T>`. Each parameter is an identifier with an optional
`:`-introduced bound list:

* a trait path, with optional type arguments: `T: Greet`, `T: Iterator<i64>`;
* the special `Drop` bound: `T: Drop`;
* the callable form `Fn(A, B) -> C` (the parentheses are part of the bound; the
  spaced `fn(A) -> B` form is a type, not a bound).

Bounds are separated by `+` (`T: Greet + Drop`), a trailing comma is allowed,
and a `>>` that closes two nested argument lists is split into two `>` tokens.
An empty list `<>` is rejected with `E0001`, and generic type aliases are
parsed but rejected with `E0001` (their message says type aliases cannot be
generic); type aliases are not generic.

### Trait declarations and implementations

A trait declaration contains method signatures only:

```ebnf
traitItem    = [ "pub" ] "trait" IDENT genericParams? "{" traitFnItem* "}" ;
traitFnItem  = [ "pub" ] "fn" IDENT "(" params? ")" ( "->" type )? ";" ;
```

Every method ends in `;` — there is no body — and its first parameter must be a
receiver (`self`, `&self` or `&mut self`). `pub` before a trait or a method is
accepted and ignored. The following shapes are rejected with `E0001`:

| Shape | Restriction |
| --- | --- |
| `fn greet(&self) -> String { … }` | default method bodies are not supported |
| `fn name() -> String;` | trait methods must take a self receiver |
| `fn convert<T>(&self, value: T) -> i64;` | trait methods cannot be generic |
| `trait A: B { … }` | supertraits are not supported |

An implementation writes `impl Trait<Args> for Type { … }`; the `for` clause is
only for trait implementations (inherent impls are `impl Type { … }`) and the
target must be a user struct or enum. Implementations may be generic
(`impl<T> Iterator<T> for VecIter<T>`), and every signature must match the
declaration. `impl Drop`, `impl Copy` and `impl Clone` keep their special
forms (see [Ownership and moves](11-ownership-and-moves.md) and
[Structs, enums and impl](10-structs-enums-and-impl.md#drop-hooks)).

### Turbofish

Explicit type arguments are written with the Rust-style turbofish `::<…>`:

* a generic function: `largest::<i64>(values)`;
* a generic associated function: `Pair::<i64>::new(3, 4)` and
  `HashMap::<String, i64>::new()`;
* a generic struct literal: `Pair::<i64> { first: 3, second: 4 }`;
* a method call: `value.method::<i64>()` parses, but generic methods do not
  exist in 1.5.0, so the type checker rejects the turbofish with `E0107`
  (`method get takes 0 generic arguments but 1 was supplied`).

In expression position use the turbofish; in type position the arguments are
written without `::`: `let pair: Pair<i64> = Pair::<i64>::new(1, 2);`.

### `Self` in expression position

Inside an `impl` block, `Self` names the implementing type. Besides `-> Self`
and parameter types it can construct values: `Self { field: value }` and
`Self::new(args)`; both work in generic and non-generic impls.

### `?` after block-like expressions

The postfix `?` operator may follow any expression, including block-like ones
(`if`/`else`, `match`, and plain blocks). Parentheses are optional; wrapping a
block-like operand usually reads better:

```ls
fn parse(flag: bool) -> Result<i64, String> {
    let value = (if flag { Ok(1) } else { Err(String::from("no")) })?;
    Ok(value + 1)
}

fn main() {
    println!("{:?}", parse(true));
    println!("{:?}", parse(false));
}
```

Output:
```text
Ok(2)
Err("no")
```

## What next

* Every token and literal: [Lexical structure](03-lexical-structure.md).
* What the type checker knows about types: [Types](04-types.md).
* Define your own data: [Structs, enums and impl](10-structs-enums-and-impl.md).
* Function values and recursion: [Functions](09-functions.md).
