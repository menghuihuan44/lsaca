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
    assert!(cmake.contains("add_executable(hello main.c lsaca_rt.c)"));
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
    assert!(main_c.contains("typedef struct LsEnvHeader"));
    assert!(main_c.contains("lsaca_env_retain("));
    assert!(main_c.contains("lsaca_env_release("));
    assert!(main_c.contains("lsaca_env_drop_"));
    assert!(main_c.contains("lsaca_closure_"));
    assert!(main_c.contains("ls_alloc(sizeof(LsEnv_"));
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
