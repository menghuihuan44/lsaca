//! End-to-end coverage for the v2.4 collection entry idioms and iterator
//! terminals/adapters. The fixtures live under tests/programs; this test keeps
//! the expectations next to them without touching the main e2e harness.

use std::path::PathBuf;
use std::process::Command;

use lsaca::codegen::CodegenOptions;
use lsaca::driver::{self, BuildOptions, Frontend};

const ADAPTER_STDOUT: &str = r#"filter_map 1
filter_map 2
filtered [20]
left 1
right 10
zipped [(1, 10)]
uneven [(1, 10), (2, 20)]
chain left 1
chained [1]
full chain [1, 2, 7, 8]
take_while 1
take_while 2
take_while 3
prefix [1, 2]
skip_while 1
skip_while 2
skip_while 3
suffix [3, 1]
position sees 1
position sees 2
position Some(1)
position missing None
nth Some(30)
nth missing None
last Some(6)
last empty None
"#;

const NUMERIC_STDOUT: &str = r#"i32 min Some(-2) max Some(9)
i64 min Some(-2) max Some(9)
u32 min Some(2) max Some(9)
u64 min Some(2) max Some(9)
isize min Some(-8) max Some(4)
usize min Some(2) max Some(9)
f32 min Some(-2.5) max Some(4.5)
f64 min Some(-2.5) max Some(4.5)
product i32 -6
product i64 24
product u32 24
product u64 24
product isize 24
product usize 24
product f32 4.5
product f64 3.75
empty min None max None
empty i32 product 1
empty f64 product 1
"#;

const COLLECTIONS_STDOUT: &str = r#"first 10
again 11
computed b
inserted 20
existing 20
computed c
captured 31
grown 7 21
set [2, 1, 4, 3] 4
btree [1, 2, 3] Some(1) Some(3)
bmap [("a", 1), ("b", 2)]
first key Some("a") last key Some("b")
bmap a Some(10)
deque [1, 2, 3] Some(1) Some(3)
deque get Some(20) [1, 20, 3]
deque from [7, 8]
heap [3, 2, 1]
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
fn v24_collections_and_iterator_fixtures() {
    if let Err(error) = driver::find_c_compiler() {
        eprintln!("skipping v2.4 fixtures: {error}");
        return;
    }
    let cases = [
        (
            "collections_v24",
            include_str!("programs/collections_v24.ls"),
            COLLECTIONS_STDOUT,
        ),
        (
            "iter_v24_adapters",
            include_str!("programs/iter_v24_adapters.ls"),
            ADAPTER_STDOUT,
        ),
        (
            "iter_v24_numeric",
            include_str!("programs/iter_v24_numeric.ls"),
            NUMERIC_STDOUT,
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
