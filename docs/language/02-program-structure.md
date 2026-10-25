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
[Standard library](13-standard-library.md) are the entire prelude.

Whitespace is insignificant outside literals, and the file is processed from
beginning to end by the parser. An empty file is a syntactically valid
compilation unit, but the type checker rejects it because it has no `main`.

## The six item kinds

At the top level a file contains a sequence of **items**. There are exactly six
kinds:

| Item | Keyword | Purpose |
| --- | --- | --- |
| Function | `fn` | A named function, including `main`. |
| Struct | `struct` | A record with named fields. |
| Enum | `enum` | A tagged union of unit or tuple variants. |
| Impl | `impl` | Methods and associated functions for a struct or enum type. |
| Constant | `const` | A compile-time constant with a required type annotation. |
| Type alias | `type` | A new name for an existing type. |

Items may also appear **inside a block** — inside a function body, an `if`
branch, a loop body, and so on. Such nested items are scoped to the block that
contains them; see [Nested items and hoisting](#nested-items-and-hoisting).

Anything else at the top level is a syntax error: expressions, `let` statements,
`if`, `match`, `return` and so on only exist inside function bodies. There is no
top-level executable code and no initializer hook.

`pub` is accepted on top-level items and ignored: there is no visibility system
and no modules, so everything in the file is visible everywhere in the file.
`pub` is not accepted in front of struct fields.

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
area(circle 2) = 12.566
height = 12 m
```

The example uses all six item kinds, and the `impl` block appears before the
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
error: expected an item (fn, struct, enum, impl, const or type), found let
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

`pub` is **not** accepted on struct fields. The field list grammar does not
include it, so the parser stops at the unexpected keyword:

```ls,compile-fail
struct Point {
    pub x: i64,
    pub y: i64,
}

fn main() {
    let p = Point { x: 1, y: 2 };
    println!("{}", p.x);
}
```

Error:
```text
error: expected field name, found pub
  --> example-09.ls:2:5
   |
 2 |     pub x: i64,
   |     ^^^

error: expected ':' after field name, found identifier x
  --> example-09.ls:2:9
   |
 2 |     pub x: i64,
   |         ^

error: expected '}', found :
  --> example-09.ls:2:10
   |
 2 |     pub x: i64,
   |          ^

error: expected an item (fn, struct, enum, impl, const or type), found :
  --> example-09.ls:2:10
   |
 2 |     pub x: i64,
   |          ^

lsaca: example-09.ls: could not compile example-09.ls due to previous error(s)
```

## The complete grammar

The grammar below is the authoritative EBNF of the 1.2.0 parser. Optional parts
are written with `[...]`, repetitions with `*` and `+`, and alternatives with
`|`. It is reproduced here for reference; the individual pages explain each
construct in detail.

```ebnf
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
postfixExpr  = primary ( call | methodCall | "." IDENT | "." INT | "[" expr "]" | "?" )* ;
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
```

A few productions deserve a warning because the parser is more permissive than
the type checker: `methodCall` parses `::<...>` (turbofish) after the method
name, but the type checker rejects generic methods; and `break expr` /
`return expr` parse even where a value cannot be carried.

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

The full rules are in [Pattern matching](11-pattern-matching.md#or-patterns).

## What next

* Every token and literal: [Lexical structure](03-lexical-structure.md).
* What the type checker knows about types: [Types](04-types.md).
* Define your own data: [Structs, enums and impl](10-structs-enums-and-impl.md).
* Function values and recursion: [Functions](09-functions.md).
