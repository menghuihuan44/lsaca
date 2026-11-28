//! End-to-end coverage for the v2.5 system foundation stdlib: raw pointer
//! constructors, MaybeUninit, CString/NulError, CStr/Utf8Error, and the
//! String/Vec buffer pointers. The fixture lives under tests/programs; this
//! test keeps its expected stdout next to it without touching the main e2e
//! harness.

use std::path::PathBuf;
use std::process::Command;

use lsaca::codegen::CodegenOptions;
use lsaca::driver::{self, BuildOptions, Frontend};

const SYSTEM_STDOUT: &str = r#"null true true
null eq true
uninit 42
raw write 7 7
point 3 4
moved text 11 moved value
moved vec 2 4
cstring 5 true
bytes 5 104
rebuilt 5
rebuilt first 104
empty 0
empty view 0 0
nul 2
view 6 héllo
utf8 1
string 108 5
empty string true
vec 5 5 2
empty vec true true
"#;

fn cache_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("LSACA_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/e2e-build-cache")
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn build_program(name: &str, source: &str) -> Result<PathBuf, String> {
    let frontend = Frontend::new(format!("{name}.ls"), source);
    let program = match frontend.check() {
        Ok(program) => program,
        Err(diagnostics) => {
            return Err(format!(
                "the front end rejected {name}:\n{}",
                diagnostics.render(frontend.sources())
            ));
        }
    };
    let program_name = sanitize_name(name);
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
        Err(error) => Err(format!("building {name} failed:\n{error}")),
    }
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

#[test]
fn v25_system_stdlib_fixture() {
    if let Err(error) = driver::find_c_compiler() {
        eprintln!("skipping v2.5 fixture: {error}");
        return;
    }
    let cases = [(
        "stdlib_system",
        include_str!("programs/stdlib_system.ls"),
        SYSTEM_STDOUT,
    )];
    let mut failures = Vec::new();
    for (name, source, expected) in cases {
        let executable = match build_program(name, source) {
            Ok(executable) => executable,
            Err(error) => {
                failures.push(error);
                continue;
            }
        };
        let output = match Command::new(&executable).output() {
            Ok(output) => output,
            Err(error) => {
                failures.push(format!("could not run {name}: {error}"));
                continue;
            }
        };
        let stdout = normalize(&String::from_utf8_lossy(&output.stdout));
        let stderr = normalize(&String::from_utf8_lossy(&output.stderr));
        if stdout != expected {
            failures.push(format!(
                "{name}: stdout mismatch\n--- expected ---\n{expected:?}\n--- actual ---\n{stdout:?}"
            ));
        }
        if output.status.code() != Some(0) {
            failures.push(format!(
                "{name}: expected exit 0, got {:?}",
                output.status.code()
            ));
        }
        if stderr.contains("reference-counted allocation") {
            failures.push(format!(
                "{name}: leaked reference counted allocations:\n{stderr}"
            ));
        }
    }
    if !failures.is_empty() {
        panic!("{}", failures.join("\n\n"));
    }
}
