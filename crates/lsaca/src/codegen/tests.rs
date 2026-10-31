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
        source: Some(frontend.file().clone()),
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
    // The mutable reference parameters are aliased, not copied: the receiver
    // temp is initialised from the parameter itself.
    assert!(
        main_c.contains("t2 = p0;"),
        "the String reference parameter is not loaded directly:\n{main_c}"
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
    assert!(
        body.contains("= (int64_t)ls_str_char_count("),
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
        body.contains("v1 = ls_str_push_scalar(v1, (uint32_t)(t3));"),
        "String::push does not reassign the copy-on-write handle:\n{body}"
    );
    assert!(
        body.contains("v1 = ls_str_pop_scalar(v1, &popped7, &ch8);"),
        "String::pop does not reassign the copy-on-write handle:\n{body}"
    );
    assert!(
        body.contains("if (ls_str_char_at(v1, (int64_t)(t4), &ch5)) {"),
        "String::char_at does not use the runtime out-parameter:\n{body}"
    );
    assert!(
        body.contains("if (popped7) {") && body.contains("v4 = ls_opt_char_some(ch8);"),
        "String::pop does not build the Option<char> from the runtime result:\n{body}"
    );
    assert!(
        body.contains("v3 = ls_str_is_char_boundary(v1, (int64_t)(t6));"),
        "String::is_char_boundary is not a plain boolean query:\n{body}"
    );
    assert!(
        body.contains("v5 = ls_str_from_scalar((uint32_t)(t9));"),
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
