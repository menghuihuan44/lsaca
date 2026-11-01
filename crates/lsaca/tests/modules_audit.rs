//! Independent audit corpus for the v1.7 module system (task-52).
//!
//! The fixtures live in `crates/lsaca/tests/programs/modules/audit_*` and were
//! written from `target/v17/DESIGN.md` section 5 (modules, visibility, imports)
//! plus Rust 2021 parity rulings.  Each positive fixture is a complete multi-file
//! module program that must compile and print exactly `stdout`; each negative
//! fixture must be rejected by the front end with the expected diagnostic code
//! and message needles.
//!
//! The tables are generated from `target/v17/audit-fixtures.json`; keep both in
//! sync when a fixture is added.  Like `e2e.rs`, the compiled cases are skipped
//! when no C compiler is available so `cargo test` works without a toolchain.

use std::path::PathBuf;
use std::process::Command;

use lsaca::codegen::CodegenOptions;
use lsaca::driver::{self, BuildOptions, Frontend};

/// Positive module programs: directory name and the exact expected stdout.
const POSITIVES: &[(&str, &str)] = &[
    ("audit_borrow_self_method", "1 2\n"),
    ("audit_borrow_then_move_after_use", "5 5\n"),
    ("audit_crate_use", "10\n"),
    ("audit_cross_module_drop", "alive 1\ndrop 1\n"),
    ("audit_cross_module_method", "7 3\n"),
    ("audit_cross_module_rc_move", "hi hi 2\n"),
    ("audit_duplicate_names_in_modules_ok", "3\n"),
    ("audit_enum_variant_public", "9\n"),
    ("audit_for_consumes_module_vec", "6\n"),
    ("audit_inline_mod", "6\n"),
    ("audit_main_in_module", "5\n"),
    ("audit_mod_file_style", "4\n"),
    ("audit_move_returned_value", "2\n"),
    ("audit_private_child_access", "3\n"),
    ("audit_private_mod_root_access", "7\n"),
    ("audit_pub_crate_field", "3\n"),
    ("audit_pub_crate_method", "4 6\n"),
    ("audit_pub_crate_module", "4\n"),
    ("audit_pub_crate_same_crate", "6\n"),
    ("audit_pub_use_module", "6\n"),
    ("audit_self_use", "9\n"),
    ("audit_struct_literal_public_fields", "3 4\n"),
    ("audit_super_chain", "12\n"),
    ("audit_trait_cross_module", "3 8\n"),
    ("audit_type_alias_cross_module", "5\n"),
    ("audit_uniform_use", "5\n"),
    ("audit_uniform_use_nested", "6\n"),
    ("audit_use_alias", "5\n"),
    ("audit_use_group", "7 3\n"),
    ("audit_use_group_alias", "7 3\n"),
    ("audit_use_reexport_alias", "13\n"),
    ("audit_use_reexport_deep", "11\n"),
    ("audit_use_reexport_type", "7\n"),
    ("audit_use_self_super", "3\n"),
];

/// Negative module programs: directory name, expected diagnostic codes, and
/// message fragments that must appear in the rendered diagnostics.
const NEGATIVES: &[(&str, &[&str], &[&str])] = &[
    (
        "audit_neg_ambiguous_alias",
        &["E0252"],
        &["multiple times", "f"],
    ),
    (
        "audit_neg_borrow_then_move_cross_module_value",
        &["E0505"],
        &["cannot move out of", "main.ls"],
    ),
    (
        "audit_neg_borrow_then_move_in_module",
        &["E0505"],
        &["cannot move out of", "helper.ls"],
    ),
    ("audit_neg_circular_modules", &["E0001"], &["circular"]),
    (
        "audit_neg_duplicate_import",
        &["E0252"],
        &["multiple times"],
    ),
    ("audit_neg_duplicate_module", &["E0428"], &["dup"]),
    (
        "audit_neg_duplicate_module_file",
        &["E0761"],
        &["found at both"],
    ),
    ("audit_neg_glob", &["E0432"], &["glob"]),
    (
        "audit_neg_import_vs_item",
        &["E0255"],
        &["multiple times", "add"],
    ),
    (
        "audit_neg_missing_module",
        &["E0583"],
        &["not found", "missing"],
    ),
    (
        "audit_neg_nested_private_module",
        &["E0603"],
        &["private", "inner"],
    ),
    (
        "audit_neg_private_assoc_fn",
        &["E0624"],
        &["private", "hidden"],
    ),
    ("audit_neg_private_const", &["E0603"], &["private", "LIMIT"]),
    ("audit_neg_private_enum", &["E0603"], &["private", "Hidden"]),
    ("audit_neg_private_field", &["E0616"], &["private"]),
    (
        "audit_neg_private_field_literal",
        &["E0451"],
        &["private", "value"],
    ),
    ("audit_neg_private_fn", &["E0603"], &["private", "hidden"]),
    (
        "audit_neg_private_method",
        &["E0624"],
        &["private", "secret"],
    ),
    (
        "audit_neg_private_trait",
        &["E0603"],
        &["private", "Hidden"],
    ),
    ("audit_neg_private_type", &["E0603"], &["private", "Hidden"]),
    (
        "audit_neg_pub_use_private_item",
        &["E0364"],
        &["only_in_crate"],
    ),
    (
        "audit_neg_pub_use_private_module",
        &["E0365"],
        &["re-export", "inner"],
    ),
    (
        "audit_neg_restricted_visibility",
        &["E0001"],
        &["restricted visibility", "pub(super)"],
    ),
    ("audit_neg_super_at_root", &["E0433"], &["super"]),
    ("audit_neg_unresolved_use", &["E0432"], &["missing"]),
    (
        "audit_neg_write_while_shared_in_module",
        &["E0506"],
        &["cannot assign to", "helper.ls"],
    ),
];

fn audit_dir(case: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/programs/modules")
        .join(case)
}

/// Cache root shared with the end-to-end tests; respects `LSACA_CACHE_DIR`.
fn cache_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("LSACA_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/e2e-build-cache")
}

fn sanitize_test_name(name: &str) -> String {
    let mut output = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    output
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Compile a loaded module program and return the executable path.
fn build_loaded(frontend: &Frontend, case: &str) -> Result<PathBuf, String> {
    let program = match frontend.check() {
        Ok(program) => program,
        Err(diagnostics) => {
            return Err(format!(
                "the front end rejected {}:\n{}",
                case,
                diagnostics.render(frontend.sources())
            ));
        }
    };
    let program_name = sanitize_test_name(case);
    let options = CodegenOptions {
        program_name: program_name.clone(),
        debug_checks: true,
        emit_location_info: true,
        sources: Some(frontend.sources().clone()),
        ..CodegenOptions::default()
    };
    let generated = driver::generate_c(&program, &options);
    let build = BuildOptions::new(program_name);
    match driver::build_with_cache(&generated, &build, &cache_root()) {
        Ok(built) => Ok(built.executable),
        Err(error) => Err(format!("building {} failed:\n{}", case, error)),
    }
}

fn run_loaded(frontend: &Frontend, case: &str) -> Result<(String, String, i32), String> {
    let executable = build_loaded(frontend, case)?;
    let output = match Command::new(&executable).output() {
        Ok(output) => output,
        Err(error) => return Err(format!("could not run {}: {}", case, error)),
    };
    Ok((
        normalize(&String::from_utf8_lossy(&output.stdout)),
        normalize(&String::from_utf8_lossy(&output.stderr)),
        output.status.code().unwrap_or(-1),
    ))
}

/// Positive fixtures: every module program loads, type checks, compiles to C,
/// builds, and prints its documented output without leaking reference counts.
#[test]
fn audit_positive_module_programs_compile_and_run() {
    if driver::find_c_compiler().is_err() {
        eprintln!("skipping the module audit positives: no C compiler found");
        return;
    }
    let mut failures: Vec<String> = Vec::new();
    for (case, expected_stdout) in POSITIVES {
        let path = audit_dir(case).join("main.ls");
        let frontend = match Frontend::load(&path) {
            Ok(frontend) => frontend,
            Err(error) => {
                failures.push(format!("{}: could not load the program: {}", case, error));
                continue;
            }
        };
        match run_loaded(&frontend, case) {
            Ok((stdout, stderr, exit)) => {
                if stdout != *expected_stdout {
                    failures.push(format!(
                        "{}: stdout mismatch\n--- expected ---\n{:?}\n--- actual ---\n{:?}",
                        case, expected_stdout, stdout
                    ));
                }
                if exit != 0 {
                    failures.push(format!("{}: exit code {}, expected 0", case, exit));
                }
                if stderr.contains("reference-counted allocation") {
                    failures.push(format!(
                        "{}: leaked reference counted allocations\n--- stderr ---\n{}",
                        case, stderr
                    ));
                }
            }
            Err(error) => failures.push(format!("{}: {}", case, error)),
        }
    }
    if !failures.is_empty() {
        panic!(
            "{} of {} module audit positives failed:\n\n{}",
            failures.len(),
            POSITIVES.len(),
            failures.join("\n\n")
        );
    }
}

/// Negative fixtures: visibility, import, duplicate-name, module-file, and
/// borrow/move violations must be rejected with the documented error code and
/// message fragments.
#[test]
fn audit_negative_module_programs_are_rejected() {
    let mut failures: Vec<String> = Vec::new();
    for (case, expected_codes, needles) in NEGATIVES {
        let path = audit_dir(case).join("main.ls");
        let frontend = match Frontend::load(&path) {
            Ok(frontend) => frontend,
            Err(error) => {
                failures.push(format!("{}: could not load the program: {}", case, error));
                continue;
            }
        };
        let diagnostics = match frontend.check() {
            Ok(_) => {
                failures.push(format!("{}: the program was accepted", case));
                continue;
            }
            Err(diagnostics) => diagnostics,
        };
        let codes: Vec<&str> = diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.code)
            .collect();
        for expected in *expected_codes {
            if !codes.contains(expected) {
                failures.push(format!(
                    "{}: expected diagnostic {}, got {:?}",
                    case, expected, codes
                ));
            }
        }
        let rendered = diagnostics.render(frontend.sources());
        for needle in *needles {
            if !rendered.contains(needle) {
                failures.push(format!(
                    "{}: diagnostics do not mention {:?}\n--- diagnostics ---\n{}",
                    case, needle, rendered
                ));
            }
        }
    }
    if !failures.is_empty() {
        panic!(
            "{} of {} module audit negatives failed:\n\n{}",
            failures.len(),
            NEGATIVES.len(),
            failures.join("\n\n")
        );
    }
}
