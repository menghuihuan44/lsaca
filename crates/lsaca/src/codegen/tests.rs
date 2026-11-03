//! Unit tests for the C code generator.
//!
//! Each test compiles a small source program through the front end, runs the
//! generator and inspects the produced main.c text. Building and running the
//! generated C project is covered by the end to end test suite, so these tests
//! stay fast and do not need a C toolchain.

use super::{CodegenOptions, generate};
use crate::driver::Frontend;

fn generate_project_source(name: &str, source: &str, debug_checks: bool) -> super::Generated {
    let frontend = Frontend::new(format!("{}.ls", name), source);
    let program = match frontend.check() {
        Ok(program) => program,
        Err(diagnostics) => panic!(
            "front end rejected the test program:\n{}",
            diagnostics.render(frontend.file())
        ),
    };
    let options = CodegenOptions {
        program_name: name.to_string(),
        debug_checks,
        sources: Some(frontend.sources().clone()),
        ..CodegenOptions::default()
    };
    generate(&program, &options)
}

fn generate_source(name: &str, source: &str, debug_checks: bool) -> (String, String) {
    let generated = generate_project_source(name, source, debug_checks);
    (generated.main_c, generated.cmake)
}

const HELLO: &str = "fn main() {\n    println!(\"hello, world!\");\n}\n";

#[test]
fn emits_project_scaffolding_and_main_wrapper() {
    let (main_c, cmake) = generate_source("hello", HELLO, true);
    assert!(main_c.contains("#include <stdint.h>"));
    assert!(main_c.contains("#include <stdbool.h>"));
    assert!(main_c.contains("#include \"lsaca_rt.h\""));
    assert!(main_c.contains("typedef struct LsUnit"));
    assert!(main_c.contains("int main(int argc, char** argv)"));
    assert!(main_c.contains("lsaca_cast_f2i_i64("));
    assert!(main_c.contains("lsaca_cast_f2i_u8("));

    let wrapper = main_c.find("int main(int argc, char** argv)").unwrap();
    let tail = &main_c[wrapper..];
    let init = tail.find("ls_runtime_init(argc, argv);").unwrap();
    let user_main = tail.find("lsaca_fn_main();").unwrap();
    let shutdown = tail.find("ls_runtime_shutdown();").unwrap();
    assert!(init < user_main);
    assert!(user_main < shutdown);
    assert!(tail.contains("return 0;"));

    assert!(cmake.contains("project(hello C)"));
    let runtime_sources: Vec<&str> = crate::runtime::sources()
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| name.ends_with(".c"))
        .collect();
    let expected_sources = format!("add_executable(hello main.c {})", runtime_sources.join(" "));
    assert!(cmake.contains(&expected_sources));
    assert!(cmake.contains("CMAKE_C_STANDARD 17"));
}

const STRUCTS: &str = "struct Point {\n    x: f64,\n    y: f64,\n}\n\nfn main() {\n    let p = Point { x: 1.0, y: 2.0 };\n    println!(\"{} {}\", p.x, p.y);\n}\n";

#[test]
fn emits_struct_helpers() {
    let (main_c, _) = generate_source("structs", STRUCTS, true);
    assert!(main_c.contains("struct LsPoint {"));
    assert!(main_c.contains("double f_x;"));
    assert!(main_c.contains("double f_y;"));
    assert!(main_c.contains("lsaca_copy_Point("));
    assert!(main_c.contains("lsaca_release_Point("));
    assert!(main_c.contains("lsaca_debug_Point("));
    assert!(main_c.contains("lsaca_eq_Point("));
    assert!(main_c.contains(".f_x = "));
}

const ENUM: &str = "enum Shape {\n    Circle(f64),\n    Empty,\n}\n\nfn main() {\n    let s = Shape::Circle(1.0);\n    match s {\n        Shape::Circle(radius) => println!(\"{}\", radius),\n        Shape::Empty => println!(\"empty\"),\n    }\n}\n";

#[test]
fn emits_enum_tagged_union_layout() {
    let (main_c, _) = generate_source("enums", ENUM, true);
    assert!(main_c.contains("struct LsShape {"));
    assert!(main_c.contains("int32_t tag;"));
    assert!(main_c.contains("union {"));
    assert!(main_c.contains("v_Circle"));
    assert!(!main_c.contains("v_Empty"));
    assert!(main_c.contains("lsaca_release_Shape("));
    assert!(main_c.contains("case 0: {"));
}

const UNIT_ENUM: &str = "enum Flag {\n    On,\n    Off,\n}\n\nfn main() {\n    let f = Flag::On;\n    println!(\"{}\", 1);\n}\n";

#[test]
fn omits_union_for_payload_free_enums() {
    let (main_c, _) = generate_source("flags", UNIT_ENUM, true);
    assert!(main_c.contains("struct LsFlag {"));
    assert!(main_c.contains("int32_t tag;"));
    assert!(!main_c.contains("union {"));
}

const VEC_STRING: &str = "fn main() {\n    let mut names: Vec<String> = Vec::new();\n    names.push(\"a\");\n    println!(\"{}\", names.len());\n}\n";

#[test]
fn does_not_redefine_runtime_provided_types() {
    let (main_c, _) = generate_source("frozen", VEC_STRING, true);
    assert!(!main_c.contains("struct LsVec_String {"));
    assert!(!main_c.contains("struct LsOpt_String {"));
    assert!(!main_c.contains("struct LsRes_String_String {"));
    assert!(main_c.contains("lsaca_copy_Vec_String("));
    assert!(main_c.contains("lsaca_debug_Vec_String("));
    assert!(main_c.contains("ls_vec_string_push("));
}

const ARITH: &str = "fn main() {\n    let a = 1;\n    let b = 2;\n    let c = a + b;\n    let d = a / b;\n    println!(\"{} {}\", c, d);\n}\n";

#[test]
fn debug_checks_emit_checked_arithmetic_with_locations() {
    let (main_c, _) = generate_source("arith", ARITH, true);
    assert!(main_c.contains("ls_add_i64_checked("));
    assert!(main_c.contains("\"arith.ls\""));
}

#[test]
fn release_builds_wrap_addition_but_keep_division_checked() {
    let (main_c, _) = generate_source("arith", ARITH, false);
    assert!(!main_c.contains("ls_add_i64_checked("));
    assert!(main_c.contains("ls_div_i64_checked("));
}

#[test]
fn generation_is_deterministic() {
    let source = "struct Pair {\n    a: i64,\n    b: String,\n}\n\nfn main() {\n    let p = Pair { a: 1, b: \"x\" };\n    println!(\"{} {}\", p.a, p.b);\n}\n";
    let (first_c, first_cmake) = generate_source("determinism", source, true);
    let (second_c, second_cmake) = generate_source("determinism", source, true);
    assert_eq!(first_c, second_c);
    assert_eq!(first_cmake, second_cmake);
    assert!(!first_c.is_empty());
}

const CLOSURES: &str = "fn apply_twice(f: fn(i64) -> i64, value: i64) -> i64 {\n    f(f(value))\n}\n\nfn main() {\n    let base = 10;\n    let add_base = |x: i64| x + base;\n    println!(\"{}\", apply_twice(add_base, 3));\n}\n";

#[test]
fn emits_closure_environment_support() {
    let (main_c, _) = generate_source("closures", CLOSURES, true);
    assert!(!main_c.contains("LsEnvHeader"));
    assert!(main_c.contains("lsaca_env_retain("));
    assert!(main_c.contains("lsaca_env_release("));
    assert!(main_c.contains("lsaca_env_drop_"));
    assert!(main_c.contains("lsaca_trace_env_"));
    assert!(main_c.contains("lsaca_closure_"));
    assert!(main_c.contains("ls_alloc_managed(sizeof(LsEnv_"));
    assert!(main_c.contains("fn_ptr"));
    assert!(main_c.contains("lsaca_thunk_"));
}

const FORMAT: &str = "fn main() {\n    let x = 7;\n    println!(\"x = {}, {:?}\", x, \"hi\");\n}\n";

#[test]
fn lowers_formatting_and_printing() {
    let (main_c, _) = generate_source("format", FORMAT, true);
    assert!(main_c.contains("ls_format_begin("));
    assert!(main_c.contains("ls_format_push_cstr("));
    assert!(main_c.contains("ls_fmt_i64("));
    assert!(main_c.contains("ls_fmt_debug_string("));
    assert!(main_c.contains("ls_println("));
    assert!(main_c.contains("ls_release("));
}

#[test]
fn lowers_panics_and_assertions() {
    let (panic_c, _) = generate_source("panics", "fn main() {\n    panic!(\"boom\");\n}\n", true);
    assert!(panic_c.contains("ls_panic_at("));
    assert!(panic_c.contains("boom"));

    let (assert_c, _) = generate_source("asserts", "fn main() {\n    assert!(1 == 2);\n}\n", true);
    assert!(assert_c.contains("assertion failed"));
    assert!(assert_c.contains("ls_panic_cstr("));

    let (eq_c, _) = generate_source("assert_eq", "fn main() {\n    assert_eq!(1, 2);\n}\n", true);
    assert!(eq_c.contains("left == right"));
    assert!(eq_c.contains("left:"));
    assert!(eq_c.contains("right:"));
}

#[test]
fn lowers_assert_ne_with_its_own_diagnostic() {
    // `assert_ne!` reuses the equality expression, but panics when the operands
    // compare equal: the guard must not carry the extra logical negation that
    // `assert_eq!` keeps for byte-identical output.
    let (ne_c, _) = generate_source(
        "assert_ne",
        "fn main() {\n    let a = 1;\n    let b = 2;\n    assert_ne!(a, b);\n}\n",
        true,
    );
    assert!(
        ne_c.contains("left != right"),
        "default assert_ne message missing:\n{ne_c}"
    );
    let ne_body = main_function_body(&ne_c);
    let ne_condition = panic_condition_line(ne_body);
    assert!(
        ne_condition.trim_start().starts_with("if ("),
        "assert_ne guard is not a positive equality test: {ne_condition}"
    );
    assert!(
        !ne_condition.trim_start().starts_with("if (!"),
        "assert_ne guard must not be negated: {ne_condition}"
    );
    assert!(
        ne_condition.contains("=="),
        "assert_ne guard does not compare for equality: {ne_condition}"
    );
    // The `assert_eq!` guard keeps its historical negated shape.
    let (eq_c, _) = generate_source(
        "assert_eq_guard",
        "fn main() {\n    assert_eq!(1, 2);\n}\n",
        true,
    );
    let eq_body = main_function_body(&eq_c);
    let eq_condition = panic_condition_line(eq_body);
    assert!(
        eq_condition.trim_start().starts_with("if (!"),
        "assert_eq guard lost its negated shape: {eq_condition}"
    );
    // Both macros accept the optional custom format message, which replaces the
    // default left/right diagnostic.
    let (custom_ne_c, _) = generate_source(
        "assert_ne_message",
        "fn main() {\n    let a = 1;\n    assert_ne!(a, a + 1, \"numbers must differ\");\n}\n",
        true,
    );
    assert!(
        custom_ne_c.contains("numbers must differ"),
        "custom assert_ne message missing:\n{custom_ne_c}"
    );
    assert!(
        !custom_ne_c.contains("left != right"),
        "custom assert_ne message should replace the default diagnostic:\n{custom_ne_c}"
    );
}

const VEC: &str =
    "fn main() {\n    let v = vec![10, 20, 30];\n    let x = v[1];\n    println!(\"{}\", x);\n}\n";

#[test]
fn emits_vec_storage_and_bounds_checks() {
    let (main_c, _) = generate_source("vecs", VEC, true);
    assert!(main_c.contains("struct LsVec_i64 {"));
    assert!(main_c.contains("ls_vec_alloc("));
    assert!(main_c.contains("ls_panic_bounds("));
    assert!(main_c.contains("lsaca_release_Vec_i64("));
}

const VEC_CAPACITY: &str = r#"
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let mut numbers: Vec<i64> = Vec::with_capacity(8);
    numbers.push(1);
    let mut points: Vec<Point> = Vec::with_capacity(4);
    points.push(Point { x: 1, y: 2 });
    let mut names: Vec<String> = Vec::with_capacity(2);
    names.push(String::from("hi"));
    println!("{} {} {}", numbers.len(), points.len(), names.len());
}
"#;

#[test]
fn capacity_constructor_casts_to_the_element_pointer_type() {
    let (main_c, _) = generate_source("capacity", VEC_CAPACITY, true);
    assert!(main_c.contains("(int64_t*)ls_vec_alloc("));
    assert!(main_c.contains("(LsPoint*)ls_vec_alloc("));
    // A String vector uses the frozen shared representation and its helper.
    assert!(main_c.contains("ls_vec_string_with_capacity("));
    // The negative capacity guard panics instead of allocating.
    assert!(main_c.contains("capacity overflow"));
    // Regression guard: the cast used to drop the pointer star, which does not
    // compile (a scalar cast to a struct type, an integer assigned to a pointer).
    assert!(!main_c.contains("(int64_t)ls_vec_alloc("));
    assert!(!main_c.contains("(LsPoint)ls_vec_alloc("));
}

#[test]
fn omits_location_information_when_disabled() {
    let frontend = Frontend::new("noloc.ls", ARITH);
    let program = frontend.check().expect("test program should be valid");
    let options = CodegenOptions {
        program_name: "noloc".to_string(),
        debug_checks: true,
        emit_location_info: false,
        sources: Some(frontend.sources().clone()),
        ..CodegenOptions::default()
    };
    let main_c = generate(&program, &options).main_c;
    assert!(main_c.contains("ls_add_i64_checked("));
    assert!(main_c.contains("NULL, 0, 0"));
    assert!(!main_c.contains("\"noloc.ls\""));
}

const RC_CELLS: &str = r#"
struct Node {
    value: i64,
    next: Option<Rc<Node>>,
}

fn main() {
    let a = Rc::new(Node { value: 1, next: None });
    let b = Rc::new(Node { value: 2, next: None });
    let a2 = a.clone();
    b.set(Node { value: 2, next: Some(a2) });
    a.set(Node { value: 1, next: Some(b) });
    println!("{}", a.get().value);
}
"#;

#[test]
fn emits_rc_cells_and_cycle_tracing() {
    let (main_c, _) = generate_source("rc_cells", RC_CELLS, true);
    // A reference counted cell is a managed allocation that knows how to drop
    // and how to trace its payload.
    assert!(main_c.contains("typedef struct LsRc_Node LsRc_Node;"));
    assert!(main_c.contains("struct LsRc_Node {"));
    assert!(main_c.contains("LsNode payload;"));
    assert!(
        main_c.contains(
            "ls_alloc_managed(sizeof(LsRc_Node), lsaca_rc_drop_Node, lsaca_rc_trace_Node)"
        )
    );
    // Copying and releasing an Rc handle retains and releases the cell.
    assert!(main_c.contains("(LsRc_Node*)ls_retain("));
    assert!(main_c.contains("ls_release("));
    // The generated tracing functions visit the managed handles stored in a
    // value, which is what the trial-deletion collector walks.
    assert!(main_c.contains("void lsaca_trace_Node("));
    assert!(main_c.contains("void lsaca_trace_Opt_Rc_Node("));
    assert!(main_c.contains("void lsaca_rc_trace_Node("));
    assert!(main_c.contains("visit((void*)("));
    // Rc::new / get / set / strong_count helpers exist.
    assert!(main_c.contains("LsRc_Node* lsaca_rc_new_Node("));
    assert!(main_c.contains("LsNode lsaca_rc_get_Node("));
    assert!(main_c.contains("void lsaca_rc_set_Node("));
}

/// Return the end of the generated call line and the last argument passed to
/// the helper: the first argument of a call like "lsaca_rc_set_i64(v0, t1);"
/// or of "t2 = (LsRc_i64*)lsaca_rc_new_i64(t3);".
fn last_call_argument(main_c: &str, marker: &str) -> (usize, String) {
    let call = main_c
        .find(marker)
        .unwrap_or_else(|| panic!("generated C does not contain {}", marker));
    let line_start = main_c[..call]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let line_end = main_c[call..].find(';').map(|index| call + index).unwrap();
    let line = &main_c[line_start..=line_end];
    let open = line.find('(').expect("call has arguments");
    let close = line.rfind(')').expect("call has arguments");
    let arguments = &line[open + 1..close];
    let last = arguments
        .rsplit(',')
        .next()
        .unwrap_or(arguments)
        .trim()
        .to_string();
    (line_end, last)
}

#[test]
fn does_not_release_ownership_of_moved_rc_arguments() {
    let source = r#"
fn main() {
    let a = Rc::new(String::from("a"));
    a.set(String::from("b"));
    println!("{}", a.get());
}
"#;
    let (main_c, _) = generate_source("rc_moves", source, true);

    // Rc::new takes ownership of its payload, so the temporary passed to the
    // helper must not be released again after the cell stored it.
    let (new_end, new_argument) = last_call_argument(&main_c, "lsaca_rc_new_String(");
    let release_new = format!("ls_release({});", new_argument);
    assert!(
        !main_c[new_end..].contains(&release_new),
        "the payload passed to Rc::new is released after being moved into the cell"
    );

    // Rc::set releases the old payload itself and takes ownership of the new
    // one, so the argument must not be released by the caller either.
    let (set_end, set_argument) = last_call_argument(&main_c, "lsaca_rc_set_String(");
    let release_set = format!("ls_release({});", set_argument);
    assert!(
        !main_c[set_end..].contains(&release_set),
        "the payload passed to Rc::set is released after being moved into the cell"
    );
}

const BOX_PROGRAM: &str = r#"
struct Node {
    value: i64,
    next: Option<Box<Node>>,
}

fn main() {
    let a = Box::new(10);
    let mut b = a.clone();
    b.set(20);
    println!("{} {}", a.get(), b.get());
    println!("{}", b == Box::new(20));
    let chain = Node {
        value: 1,
        next: Some(Box::new(Node { value: 2, next: None })),
    };
    println!("{}", chain.value);
}
"#;

#[test]
fn emits_box_allocation_copy_and_helpers() {
    let (main_c, _) = generate_source("box_fixture", BOX_PROGRAM, true);

    // Every box is one managed allocation that the cycle collector traces
    // through, like the Rc<T> cells.
    assert!(main_c.contains("typedef struct LsBox_i64 LsBox_i64;"));
    assert!(main_c.contains("struct LsBox_i64 {"));
    assert!(main_c.contains("int64_t payload;"));
    assert!(
        main_c.contains(
            "ls_alloc_managed(sizeof(LsBox_i64), lsaca_box_drop_i64, lsaca_box_trace_i64)"
        )
    );
    // new/get/set helpers and the deep copy that gives Box value semantics.
    assert!(main_c.contains("LsBox_i64* lsaca_box_new_i64("));
    assert!(main_c.contains("int64_t lsaca_box_get_i64("));
    assert!(main_c.contains("void lsaca_box_set_i64("));
    assert!(main_c.contains("LsBox_i64* lsaca_copy_Box_i64("));
    // Recursive struct fields use a pointer to the box cell.
    assert!(main_c.contains("LsBox_Node*"));
    assert!(main_c.contains("visit((void*)("));
}

const FORMAT_SPEC_PROGRAM: &str = r#"
fn main() {
    let name = String::from("lsaca");
    println!("[{:*>8}]", name);
    println!("[{:08.2}]", 3.14159);
    println!("[{:.3}]", 2.0 / 3.0);
    println!("[{:^20}]", 7);
    println!("[{:.2}]", name);
}
"#;

#[test]
fn emits_width_and_precision_format_helpers() {
    let (main_c, _) = generate_source("format_spec_fixture", FORMAT_SPEC_PROGRAM, true);

    // Widths and alignment are delegated to the runtime, which counts
    // codepoints and inserts the fill character.
    assert!(main_c.contains("ls_fmt_apply_width("));
    assert!(main_c.contains("LS_ALIGN_CENTER"));
    assert!(main_c.contains("ls_format_begin(0)"));
    // Precision goes through the dedicated formatters.
    assert!(main_c.contains("ls_fmt_f64_prec("));
    assert!(main_c.contains("ls_fmt_str_prec("));
    // The temporary piece is released by the generated code, so formatted
    // output does not leak reference counted allocations.
    let mut formatted = 0;
    for line in main_c.lines() {
        if let Some(index) = line.find(" = ls_format_begin(0);") {
            let name = line[..index].trim();
            let released = main_c.contains(&format!("ls_release((void*)({}));", name))
                || main_c.contains(&format!("ls_release({});", name));
            assert!(released, "the format buffer {} is not released", name);
            formatted += 1;
        }
    }
    // Two holes need a temporary piece (the two width specs); the precision
    // only holes append directly to the accumulator.
    assert!(formatted >= 2, "no formatted buffers were generated");
}

/// Slice of the generated project between the user main function and the C
/// entry point wrapper.
fn main_function_body(main_c: &str) -> &str {
    let start = main_c
        .rfind("LsUnit lsaca_fn_main()")
        .expect("the user main function is not emitted");
    let end = main_c[start..]
        .find("\nint main(int argc, char** argv)")
        .map(|offset| start + offset)
        .unwrap_or(main_c.len());
    &main_c[start..end]
}

/// Slice of the emitted user main function only: its definition and body
/// without the functions emitted after it.
fn user_main_body(main_c: &str) -> &str {
    let body = main_function_body(main_c);
    match body.find("\n}\n") {
        Some(end) => &body[..end + 3],
        None => body,
    }
}

/// The `if (...)` guard of the first default-diagnostic panic in `body`.
///
/// `assert_eq!`/`assert_ne!` build their default diagnostic with
/// `ls_format_begin`, so the guard is the last `if` line before it.
fn panic_condition_line(body: &str) -> &str {
    let panic_at = body
        .find("ls_format_begin(0)")
        .expect("expected a default diagnostic panic");
    body[..panic_at]
        .lines()
        .rev()
        .find(|line| line.trim_start().starts_with("if ("))
        .expect("expected a guarded panic")
}

const MOVE_BIND_PROGRAM: &str = r#"
fn main() {
    let mut values: Vec<i64> = Vec::with_capacity(4);
    values.push(1);
    let mut text = String::new();
    text.push_str("x");
    println!("{} {}", values.len(), text.len());
}
"#;

#[test]
fn moves_fresh_values_into_locals_instead_of_copying() {
    let (main_c, _) = generate_source("move_bind_fixture", MOVE_BIND_PROGRAM, true);
    let body = main_function_body(&main_c);

    // Vec::with_capacity initialises the local in place: no deep copy of the
    // freshly allocated buffer is made when the local is bound.
    assert!(
        body.contains(".data = (int64_t*)ls_vec_alloc("),
        "the vector buffer is not initialised in place:\n{body}"
    );
    assert!(
        !body.contains("lsaca_copy_Vec_i64("),
        "binding a fresh vector copied it:\n{body}"
    );
    // A freshly created String is assigned directly instead of being cloned
    // and released again.
    assert!(
        !body.contains(" = ls_str_clone("),
        "binding a fresh String cloned it:\n{body}"
    );
}

const WILDCARD_LET_PROGRAM: &str = r#"
fn main() {
    let item = 5;
    let _ = item;
    let text = String::from("hello");
    let _ = text;
    let _ = String::from("scratch");
    println!("{} {}", item, text);
}
"#;

#[test]
fn discards_wildcard_initialisers_without_leftover_temporaries() {
    let (main_c, _) = generate_source("wildcard_let_fixture", WILDCARD_LET_PROGRAM, true);
    let body = main_function_body(&main_c);

    // `let _ = <place>;` reads the place without moving out of it: the owner
    // keeps its value and the wildcard emits a discard, not a temporary.
    assert!(
        body.contains("(void)(v1);"),
        "the wildcard read of the scalar local was not emitted:\n{body}"
    );
    assert!(
        body.contains("(void)(v2);"),
        "the wildcard read of the String local was not emitted:\n{body}"
    );
    assert!(
        !body.contains(" = v1;") && !body.contains(" = v2;"),
        "a wildcard read materialised a copy of the place:\n{body}"
    );

    // `let _ = <owned temporary>;` still evaluates the initialiser (its side
    // effects must run) and releases the value exactly once. A leftover
    // set-but-unused temporary would break `-Werror` builds.
    let mut discarded = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("(void)(") else {
            continue;
        };
        let Some(name) = rest.strip_suffix(");") else {
            continue;
        };
        if name.starts_with('t') && name[1..].chars().all(|ch| ch.is_ascii_digit()) {
            discarded.push(name.to_string());
        }
    }
    assert_eq!(
        discarded.len(),
        1,
        "expected exactly one discarded temporary:\n{body}"
    );
    let temporary = &discarded[0];
    let release = format!("ls_release((void*)({}));", temporary);
    assert_eq!(
        body.matches(release.as_str()).count(),
        1,
        "the discarded temporary must be released exactly once:\n{body}"
    );
}

const FOR_VEC_IN_PLACE_PROGRAM: &str = r#"
fn main() {
    let mut values: Vec<i64> = Vec::new();
    values.push(1);
    values.push(2);
    let mut sum = 0;
    for value in values {
        sum += value;
    }
    println!("{}", sum);
}
"#;

#[test]
fn iterates_a_local_vector_in_place() {
    let (main_c, _) = generate_source("for_vec_in_place_fixture", FOR_VEC_IN_PLACE_PROGRAM, true);
    let body = main_function_body(&main_c);

    // for ... in vec consumes the vector: its value is moved into the loop
    // temporary and the source local is cleared, so the loop can iterate the
    // owned buffer without copying it.
    assert!(
        !body.contains("lsaca_copy_Vec_i64("),
        "the loop snapshotted the vector:\n{body}"
    );
    assert!(
        body.contains("= ((LsVec_i64){0});"),
        "the consumed vector local is not cleared:\n{body}"
    );
    assert!(
        body.contains(".len) break;"),
        "the loop does not read the consumed vector length:\n{body}"
    );
    assert!(
        body.contains(").data[i"),
        "the loop does not index the vector buffer directly:\n{body}"
    );
    assert!(
        body.contains(").len = 0;"),
        "the exhausted vector is not emptied after the loop:\n{body}"
    );
    assert!(
        body.contains("_live = false;"),
        "the moved vector local keeps its live flag:\n{body}"
    );
}

const FOR_VEC_TRANSFER_PROGRAM: &str = r#"
fn main() {
    let mut names: Vec<String> = Vec::new();
    names.push(String::from("a"));
    names.push(String::from("b"));
    let mut total = String::new();
    for name in names {
        total.push_str(name);
    }
    println!("{}", total);
}
"#;

#[test]
fn transfers_elements_out_of_a_consumed_vector() {
    let (main_c, _) = generate_source("for_vec_transfer_fixture", FOR_VEC_TRANSFER_PROGRAM, true);
    let body = main_function_body(&main_c);

    // Each element is moved out of the buffer slot and the slot is cleared, so
    // the vector teardown cannot release the element a second time.
    assert!(
        body.contains(").data[i"),
        "the loop does not index the consumed vector buffer:\n{body}"
    );
    assert!(
        body.contains("= NULL;"),
        "the transferred element slot is not cleared:\n{body}"
    );
    assert!(
        body.contains(").len = 0;"),
        "the exhausted vector is not emptied after the loop:\n{body}"
    );
    // No snapshot copy of the vector is taken anymore.
    assert!(
        !body.contains("lsaca_copy_Vec_String("),
        "the loop snapshotted the vector:\n{body}"
    );
    // Nothing is left to release, so no break cleanup loop is emitted.
    assert!(
        !body.contains("+ 1; k"),
        "a cleanup loop was emitted for a loop without break:\n{body}"
    );
}

const FOR_VEC_BREAK_PROGRAM: &str = r#"
fn main() {
    let mut names: Vec<String> = Vec::new();
    names.push(String::from("a"));
    names.push(String::from("b"));
    names.push(String::from("c"));
    for name in names {
        println!("{}", name);
        break;
    }
}
"#;

#[test]
fn releases_the_remaining_elements_when_a_vector_loop_breaks() {
    let (main_c, _) = generate_source("for_vec_break_fixture", FOR_VEC_BREAK_PROGRAM, true);
    let body = main_function_body(&main_c);

    // A break leaves the elements after the current slot untransferred. A
    // cleanup loop releases them (starting after the current element, whose
    // slot was already cleared) before the vector buffer itself is freed.
    assert!(
        body.contains("+ 1; k"),
        "the break path does not release the tail elements:\n{body}"
    );
    assert!(
        body.contains(".len; k"),
        "the cleanup loop does not walk the remembered length:\n{body}"
    );
    assert!(
        body.contains(").len = 0;"),
        "the exhausted vector is not emptied after the loop:\n{body}"
    );
}

const VEC_PUSH_PROGRAM: &str = r#"
fn main() {
    let mut values: Vec<i64> = Vec::new();
    values.push(7);
    println!("{}", values.len());
}
"#;

#[test]
fn emits_inline_vec_push_fast_path_and_by_value_slow_helper() {
    let (main_c, _) = generate_source("vec_push_fixture", VEC_PUSH_PROGRAM, true);
    let body = main_function_body(&main_c);

    // The fast path appends into spare capacity without taking the address of
    // the vector, so the length and capacity can stay in registers.
    assert!(
        body.contains("= (v1).len;"),
        "the push does not read the length into a local:\n{body}"
    );
    assert!(
        body.contains("if (len") && body.contains("< (v1).cap) {"),
        "the push has no inline capacity fast path:\n{body}"
    );
    assert!(
        body.contains("(v1).data[len"),
        "the fast path does not store into the buffer:\n{body}"
    );
    // Growing is delegated to a helper that takes the vector by value.
    assert!(
        body.contains("(v1) = lsaca_vec_push_slow_Vec_i64((v1),"),
        "the growing path does not use the by-value helper:\n{body}"
    );
    assert!(
        main_c.contains("LsVec_i64 lsaca_vec_push_slow_Vec_i64(LsVec_i64 v, int64_t value);"),
        "the by-value push helper is not declared"
    );
}

const FORMAT_INTO_PUSH_PROGRAM: &str = r#"
fn main() {
    let mut text = String::new();
    text.push_str(format!("{},", 7));
    println!("{}", text.len());
}
"#;

#[test]
fn fuses_format_into_a_push_on_a_local_string() {
    let (main_c, _) = generate_source("format_push_fixture", FORMAT_INTO_PUSH_PROGRAM, true);
    let body = main_function_body(&main_c);

    // The formatted argument is appended straight to the receiver instead of
    // allocating a temporary format buffer.
    assert!(
        !body.contains("ls_str_push_str("),
        "the formatted push still goes through a temporary String:\n{body}"
    );
    let push_line = body
        .lines()
        .find(|line| line.contains(" = ls_format_push_cstr("))
        .expect("no literal append to the receiver");
    let receiver = push_line
        .split(" = ")
        .next()
        .expect("malformed append line")
        .trim();
    assert!(
        body.contains(&format!("{} = ls_fmt_i64({}, ", receiver, receiver)),
        "the formatted argument is not appended to the receiver:\n{body}"
    );
    assert!(
        body.contains(&format!("ls_format_push_cstr({}, {:?})", receiver, ",")),
        "the literal part of the format is not appended to the receiver:\n{body}"
    );
    // The only remaining format buffer is the one built by println!. The
    // standard-library forwarder emitted after main has its own unreachable
    // panic path, so only the user main body is inspected.
    let main_only = user_main_body(&main_c);
    assert_eq!(
        main_only.matches("ls_format_begin(0)").count(),
        1,
        "a temporary format buffer was created for the push:\n{body}"
    );
}

const TRIVIAL_ELEMENT_PROGRAM: &str = r#"
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let mut points: Vec<Point> = Vec::with_capacity(2);
    points.push(Point { x: 1, y: 2 });
    points.push(Point { x: 3, y: 4 });
    let mut total = 0;
    for point in points {
        total += point.x + point.y;
    }
    println!("{}", total);
}
"#;

#[test]
fn trivially_copyable_elements_skip_deep_copy_and_release_loops() {
    let (main_c, _) = generate_source("trivial_elements_fixture", TRIVIAL_ELEMENT_PROGRAM, true);
    let body = main_function_body(&main_c);

    // Releasing a Point value is a no-op: it only owns scalars.
    assert!(
        main_c.contains("void lsaca_release_Point(LsPoint v) {\n  (void)v;\n}"),
        "the Point release helper is not a no-op"
    );
    // Releasing a vector of Points only frees the raw buffer.
    assert!(
        main_c.contains(
            "void lsaca_release_Vec_Point(LsVec_Point v) {\n  ls_vec_free_bytes((void*)v.data);\n}"
        ),
        "the Vec<Point> release helper still walks the elements"
    );
    // Copying the vector clones the buffer in one call.
    assert!(
        main_c.contains(
            "out.data = (LsPoint*)ls_vec_clone_bytes((const void*)v.data, v.len, sizeof(LsPoint));"
        ),
        "the Vec<Point> copy helper does not clone the buffer"
    );
    // Iterating the vector needs no per-element copies either.
    assert!(
        !body.contains("lsaca_copy_Vec_Point("),
        "the loop copied the vector of trivially copyable elements:\n{body}"
    );
}

const DROP_HOOK_PROGRAM: &str = r#"
struct Holder {
    name: String,
}

impl Drop for Holder {
    fn drop(&mut self) {
        println!("dropping {}", self.name);
    }
}

fn main() {
    let holder = Holder { name: String::from("h") };
    println!("{}", holder.name);
}
"#;

#[test]
fn releases_through_drop_hooks_before_owning_fields() {
    let (main_c, _) = generate_source("drop_hook_fixture", DROP_HOOK_PROGRAM, true);

    // The release helper runs the user hook first and only then releases the
    // owning fields, so the hook observes a fully initialised value.
    let helper = "void lsaca_release_Holder(LsHolder v) {";
    let start = main_c
        .find(helper)
        .unwrap_or_else(|| panic!("the Holder release helper is missing:\n{main_c}"));
    let tail = &main_c[start..];
    let end = tail
        .find("\n}")
        .expect("the release helper is unterminated");
    let body = &tail[..end];
    let hook = body
        .find("lsaca_fn_Holder_drop(&v);")
        .unwrap_or_else(|| panic!("the drop hook is not called:\n{body}"));
    let field = body
        .find("ls_release((void*)(v.f_name));")
        .unwrap_or_else(|| panic!("the owning field is not released:\n{body}"));
    assert!(
        hook < field,
        "the drop hook must run before the owning fields are released:\n{body}"
    );
    assert!(
        main_c.contains("LsUnit lsaca_fn_Holder_drop(LsHolder* p0);"),
        "the drop hook is not declared"
    );
    // The local is released through its live flag at the end of main.
    assert!(
        main_c.contains("if (v1_live) {") && main_c.contains("lsaca_release_Holder(v1);"),
        "the local holder is not released through its live flag:\n{main_c}"
    );
}

const ENUM_DROP_HOOK_PROGRAM: &str = r#"
enum Shape {
    Circle(i64),
    Empty,
}

impl Drop for Shape {
    fn drop(&mut self) {
        println!("drop shape");
    }
}

fn main() {
    let circle = Shape::Circle(3);
    match circle {
        Shape::Circle(radius) => println!("{}", radius),
        Shape::Empty => println!("empty"),
    }
}
"#;

#[test]
fn releases_enums_through_drop_hooks_before_payloads() {
    let (main_c, _) = generate_source("enum_drop_hook_fixture", ENUM_DROP_HOOK_PROGRAM, true);

    assert!(
        main_c.contains(
            "void lsaca_release_Shape(LsShape v) {\n  lsaca_fn_Shape_drop(&v);\n  switch (v.tag) {"
        ),
        "the enum drop hook does not run before the payload switch:\n{main_c}"
    );
    assert!(
        main_c.contains("LsUnit lsaca_fn_Shape_drop(LsShape* p0);"),
        "the enum drop hook is not declared"
    );
}

const MOVE_BINDING_PROGRAM: &str = r#"
struct Holder {
    name: String,
}

fn main() {
    let holder = Holder { name: String::from("h") };
    let other = holder;
    println!("{}", other.name);
}
"#;

#[test]
fn moves_owned_bindings_and_clears_the_source() {
    let (main_c, _) = generate_source("move_binding_fixture", MOVE_BINDING_PROGRAM, true);
    let body = main_function_body(&main_c);

    // Both owned locals are predeclared zeroed with a live flag.
    assert!(
        body.contains("bool v1_live = false;"),
        "the moved-from local has no live flag:\n{body}"
    );
    assert!(
        body.contains("bool v2_live = false;"),
        "the moved-to local has no live flag:\n{body}"
    );
    // A move transfers the value and clears the source; no deep copy is made.
    assert!(
        !body.contains("lsaca_copy_Holder("),
        "binding by move copied the value:\n{body}"
    );
    assert!(
        body.contains("v1 = ((LsHolder){0});"),
        "the moved-from local is not cleared:\n{body}"
    );
    assert!(
        body.contains("v1_live = false;"),
        "the moved-from local keeps its live flag:\n{body}"
    );
    // Cleanup is guarded by liveness so the value is released exactly once.
    assert!(
        body.contains("if (v2_live) {"),
        "the moved-to local is not released through its live flag:\n{body}"
    );
}

const MATCH_DROP_PAYLOAD_PROGRAM: &str = r#"
struct Loud {
    value: i64,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("dropped {}", self.value);
    }
}

enum Holder {
    Filled(Loud),
    Empty,
}

fn main() {
    let first = Holder::Filled(Loud { value: 11 });
    let second = first;
    match second {
        Holder::Filled(item) => { println!("got {}", item.value); }
        Holder::Empty => { println!("empty"); }
    }
    println!("done");
}
"#;

#[test]
fn moves_drop_hook_payloads_out_of_matched_enums() {
    let (main_c, _) = generate_source("drop_payload_match", MATCH_DROP_PAYLOAD_PROGRAM, true);
    let body = main_function_body(&main_c);
    assert_drop_payload_move(body, ".u.v_Filled.f0");
}

const OPTION_DROP_PAYLOAD_PROGRAM: &str = r#"
struct Loud {
    value: i64,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("dropped {}", self.value);
    }
}

fn main() {
    let holder: Option<Loud> = Some(Loud { value: 7 });
    match holder {
        Some(item) => { println!("got {}", item.value); }
        None => { println!("none"); }
    }
    println!("done");
}
"#;

#[test]
fn moves_drop_hook_payloads_out_of_matched_options() {
    let (main_c, _) = generate_source("drop_payload_option", OPTION_DROP_PAYLOAD_PROGRAM, true);
    let body = main_function_body(&main_c);
    assert_drop_payload_move(body, ".u.some");
}

const IF_LET_DROP_PAYLOAD_PROGRAM: &str = r#"
struct Loud {
    value: i64,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("dropped {}", self.value);
    }
}

enum Holder {
    Filled(Loud),
    Empty,
}

fn main() {
    let holder = Holder::Filled(Loud { value: 7 });
    if let Holder::Filled(item) = holder {
        println!("got {}", item.value);
    }
    println!("done");
}
"#;

#[test]
fn moves_drop_hook_payloads_out_of_if_let_bindings() {
    let (main_c, _) = generate_source("drop_payload_if_let", IF_LET_DROP_PAYLOAD_PROGRAM, true);
    let body = main_function_body(&main_c);
    assert_drop_payload_move(body, ".u.v_Filled.f0");
}

const MANAGED_DROP_PAYLOAD_PROGRAM: &str = r#"
struct Loud {
    name: String,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("dropped {}", self.name);
    }
}

enum Holder {
    Filled(Loud),
    Empty,
}

fn main() {
    let holder = Holder::Filled(Loud { name: String::from("x") });
    match holder {
        Holder::Filled(item) => { println!("got {}", item.name); }
        Holder::Empty => { println!("empty"); }
    }
    println!("done");
}
"#;

#[test]
fn moves_drop_hook_payloads_with_managed_fields() {
    let (main_c, _) = generate_source("drop_payload_managed", MANAGED_DROP_PAYLOAD_PROGRAM, true);
    let body = main_function_body(&main_c);
    assert_drop_payload_move(body, ".u.v_Filled.f0");
}

const GUARDED_DROP_PAYLOAD_PROGRAM: &str = r#"
struct Loud {
    value: i64,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("dropped {}", self.value);
    }
}

enum Holder {
    Filled(Loud),
    Empty,
}

fn main() {
    let holder = Holder::Filled(Loud { value: 7 });
    match holder {
        Holder::Filled(item) if item.value > 3 => { println!("big {}", item.value); }
        Holder::Filled(item) => { println!("small {}", item.value); }
        Holder::Empty => { println!("empty"); }
    }
    println!("done");
}
"#;

#[test]
fn moves_drop_hook_payloads_out_of_guarded_arms() {
    let (main_c, _) = generate_source("drop_payload_guard", GUARDED_DROP_PAYLOAD_PROGRAM, true);
    let body = main_function_body(&main_c);
    assert_drop_payload_move(body, ".u.v_Filled.f0");
    // The guard reads a shallow alias first and the payload is stolen only
    // after the guard accepted the arm, so the transfer appears twice while
    // the payload is dropped exactly once.
    assert!(
        body.matches(".u.v_Filled.f0;").count() >= 2,
        "the guarded arm does not bind before the guard and steal afterwards:\n{body}"
    );
}

/// Asserts that a by-value binding of a Drop-hook payload moves the payload
/// instead of deep-copying it.
///
/// `slot_field` is the payload place inside the scrutinee, for example
/// `.u.v_Filled.f0` or `.u.some`. The payload must be assigned to the binding
/// and then zeroed in the scrutinee, whose liveness flag is cleared so its
/// guarded release does not run the user Drop hook a second time.
fn assert_drop_payload_move(body: &str, slot_field: &str) {
    assert!(
        !body.contains("lsaca_copy_Loud("),
        "the Drop-hook payload was deep-copied instead of moved:\n{body}"
    );
    let transfer = format!("{slot_field};");
    let transfer_at = body
        .find(&transfer)
        .unwrap_or_else(|| panic!("the payload is not moved into the binding:\n{body}"));
    let line_start = body[..transfer_at]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let line_end = body[transfer_at..]
        .find('\n')
        .map(|index| transfer_at + index)
        .unwrap_or(body.len());
    let line = &body[line_start..line_end];
    assert!(
        line.contains(" = ") && !line.contains("lsaca_copy"),
        "the payload binding does not steal the value: {line}"
    );
    let zeroed = format!("{slot_field} = ((LsLoud){{0}});");
    let zeroed_at = body
        .find(&zeroed)
        .unwrap_or_else(|| panic!("the matched payload slot is not cleared:\n{body}"));
    assert!(
        transfer_at < zeroed_at,
        "the payload slot is cleared before the binding steals it:\n{body}"
    );
    assert!(
        body[zeroed_at..].contains("_live = false;"),
        "the matched scrutinee keeps its live flag, so its release would drop the payload again:\n{body}"
    );
}

const CLONE_PROGRAM: &str = r#"
struct Pair {
    a: i64,
    b: i64,
}

fn main() {
    let pair = Pair { a: 1, b: 2 };
    let copy = pair.clone();
    let number = 5;
    let same = number.clone();
    let text = String::from("x");
    let owned = text.clone();
    println!("{} {} {} {}", pair.a, copy.b, same, owned);
}
"#;

#[test]
fn clones_copy_receivers_by_assignment_and_managed_receivers_by_retain() {
    let (main_c, _) = generate_source("clone_fixture", CLONE_PROGRAM, true);
    let body = main_function_body(&main_c);

    // Copy receivers are copied with a plain assignment and no helper call.
    assert!(
        !body.contains("lsaca_copy_Pair("),
        "cloning a Copy struct called the deep copy helper:\n{body}"
    );
    assert!(
        body.contains("v2 = v1;"),
        "the Copy struct clone is not a plain assignment:\n{body}"
    );
    assert!(
        body.contains("v4 = v3;"),
        "the Copy scalar clone is not a plain assignment:\n{body}"
    );
    // The String clone retains the shared buffer instead of deep copying it.
    assert!(
        body.contains("v6 = ((LsString*)ls_retain((void*)(v5)));"),
        "the String clone does not retain the handle:\n{body}"
    );
    assert!(
        !body.contains("ls_str_clone(v5)"),
        "the String clone deep copied the bytes:\n{body}"
    );
}

const MOVE_OPTION_PROGRAM: &str = r#"
struct Point {
    x: i64,
    y: i64,
}

impl Drop for Point {
    fn drop(&mut self) {
        println!("drop {}", self.x);
    }
}

fn main() {
    let point = Point { x: 7, y: 0 };
    let option: Option<Point> = Some(point);
    let value = option.unwrap();
    println!("{}", value.x);
}
"#;

#[test]
fn moves_the_payload_out_of_a_consumed_option() {
    let (main_c, _) = generate_source("unwrap_fixture", MOVE_OPTION_PROGRAM, true);
    let body = main_function_body(&main_c);

    // The option is moved into a shell temp whose tag is checked first, then
    // the payload is moved straight out without a deep copy.
    assert!(
        body.contains("= ((LsOpt_Point){0});"),
        "the consumed option shell is not zeroed:\n{body}"
    );
    assert!(
        body.contains(").u.some;"),
        "the payload is not moved out of the shell:\n{body}"
    );
    assert!(
        !body.contains("lsaca_copy_Opt_Point("),
        "the consumed option was copied:\n{body}"
    );
    assert!(
        !body.contains("lsaca_copy_Point("),
        "the unwrapped payload was copied:\n{body}"
    );
    // The moved payload local is released through its live flag.
    assert!(
        body.contains("if (v3_live) {"),
        "the moved payload is not released through its live flag:\n{body}"
    );
}

const CLOSURE_CAPTURE_PROGRAM: &str = r#"
fn main() {
    let text = String::from("captured");
    let show = || {
        println!("{}", text);
    };
    show();
}
"#;

#[test]
fn closure_captures_move_owned_locals_into_the_environment() {
    let (main_c, _) = generate_source("closure_capture_fixture", CLOSURE_CAPTURE_PROGRAM, true);
    let body = main_function_body(&main_c);

    // The owned capture is moved into its environment slot and the local is
    // cleared, so the environment drop function releases it exactly once.
    assert!(
        body.contains("->c0 = v1;"),
        "the capture was not moved into the environment:\n{body}"
    );
    assert!(
        body.contains("v1 = NULL;"),
        "the moved capture local is not cleared:\n{body}"
    );
    assert!(
        body.contains("v1_live = false;"),
        "the moved capture local keeps its live flag:\n{body}"
    );
    assert!(
        main_c.contains("void lsaca_env_drop_0(void* self) {"),
        "the closure environment has no drop function"
    );
    assert!(
        main_c.contains("ls_release((void*)(env1->c0));"),
        "the closure environment does not release the capture"
    );
}

const RETURN_MOVE_PROGRAM: &str = r#"
fn pick(flag: bool, a: String, b: String) -> String {
    if flag {
        a
    } else {
        b
    }
}

fn main() {
    let value = pick(true, String::from("A"), String::from("B"));
    println!("{}", value);
}
"#;

#[test]
fn returns_tail_values_by_moving_them() {
    let (main_c, _) = generate_source("return_move_fixture", RETURN_MOVE_PROGRAM, true);
    let body = main_function_body(&main_c);

    // The call result is moved straight into the local's slot and becomes
    // live there, without an intermediate owned temporary.
    assert!(
        body.contains("= lsaca_fn_pick("),
        "the returned value is not moved into its local:\n{body}"
    );
    assert!(
        body.contains("v1_live = true;"),
        "the local receiving the returned value is not marked live:\n{body}"
    );

    // Both tail expressions move their parameter into the result slot and
    // clear the parameter, so the guarded parameter release is skipped.
    assert!(
        main_c.contains("result1 = p1;"),
        "the then tail does not move its parameter into the result:\n{main_c}"
    );
    assert!(
        main_c.contains("result1 = p2;"),
        "the else tail does not move its parameter into the result:\n{main_c}"
    );
    assert!(
        main_c.contains("p1 = NULL;"),
        "the moved tail parameter is not cleared:\n{main_c}"
    );
    assert!(
        main_c.contains("if (p1_live) {"),
        "the parameter release is not guarded by its live flag:\n{main_c}"
    );
}

const MUT_HEAP_REFS_PROGRAM: &str = r#"
fn append(text: &mut String, names: &mut Vec<String>) {
    text.push_str("!");
    names.push(String::from("x"));
}

fn main() {
    let mut text = String::from("hi");
    let mut names: Vec<String> = Vec::new();
    append(&mut text, &mut names);
    println!("{} {}", text, names.len());
}
"#;

#[test]
fn mutates_heap_values_through_mutable_references() {
    let (main_c, _) = generate_source("mut_heap_refs_fixture", MUT_HEAP_REFS_PROGRAM, true);

    assert!(
        main_c.contains("LsUnit lsaca_fn_append(LsString** p0, LsVec_String* p1);"),
        "the mutable reference parameters are not emitted as mutable pointers"
    );
    // Mutating methods write through the reference without copying or
    // releasing the referent. The receiver is loaded into a pointer temp that
    // is dereferenced in place, so the mutation reaches the caller's value.
    assert!(
        main_c.contains("= ls_str_push_str((*("),
        "the String mutation does not write through the reference:\n{main_c}"
    );
    assert!(
        main_c.contains("ls_vec_string_push(&((*("),
        "the Vec mutation does not write through the reference:\n{main_c}"
    );
    // The mutable reference parameters are mutated in place: the receiver is
    // dereferenced directly instead of being copied into a temporary.
    assert!(
        main_c.contains("(*(p0)) = ls_str_push_str("),
        "the String reference parameter is not mutated in place:\n{main_c}"
    );
}

const DROP_MATCH_PROGRAM: &str = r#"
struct Loud {
    name: String,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let first = Loud { name: String::from("a") };
    match first {
        value => {
            println!("bound {}", value.name);
        }
    }
    let second = Loud { name: String::from("b") };
    match second {
        value if value.name == "zzz" => {
            println!("never {}", value.name);
        }
        other => {
            println!("fallback {}", other.name);
        }
    }
}
"#;

#[test]
fn match_whole_value_bindings_move_from_the_scrutinee() {
    let (main_c, _) = generate_source("drop_match_fixture", DROP_MATCH_PROGRAM, true);
    let body = main_function_body(&main_c);

    // A whole-value by-value binding of a non-Copy scrutinee moves the value
    // out of the owning local: copying would release it twice and run a Drop
    // hook twice. This mirrors Rust's move semantics for pattern bindings.
    assert!(
        !body.contains("lsaca_copy_Loud("),
        "a whole-value match binding copied the value instead of moving it:\n{body}"
    );
    assert!(
        body.contains("_live = true;"),
        "the moved binding is not marked live:\n{body}"
    );
    assert!(
        body.contains("_live = false;"),
        "the moved-from scrutinee is not marked dead:\n{body}"
    );
    // The scrutinee value is owned by a local with a live flag, so every exit
    // (normal match end, guard restore, return, break) drops it exactly once.
    assert!(
        body.contains("if (t") && body.contains("lsaca_release_Loud(t"),
        "the scrutinee release is not guarded by its live flag:\n{body}"
    );
}

const INTERNED_LITERALS_PROGRAM: &str = r#"
fn main() {
    let first = "same";
    let second = "same";
    println!("{} {}", first, second);
}
"#;

#[test]
fn interns_string_literals_as_immortal_statics() {
    let (main_c, _) = generate_source("interned_literals_fixture", INTERNED_LITERALS_PROGRAM, true);
    let body = main_function_body(&main_c);

    // One static byte array plus one handle per *distinct* literal; every
    // occurrence shares the interned allocation.
    assert_eq!(
        main_c.matches("static const char lsaca_lit_data_").count(),
        1,
        "the repeated literal was not deduplicated:\n{main_c}"
    );
    assert!(
        main_c.contains("static const char lsaca_lit_data_0[] = \"same\";"),
        "the literal bytes are not emitted as a static array:\n{main_c}"
    );
    assert!(
        main_c.contains("static LsString* lsaca_lit_0 = NULL;"),
        "the literal handle is not declared:\n{main_c}"
    );
    assert!(
        main_c.contains("lsaca_lit_0 = ls_str_static(lsaca_lit_data_0, (size_t)4);"),
        "the literal is not interned with its explicit byte length:\n{main_c}"
    );
    assert!(
        body.contains("((LsString*)ls_retain((void*)(lsaca_lit_0)))"),
        "evaluating the literal does not retain the interned handle:\n{body}"
    );
    // The old allocation per evaluation is gone.
    assert!(
        !body.contains("ls_str_cstr(") && !body.contains("ls_str_from_parts("),
        "a string literal still allocates a fresh runtime string:\n{body}"
    );

    // The table must be declared before the first body that mentions it, and
    // initialised in the C wrapper after the runtime starts.
    let declaration = main_c
        .find("static LsString* lsaca_lit_0 = NULL;")
        .expect("literal handle declaration missing");
    let first_use = main_c
        .find("((LsString*)ls_retain((void*)(lsaca_lit_0)))")
        .expect("literal use missing");
    assert!(
        declaration < first_use,
        "the literal is used before it is declared"
    );
    let wrapper = main_c
        .find("int main(int argc, char** argv)")
        .expect("main wrapper missing");
    let tail = &main_c[wrapper..];
    let runtime_init = tail.find("ls_runtime_init(argc, argv);").unwrap();
    let literal_init = tail.find("lsaca_init_interned_literals();").unwrap();
    let user_main = tail.find("lsaca_fn_main();").unwrap();
    assert!(
        runtime_init < literal_init && literal_init < user_main,
        "the interned literals are not initialised before user code runs:\n{tail}"
    );
}

const EMBEDDED_NUL_PROGRAM: &str = r#"
fn main() {
    let text = "a\0b";
    println!("{}", text.len());
}
"#;

#[test]
fn keeps_embedded_nul_bytes_in_interned_literals() {
    let (main_c, _) = generate_source("embedded_nul_fixture", EMBEDDED_NUL_PROGRAM, true);

    // Octal escapes keep the NUL byte inside the static array, and the
    // explicit byte length keeps it inside the runtime string (a strlen based
    // C string would truncate to "a").
    assert!(
        main_c.contains("static const char lsaca_lit_data_0[] = \"a\\000b\";"),
        "the NUL byte is not preserved in the static literal array:\n{main_c}"
    );
    assert!(
        main_c.contains("lsaca_lit_0 = ls_str_static(lsaca_lit_data_0, (size_t)3);"),
        "the literal byte length does not count embedded NULs:\n{main_c}"
    );
    assert!(
        !main_c.contains("(size_t)2);") && !main_c.contains("ls_str_cstr("),
        "the literal is still built through a strlen based builder:\n{main_c}"
    );
}

const STRING_PATTERN_PROGRAM: &str = r#"
fn main() {
    let text = String::from("a\0b");
    match text {
        "a\0b" => {
            println!("matched");
        }
        _ => {
            println!("fallback");
        }
    }
}
"#;

#[test]
fn compares_string_literal_patterns_against_interned_literals() {
    let (main_c, _) = generate_source("string_pattern_fixture", STRING_PATTERN_PROGRAM, true);
    let body = main_function_body(&main_c);

    // A literal pattern compares the borrowed scrutinee against the interned
    // handle: no allocation, no temporary release.
    assert!(
        body.contains("if (!ls_str_eq((") && body.contains("(lsaca_lit_0))"),
        "the literal pattern does not compare against the interned literal:\n{body}"
    );
    assert!(
        !main_c.contains("ls_str_cstr(") && !main_c.contains("ls_str_from_parts("),
        "the literal pattern still allocates a runtime string:\n{main_c}"
    );
    // The scrutinee and pattern share the same literal.
    assert_eq!(
        main_c.matches("static const char lsaca_lit_data_").count(),
        1,
        "the match pattern literal was not interned together with the value literal:\n{main_c}"
    );
}

const NO_STRING_LITERAL_PROGRAM: &str = r#"
fn main() {
    let value = 6;
    let doubled = value * 2;
    println!("{}", doubled);
}
"#;

#[test]
fn emits_no_literal_table_for_programs_without_string_literals() {
    let (main_c, _) = generate_source("no_literals_fixture", NO_STRING_LITERAL_PROGRAM, true);

    // Programs without string literals keep the previous output shape: no
    // literal table, no startup hook.
    assert!(
        !main_c.contains("lsaca_lit_"),
        "a program without string literals emitted a literal table:\n{main_c}"
    );
    assert!(
        !main_c.contains("lsaca_init_interned_literals"),
        "a program without string literals emitted the startup hook:\n{main_c}"
    );
    let wrapper = main_c
        .find("int main(int argc, char** argv)")
        .expect("main wrapper missing");
    let tail = &main_c[wrapper..];
    assert!(
        !tail.contains("lsaca_init"),
        "the wrapper of a literal-free program calls the interned literal setup:\n{tail}"
    );
}

const CHAR_LITERALS_PROGRAM: &str = r#"
fn main() {
    let a = 'a';
    let b: char = '\n';
    if a == 'a' {
        println!("{}", a);
    }
    println!("{:?}", b);
}
"#;

#[test]
fn emits_char_literals_comparisons_and_formatting() {
    let (main_c, _) = generate_source("char_literals_fixture", CHAR_LITERALS_PROGRAM, true);
    let body = main_function_body(&main_c);

    // Characters are plain uint32_t scalars: no retain/release, no live flag.
    assert!(
        main_c.contains("uint32_t v1 = ((uint32_t)0);"),
        "a char local is not initialised as a uint32_t:\n{main_c}"
    );
    assert!(
        !body.contains("v1_live") && !body.contains("v2_live"),
        "a char local carries an ownership live flag:\n{body}"
    );
    assert!(
        body.contains("v1 = ((uint32_t)0x61);"),
        "a char literal is not emitted as a scalar literal:\n{body}"
    );
    assert!(
        body.contains("v2 = ((uint32_t)0xA);"),
        "an escaped char literal is not emitted as a scalar literal:\n{body}"
    );
    assert!(
        body.contains("c2 = ((v1) == (t3));"),
        "char equality is not a plain scalar comparison:\n{body}"
    );
    assert!(
        body.contains("= ls_fmt_char(acc") && body.contains("= ls_fmt_char_debug(acc"),
        "char Display and Debug formatting do not use the runtime helpers:\n{body}"
    );
}

const CHAR_WIDTH_PROGRAM: &str = r#"
fn main() {
    let c = 'x';
    println!("[{:>5}]", c);
}
"#;

#[test]
fn applies_width_to_char_formatting() {
    let (main_c, _) = generate_source("char_width_fixture", CHAR_WIDTH_PROGRAM, true);
    let body = main_function_body(&main_c);

    assert!(
        body.contains("= ls_fmt_char(") && body.contains("= ls_fmt_apply_width("),
        "char width formatting does not format the scalar before padding:\n{body}"
    );
}

const CHAR_CASTS_PROGRAM: &str = r#"
fn main() {
    let c = 'A';
    let n = c as u8;
    let back = n as char;
    println!("{} {} {}", c, n, back);
}
"#;

#[test]
fn casts_between_chars_and_integers() {
    let (main_c, _) = generate_source("char_casts_fixture", CHAR_CASTS_PROGRAM, true);
    let body = main_function_body(&main_c);

    assert!(
        body.contains("v2 = (uint8_t)(t1);"),
        "char to integer cast is not a plain numeric cast:\n{body}"
    );
    assert!(
        body.contains("v3 = (uint32_t)(t2);"),
        "integer to char cast is not a plain numeric cast:\n{body}"
    );
}

const LOCATED_IO_PROGRAM: &str = r#"
fn main() {
    let line = read_line();
    let all = args();
    println!("{} {}", line.is_some(), all.len());
}
"#;

#[test]
fn emits_located_io_calls_with_source_locations() {
    let (main_c, _) = generate_source("located_io_fixture", LOCATED_IO_PROGRAM, true);

    assert!(
        main_c.contains("ls_read_line_at(\"located_io_fixture.ls\", "),
        "read_line does not pass its source location:\n{main_c}"
    );
    assert!(
        main_c.contains("ls_args_at(\"located_io_fixture.ls\", "),
        "args does not pass its source location:\n{main_c}"
    );
    assert!(
        !main_c.contains("= ls_read_line();") && !main_c.contains("= ls_args();"),
        "the legacy unlocated IO calls are still emitted:\n{main_c}"
    );
}

const VEC_CHAR_PROGRAM: &str = r#"
fn main() {
    let text = String::from("héllo");
    let chars = text.chars();
    let count = text.char_count();
    let first = text.char_at(0);
    println!("{} {} {:?}", chars.len(), count, first);
    let mut total = 0;
    for c in chars {
        total += 1;
    }
    println!("{}", total);
}
"#;

#[test]
fn reuses_the_runtime_layout_for_vec_char() {
    let (main_c, _) = generate_source("vec_char_fixture", VEC_CHAR_PROGRAM, true);
    let body = main_function_body(&main_c);

    // LsVec_char and LsOpt_char are provided by the runtime header: the
    // generated program must not redefine their layout.
    assert!(
        !main_c.contains("struct LsVec_char {") && !main_c.contains("struct LsOpt_char {"),
        "the generated program redefines the frozen runtime char types:\n{main_c}"
    );
    // The frozen layout still gets the generic copy/release/debug helpers.
    assert!(
        main_c.contains("LsVec_char lsaca_copy_Vec_char("),
        "Vec<char> has no copy helper:\n{main_c}"
    );
    assert!(
        main_c.contains("void lsaca_release_Vec_char("),
        "Vec<char> has no release helper:\n{main_c}"
    );
    assert!(
        main_c.contains("LsVec_char lsaca_vec_push_slow_Vec_char(LsVec_char v, uint32_t value);"),
        "Vec<char> has no dynamic push helper with the scalar element type:\n{main_c}"
    );
    assert!(
        body.contains("= ls_str_chars("),
        "String::chars does not return the runtime Vec<char> directly:\n{body}"
    );
    assert!(
        !main_c.contains("(LsVec_char)ls_str_chars("),
        "the Vec<char> result is cast by value, which -Wpedantic rejects:\n{main_c}"
    );
    // char_count is a plain scalar conversion, either inlined or routed through
    // the reachable standard-library forwarder.
    assert!(
        body.contains("= (int64_t)ls_str_char_count(") || body.contains("String_char_count_args("),
        "char_count is not a scalar conversion:\n{body}"
    );
}

const MAP_CTRL_PROGRAM: &str = r#"
fn main() {
    let mut map: HashMap<String, i64> = HashMap::new();
    map.insert(String::from("a"), 1);
    println!("{}", map.len());
}
"#;

#[test]
fn frees_the_hash_map_control_bytes() {
    let (main_c, _) = generate_source("map_ctrl_fixture", MAP_CTRL_PROGRAM, true);
    let body = main_function_body(&main_c);

    // The Swiss-table control bytes replace the old state array; the drop
    // helper must free them and must not touch the removed field.
    assert!(
        main_c.contains("free(self->table.ctrl);"),
        "the map drop helper does not free the control bytes:\n{main_c}"
    );
    assert!(
        !main_c.contains("table.states"),
        "the generated code still references the removed map state array:\n{main_c}"
    );
    assert!(
        body.contains("lsaca_map_insert_String_i64("),
        "the map insert call is missing:\n{body}"
    );
}

const CHAR_METHODS_PROGRAM: &str = r#"
fn main() {
    let mut s = String::from("aé");
    s.push('!');
    let c = s.char_at(0);
    let boundary = s.is_char_boundary(1);
    let popped = s.pop();
    let fresh = String::from_char('z');
    println!("{:?} {:?} {} {}", c, popped, boundary, fresh);
}
"#;

#[test]
fn emits_char_string_methods() {
    let (main_c, _) = generate_source("char_methods_fixture", CHAR_METHODS_PROGRAM, true);
    let body = main_function_body(&main_c);

    // push/pop are copy-on-write handle replacements: the updated handle is
    // assigned back to the receiver place, not to the unit target.
    assert!(
        body.contains("v1 = ls_str_push_scalar(v1, (uint32_t)("),
        "String::push does not reassign the copy-on-write handle:\n{body}"
    );
    let pop_line = body
        .lines()
        .find(|line| line.contains("v1 = ls_str_pop_scalar(v1, &popped") && line.contains(", &ch"))
        .unwrap_or_else(|| {
            panic!("String::pop does not reassign the copy-on-write handle:\n{body}")
        });
    let popped = pop_line
        .split("&popped")
        .nth(1)
        .and_then(|tail| tail.split(',').next())
        .expect("malformed pop line");
    assert!(
        body.contains(&format!("if (popped{}) {{", popped)),
        "String::pop does not branch on the runtime out-parameter:\n{body}"
    );
    assert!(
        body.contains("= ls_opt_char_some(ch"),
        "String::pop does not build the Option<char> from the runtime result:\n{body}"
    );
    // char_at and is_char_boundary are standard-library methods: they expand to
    // the intrinsic emitter, either directly or through the generated forwarder
    // that stays reachable.
    assert!(
        body.contains("ls_str_char_at("),
        "String::char_at does not use the runtime out-parameter:\n{body}"
    );
    assert!(
        body.contains("ls_str_is_char_boundary("),
        "String::is_char_boundary is not a plain boolean query:\n{body}"
    );
    assert!(
        body.contains("= ls_str_from_scalar((uint32_t)("),
        "String::from_char does not use the runtime helper:\n{body}"
    );
    // Struct results must not be cast to their own type: ISO C forbids a
    // non-scalar cast and the generated project must stay -Wpedantic clean.
    assert!(
        !main_c.contains("(LsOpt_char)ls_opt_char_some"),
        "Option<char> values are cast by value:\n{main_c}"
    );
}

const MATCH_MOVE_PROGRAM: &str = r#"
struct Tree {
    value: i64,
    left: Option<Box<Tree>>,
    right: Option<Box<Tree>>,
}

fn total(node: Option<Box<Tree>>) -> i64 {
    match node {
        None => 0,
        Some(boxed) => {
            let inner = boxed.get();
            inner.value + total(inner.left.clone()) + total(inner.right.clone())
        }
    }
}

fn guarded(node: Option<Box<Tree>>) -> i64 {
    match node {
        Some(boxed) if boxed.get().value > 5 => 1,
        _ => 0,
    }
}

fn main() {
    let tree = Some(Box::new(Tree { value: 7, left: None, right: None }));
    let other = Some(Box::new(Tree { value: 9, left: None, right: None }));
    println!("{} {}", total(tree), guarded(other));
}
"#;

/// Slice of `main_c` holding the top-level definition of the function whose
/// generated signature starts with `signature`.
fn function_definition<'a>(main_c: &'a str, signature: &str) -> &'a str {
    let start = main_c
        .rfind(signature)
        .unwrap_or_else(|| panic!("missing generated function {signature}:\n{main_c}"));
    let after = &main_c[start..];
    let end = after.find("\n}\n").map_or(after.len(), |index| index + 1);
    &after[..end]
}

#[test]
fn moves_owned_match_bindings_out_of_the_scrutinee() {
    let (main_c, _) = generate_source("match_move_fixture", MATCH_MOVE_PROGRAM, true);
    let total = function_definition(&main_c, "int64_t lsaca_fn_total(");

    // A consuming, unguarded match arm binds by move: the Option payload is
    // stolen from the scrutinee local and the emptied slot is raw-zeroed.
    assert!(
        total.contains("u.some = ((LsBox_Tree*){0});"),
        "the match binding does not clear the moved scrutinee slot:\n{total}"
    );
    assert!(
        !total.contains("lsaca_copy_Box_Tree("),
        "the match binding deep-copies the Box payload instead of moving it:\n{total}"
    );
    assert!(
        total.contains("_live = true;"),
        "the moved binding has no liveness flag:\n{total}"
    );
    // The scrutinee is still released exactly once; releasing the zeroed
    // payload slot is a NULL-safe no-op.
    assert!(
        total.contains("lsaca_release_Opt_Box_Tree("),
        "the consumed match scrutinee is not released:\n{total}"
    );

    // A guard reads its bindings before the arm is selected, so guarded arms
    // keep the copying form: the value may still belong to a later arm.
    let guarded = function_definition(&main_c, "int64_t lsaca_fn_guarded(");
    assert!(
        guarded.contains("lsaca_copy_Box_Tree("),
        "a guarded match arm must not move its bindings out:\n{guarded}"
    );
}

#[test]
fn single_translation_unit_output_stays_byte_identical() {
    let (main_c, _) = generate_source("hello", HELLO, true);
    assert_eq!(
        main_c,
        include_str!("testdata/single_tu_hello.c"),
        "the generated main.c for a small program must not change when translation units are introduced"
    );
}

#[test]
fn small_programs_emit_a_single_translation_unit() {
    let generated = generate_project_source("hello", HELLO, false);
    assert!(
        generated.units.is_empty(),
        "small program emitted extra translation units: {:?}",
        generated
            .units
            .iter()
            .map(|unit| &unit.file_name)
            .collect::<Vec<_>>()
    );
}

/// Build a source program whose emitted function bodies exceed the split
/// threshold so that the generator distributes user functions over units.
fn split_threshold_program() -> String {
    let mut source = String::new();
    for function in 0..120 {
        source.push_str(&format!("fn f{function}(x: i64) -> i64 {{\n    x"));
        for term in 0..60 {
            source.push_str(&format!(" + {term}"));
        }
        source.push_str("\n}\n\n");
    }
    source.push_str("fn banner() -> String {\n    String::from(\"banner\")\n}\n\n");
    source.push_str("fn main() {\n    let label = \"split\";\n    let mut total = 0;\n");
    for function in 0..120 {
        source.push_str(&format!("    total += f{function}({function});\n"));
    }
    source.push_str("    println!(\"{} {}\", label, total);\n}\n");
    source
}

#[test]
fn splits_large_programs_into_deterministic_translation_units() {
    let source = split_threshold_program();
    let generated = generate_project_source("split_fixture", &source, true);

    assert!(
        (2..=8).contains(&generated.units.len()),
        "unexpected translation unit count: {}",
        generated.units.len()
    );
    let names: Vec<&str> = generated
        .units
        .iter()
        .map(|unit| unit.file_name.as_str())
        .collect();
    for (index, unit) in generated.units.iter().enumerate() {
        assert_eq!(unit.file_name, format!("lsaca_unit_{}.c", index + 1));
        assert!(!unit.contents.is_empty(), "{} is empty", unit.file_name);
        // Every unit repeats the shared prefix so that it compiles on its own.
        assert!(unit.contents.contains("#include \"lsaca_rt.h\""));
        assert!(unit.contents.contains("typedef struct LsUnit"));
        // The entry point and the C wrapper stay in main.c.
        assert!(!unit.contents.contains("int main(int argc, char** argv)"));
        assert!(!unit.contents.contains("LsUnit lsaca_fn_main()\n{"));
        // Interned literal handles are defined in main.c and declared here.
        assert!(unit.contents.contains("extern LsString* lsaca_lit_0;"));
        assert!(!unit.contents.contains("LsString* lsaca_lit_0 = NULL;"));
    }

    // main.c keeps the literal table definitions and the entry point.
    assert!(
        generated
            .main_c
            .contains("static const char lsaca_lit_data_0[]")
    );
    assert!(generated.main_c.contains("LsString* lsaca_lit_0 = NULL;"));
    assert!(
        !generated
            .main_c
            .contains("static LsString* lsaca_lit_0 = NULL;")
    );
    assert!(generated.main_c.contains("LsUnit lsaca_fn_main()"));
    assert!(generated.main_c.contains("int main(int argc, char** argv)"));

    // Every user function is defined exactly once across all translation units.
    let mut combined = generated.main_c.clone();
    for unit in &generated.units {
        combined.push_str(&unit.contents);
    }
    assert!(
        combined.contains("((LsString*)ls_retain((void*)(lsaca_lit_0)))"),
        "the split output never references an interned literal handle"
    );
    for function in 0..120 {
        let definition = format!("int64_t lsaca_fn_f{function}(int64_t p0)\n{{");
        assert_eq!(
            combined.matches(&definition).count(),
            1,
            "function f{function} is not defined exactly once"
        );
    }

    // The build lists main.c first, then the units, then the runtime sources.
    let unit_list = names.join(" ");
    let expected = format!("add_executable(split_fixture main.c {unit_list} ");
    assert!(
        generated.cmake.contains(&expected),
        "cmake does not list the translation units after main.c:\n{}",
        generated.cmake
    );

    // Emission is deterministic.
    let again = generate_project_source("split_fixture", &source, true);
    assert_eq!(again.main_c, generated.main_c);
    assert_eq!(again.units.len(), generated.units.len());
    for (left, right) in again.units.iter().zip(generated.units.iter()) {
        assert_eq!(left.file_name, right.file_name);
        assert_eq!(left.contents, right.contents);
    }
}
/// Builds `fn main() { let value = 10 / 0; }` with the division carrying `span`.
fn division_program(name: &str, span: crate::span::Span) -> crate::hir::Program {
    use crate::hir::{
        HBinaryOp, HBlock, HExpr, HExprKind, HFn, HLit, HPat, HPatKind, HStmt, Program,
    };
    use crate::types::{IntTy, Ty};

    fn int_literal(value: i128) -> HExpr {
        HExpr::new(
            Ty::i64(),
            HExprKind::Lit(HLit::Int {
                value,
                int_ty: IntTy::I64,
            }),
            crate::span::Span::empty(0),
        )
    }

    let divide = HExpr::new(
        Ty::i64(),
        HExprKind::Binary {
            op: HBinaryOp::Div,
            lhs: Box::new(int_literal(10)),
            rhs: Box::new(int_literal(0)),
        },
        span,
    );
    let pat = HPat {
        ty: Ty::i64(),
        kind: HPatKind::Bind {
            c_name: "v1".to_string(),
            name: "value".to_string(),
        },
        span: crate::span::Span::empty(0),
    };
    let main = HFn {
        name: "main".to_string(),
        c_name: "lsaca_fn_main".to_string(),
        params: Vec::new(),
        ret: Ty::unit(),
        self_kind: None,
        intrinsic: false,
        body: HBlock {
            stmts: vec![HStmt::Let {
                pat,
                init: Some(divide),
                span: crate::span::Span::empty(0),
            }],
            tail: None,
            span: crate::span::Span::empty(0),
        },
        span: crate::span::Span::empty(0),
    };
    let mut program = Program {
        name: name.to_string(),
        structs: Vec::new(),
        enums: Vec::new(),
        fns: vec![main],
        consts: Vec::new(),
        closures: Vec::new(),
        main_fn: 0,
        copy_structs: Vec::new(),
        copy_enums: Vec::new(),
        drop_structs: Vec::new(),
        drop_enums: Vec::new(),
        trait_objects: Vec::new(),
        vtables: Vec::new(),
    };
    program.classify_types(Vec::new(), Vec::new());
    program
}

#[test]
fn resolves_panic_locations_through_a_multi_file_source_map() {
    use crate::span::{SourceMap, Span};

    // Two files in one global source space: the division lives in the second.
    let mut sources = SourceMap::new();
    sources.add_file("a.ls", "fn other() {}\n");
    let b_text = "fn helper() {\n    let value = 10 / 0;\n}\n";
    let b_index = sources.add_file("b.ls", b_text);
    let b_base = sources.file(b_index).base();
    let local_start = b_text.find("10 / 0").expect("marker is present") as u32;
    let span = Span::new(b_base + local_start, b_base + local_start + 6);

    let (file, line, col) = sources.location(span).expect("the span resolves");
    assert_eq!(file.name(), "b.ls");
    assert_eq!((line, col), (2, 17));

    let program = division_program("multi", span);
    let options = CodegenOptions {
        program_name: "multi".to_string(),
        sources: Some(sources.clone()),
        ..CodegenOptions::default()
    };
    let main_c = generate(&program, &options).main_c;
    assert!(
        main_c.contains("\"b.ls\", 2, 17)"),
        "the panic location does not point into b.ls:\n{main_c}"
    );
    assert!(
        main_c.contains("ls_div_i64_checked("),
        "the division is not checked:\n{main_c}"
    );

    // Without a source map the location falls back to the program name at 0:0.
    let fallback_options = CodegenOptions {
        program_name: "multi".to_string(),
        ..CodegenOptions::default()
    };
    let fallback = generate(&program, &fallback_options).main_c;
    assert!(
        fallback.contains("\"multi\", 0, 0)"),
        "a missing source map did not fall back to the program name:\n{fallback}"
    );
    assert!(!fallback.contains("\"b.ls\""));

    // An empty map resolves no spans and uses the same fallback.
    let empty_options = CodegenOptions {
        program_name: "multi".to_string(),
        sources: Some(SourceMap::new()),
        ..CodegenOptions::default()
    };
    let empty = generate(&program, &empty_options).main_c;
    assert!(
        empty.contains("\"multi\", 0, 0)"),
        "an empty source map did not fall back to the program name:\n{empty}"
    );
}

/// Builds an empty checked block.
fn empty_block() -> crate::hir::HBlock {
    crate::hir::HBlock {
        stmts: Vec::new(),
        tail: None,
        span: crate::span::Span::empty(0),
    }
}

/// Builds a function with a body and no intrinsic marker.
fn checked_function(
    name: &str,
    c_name: &str,
    params: Vec<crate::hir::HParam>,
    ret: crate::types::Ty,
    body: crate::hir::HBlock,
) -> crate::hir::HFn {
    crate::hir::HFn {
        name: name.to_string(),
        c_name: c_name.to_string(),
        params,
        ret,
        self_kind: None,
        intrinsic: false,
        body,
        span: crate::span::Span::empty(0),
    }
}

/// Builds an extern standard library intrinsic: no C definition is emitted.
fn intrinsic_function(
    name: &str,
    c_name: &str,
    params: Vec<crate::hir::HParam>,
    ret: crate::types::Ty,
) -> crate::hir::HFn {
    crate::hir::HFn {
        name: name.to_string(),
        c_name: c_name.to_string(),
        params,
        ret,
        self_kind: None,
        intrinsic: true,
        body: empty_block(),
        span: crate::span::Span::empty(0),
    }
}

/// Builds a whole program around the given function definitions.
fn program_from(name: &str, fns: Vec<crate::hir::HFn>, main_fn: usize) -> crate::hir::Program {
    let mut program = crate::hir::Program {
        name: name.to_string(),
        structs: Vec::new(),
        enums: Vec::new(),
        fns,
        consts: Vec::new(),
        closures: Vec::new(),
        main_fn,
        copy_structs: Vec::new(),
        copy_enums: Vec::new(),
        drop_structs: Vec::new(),
        drop_enums: Vec::new(),
        trait_objects: Vec::new(),
        vtables: Vec::new(),
    };
    program.classify_types(Vec::new(), Vec::new());
    program
}

#[test]
fn emits_isize_and_usize_integer_types() {
    use crate::hir::{HExpr, HExprKind, HLit, HPat, HPatKind, HStmt};
    use crate::types::{IntTy, Ty};

    fn int_literal(value: i128, int_ty: IntTy) -> HExpr {
        HExpr::new(
            Ty::Int(int_ty),
            HExprKind::Lit(HLit::Int { value, int_ty }),
            crate::span::Span::empty(0),
        )
    }
    fn binding(c_name: &str, name: &str, ty: Ty) -> HPat {
        HPat {
            ty,
            kind: HPatKind::Bind {
                c_name: c_name.to_string(),
                name: name.to_string(),
            },
            span: crate::span::Span::empty(0),
        }
    }

    let body = crate::hir::HBlock {
        stmts: vec![
            HStmt::Let {
                pat: binding("v1", "offset", Ty::Int(IntTy::Isize)),
                init: Some(int_literal(-5, IntTy::Isize)),
                span: crate::span::Span::empty(0),
            },
            HStmt::Let {
                pat: binding("v2", "length", Ty::Int(IntTy::Usize)),
                init: Some(int_literal(7, IntTy::Usize)),
                span: crate::span::Span::empty(0),
            },
        ],
        tail: None,
        span: crate::span::Span::empty(0),
    };
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    let program = program_from("integers", vec![main], 0);
    let main_c = generate(&program, &CodegenOptions::default()).main_c;

    assert!(
        main_c.contains("intptr_t v1 = ((intptr_t)0);"),
        "isize locals are not intptr_t:\n{main_c}"
    );
    assert!(
        main_c.contains("uintptr_t v2 = ((uintptr_t)0u);"),
        "usize locals are not uintptr_t:\n{main_c}"
    );
    assert!(main_c.contains("v1 = ((intptr_t)-5);"));
    assert!(main_c.contains("v2 = ((uintptr_t)7u);"));
    assert!(main_c.contains("intptr_t lsaca_cast_f2i_isize(double value) {"));
    assert!(main_c.contains("uintptr_t lsaca_cast_f2i_usize(double value) {"));
}

/// The runtime defines its checked arithmetic, `abs`, `min` and `max`
/// helpers for the fixed-width integer types only, so pointer sized integers
/// must call the `i64`/`u64` families with casts instead of emitting calls to
/// helpers the runtime does not define.
#[test]
fn maps_pointer_sized_integers_onto_the_fixed_width_runtime_helpers() {
    let (main_c, _) = generate_source(
        "isize_ops",
        r#"fn main() {
    let a: isize = -5;
    let b: usize = 7;
    let c: isize = a + 3;
    let d: usize = b * 2;
    let e: isize = abs(a);
    let f: isize = min(a, e);
    let g: usize = max(b, d);
    println!("{} {} {} {} {} {}", c, d, e, f, g, a - b as isize);
}
"#,
        true,
    );
    assert!(
        main_c.contains("(intptr_t)(ls_add_i64_checked((int64_t)("),
        "isize addition must use the i64 runtime family:\n{main_c}"
    );
    assert!(
        main_c.contains("(uintptr_t)(ls_mul_u64_checked((uint64_t)("),
        "usize multiplication must use the u64 runtime family:\n{main_c}"
    );
    assert!(
        main_c.contains("(intptr_t)(ls_sub_i64_checked((int64_t)("),
        "isize subtraction must use the i64 runtime family:\n{main_c}"
    );
    assert!(
        main_c.contains("(intptr_t)(ls_abs_i64_checked((int64_t)("),
        "isize abs must use the i64 runtime family:\n{main_c}"
    );
    assert!(
        main_c.contains("ls_min_i64((int64_t)("),
        "isize min must use the i64 runtime family:\n{main_c}"
    );
    assert!(
        main_c.contains("ls_max_u64((uint64_t)("),
        "usize max must use the u64 runtime family:\n{main_c}"
    );
    for undefined in [
        "ls_add_isize",
        "ls_mul_usize",
        "ls_abs_isize",
        "ls_min_isize",
        "ls_max_usize",
    ] {
        assert!(
            !main_c.contains(undefined),
            "generated C calls the undefined runtime helper {undefined}:\n{main_c}"
        );
    }
}

#[test]
fn expands_intrinsic_calls_without_emitting_a_c_function() {
    use crate::hir::{HExpr, HExprKind, HLit, HParam, HPat, HPatKind, HStmt};
    use crate::types::{FloatTy, Ty};

    let sqrt = intrinsic_function(
        "__sqrt",
        "lsaca_fn___sqrt",
        vec![HParam {
            name: "value".to_string(),
            c_name: "p0".to_string(),
            ty: Ty::f64(),
        }],
        Ty::f64(),
    );
    let argument = HExpr::new(
        Ty::f64(),
        HExprKind::Lit(HLit::Float {
            value: 9.0,
            float_ty: FloatTy::F64,
        }),
        crate::span::Span::empty(0),
    );
    let call = HExpr::new(
        Ty::f64(),
        HExprKind::DirectCall {
            fn_id: 0,
            receiver: None,
            args: vec![argument],
        },
        crate::span::Span::empty(0),
    );
    let pat = HPat {
        ty: Ty::f64(),
        kind: HPatKind::Bind {
            c_name: "v1".to_string(),
            name: "root".to_string(),
        },
        span: crate::span::Span::empty(0),
    };
    let body = crate::hir::HBlock {
        stmts: vec![HStmt::Let {
            pat,
            init: Some(call),
            span: crate::span::Span::empty(0),
        }],
        tail: None,
        span: crate::span::Span::empty(0),
    };
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    let program = program_from("intrinsic", vec![sqrt, main], 1);
    let main_c = generate(&program, &CodegenOptions::default()).main_c;

    assert!(
        !main_c.contains("double lsaca_fn___sqrt(double p0);"),
        "the intrinsic must not get a prototype:\n{main_c}"
    );
    assert_eq!(
        main_c.matches("lsaca_fn___sqrt").count(),
        0,
        "the intrinsic got a C prototype, definition or thunk:\n{main_c}"
    );
    assert!(
        main_c.contains("ls_sqrt_f64("),
        "the intrinsic call was not expanded:\n{main_c}"
    );
}

/// Intrinsics are expanded at their call sites and never get a C declaration.
#[test]
fn omits_c_declarations_for_intrinsic_functions() {
    use crate::hir::{HBlock, HBuiltinAssoc, HExpr, HExprKind, HParam, HPat, HPatKind, HStmt};
    use crate::span::Span;
    use crate::types::Ty;

    fn binding(c_name: &str, name: &str, ty: Ty) -> HPat {
        HPat {
            ty,
            kind: HPatKind::Bind {
                c_name: c_name.to_string(),
                name: name.to_string(),
            },
            span: Span::empty(0),
        }
    }

    let empty = Span::empty(0);
    let string_ty = Ty::string();
    let shared_string = Ty::Ref {
        mutable: false,
        inner: Box::new(string_ty.clone()),
    };
    let intrinsic = intrinsic_function(
        "__str_len",
        "lsaca_fn___str_len",
        vec![HParam {
            name: "self".to_string(),
            c_name: "p0".to_string(),
            ty: shared_string,
        }],
        Ty::i64(),
    );
    let make_string = HExpr::new(
        string_ty.clone(),
        HExprKind::BuiltinAssoc {
            func: HBuiltinAssoc::StringNew,
            args: Vec::new(),
            type_args: Vec::new(),
        },
        empty,
    );
    let len_call = HExpr::new(
        Ty::i64(),
        HExprKind::DirectCall {
            fn_id: 0,
            receiver: None,
            args: vec![HExpr::new(
                string_ty.clone(),
                HExprKind::Local {
                    name: "value".to_string(),
                    c_name: "v1".to_string(),
                },
                empty,
            )],
        },
        empty,
    );
    let body = HBlock {
        stmts: vec![
            HStmt::Let {
                pat: binding("v1", "value", string_ty),
                init: Some(make_string),
                span: empty,
            },
            HStmt::Let {
                pat: binding("v2", "length", Ty::i64()),
                init: Some(len_call),
                span: empty,
            },
        ],
        tail: None,
        span: empty,
    };
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    let program = program_from("intrinsic_declaration", vec![intrinsic, main], 1);
    let main_c = generate(&program, &CodegenOptions::default()).main_c;

    assert!(
        !main_c.contains("lsaca_fn___str_len"),
        "the intrinsic got a C declaration:\n{main_c}"
    );
    assert!(
        main_c.contains("ls_str_len("),
        "the intrinsic call was not expanded:\n{main_c}"
    );
}

/// A standard-library method that only forwards its parameters to one intrinsic
/// is expanded at its call site: the generated C calls the intrinsic emitter
/// directly and the wrapper never exists as a C function.
#[test]
fn expands_standard_library_forwarding_wrappers_at_the_call_site() {
    use crate::hir::{HBlock, HBuiltinAssoc, HExpr, HExprKind, HParam, HPat, HPatKind, HStmt};
    use crate::span::{SourceMap, Span};
    use crate::types::{SelfKind, Ty};

    fn binding(c_name: &str, name: &str, ty: Ty) -> HPat {
        HPat {
            ty,
            kind: HPatKind::Bind {
                c_name: c_name.to_string(),
                name: name.to_string(),
            },
            span: Span::empty(0),
        }
    }

    let empty = Span::empty(0);
    let string_ty = Ty::string();
    let shared_string = Ty::Ref {
        mutable: false,
        inner: Box::new(string_ty.clone()),
    };

    // The wrapper lives in a standard-library source file, so its span must
    // resolve inside a std/ file of the source map.
    let mut sources = SourceMap::new();
    let std_index = sources.add_file(
        "std/string.ls",
        "pub fn len(&self) -> i64 { return __str_len(self); }\n",
    );
    let std_base = sources.file(std_index).base();
    let wrapper_span = Span::new(std_base, std_base + 1);

    let intrinsic = intrinsic_function(
        "__str_len",
        "lsaca_fn___str_len",
        vec![HParam {
            name: "self".to_string(),
            c_name: "p0".to_string(),
            ty: shared_string.clone(),
        }],
        Ty::i64(),
    );
    let forward = HExpr::new(
        Ty::i64(),
        HExprKind::DirectCall {
            fn_id: 0,
            receiver: None,
            args: vec![HExpr::new(
                shared_string,
                HExprKind::Local {
                    name: "self".to_string(),
                    c_name: "p0".to_string(),
                },
                empty,
            )],
        },
        empty,
    );
    let returned = HExpr::new(
        Ty::unit(),
        HExprKind::Return(Some(Box::new(forward))),
        empty,
    );
    let wrapper = crate::hir::HFn {
        name: "len".to_string(),
        c_name: "string_lsaca_fn_len".to_string(),
        params: vec![HParam {
            name: "self".to_string(),
            c_name: "p0".to_string(),
            ty: Ty::ref_to(string_ty.clone()),
        }],
        ret: Ty::i64(),
        self_kind: Some(SelfKind::Ref),
        intrinsic: false,
        body: HBlock {
            stmts: vec![HStmt::Expr(returned)],
            tail: None,
            span: wrapper_span,
        },
        span: wrapper_span,
    };
    let make_string = HExpr::new(
        string_ty.clone(),
        HExprKind::BuiltinAssoc {
            func: HBuiltinAssoc::StringNew,
            args: Vec::new(),
            type_args: Vec::new(),
        },
        empty,
    );
    let len_call = HExpr::new(
        Ty::i64(),
        HExprKind::DirectCall {
            fn_id: 1,
            receiver: Some(Box::new(HExpr::new(
                string_ty.clone(),
                HExprKind::Local {
                    name: "value".to_string(),
                    c_name: "v1".to_string(),
                },
                empty,
            ))),
            args: Vec::new(),
        },
        empty,
    );
    let body = HBlock {
        stmts: vec![
            HStmt::Let {
                pat: binding("v1", "value", string_ty),
                init: Some(make_string),
                span: empty,
            },
            HStmt::Let {
                pat: binding("v2", "length", Ty::i64()),
                init: Some(len_call),
                span: empty,
            },
        ],
        tail: None,
        span: empty,
    };
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    let program = program_from("wrapper_inlining", vec![intrinsic, wrapper, main], 2);
    let options = CodegenOptions {
        program_name: "wrapper_inlining".to_string(),
        sources: Some(sources),
        ..CodegenOptions::default()
    };
    let main_c = generate(&program, &options).main_c;

    assert!(
        main_c.contains("= (int64_t)ls_str_len("),
        "the forwarded call was not expanded to the intrinsic emitter:\n{main_c}"
    );
    assert!(
        !main_c.contains("string_lsaca_fn_len"),
        "the expanded wrapper still has a C declaration:\n{main_c}"
    );
    assert!(
        !main_c.contains("lsaca_fn___str_len"),
        "the intrinsic got a C declaration:\n{main_c}"
    );
}

fn phase_b_int(value: i128) -> crate::hir::HExpr {
    use crate::hir::{HExpr, HExprKind, HLit};
    use crate::span::Span;
    use crate::types::{IntTy, Ty};
    HExpr::new(
        Ty::i64(),
        HExprKind::Lit(HLit::Int {
            value,
            int_ty: IntTy::I64,
        }),
        Span::empty(0),
    )
}

fn phase_b_bind(c_name: &str, name: &str, ty: crate::types::Ty) -> crate::hir::HPat {
    use crate::hir::{HPat, HPatKind};
    use crate::span::Span;
    HPat {
        ty,
        kind: HPatKind::Bind {
            c_name: c_name.to_string(),
            name: name.to_string(),
        },
        span: Span::empty(0),
    }
}

fn phase_b_let(
    c_name: &str,
    name: &str,
    ty: crate::types::Ty,
    init: crate::hir::HExpr,
) -> crate::hir::HStmt {
    use crate::hir::HStmt;
    use crate::span::Span;
    HStmt::Let {
        pat: phase_b_bind(c_name, name, ty),
        init: Some(init),
        span: Span::empty(0),
    }
}

fn phase_b_local(name: &str, c_name: &str, ty: crate::types::Ty) -> crate::hir::HExpr {
    use crate::hir::{HExpr, HExprKind};
    use crate::span::Span;
    HExpr::new(
        ty,
        HExprKind::Local {
            name: name.to_string(),
            c_name: c_name.to_string(),
        },
        Span::empty(0),
    )
}

fn phase_b_method(
    name: &str,
    c_name: &str,
    params: Vec<crate::hir::HParam>,
    ret: crate::types::Ty,
    self_kind: crate::types::SelfKind,
) -> crate::hir::HFn {
    use crate::hir::HFn;
    use crate::span::Span;
    HFn {
        name: name.to_string(),
        c_name: c_name.to_string(),
        params,
        ret,
        self_kind: Some(self_kind),
        intrinsic: false,
        body: empty_block(),
        span: Span::empty(0),
    }
}

fn phase_b_block(stmts: Vec<crate::hir::HStmt>) -> crate::hir::HBlock {
    use crate::hir::HBlock;
    use crate::span::Span;
    HBlock {
        stmts,
        tail: None,
        span: Span::empty(0),
    }
}

fn phase_b_slice_reference(ty: crate::types::Ty) -> crate::types::Ty {
    use crate::types::Ty;
    Ty::Ref {
        mutable: false,
        inner: Box::new(ty),
    }
}

fn phase_b_display_program() -> crate::hir::Program {
    use crate::hir::{
        HBuiltinAssoc, HExpr, HExprKind, HParam, HTraitMethodSig, HTraitObject, HVtable,
    };
    use crate::span::Span;
    use crate::types::{SelfKind, Ty};

    let empty = Span::empty(0);
    let string_ty = Ty::string();
    let receiver = HParam {
        name: "self".to_string(),
        c_name: "p0".to_string(),
        ty: phase_b_slice_reference(string_ty.clone()),
    };
    let fmt = phase_b_method(
        "fmt",
        "display_fmt",
        vec![receiver.clone()],
        Ty::unit(),
        SelfKind::Ref,
    );
    let len = phase_b_method(
        "len",
        "display_len",
        vec![
            receiver,
            HParam {
                name: "extra".to_string(),
                c_name: "p1".to_string(),
                ty: Ty::i64(),
            },
        ],
        Ty::i64(),
        SelfKind::Ref,
    );
    let dyn_ty = Ty::Dyn(0);
    let box_dyn = Ty::Box(Box::new(dyn_ty.clone()));
    let make_string = HExpr::new(
        string_ty.clone(),
        HExprKind::BuiltinAssoc {
            func: HBuiltinAssoc::StringNew,
            args: Vec::new(),
            type_args: Vec::new(),
        },
        empty,
    );
    let string_ref = HExpr::new(
        phase_b_slice_reference(string_ty.clone()),
        HExprKind::Ref {
            mutable: false,
            expr: Box::new(phase_b_local("text", "v1", string_ty.clone())),
        },
        empty,
    );
    let coerce_ref = HExpr::new(
        phase_b_slice_reference(dyn_ty.clone()),
        HExprKind::DynCoerce {
            value: Box::new(string_ref),
            vtable: 0,
        },
        empty,
    );
    let call_ref = HExpr::new(
        Ty::i64(),
        HExprKind::DynCall {
            object: Box::new(phase_b_local(
                "value",
                "v2",
                phase_b_slice_reference(dyn_ty.clone()),
            )),
            trait_object: 0,
            slot: 1,
            args: vec![phase_b_int(5)],
        },
        empty,
    );
    let box_make = HExpr::new(
        Ty::Box(Box::new(string_ty.clone())),
        HExprKind::BuiltinAssoc {
            func: HBuiltinAssoc::BoxNew,
            args: vec![make_string.clone()],
            type_args: Vec::new(),
        },
        empty,
    );
    let coerce_box = HExpr::new(
        box_dyn.clone(),
        HExprKind::DynCoerce {
            value: Box::new(box_make),
            vtable: 0,
        },
        empty,
    );
    let call_box = HExpr::new(
        Ty::i64(),
        HExprKind::DynCall {
            object: Box::new(phase_b_local("boxed", "v4", box_dyn)),
            trait_object: 0,
            slot: 1,
            args: vec![phase_b_int(7)],
        },
        empty,
    );
    let body = phase_b_block(vec![
        phase_b_let("v1", "text", string_ty.clone(), make_string),
        phase_b_let(
            "v2",
            "value",
            phase_b_slice_reference(dyn_ty.clone()),
            coerce_ref,
        ),
        phase_b_let("v3", "length", Ty::i64(), call_ref),
        phase_b_let("v4", "boxed", Ty::Box(Box::new(dyn_ty)), coerce_box),
        phase_b_let("v5", "boxed_length", Ty::i64(), call_box),
    ]);
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    let mut program = program_from("phase_b_display", vec![fmt, len, main], 2);
    program.trait_objects.push(HTraitObject {
        name: "Display".to_string(),
        args: Vec::new(),
        c_name: "LsVTable_Display".to_string(),
        methods: vec![
            HTraitMethodSig {
                name: "fmt".to_string(),
                receiver: SelfKind::Ref,
                params: Vec::new(),
                ret: Ty::unit(),
            },
            HTraitMethodSig {
                name: "len".to_string(),
                receiver: SelfKind::Ref,
                params: vec![Ty::i64()],
                ret: Ty::i64(),
            },
        ],
    });
    program.vtables.push(HVtable {
        trait_object: 0,
        self_ty: string_ty,
        methods: vec![0, 1],
    });
    program
}

fn phase_b_slice_program() -> crate::hir::Program {
    use crate::hir::{HBuiltinAssoc, HExpr, HExprKind};
    use crate::span::Span;
    use crate::types::Ty;

    let empty = Span::empty(0);
    let vec_ty = Ty::vec(Ty::i64());
    let make_vec = HExpr::new(
        vec_ty.clone(),
        HExprKind::BuiltinAssoc {
            func: HBuiltinAssoc::VecNew,
            args: Vec::new(),
            type_args: Vec::new(),
        },
        empty,
    );
    let slice_ty = Ty::Slice(Box::new(Ty::i64()));
    let slice_ref = phase_b_slice_reference(slice_ty.clone());
    let slice_expr = HExpr::new(
        slice_ref.clone(),
        HExprKind::Ref {
            mutable: false,
            expr: Box::new(HExpr::new(
                slice_ty,
                HExprKind::Slice {
                    base: Box::new(phase_b_local("items", "v1", vec_ty.clone())),
                    start: Some(Box::new(phase_b_int(1))),
                    end: Some(Box::new(phase_b_int(3))),
                    inclusive: false,
                },
                empty,
            )),
        },
        empty,
    );
    let body = phase_b_block(vec![
        phase_b_let("v1", "items", vec_ty.clone(), make_vec),
        phase_b_let("v2", "window", slice_ref, slice_expr),
    ]);
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    program_from("phase_b_slice", vec![main], 0)
}

fn phase_b_str_slice_program() -> crate::hir::Program {
    use crate::hir::{HBuiltinAssoc, HExpr, HExprKind};
    use crate::span::Span;
    use crate::types::Ty;

    let empty = Span::empty(0);
    let string_ty = Ty::string();
    let make_string = HExpr::new(
        string_ty.clone(),
        HExprKind::BuiltinAssoc {
            func: HBuiltinAssoc::StringNew,
            args: Vec::new(),
            type_args: Vec::new(),
        },
        empty,
    );
    let str_ty = Ty::str_();
    let str_ref = phase_b_slice_reference(str_ty.clone());
    let slice_expr = HExpr::new(
        str_ref.clone(),
        HExprKind::Ref {
            mutable: false,
            expr: Box::new(HExpr::new(
                str_ty,
                HExprKind::Slice {
                    base: Box::new(phase_b_local("text", "v1", string_ty.clone())),
                    start: Some(Box::new(phase_b_int(1))),
                    end: Some(Box::new(phase_b_int(3))),
                    inclusive: false,
                },
                empty,
            )),
        },
        empty,
    );
    let body = phase_b_block(vec![
        phase_b_let("v1", "text", string_ty.clone(), make_string),
        phase_b_let("v2", "word", str_ref, slice_expr),
    ]);
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    program_from("phase_b_str_slice", vec![main], 0)
}

fn phase_b_intrinsic_program() -> crate::hir::Program {
    use crate::hir::{HBuiltinAssoc, HExpr, HExprKind, HParam};
    use crate::span::Span;
    use crate::types::Ty;

    let empty = Span::empty(0);
    let slice_ref = phase_b_slice_reference(Ty::Slice(Box::new(Ty::i64())));
    let str_ref = phase_b_slice_reference(Ty::str_());
    let slice_len = intrinsic_function(
        "__slice_len",
        "lsaca_fn___slice_len",
        vec![HParam {
            name: "slice".to_string(),
            c_name: "p0".to_string(),
            ty: slice_ref.clone(),
        }],
        Ty::i64(),
    );
    let str_len = intrinsic_function(
        "__str_len",
        "lsaca_fn___str_len",
        vec![HParam {
            name: "text".to_string(),
            c_name: "p0".to_string(),
            ty: str_ref.clone(),
        }],
        Ty::i64(),
    );
    let str_from_str = intrinsic_function(
        "__str_from_str",
        "lsaca_fn___str_from_str",
        vec![HParam {
            name: "text".to_string(),
            c_name: "p0".to_string(),
            ty: str_ref.clone(),
        }],
        Ty::string(),
    );
    let vec_ty = Ty::vec(Ty::i64());
    let make_vec = HExpr::new(
        vec_ty.clone(),
        HExprKind::BuiltinAssoc {
            func: HBuiltinAssoc::VecNew,
            args: Vec::new(),
            type_args: Vec::new(),
        },
        empty,
    );
    let make_string = HExpr::new(
        Ty::string(),
        HExprKind::BuiltinAssoc {
            func: HBuiltinAssoc::StringNew,
            args: Vec::new(),
            type_args: Vec::new(),
        },
        empty,
    );
    let slice_arg = HExpr::new(
        slice_ref,
        HExprKind::Ref {
            mutable: false,
            expr: Box::new(HExpr::new(
                Ty::Slice(Box::new(Ty::i64())),
                HExprKind::Slice {
                    base: Box::new(phase_b_local("items", "v1", vec_ty.clone())),
                    start: Some(Box::new(phase_b_int(1))),
                    end: None,
                    inclusive: false,
                },
                empty,
            )),
        },
        empty,
    );
    let str_arg = HExpr::new(
        str_ref.clone(),
        HExprKind::Ref {
            mutable: false,
            expr: Box::new(HExpr::new(
                Ty::str_(),
                HExprKind::Slice {
                    base: Box::new(phase_b_local("text", "v3", Ty::string())),
                    start: None,
                    end: None,
                    inclusive: false,
                },
                empty,
            )),
        },
        empty,
    );
    let str_range_arg = HExpr::new(
        str_ref,
        HExprKind::Ref {
            mutable: false,
            expr: Box::new(HExpr::new(
                Ty::str_(),
                HExprKind::Slice {
                    base: Box::new(phase_b_local("text", "v3", Ty::string())),
                    start: Some(Box::new(phase_b_int(0))),
                    end: Some(Box::new(phase_b_int(2))),
                    inclusive: false,
                },
                empty,
            )),
        },
        empty,
    );
    let body = phase_b_block(vec![
        phase_b_let("v1", "items", vec_ty.clone(), make_vec),
        phase_b_let(
            "v2",
            "count",
            Ty::i64(),
            HExpr::new(
                Ty::i64(),
                HExprKind::DirectCall {
                    fn_id: 0,
                    receiver: None,
                    args: vec![slice_arg],
                },
                empty,
            ),
        ),
        phase_b_let("v3", "text", Ty::string(), make_string),
        phase_b_let(
            "v4",
            "bytes",
            Ty::i64(),
            HExpr::new(
                Ty::i64(),
                HExprKind::DirectCall {
                    fn_id: 1,
                    receiver: None,
                    args: vec![str_arg],
                },
                empty,
            ),
        ),
        phase_b_let(
            "v5",
            "copy",
            Ty::string(),
            HExpr::new(
                Ty::string(),
                HExprKind::DirectCall {
                    fn_id: 2,
                    receiver: None,
                    args: vec![str_range_arg],
                },
                empty,
            ),
        ),
    ]);
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    program_from(
        "phase_b_intrinsics",
        vec![slice_len, str_len, str_from_str, main],
        3,
    )
}

fn assert_ordered(text: &str, needles: &[&str]) {
    let mut last = 0;
    for needle in needles {
        let position = text
            .find(needle)
            .unwrap_or_else(|| panic!("missing generated text: {needle}"));
        assert!(position >= last, "generated text out of order: {needle}",);
        last = position;
    }
}

#[test]
fn emits_trait_object_vtables_with_lifecycle_and_method_slots() {
    let program = phase_b_display_program();
    let main_c = generate(&program, &CodegenOptions::default()).main_c;

    assert!(main_c.contains("typedef struct LsDyn0 LsDyn0;"));
    assert!(main_c.contains("typedef struct LsBox_Dyn0 LsBox_Dyn0;"));
    assert!(main_c.contains("typedef struct LsVTable_Display LsVTable_Display;"));
    assert!(
        main_c.contains("struct LsDyn0 {\n  void* data;\n  const LsVTable_Display* vtable;\n};")
    );
    assert!(main_c.contains("struct LsVTable_Display {"));
    assert!(main_c.contains("void (*drop)(void* data);"));
    assert!(main_c.contains("void* (*clone)(void* data);"));
    assert!(main_c.contains("void (*trace)(void* data, LsTraceVisit visit);"));
    assert!(main_c.contains("LsString* (*debug)(LsString* acc, void* data);"));
    assert!(main_c.contains("LsUnit (*m0)(void* data);"));
    assert!(main_c.contains("int64_t (*m1)(void* data, int64_t);"));

    let table = "const LsVTable_Display lsaca_vtable_0 = {\n  .drop = lsaca_vtable_0_drop,\n  .clone = lsaca_vtable_0_clone,\n  .trace = lsaca_vtable_0_trace,\n  .debug = lsaca_vtable_0_debug,\n  .m0 = lsaca_vtable_0_m0,\n  .m1 = lsaca_vtable_0_m1,\n};";
    assert!(
        main_c.contains(table),
        "the vtable lists slots in the declared order"
    );

    assert!(main_c.contains("void lsaca_vtable_0_drop(void* data) {"));
    assert!(main_c.contains("    ls_release(data);"));
    assert!(main_c.contains("void* lsaca_vtable_0_clone(void* data) {"));
    assert!(main_c.contains(
        "clone->payload = ((LsString*)ls_retain((void*)((*((const LsString**)data)))));",
    ));
    assert!(main_c.contains("  return (void*)clone;"));
    assert!(main_c.contains("void lsaca_vtable_0_trace(void* data, LsTraceVisit visit) {"));
    assert!(main_c.contains("LsString* lsaca_vtable_0_debug(LsString* acc, void* data) {"));
    assert!(main_c.contains("acc = ls_fmt_debug_string(acc, ((*value)));"));
    assert!(main_c.contains("return display_fmt(((LsString**)data));"));
    assert!(main_c.contains("return display_len(((LsString**)data), a0);"));
}

#[test]
fn lowers_trait_object_coercions_and_dynamic_calls() {
    let program = phase_b_display_program();
    let main_c = generate(&program, &CodegenOptions::default()).main_c;

    assert!(main_c.contains("LsDyn0 v2 = ((LsDyn0){0});"));
    assert!(main_c.contains("t3 = (const LsString**)&(v1);"));
    assert!(main_c.contains("v2 = (LsDyn0){ .data = (void*)(t3), .vtable = &lsaca_vtable_0 };",));
    assert!(main_c.contains("v3 = (v2).vtable->m1((v2).data, t4);"));
    assert!(main_c.contains(
        "v4 = lsaca_box_new_Dyn0((LsDyn0){ .data = (void*)(t5), .vtable = &lsaca_vtable_0 });",
    ));
    assert!(main_c.contains("v5 = ((v4)->payload).vtable->m1(((v4)->payload).data, t7);"));
    assert!(main_c.contains("if (v4_live) {"));
    assert!(main_c.contains("lsaca_release_Box_Dyn0(v4);"));
    assert!(main_c.contains("fat.data = v->payload.vtable->clone(v->payload.data);"));
}

#[test]
fn lowers_slice_ranges_with_the_rust_panic_wording() {
    let program = phase_b_slice_program();
    let main_c = generate(&program, &CodegenOptions::default()).main_c;

    assert!(main_c.contains(" = (v1).data;"));
    assert!(main_c.contains(" = (v1).len;"));
    assert!(main_c.contains("int64_t slice_start"));
    assert!(main_c.contains("int64_t slice_end"));
    assert_ordered(
        &main_c,
        &[
            "slice index starts at ",
            "range end index ",
            "range start index ",
        ],
    );
    assert!(main_c.contains(" out of range for slice of length "));
    assert!(main_c.contains(".data = slice_output"));
    assert!(main_c.contains(".len = (size_t)((slice_end"));
    assert!(main_c.contains("v2 = (LsSlice_i64){ .data = "));
}

#[test]
fn lowers_str_ranges_through_the_runtime_validator() {
    let program = phase_b_str_slice_program();
    let main_c = generate(&program, &CodegenOptions::default()).main_c;

    assert!(main_c.contains("ls_str_data(v1);"));
    assert!(main_c.contains("ls_str_len(v1);"));
    assert!(main_c.contains("slice index starts at "));
    assert!(!main_c.contains("range end index"));
    assert!(!main_c.contains("range start index"));
    assert!(main_c.contains("ls_str_substring("));
    assert!(main_c.contains(".data = ((uint8_t*)(slice_data"));
    assert!(main_c.contains("v2 = (LsStr){ .data = "));
}

#[test]
fn routes_pointer_and_length_intrinsics() {
    let program = phase_b_intrinsic_program();
    let main_c = generate(&program, &CodegenOptions::default()).main_c;

    assert!(main_c.contains("v2 = (int64_t)((slice"));
    assert!(main_c.contains("v4 = (int64_t)((str"));
    assert!(main_c.contains("v5 = ls_str_from_parts((const char*)((str"));
    assert!(!main_c.contains("lsaca_fn___slice_len"));
    assert!(!main_c.contains("lsaca_fn___str_len"));
    assert!(!main_c.contains("lsaca_fn___str_from_str"));
}

#[test]
#[should_panic(expected = "cannot take a slice of this value")]
fn rejects_slices_of_values_without_a_buffer() {
    use crate::hir::{HExpr, HExprKind};
    use crate::span::Span;
    use crate::types::Ty;

    let empty = Span::empty(0);
    let slice_ty = Ty::Slice(Box::new(Ty::i64()));
    let slice_ref = phase_b_slice_reference(slice_ty.clone());
    let slice_expr = HExpr::new(
        slice_ref.clone(),
        HExprKind::Ref {
            mutable: false,
            expr: Box::new(HExpr::new(
                slice_ty,
                HExprKind::Slice {
                    base: Box::new(phase_b_local("number", "v1", Ty::i64())),
                    start: None,
                    end: None,
                    inclusive: false,
                },
                empty,
            )),
        },
        empty,
    );
    let body = phase_b_block(vec![
        phase_b_let("v1", "number", Ty::i64(), phase_b_int(4)),
        phase_b_let("v2", "window", slice_ref, slice_expr),
    ]);
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    let program = program_from("phase_b_bad_slice", vec![main], 0);
    let _ = generate(&program, &CodegenOptions::default());
}

#[test]
#[should_panic(expected = "no codegen emitter for intrinsic")]
fn rejects_unknown_intrinsics_with_an_internal_error() {
    use crate::hir::{HExpr, HExprKind, HLit, HParam, HStmt};
    use crate::types::{IntTy, Ty};

    let unknown = intrinsic_function(
        "__unknown",
        "lsaca_fn___unknown",
        vec![HParam {
            name: "value".to_string(),
            c_name: "p0".to_string(),
            ty: Ty::i64(),
        }],
        Ty::i64(),
    );
    let argument = HExpr::new(
        Ty::i64(),
        HExprKind::Lit(HLit::Int {
            value: 1,
            int_ty: IntTy::I64,
        }),
        crate::span::Span::empty(0),
    );
    let call = HExpr::new(
        Ty::i64(),
        HExprKind::DirectCall {
            fn_id: 0,
            receiver: None,
            args: vec![argument],
        },
        crate::span::Span::empty(0),
    );
    let body = crate::hir::HBlock {
        stmts: vec![HStmt::Expr(call)],
        tail: None,
        span: crate::span::Span::empty(0),
    };
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    let program = program_from("unknown_intrinsic", vec![unknown, main], 1);
    let _ = generate(&program, &CodegenOptions::default());
}

/// Builds a one-parameter intrinsic whose HIR name carries an instantiation
/// suffix and wires a call to it in main.
fn generic_push_program(instance: &str, c_name: &str) -> crate::hir::Program {
    use crate::hir::{HBuiltinAssoc, HExpr, HExprKind, HLit, HParam, HPat, HPatKind, HStmt};
    use crate::types::{IntTy, Ty};

    fn int_literal(value: i128) -> HExpr {
        HExpr::new(
            Ty::i64(),
            HExprKind::Lit(HLit::Int {
                value,
                int_ty: IntTy::I64,
            }),
            crate::span::Span::empty(0),
        )
    }
    fn binding(c_name: &str, name: &str, ty: Ty) -> HPat {
        HPat {
            ty,
            kind: HPatKind::Bind {
                c_name: c_name.to_string(),
                name: name.to_string(),
            },
            span: crate::span::Span::empty(0),
        }
    }

    let element = Ty::i64();
    let vector = Ty::vec(element.clone());
    let receiver_ty = Ty::Ref {
        mutable: true,
        inner: Box::new(vector.clone()),
    };
    let push = intrinsic_function(
        instance,
        c_name,
        vec![
            HParam {
                name: "self".to_string(),
                c_name: "p0".to_string(),
                ty: receiver_ty.clone(),
            },
            HParam {
                name: "value".to_string(),
                c_name: "p1".to_string(),
                ty: element,
            },
        ],
        Ty::unit(),
    );
    let local = HExpr::new(
        vector.clone(),
        HExprKind::Local {
            name: "v".to_string(),
            c_name: "v1".to_string(),
        },
        crate::span::Span::empty(0),
    );
    let receiver = HExpr::new(
        receiver_ty,
        HExprKind::Ref {
            mutable: true,
            expr: Box::new(local),
        },
        crate::span::Span::empty(0),
    );
    let call = HExpr::new(
        Ty::unit(),
        HExprKind::DirectCall {
            fn_id: 0,
            receiver: None,
            args: vec![receiver, int_literal(1)],
        },
        crate::span::Span::empty(0),
    );
    let init = HExpr::new(
        vector.clone(),
        HExprKind::BuiltinAssoc {
            func: HBuiltinAssoc::VecNew,
            args: Vec::new(),
            type_args: Vec::new(),
        },
        crate::span::Span::empty(0),
    );
    let body = crate::hir::HBlock {
        stmts: vec![
            HStmt::Let {
                pat: binding("v1", "v", vector),
                init: Some(init),
                span: crate::span::Span::empty(0),
            },
            HStmt::Expr(call),
        ],
        tail: None,
        span: crate::span::Span::empty(0),
    };
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    program_from("generic_intrinsic", vec![push, main], 1)
}

/// Generic intrinsic instances are registered under a name with the type
/// arguments appended (for example __vec_push<i64>), while the emitter tables
/// are keyed by the declared name. Both the instantiated and the bare spelling
/// must reach the same emitter; a name without an emitter is an internal error
/// instead of invalid C.
#[test]
fn routes_generic_intrinsic_instances_by_their_declaration_name() {
    for (instance, c_name) in [
        ("__vec_push<i64>", "lsaca_fn___vec_push_i64"),
        ("__vec_push", "lsaca_fn___vec_push"),
    ] {
        let program = generic_push_program(instance, c_name);
        let main_c = generate(&program, &CodegenOptions::default()).main_c;
        assert!(
            main_c.contains("lsaca_vec_push_slow_Vec_i64("),
            "the intrinsic {instance} was not expanded to the vector fast path:\n{main_c}"
        );
        assert_eq!(
            main_c.matches(c_name).count(),
            0,
            "the intrinsic {instance} got a C declaration:\n{main_c}"
        );
    }
}

/// The suffix may carry more than one type argument (HashMap methods are
/// generic over both the key and the value), so the normalisation cannot just
/// drop a single trailing identifier.
#[test]
fn routes_intrinsic_instances_with_two_type_arguments() {
    use crate::hir::{HBuiltinAssoc, HExpr, HExprKind, HLit, HParam, HPat, HPatKind, HStmt};
    use crate::types::{IntTy, Ty};

    let map_ty = Ty::HashMap(Box::new(Ty::string()), Box::new(Ty::i64()));
    let receiver_ty = Ty::Ref {
        mutable: true,
        inner: Box::new(map_ty.clone()),
    };
    let insert = intrinsic_function(
        "__map_insert<String, i64>",
        "lsaca_fn___map_insert_String_i64",
        vec![
            HParam {
                name: "self".to_string(),
                c_name: "p0".to_string(),
                ty: receiver_ty.clone(),
            },
            HParam {
                name: "key".to_string(),
                c_name: "p1".to_string(),
                ty: Ty::string(),
            },
            HParam {
                name: "value".to_string(),
                c_name: "p2".to_string(),
                ty: Ty::i64(),
            },
        ],
        Ty::unit(),
    );
    let map_local = HExpr::new(
        map_ty.clone(),
        HExprKind::Local {
            name: "m".to_string(),
            c_name: "v1".to_string(),
        },
        crate::span::Span::empty(0),
    );
    let map_receiver = HExpr::new(
        receiver_ty,
        HExprKind::Ref {
            mutable: true,
            expr: Box::new(map_local),
        },
        crate::span::Span::empty(0),
    );
    let key_local = HExpr::new(
        Ty::string(),
        HExprKind::Local {
            name: "key".to_string(),
            c_name: "v2".to_string(),
        },
        crate::span::Span::empty(0),
    );
    let value = HExpr::new(
        Ty::i64(),
        HExprKind::Lit(HLit::Int {
            value: 1,
            int_ty: IntTy::I64,
        }),
        crate::span::Span::empty(0),
    );
    let call = HExpr::new(
        Ty::unit(),
        HExprKind::DirectCall {
            fn_id: 0,
            receiver: None,
            args: vec![map_receiver, key_local, value],
        },
        crate::span::Span::empty(0),
    );
    let map_init = HExpr::new(
        map_ty.clone(),
        HExprKind::BuiltinAssoc {
            func: HBuiltinAssoc::HashMapNew,
            args: Vec::new(),
            type_args: vec![Ty::string(), Ty::i64()],
        },
        crate::span::Span::empty(0),
    );
    let key_init = HExpr::new(
        Ty::string(),
        HExprKind::Lit(HLit::Str("a".to_string())),
        crate::span::Span::empty(0),
    );
    let map_pat = HPat {
        ty: map_ty,
        kind: HPatKind::Bind {
            c_name: "v1".to_string(),
            name: "m".to_string(),
        },
        span: crate::span::Span::empty(0),
    };
    let key_pat = HPat {
        ty: Ty::string(),
        kind: HPatKind::Bind {
            c_name: "v2".to_string(),
            name: "key".to_string(),
        },
        span: crate::span::Span::empty(0),
    };
    let body = crate::hir::HBlock {
        stmts: vec![
            HStmt::Let {
                pat: map_pat,
                init: Some(map_init),
                span: crate::span::Span::empty(0),
            },
            HStmt::Let {
                pat: key_pat,
                init: Some(key_init),
                span: crate::span::Span::empty(0),
            },
            HStmt::Expr(call),
        ],
        tail: None,
        span: crate::span::Span::empty(0),
    };
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    let program = program_from("generic_map", vec![insert, main], 1);
    let main_c = generate(&program, &CodegenOptions::default()).main_c;
    assert!(
        main_c.contains("lsaca_map_insert_String_i64("),
        "the two-argument intrinsic instance was not expanded:\n{main_c}"
    );
    assert_eq!(
        main_c.matches("lsaca_fn___map_insert_String_i64").count(),
        0,
        "the intrinsic got a C declaration:\n{main_c}"
    );
}

#[test]
#[should_panic(expected = "__bogus<i64>")]
fn reports_the_full_generic_name_of_unknown_intrinsics() {
    use crate::hir::{HExpr, HExprKind, HLit, HParam, HStmt};
    use crate::types::{IntTy, Ty};

    let unknown = intrinsic_function(
        "__bogus<i64>",
        "lsaca_fn___bogus_i64",
        vec![HParam {
            name: "value".to_string(),
            c_name: "p0".to_string(),
            ty: Ty::i64(),
        }],
        Ty::i64(),
    );
    let argument = HExpr::new(
        Ty::i64(),
        HExprKind::Lit(HLit::Int {
            value: 1,
            int_ty: IntTy::I64,
        }),
        crate::span::Span::empty(0),
    );
    let call = HExpr::new(
        Ty::i64(),
        HExprKind::DirectCall {
            fn_id: 0,
            receiver: None,
            args: vec![argument],
        },
        crate::span::Span::empty(0),
    );
    let body = crate::hir::HBlock {
        stmts: vec![HStmt::Expr(call)],
        tail: None,
        span: crate::span::Span::empty(0),
    };
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    let program = program_from("unknown_generic_intrinsic", vec![unknown, main], 1);
    let _ = generate(&program, &CodegenOptions::default());
}

/// Standard library impl blocks declare their receiver as a reference
/// (self: &mut Vec<T>), so the builtin emitters must look through the
/// reference when choosing the vector representation: the element-wise fast
/// path for Vec<i64> and the frozen runtime helpers for Vec<String>.
#[test]
fn pushes_through_mutable_vector_references_with_the_element_representation() {
    let (main_c, _) = generate_source(
        "vec_mut_refs",
        r#"fn add(values: &mut Vec<i64>) {
    values.push(7);
}

fn add_names(names: &mut Vec<String>) {
    names.push(String::from("x"));
}

fn main() {
    let mut values: Vec<i64> = Vec::new();
    add(&mut values);
    let mut names: Vec<String> = Vec::new();
    add_names(&mut names);
    println!("{} {}", values.len(), names.len());
}
"#,
        true,
    );
    assert!(
        main_c.contains("lsaca_vec_push_slow_Vec_i64("),
        "a push through &mut Vec<i64> did not use the element fast path:\n{main_c}"
    );
    assert!(
        main_c.contains("ls_vec_string_push("),
        "a push through &mut Vec<String> did not use the frozen runtime helper:\n{main_c}"
    );
    assert!(
        !main_c.contains("lsaca_vec_push_slow_Vec_String("),
        "a Vec<String> push must not use the non-frozen element path:\n{main_c}"
    );
}

/// Reachability pruning drops functions that the entry point never references,
/// which keeps the standard-library prelude out of every generated program.
#[test]
fn prunes_functions_unreachable_from_the_entry_point() {
    use crate::hir::{HExpr, HExprKind, HStmt};
    use crate::types::Ty;

    let unused = checked_function(
        "unused",
        "lsaca_fn_unused",
        Vec::new(),
        Ty::i64(),
        empty_block(),
    );
    let used = checked_function(
        "used",
        "lsaca_fn_used",
        Vec::new(),
        Ty::i64(),
        empty_block(),
    );
    let call = HExpr::new(
        Ty::i64(),
        HExprKind::DirectCall {
            fn_id: 1,
            receiver: None,
            args: Vec::new(),
        },
        crate::span::Span::empty(0),
    );
    let body = crate::hir::HBlock {
        stmts: vec![HStmt::Expr(call)],
        tail: None,
        span: crate::span::Span::empty(0),
    };
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    let program = program_from("pruning", vec![unused, used, main], 2);
    let generated = generate(&program, &CodegenOptions::default());

    assert!(generated.main_c.contains("int64_t lsaca_fn_used()"));
    assert!(
        !generated.main_c.contains("lsaca_fn_unused"),
        "the unreachable function was emitted:\n{}",
        generated.main_c
    );

    let reachability =
        super::reachability::Reachability::compute(&program, &CodegenOptions::default(), &[]);
    assert!(!reachability.is_reachable(0));
    assert!(reachability.is_reachable(1));
    assert!(reachability.is_reachable(2));
    assert!(!reachability.keeps_everything());
}

/// Calling a function value whose identity the walker cannot prove keeps every
/// function: pruning must stay conservative.
#[test]
fn keeps_every_function_when_a_call_target_is_indirect() {
    use crate::hir::{HExpr, HExprKind, HStmt};
    use crate::types::Ty;

    let unused = checked_function(
        "unused",
        "lsaca_fn_unused",
        Vec::new(),
        Ty::i64(),
        empty_block(),
    );
    let callee = HExpr::new(
        Ty::unit(),
        HExprKind::Local {
            name: "callback".to_string(),
            c_name: "v1".to_string(),
        },
        crate::span::Span::empty(0),
    );
    let call = HExpr::new(
        Ty::unit(),
        HExprKind::Call {
            callee: Box::new(callee),
            args: Vec::new(),
        },
        crate::span::Span::empty(0),
    );
    let body = crate::hir::HBlock {
        stmts: vec![HStmt::Expr(call)],
        tail: None,
        span: crate::span::Span::empty(0),
    };
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    let program = program_from("indirect", vec![unused, main], 1);
    let reachability =
        super::reachability::Reachability::compute(&program, &CodegenOptions::default(), &[]);

    assert!(reachability.keeps_everything());
    assert!(reachability.is_reachable(0));
}

/// Every intrinsic of the frozen Phase C ABI has an emitter.
///
/// A program that calls each declared intrinsic once must generate C instead
/// of failing with an internal error, and each call site must reach the
/// matching runtime function. Building all calls in one program keeps the
/// test close to the real call shapes of the standard library: borrowed byte
/// slices, option results built from borrowed runtime handles, managed vector
/// blocks that are stolen from and formatted time tuples.
#[test]
fn expands_every_phase_c_intrinsic_into_a_runtime_call() {
    use crate::hir::{HBuiltinAssoc, HExpr, HExprKind, HLit, HParam, HPat, HPatKind, HStmt};
    use crate::span::Span;
    use crate::types::{IntTy, Ty};

    let empty = Span::empty(0);
    let byte = Ty::Int(IntTy::U8);
    let bytes_ty = Ty::vec(byte.clone());
    let slice_ty = Ty::slice(byte.clone());
    let shared_bytes = Ty::Ref {
        mutable: false,
        inner: Box::new(slice_ty.clone()),
    };
    let mut_bytes = Ty::Ref {
        mutable: true,
        inner: Box::new(slice_ty.clone()),
    };
    let option_string = Ty::option(Ty::string());
    let string_vector = Ty::vec(Ty::string());
    let read_dir = Ty::option(string_vector.clone());
    let metadata = Ty::tuple(vec![Ty::i64(), Ty::i64(), Ty::i64(), Ty::i64()]);
    let parts = Ty::tuple(vec![Ty::i64(); 9]);

    // Name, parameter types, return type and the runtime entry point the call
    // site must reach, in Appendix C order.
    let table: [(&str, Vec<Ty>, Ty, &str); 40] = [
        ("__io_stdin", Vec::new(), Ty::i64(), "ls_io_stdin()"),
        ("__io_stdout", Vec::new(), Ty::i64(), "ls_io_stdout()"),
        ("__io_stderr", Vec::new(), Ty::i64(), "ls_io_stderr()"),
        ("__io_open", vec![Ty::string()], Ty::i64(), "ls_io_open("),
        (
            "__io_create",
            vec![Ty::string()],
            Ty::i64(),
            "ls_io_create(",
        ),
        ("__io_close", vec![Ty::i64()], Ty::i64(), "ls_io_close("),
        (
            "__io_read",
            vec![Ty::i64(), mut_bytes.clone()],
            Ty::i64(),
            "ls_io_read(",
        ),
        (
            "__io_write",
            vec![Ty::i64(), shared_bytes.clone()],
            Ty::i64(),
            "ls_io_write(",
        ),
        (
            "__io_seek",
            vec![Ty::i64(), Ty::i64(), Ty::i64()],
            Ty::i64(),
            "ls_io_seek(",
        ),
        ("__io_flush", vec![Ty::i64()], Ty::i64(), "ls_io_flush("),
        ("__io_size", vec![Ty::i64()], Ty::i64(), "ls_io_size("),
        (
            "__io_set_nonblocking",
            vec![Ty::i64(), Ty::Bool],
            Ty::i64(),
            "ls_io_set_nonblocking(",
        ),
        (
            "__io_last_error_kind",
            Vec::new(),
            Ty::i64(),
            "ls_io_last_error_kind()",
        ),
        (
            "__io_last_error_message",
            Vec::new(),
            Ty::string(),
            "ls_io_last_error_message()",
        ),
        (
            "__io_read_dir",
            vec![Ty::string()],
            read_dir.clone(),
            "ls_io_read_dir(",
        ),
        (
            "__io_metadata",
            vec![Ty::string()],
            metadata.clone(),
            "ls_io_metadata(",
        ),
        (
            "__io_remove_file",
            vec![Ty::string()],
            Ty::i64(),
            "ls_io_remove_file(",
        ),
        (
            "__io_remove_dir",
            vec![Ty::string()],
            Ty::i64(),
            "ls_io_remove_dir(",
        ),
        (
            "__io_remove_dir_all",
            vec![Ty::string()],
            Ty::i64(),
            "ls_io_remove_dir_all(",
        ),
        (
            "__io_create_dir",
            vec![Ty::string()],
            Ty::i64(),
            "ls_io_create_dir(",
        ),
        (
            "__io_create_dir_all",
            vec![Ty::string()],
            Ty::i64(),
            "ls_io_create_dir_all(",
        ),
        (
            "__io_rename",
            vec![Ty::string(), Ty::string()],
            Ty::i64(),
            "ls_io_rename(",
        ),
        (
            "__io_canonicalize",
            vec![Ty::string()],
            option_string.clone(),
            "ls_io_canonicalize(",
        ),
        (
            "__io_current_dir",
            Vec::new(),
            option_string.clone(),
            "ls_io_current_dir()",
        ),
        (
            "__io_set_current_dir",
            vec![Ty::string()],
            Ty::i64(),
            "ls_io_set_current_dir(",
        ),
        (
            "__io_temp_dir",
            Vec::new(),
            Ty::string(),
            "ls_io_temp_dir()",
        ),
        (
            "__net_listen",
            vec![Ty::string()],
            Ty::i64(),
            "ls_net_listen(",
        ),
        (
            "__net_local_port",
            vec![Ty::i64()],
            Ty::i64(),
            "ls_net_local_port(",
        ),
        ("__net_accept", vec![Ty::i64()], Ty::i64(), "ls_net_accept("),
        (
            "__net_connect",
            vec![Ty::string()],
            Ty::i64(),
            "ls_net_connect(",
        ),
        (
            "__net_set_timeout",
            vec![Ty::i64(), Ty::i64()],
            Ty::i64(),
            "ls_net_set_timeout(",
        ),
        (
            "__net_shutdown",
            vec![Ty::i64(), Ty::i64()],
            Ty::i64(),
            "ls_net_shutdown(",
        ),
        ("__time_now_ms", Vec::new(), Ty::i64(), "ls_time_now_ms()"),
        (
            "__time_monotonic_ns",
            Vec::new(),
            Ty::i64(),
            "ls_time_monotonic_ns()",
        ),
        (
            "__time_sleep_ms",
            vec![Ty::i64()],
            Ty::unit(),
            "ls_time_sleep_ms(",
        ),
        (
            "__time_local_parts",
            vec![Ty::i64()],
            parts.clone(),
            "ls_time_local_parts(",
        ),
        (
            "__time_utc_parts",
            vec![Ty::i64()],
            parts.clone(),
            "ls_time_utc_parts(",
        ),
        (
            "__env_var",
            vec![Ty::string()],
            option_string.clone(),
            "ls_env_var(",
        ),
        (
            "__env_vars",
            Vec::new(),
            string_vector.clone(),
            "ls_env_vars()",
        ),
        (
            "__proc_args",
            Vec::new(),
            string_vector.clone(),
            "ls_args_at(",
        ),
    ];

    let bytes_local = HExpr::new(
        bytes_ty.clone(),
        HExprKind::Local {
            name: "bytes".to_string(),
            c_name: "v1".to_string(),
        },
        empty,
    );

    fn dummy_argument(ty: &Ty, bytes_ty: &Ty, bytes_local: &HExpr) -> HExpr {
        let empty = Span::empty(0);
        match ty {
            Ty::Int(_) => HExpr::new(
                ty.clone(),
                HExprKind::Lit(HLit::Int {
                    value: 0,
                    int_ty: IntTy::I64,
                }),
                empty,
            ),
            Ty::Bool => HExpr::new(ty.clone(), HExprKind::Lit(HLit::Bool(false)), empty),
            Ty::String => HExpr::new(
                Ty::string(),
                HExprKind::BuiltinAssoc {
                    func: HBuiltinAssoc::StringNew,
                    args: Vec::new(),
                    type_args: Vec::new(),
                },
                empty,
            ),
            Ty::Ref { mutable, inner } if inner.is_slice() => {
                let _ = bytes_ty;
                let base = HExpr::new(
                    (**inner).clone(),
                    HExprKind::Slice {
                        base: Box::new(bytes_local.clone()),
                        start: None,
                        end: None,
                        inclusive: false,
                    },
                    empty,
                );
                HExpr::new(
                    ty.clone(),
                    HExprKind::Ref {
                        mutable: *mutable,
                        expr: Box::new(base),
                    },
                    empty,
                )
            }
            other => panic!("no dummy argument for {:?}", other),
        }
    }

    let mut fns = Vec::new();
    for (name, params, ret, _) in &table {
        let declared = params
            .iter()
            .enumerate()
            .map(|(index, ty)| HParam {
                name: format!("a{}", index),
                c_name: format!("p{}", index),
                ty: ty.clone(),
            })
            .collect();
        let c_name = format!("lsaca_fn_{}", name.trim_start_matches("__"));
        fns.push(intrinsic_function(name, &c_name, declared, ret.clone()));
    }

    let mut stmts = vec![HStmt::Let {
        pat: HPat {
            ty: bytes_ty.clone(),
            kind: HPatKind::Bind {
                c_name: "v1".to_string(),
                name: "bytes".to_string(),
            },
            span: empty,
        },
        init: Some(HExpr::new(
            bytes_ty.clone(),
            HExprKind::BuiltinAssoc {
                func: HBuiltinAssoc::VecNew,
                args: Vec::new(),
                type_args: vec![byte.clone()],
            },
            empty,
        )),
        span: empty,
    }];
    for (fn_id, (_, params, ret, _)) in table.iter().enumerate() {
        let args = params
            .iter()
            .map(|ty| dummy_argument(ty, &bytes_ty, &bytes_local))
            .collect();
        stmts.push(HStmt::Expr(HExpr::new(
            ret.clone(),
            HExprKind::DirectCall {
                fn_id,
                receiver: None,
                args,
            },
            empty,
        )));
    }
    let body = crate::hir::HBlock {
        stmts,
        tail: None,
        span: empty,
    };
    let main = checked_function("main", "lsaca_fn_main", Vec::new(), Ty::unit(), body);
    fns.push(main);
    let main_index = fns.len() - 1;
    let program = program_from("phase_c", fns, main_index);
    let main_c = generate(&program, &CodegenOptions::default()).main_c;

    assert_eq!(
        table.len(),
        super::phase_c::PHASE_C_INTRINSICS.len(),
        "the frozen Phase C ABI table changed size"
    );
    for (name, _, _, runtime) in &table {
        assert!(
            super::phase_c::is_phase_c_intrinsic(name),
            "the Phase C ABI list is missing {}",
            name
        );
        assert!(
            main_c.contains(runtime),
            "the intrinsic {} did not reach {}: {}",
            name,
            runtime,
            main_c
        );
        let c_name = format!("lsaca_fn_{}", name.trim_start_matches("__"));
        assert!(
            !main_c.contains(&c_name),
            "the intrinsic {} got a C declaration: {}",
            name,
            main_c
        );
    }
    assert!(
        !super::phase_c::is_phase_c_intrinsic("__io_bogus"),
        "an unknown name must not be claimed by the Phase C ABI"
    );
    assert!(
        main_c.contains("(void*)(("),
        "the read buffer is not passed as a byte pointer: {}",
        main_c
    );
    assert!(
        main_c.contains("(const void*)(("),
        "the write buffer is not passed as a const byte pointer: {}",
        main_c
    );
    assert!(
        main_c.contains(".tag = 1;"),
        "option results are not marked as present: {}",
        main_c
    );
    assert!(
        main_c.contains("->len = 0;"),
        "the managed vector block was not cleared before release: {}",
        main_c
    );
    assert!(
        main_c.contains("read_dir_block"),
        "the read directory block was not hoisted: {}",
        main_c
    );
    assert!(
        main_c.contains("&part_year"),
        "the formatted time tuple out parameters are not passed: {}",
        main_c
    );
    assert!(
        main_c.contains("ls_io_temp_dir()"),
        "the temporary directory path is not guarded: {}",
        main_c
    );
}

const FUSED_PIPELINE: &str = "fn main() {\n    let mut values: Vec<i64> = Vec::new();\n    values.push(2);\n    values.push(3);\n    values.push(4);\n    let total: i64 = values.iter().filter(|x| *x % 2 == 0).map(|x| x * x).fold(0, |acc, x| acc + x);\n    println!(\"{}\", total);\n}\n";

#[test]
fn fuses_iterator_pipeline_into_a_c_loop() {
    let (main_c, _) = generate_source("fused_pipeline", FUSED_PIPELINE, false);
    let body = user_main_body(&main_c);
    assert!(
        body.contains("for (;;)"),
        "the iterator pipeline did not fuse into a C loop: {}",
        body
    );
    assert!(
        body.contains(".len) break;") && body.contains("data[i"),
        "the fused loop does not walk the source vector in place: {}",
        body
    );
    assert!(
        !body.contains("iter_lsaca_fn___lsaca_it_"),
        "the fused pipeline still calls the generic iterator helpers: {}",
        body
    );
    assert!(
        !body.contains("lsaca_closure_"),
        "the fused pipeline still materialises closures: {}",
        body
    );
}

const FUSED_FOR_LOOP: &str = "fn main() {\n    let mut values: Vec<i64> = Vec::new();\n    values.push(1);\n    values.push(2);\n    values.push(3);\n    let mut total = 0;\n    for value in values.iter().filter(|x| *x % 2 == 0) {\n        total += value;\n    }\n    println!(\"{}\", total);\n}\n";

#[test]
fn fuses_for_loop_over_iterator_pipeline() {
    let (main_c, _) = generate_source("fused_for", FUSED_FOR_LOOP, false);
    let body = user_main_body(&main_c);
    assert!(
        body.contains("data[i") && body.contains(".len) break;"),
        "the for loop over the iterator pipeline did not fuse: {}",
        body
    );
    assert!(
        !body.contains("iter_lsaca_fn___lsaca_it_"),
        "the fused for loop still calls the generic iterator helpers: {}",
        body
    );
    assert!(
        !body.contains("lsaca_closure_"),
        "the fused for loop still materialises closures: {}",
        body
    );
}

const FUSED_ENUMERATE: &str = "fn main() {\n    let values: Vec<i64> = vec![4, 5, 6];\n    let mut total = 0;\n    for pair in values.iter().enumerate() {\n        let (index, value) = pair;\n        total += index * value;\n    }\n    println!(\"{}\", total);\n}\n";

#[test]
fn fuses_enumerate_pairs_without_nesting_the_element_tuple() {
    let (main_c, _) = generate_source("fused_enumerate", FUSED_ENUMERATE, false);
    let body = user_main_body(&main_c);
    assert!(
        body.contains("for (;;)"),
        "the enumerate loop did not fuse: {}",
        body
    );
    assert!(
        body.contains("LsTup2_i64_i64 pair"),
        "the fused enumerate pair must be a plain (i64, i64) tuple: {}",
        body
    );
    assert!(
        !body.contains("LsTup2_i64_LsTup2"),
        "the fused enumerate pair nested the element tuple: {}",
        body
    );
}

const CAPTURING_PIPELINE: &str = "fn main() {\n    let base = 10;\n    let mut values: Vec<i64> = Vec::new();\n    values.push(2);\n    values.push(3);\n    let total: i64 = values.iter().map(|x| x + base).fold(0, |acc, x| acc + x);\n    println!(\"{}\", total);\n}\n";

#[test]
fn keeps_generic_lowering_for_capturing_closures() {
    let (main_c, _) = generate_source("fused_capture", CAPTURING_PIPELINE, false);
    let body = user_main_body(&main_c);
    assert!(
        body.contains("iter_lsaca_fn___lsaca_it_map_args_"),
        "the capturing pipeline must fall back to the generic iterator helpers: {}",
        body
    );
    assert!(
        body.contains("lsaca_closure_"),
        "the capturing pipeline must materialise the closure: {}",
        body
    );
    assert!(
        !body.contains("data[i"),
        "the capturing pipeline must not fuse into a loop: {}",
        body
    );
}

const CALLS_FUNCTION_VALUE_WITHOUT_COPY: &str = "fn main() {\n    let double = |x: i64| x * 2;\n    let mut total = 0;\n    let mut i = 0;\n    while i < 3 {\n        total += double(i);\n        i += 1;\n    }\n    println!(\"{}\", total);\n}\n";

#[test]
fn calls_a_function_value_through_its_place_without_copying() {
    let (main_c, _) = generate_source("fn_value_place", CALLS_FUNCTION_VALUE_WITHOUT_COPY, false);
    let body = user_main_body(&main_c);
    assert!(
        body.contains(".fn_ptr("),
        "the function value is not called through its C struct: {}",
        body
    );
    assert!(
        !body.contains("lsaca_copy_Fn1_i64_to_i64"),
        "calling a function value from a place must not retain its environment: {}",
        body
    );
    assert!(
        !body.contains("lsaca_env_retain("),
        "calling a function value from a place must not retain its environment: {}",
        body
    );
}

const MAP_PLACE_KEYS: &str = "fn main() {\n    let mut counts: HashMap<String, i64> = HashMap::new();\n    counts.insert(format!(\"a\"), 1);\n    let absent = format!(\"b\");\n    let hit = counts.contains_key(absent);\n    let value = counts.get(absent).unwrap_or(0);\n    counts.remove(absent);\n    println!(\"{}, {}\", hit, value);\n}\n";

#[test]
fn borrows_place_keys_in_map_lookups() {
    let (main_c, _) = generate_source("map_place_key", MAP_PLACE_KEYS, false);
    let body = user_main_body(&main_c);
    for lookup in [
        "lsaca_map_contains_String_i64(",
        "lsaca_map_get_String_i64(",
        "lsaca_map_remove_String_i64(",
    ] {
        assert!(
            body.contains(lookup),
            "the map lookup {} is missing from the emitted main: {}",
            lookup,
            body
        );
    }
    assert!(
        !body.contains("ls_retain("),
        "a map lookup over a place key must not retain the key: {}",
        body
    );
}
