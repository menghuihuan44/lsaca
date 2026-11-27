//! End-to-end coverage for the v2.3 Unicode character methods and the new
//! Option/Result combinators. The fixtures live under tests/programs; this test
//! keeps the expectations next to them without touching the main e2e harness.

use std::path::PathBuf;
use std::process::Command;

use lsaca::codegen::CodegenOptions;
use lsaca::driver::{self, BuildOptions, Frontend};

const OPTION_STDOUT: &str = "10 -1\n5\n7\n5 9\ntrue false\nfalse true\n6 -1\nLSACA\nnone\nlsaca\nfallback\nlsaca\nfresh\ntrue false\ntrue false\njoined\nnone\nerr missing\nok lsaca\n";

const RESULT_STDOUT: &str = "8 0\n5 4\n7\n4\n4\ntrue false\ntrue false\ninspect ok 4\n4\ninspect err 4\nboom\n4 4\nKEPT\nnone\nKEPT\nLOST\nbackup\nkept\nLOST\ntrue false\ntrue false\ninspect text 4\nkept\ninspect text err 4\nlost\nkept\nLOST\nerr missing\n";

const CHAR_STDOUT: &str = "true true false\ntrue false true\ntrue true false\ntrue true\ntrue false\ntrue true\ntrue true\ntrue false\ntrue true\ntrue true\ntrue true false\ntrue true true\ntrue true true false\nA z\nÉ é Ÿ\nω Ω Σ\nж Я Ё\n山 中\nS 83\ni 105\nI 73\nF 70\nß 223\nSS\n";

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
fn v23_combinators_and_unicode_fixtures() {
    if let Err(error) = driver::find_c_compiler() {
        eprintln!("skipping v2.3 fixtures: {error}");
        return;
    }
    let cases = [
        (
            "option_combinators",
            include_str!("programs/option_combinators.ls"),
            OPTION_STDOUT,
        ),
        (
            "result_combinators",
            include_str!("programs/result_combinators.ls"),
            RESULT_STDOUT,
        ),
        (
            "char_unicode",
            include_str!("programs/char_unicode.ls"),
            CHAR_STDOUT,
        ),
    ];
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
