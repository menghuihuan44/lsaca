//! Unit tests for the C code generator.
//!
//! Each test compiles a small source program through the front end, runs the
//! generator and inspects the produced main.c text. Building and running the
//! generated C project is covered by the end to end test suite, so these tests
//! stay fast and do not need a C toolchain.

use super::{CodegenOptions, generate};
use crate::driver::Frontend;

fn generate_source(name: &str, source: &str, debug_checks: bool) -> (String, String) {
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
        source: Some(frontend.file().clone()),
        ..CodegenOptions::default()
    };
    let generated = generate(&program, &options);
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
        source: Some(frontend.file().clone()),
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
    let a2 = a;
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
    let mut b = a;
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

    assert!(
        !body.contains("lsaca_copy_Vec_i64("),
        "the loop snapshotted the vector:\n{body}"
    );
    assert!(
        body.contains(">= (v1).len) break;"),
        "the loop does not read the vector length in place:\n{body}"
    );
    assert!(
        body.contains("= (v1).data[i"),
        "the loop does not index the vector buffer directly:\n{body}"
    );
}

const FOR_VEC_SNAPSHOT_PROGRAM: &str = r#"
fn main() {
    let mut values: Vec<i64> = Vec::new();
    values.push(1);
    values.push(2);
    let mut sum = 0;
    for value in values {
        sum += value + values.len();
    }
    println!("{}", sum);
}
"#;

#[test]
fn snapshots_a_local_vector_when_the_body_references_it() {
    let (main_c, _) = generate_source("for_vec_snapshot_fixture", FOR_VEC_SNAPSHOT_PROGRAM, true);
    let body = main_function_body(&main_c);

    assert!(
        body.contains("= lsaca_copy_Vec_i64(v1);"),
        "the loop mutates or aliases the vector but no snapshot is taken:\n{body}"
    );
    assert!(
        body.contains(">= (t"),
        "the loop does not iterate the snapshot:\n{body}"
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
    // The only remaining format buffer is the one built by println!.
    assert_eq!(
        body.matches("ls_format_begin(0)").count(),
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
