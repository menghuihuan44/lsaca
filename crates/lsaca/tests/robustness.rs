//! Robustness and fuzz tests for the v1.7 module tree and package front end.
//!
//! Every case runs inside a bounded worker thread (see run_bounded) so a
//! regression that hangs the loader fails the test instead of hanging CI.
//! Cases are intentionally adversarial: malformed module trees, malformed
//! manifests, abused dependency graphs and random parser input. A case passes
//! when the compiler reports a normal diagnostic and never panics.
//!
//! Scratch files live under CARGO_TARGET_TMPDIR (or target/) and are removed
//! when the test finishes.

use std::fs;
use std::panic;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use lsaca::diag::Diagnostics;
use lsaca::driver::{Frontend, LoadError};
use lsaca::span::{SourceFile, SourceMap};
use lsaca::{lexer, package, parser};

// ---------------------------------------------------------------------------
// Bounded worker and scratch directories
// ---------------------------------------------------------------------------

const CASE_BUDGET: Duration = Duration::from_secs(30);
const SLOW_BUDGET: Duration = Duration::from_secs(180);

static SCRATCH_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Run one robustness case on a worker thread with a hard time bound.
///
/// A case that panics fails the test with the original panic. A case that
/// exceeds the budget fails the test with a clear message; the worker thread
/// is abandoned (the test binary exits afterwards).
fn run_bounded<F>(case: &'static str, budget: Duration, body: F)
where
    F: FnOnce() + Send + 'static,
{
    let (sender, receiver) = mpsc::channel::<bool>();
    let handle = std::thread::Builder::new()
        .name(format!("robustness-{case}"))
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            let outcome = panic::catch_unwind(panic::AssertUnwindSafe(body));
            let _ = sender.send(outcome.is_ok());
            if let Err(payload) = outcome {
                panic::resume_unwind(payload);
            }
        })
        .expect("failed to spawn the robustness worker thread");
    match receiver.recv_timeout(budget) {
        Ok(healthy) => {
            let joined = handle.join();
            assert!(healthy, "robustness case {case} reported a worker failure");
            if let Err(payload) = joined {
                panic::resume_unwind(payload);
            }
        }
        Err(_) => panic!(
            "robustness case {case} did not finish within {} seconds",
            budget.as_secs()
        ),
    }
}

fn case<F>(case: &'static str, body: F)
where
    F: FnOnce() + Send + 'static,
{
    run_bounded(case, CASE_BUDGET, body);
}

fn slow_case<F>(case: &'static str, body: F)
where
    F: FnOnce() + Send + 'static,
{
    run_bounded(case, SLOW_BUDGET, body);
}

/// A unique scratch directory removed on drop.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let root = scratch_root();
        fs::create_dir_all(&root).expect("failed to create the robustness scratch root");
        let unique = SCRATCH_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = root.join(format!(
            "{}-{}-{}",
            std::process::id(),
            unique,
            sanitize_tag(tag)
        ));
        fs::create_dir_all(&dir).expect("failed to create a robustness scratch directory");
        Scratch { dir }
    }

    fn path(&self, relative: &str) -> PathBuf {
        let mut path = self.dir.clone();
        for part in relative.split('/') {
            if !part.is_empty() && part != "." {
                path.push(part);
            }
        }
        path
    }

    fn write(&self, relative: &str, contents: &str) {
        self.write_bytes(relative, contents.as_bytes());
    }

    fn write_bytes(&self, relative: &str, contents: &[u8]) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create a scratch subdirectory");
        }
        fs::write(&path, contents).expect("failed to write a scratch file");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn scratch_root() -> PathBuf {
    if let Some(root) = std::env::var_os("CARGO_TARGET_TMPDIR") {
        return PathBuf::from(root);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
}

fn sanitize_tag(tag: &str) -> String {
    tag.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

// ---------------------------------------------------------------------------
// Diagnostic helpers
// ---------------------------------------------------------------------------

fn messages(diagnostics: &Diagnostics) -> String {
    diagnostics
        .as_slice()
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect::<Vec<_>>()
        .join("; ")
}

fn has_code(diagnostics: &Diagnostics, code: &str) -> bool {
    diagnostics
        .as_slice()
        .iter()
        .any(|diagnostic| matches!(diagnostic.code, Some(found) if found == code))
}

/// Fail when the compiler produced something that looks like a crash.
fn assert_clean_failure(case_name: &str, text: &str) {
    assert!(
        !text.trim().is_empty(),
        "{case_name}: expected a diagnostic, got an empty message"
    );
    for marker in [
        "panicked at",
        "the compiler crashed",
        "internal compiler error",
        "stack overflow",
        "RUST_BACKTRACE",
    ] {
        assert!(
            !text.contains(marker),
            "{case_name}: diagnostic contains crash marker {marker}: {text}"
        );
    }
}

fn assert_clean_diagnostics(case_name: &str, diagnostics: &Diagnostics) {
    assert_clean_failure(case_name, &messages(diagnostics));
}

fn frontend_errors(frontend: &Frontend) -> Diagnostics {
    match frontend.check() {
        Ok(_) => Diagnostics::new(),
        Err(diagnostics) => diagnostics,
    }
}

fn rendered(frontend: &Frontend, diagnostics: &Diagnostics) -> String {
    diagnostics.render(frontend.sources())
}

fn expect_clean_check(frontend: &Frontend) {
    match frontend.check() {
        Ok(_) => {}
        Err(diagnostics) => panic!(
            "expected the program to check cleanly, got:\n{}",
            diagnostics.render(frontend.sources())
        ),
    }
}

fn load_ok(scratch: &Scratch, relative: &str) -> Frontend {
    let path = scratch.path(relative);
    match Frontend::load(&path) {
        Ok(frontend) => frontend,
        Err(error) => panic!("failed to load {relative}: {error}"),
    }
}

fn failed_load_diagnostics(scratch: &Scratch, relative: &str) -> (Frontend, Diagnostics) {
    let frontend = load_ok(scratch, relative);
    let diagnostics = frontend_errors(&frontend);
    (frontend, diagnostics)
}

fn source_diagnostics(name: &str, source: &str) -> Diagnostics {
    let file = SourceFile::new(name, source);
    let (tokens, mut diagnostics) = lexer::tokenize(source);
    let (_, parse_diagnostics) = parser::parse(&tokens, &file);
    diagnostics.extend(parse_diagnostics);
    diagnostics
}

// ---------------------------------------------------------------------------
// Package helpers
// ---------------------------------------------------------------------------

fn write_package(
    scratch: &Scratch,
    relative: &str,
    manifest: &str,
    main: Option<&str>,
    library: Option<&str>,
) -> PathBuf {
    scratch.write(&format!("{relative}/lsaca.toml"), manifest);
    if let Some(source) = main {
        scratch.write(&format!("{relative}/src/main.ls"), source);
    }
    if let Some(source) = library {
        scratch.write(&format!("{relative}/src/lib.ls"), source);
    }
    scratch.path(relative)
}

fn binary_package(scratch: &Scratch, relative: &str, manifest: &str) -> PathBuf {
    write_package(scratch, relative, manifest, Some("fn main() { }\n"), None)
}

fn library_package(scratch: &Scratch, relative: &str, manifest: &str) -> PathBuf {
    write_package(
        scratch,
        relative,
        manifest,
        None,
        Some("pub fn value() -> i64 { 1 }\n"),
    )
}

fn package_ok(scratch: &Scratch, relative: &str) -> Frontend {
    let path = scratch.path(relative);
    match Frontend::load_package(&path) {
        Ok(frontend) => frontend,
        Err(error) => panic!("package {relative} should load: {}", error.render()),
    }
}

fn package_failure(scratch: &Scratch, relative: &str) -> String {
    let path = scratch.path(relative);
    match Frontend::load_package(&path) {
        Ok(_) => panic!("package {relative} should have been rejected"),
        Err(LoadError::Io(error)) => format!("{error}"),
        Err(LoadError::Diagnostics {
            diagnostics,
            sources,
        }) => diagnostics.render(&sources),
    }
}

fn manifest_diagnostics(scratch: &Scratch, relative: &str) -> (bool, Diagnostics, SourceMap) {
    let path = scratch.path(relative);
    let mut sources = SourceMap::new();
    let mut diagnostics = Diagnostics::new();
    let parsed = package::parse_manifest(&path, &mut sources, &mut diagnostics);
    (parsed.is_some(), diagnostics, sources)
}

/// Write a module chain: main.ls declares m0, m0 declares m1, and so on.
///
/// Module resolution is strictly tree shaped: the file for a module lives in
/// the child directory of its parent module, so the chain is written as nested
/// directories (`m0.ls` then `m0/m1.ls` then `m0/m1/m2.ls`).
///
/// The generated names stay short on purpose: the nesting test writes hundreds
/// of levels deep, and the total generated path must stay below the platform
/// limit (macOS accepts at most 1024 bytes per path). Four-character names such
/// as `m000` overflow that budget at this depth, while the compact names used
/// here fit comfortably: the first 26 modules are single letters and the rest
/// are a letter followed by a digit.
fn write_module_chain(scratch: &Scratch, relative: &str, length: usize) {
    let name = |index: usize| {
        if index < 26 {
            char::from(b'a' + index as u8).to_string()
        } else {
            let rest = index - 26;
            format!("{}{}", char::from(b'a' + (rest / 10) as u8), rest % 10)
        }
    };
    scratch.write(
        &format!("{relative}/main.ls"),
        &format!("mod {};\nfn main() {{ }}\n", name(0)),
    );
    for index in 0..length {
        let contents = if index + 1 < length {
            format!("mod {};\n", name(index + 1))
        } else {
            "pub fn leaf() -> i64 { 7 }\n".to_string()
        };
        let mut path = relative.to_string();
        for parent in 0..index {
            path.push('/');
            path.push_str(&name(parent));
        }
        path.push('/');
        path.push_str(&name(index));
        path.push_str(".ls");
        scratch.write(&path, &contents);
    }
}

/// Write a dependency chain: app depends on dep000, which depends on dep001...
fn write_dependency_chain(scratch: &Scratch, length: usize) {
    let manifest = String::from(
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\ndep000 = { path = \"deps/dep000\" }\n",
    );
    scratch.write("app/lsaca.toml", &manifest);
    scratch.write("app/src/main.ls", "fn main() { }\n");
    for index in 0..length {
        let name = format!("dep{:03}", index);
        let next = index + 1;
        let mut dep_manifest = format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n");
        if next < length {
            dep_manifest.push_str("\n[dependencies]\n");
            dep_manifest.push_str(&format!(
                "dep{:03} = {{ path = \"../dep{:03}\" }}\n",
                next, next
            ));
        }
        scratch.write(&format!("app/deps/{name}/lsaca.toml"), &dep_manifest);
        scratch.write(
            &format!("app/deps/{name}/src/lib.ls"),
            "pub fn value() -> i64 { 1 }\n",
        );
    }
}

// ---------------------------------------------------------------------------
// Malformed module trees
// ---------------------------------------------------------------------------

#[test]
fn rejects_missing_module_files() {
    case("missing-module", || {
        let scratch = Scratch::new("missing-module");
        scratch.write("main.ls", "mod absent;\nfn main() { }\n");
        let (frontend, diagnostics) = failed_load_diagnostics(&scratch, "main.ls");
        assert!(
            has_code(&diagnostics, "E0583"),
            "expected E0583: {}",
            messages(&diagnostics)
        );
        let text = rendered(&frontend, &diagnostics);
        assert!(text.contains("file not found for module"), "{text}");
        assert!(text.contains("not found"), "{text}");
        assert_clean_failure("missing module", &text);
    });
}

#[test]
fn rejects_ambiguous_module_files() {
    case("ambiguous-module", || {
        let scratch = Scratch::new("ambiguous-module");
        scratch.write("main.ls", "mod both;\nfn main() { }\n");
        scratch.write("both.ls", "pub fn flat() -> i64 { 1 }\n");
        scratch.write("both/mod.ls", "pub fn nested() -> i64 { 2 }\n");
        let (frontend, diagnostics) = failed_load_diagnostics(&scratch, "main.ls");
        assert!(
            has_code(&diagnostics, "E0761"),
            "expected E0761: {}",
            messages(&diagnostics)
        );
        let text = rendered(&frontend, &diagnostics);
        assert!(text.contains("found at both"), "{text}");
        assert_clean_failure("ambiguous module", &text);
    });
}

#[test]
fn rejects_duplicate_module_declarations() {
    case("duplicate-module", || {
        let scratch = Scratch::new("duplicate-module");
        scratch.write("main.ls", "mod dup;\nmod dup;\nfn main() { }\n");
        scratch.write("dup.ls", "pub fn value() -> i64 { 1 }\n");
        let (frontend, diagnostics) = failed_load_diagnostics(&scratch, "main.ls");
        assert!(
            has_code(&diagnostics, "E0428"),
            "expected E0428: {}",
            messages(&diagnostics)
        );
        let text = rendered(&frontend, &diagnostics);
        assert!(text.contains("defined multiple times"), "{text}");
        assert_clean_failure("duplicate module", &text);
    });
}

#[test]
fn rejects_circular_module_files() {
    case("circular-module", || {
        let scratch = Scratch::new("circular-module");
        // Module resolution is strictly tree shaped, so the file of a module
        // always lives strictly below the directory of its parent module. The
        // one reachable cycle is a root file that declares a module with its
        // own file stem: the flat candidate is the root file itself.
        scratch.write("helper.ls", "mod helper;\n");
        let (frontend, diagnostics) = failed_load_diagnostics(&scratch, "helper.ls");
        assert!(
            has_code(&diagnostics, "E0001"),
            "expected E0001: {}",
            messages(&diagnostics)
        );
        let text = rendered(&frontend, &diagnostics);
        assert!(text.contains("circular module files"), "{text}");
        assert!(text.contains("helper.ls"), "{text}");
        assert_clean_failure("circular module", &text);
    });
}

#[test]
fn references_to_parent_files_resolve_as_missing_modules() {
    case("parent-module-reference", || {
        // Modules form a tree: a module file's children are looked up in that
        // module's own directory, so a reference back to the root file is a
        // missing module rather than a cycle.
        let scratch = Scratch::new("parent-module-reference");
        scratch.write("main.ls", "mod helper;\nfn main() { }\n");
        scratch.write("helper.ls", "mod main;\n");
        let (frontend, diagnostics) = failed_load_diagnostics(&scratch, "main.ls");
        assert!(
            has_code(&diagnostics, "E0583"),
            "expected E0583: {}",
            messages(&diagnostics)
        );
        let text = rendered(&frontend, &diagnostics);
        assert!(text.contains("file not found for module `main`"), "{text}");
        assert_clean_failure("parent module reference", &text);
    });
}

#[test]
fn rejects_self_referential_module_file() {
    case("self-module", || {
        let scratch = Scratch::new("self-module");
        scratch.write("main.ls", "mod main;\nfn main() { }\n");
        let (frontend, diagnostics) = failed_load_diagnostics(&scratch, "main.ls");
        assert!(
            has_code(&diagnostics, "E0001"),
            "expected E0001: {}",
            messages(&diagnostics)
        );
        let text = rendered(&frontend, &diagnostics);
        assert!(text.contains("circular module files"), "{text}");
        assert_clean_failure("self module", &text);
    });
}

#[test]
fn rejects_module_chain_deeper_than_256() {
    case("deep-module-chain", || {
        let scratch = Scratch::new("deep-module-chain");
        // 257 levels is the smallest chain that exceeds the 256-level limit,
        // and its generated nested path stays well under the macOS limit.
        write_module_chain(&scratch, "deep", 257);
        let (frontend, diagnostics) = failed_load_diagnostics(&scratch, "deep/main.ls");
        assert!(
            has_code(&diagnostics, "E0001"),
            "expected E0001: {}",
            messages(&diagnostics)
        );
        let text = rendered(&frontend, &diagnostics);
        assert!(text.contains("nesting is deeper than 256"), "{text}");
        assert_clean_failure("deep module chain", &text);
    });
}

#[test]
fn loads_module_chain_within_the_limit() {
    case("module-chain-ok", || {
        let scratch = Scratch::new("module-chain-ok");
        write_module_chain(&scratch, "chain", 100);
        let frontend = load_ok(&scratch, "chain/main.ls");
        assert_eq!(
            frontend.sources().len() - lsaca::stdlib::files().len(),
            101,
            "one source per loaded module file"
        );
        let diagnostics = frontend_errors(&frontend);
        assert!(!diagnostics.has_errors(), "{}", messages(&diagnostics));
    });
}

#[test]
fn allows_empty_module_files() {
    case("empty-module", || {
        let scratch = Scratch::new("empty-module");
        scratch.write("main.ls", "mod empty;\nfn main() { }\n");
        scratch.write("empty.ls", "");
        let frontend = load_ok(&scratch, "main.ls");
        assert_eq!(frontend.sources().len() - lsaca::stdlib::files().len(), 2);
        expect_clean_check(&frontend);
    });
}

#[test]
fn rejects_invalid_utf8_root_file() {
    case("invalid-utf8-root", || {
        let scratch = Scratch::new("invalid-utf8-root");
        scratch.write_bytes("main.ls", &[0x66, 0x6e, 0xff, 0xfe, 0x00, 0x01]);
        let path = scratch.path("main.ls");
        match Frontend::load(&path) {
            Ok(_) => panic!("invalid UTF-8 root file should not load"),
            Err(error) => {
                let text = format!("{error}");
                assert!(text.contains("not valid UTF-8"), "{text}");
                assert_clean_failure("invalid utf8 root", &text);
            }
        }
    });
}

#[test]
fn rejects_invalid_utf8_module_file() {
    case("invalid-utf8-module", || {
        let scratch = Scratch::new("invalid-utf8-module");
        scratch.write("main.ls", "mod broken;\nfn main() { }\n");
        scratch.write_bytes("broken.ls", &[0x6d, 0x6f, 0x64, 0x20, 0xff]);
        let (frontend, diagnostics) = failed_load_diagnostics(&scratch, "main.ls");
        assert!(
            has_code(&diagnostics, "E0583"),
            "expected E0583: {}",
            messages(&diagnostics)
        );
        let text = rendered(&frontend, &diagnostics);
        assert!(text.contains("could not read module"), "{text}");
        assert!(text.contains("not valid UTF-8"), "{text}");
        assert_clean_failure("invalid utf8 module", &text);
    });
}

#[test]
fn accepts_bom_and_crlf_sources() {
    case("bom-crlf", || {
        let scratch = Scratch::new("bom-crlf");
        let main = "\u{feff}mod helper;\r\nfn main() { }\r\n";
        scratch.write("main.ls", main);
        scratch.write("helper.ls", "\u{feff}\r\npub fn value() -> i64 { 1 }\r\n");
        let frontend = load_ok(&scratch, "main.ls");
        assert_eq!(frontend.sources().len() - lsaca::stdlib::files().len(), 2);
        expect_clean_check(&frontend);
    });
}

#[test]
fn loads_one_megabyte_module_file() {
    slow_case("large-module", || {
        let scratch = Scratch::new("large-module");
        scratch.write("main.ls", "mod big;\nfn main() { }\n");
        let mut big = String::from("// large module padding\n");
        big.push_str(&"// a filler comment line to grow the file\n".repeat(25_000));
        big.push_str("pub fn value() -> i64 { 7 }\n");
        assert!(big.len() > 1_000_000);
        scratch.write("big.ls", &big);
        let frontend = load_ok(&scratch, "main.ls");
        assert_eq!(frontend.sources().len() - lsaca::stdlib::files().len(), 2);
        assert_eq!(frontend.sources().file(1).text().len(), big.len());
    });
}

#[test]
fn reports_malformed_module_source_cleanly() {
    case("malformed-module", || {
        let scratch = Scratch::new("malformed-module");
        scratch.write("main.ls", "mod broken;\nfn main() { }\n");
        scratch.write("broken.ls", "fn broken( { let =\n");
        let (frontend, diagnostics) = failed_load_diagnostics(&scratch, "main.ls");
        assert!(
            diagnostics.has_errors(),
            "expected parse errors in the module file"
        );
        let text = rendered(&frontend, &diagnostics);
        assert_clean_failure("malformed module source", &text);
    });
}

#[test]
fn loads_nested_module_directories() {
    case("nested-module-dir", || {
        let scratch = Scratch::new("nested-module-dir");
        scratch.write("main.ls", "mod util;\nfn main() { }\n");
        scratch.write(
            "util/mod.ls",
            "mod math;\npub fn id(value: i64) -> i64 { value }\n",
        );
        scratch.write(
            "util/math.ls",
            "pub fn double(value: i64) -> i64 { value * 2 }\n",
        );
        let frontend = load_ok(&scratch, "main.ls");
        assert_eq!(frontend.sources().len() - lsaca::stdlib::files().len(), 3);
        expect_clean_check(&frontend);
    });
}

#[test]
fn loads_many_sibling_modules() {
    case("many-siblings", || {
        let scratch = Scratch::new("many-siblings");
        let mut main = String::new();
        for index in 0..50 {
            main.push_str(&format!("mod part{index};\n"));
        }
        main.push_str("fn main() { }\n");
        scratch.write("main.ls", &main);
        for index in 0..50 {
            scratch.write(&format!("part{index}.ls"), "pub fn value() -> i64 { 1 }\n");
        }
        let frontend = load_ok(&scratch, "main.ls");
        assert_eq!(frontend.sources().len() - lsaca::stdlib::files().len(), 51);
    });
}

#[test]
fn rejects_deeply_nested_inline_modules() {
    case("deep-inline-modules", || {
        let scratch = Scratch::new("deep-inline-modules");
        let mut source = String::new();
        for index in 0..300 {
            source.push_str(&format!("mod level{index} {{ "));
        }
        source.push_str("fn leaf() { } ");
        for _ in 0..300 {
            source.push('}');
        }
        source.push_str("\nfn main() { }\n");
        scratch.write("main.ls", &source);
        let (frontend, diagnostics) = failed_load_diagnostics(&scratch, "main.ls");
        assert!(
            has_code(&diagnostics, "E0001"),
            "expected E0001: {}",
            messages(&diagnostics)
        );
        let text = rendered(&frontend, &diagnostics);
        assert!(text.contains("nested too deeply"), "{text}");
        assert_clean_failure("deep inline modules", &text);
    });
}

// ---------------------------------------------------------------------------
// Malformed manifests
// ---------------------------------------------------------------------------

#[test]
fn rejects_invalid_manifest_syntax() {
    case("bad-manifests", || {
        let scratch = Scratch::new("bad-manifests");
        let cases: &[(&str, &str, &str)] = &[
            (
                "bad-toml",
                "[package\nname = \"app\"\n",
                "malformed table header",
            ),
            (
                "unknown-section",
                "[package]\nname = \"app\"\n\n[features]\nfoo = 1\n",
                "unknown section",
            ),
            (
                "unknown-key",
                "[package]\nname = \"app\"\nauthor = \"me\"\n",
                "unknown key",
            ),
            ("outside-table", "name = \"app\"\n", "inside"),
            (
                "duplicate-table",
                "[package]\nname = \"app\"\n\n[package]\nname = \"other\"\n",
                "more than once",
            ),
            (
                "duplicate-key",
                "[package]\nname = \"app\"\nname = \"other\"\n",
                "duplicate",
            ),
        ];
        for (name, manifest, expected) in cases {
            let relative = format!("manifests/{name}");
            scratch.write(&format!("{relative}/lsaca.toml"), manifest);
            let (parsed, diagnostics, sources) =
                manifest_diagnostics(&scratch, &format!("{relative}/lsaca.toml"));
            assert!(!parsed, "{name}: malformed manifest should be rejected");
            assert!(
                has_code(&diagnostics, "E0001"),
                "{name}: expected E0001: {}",
                messages(&diagnostics)
            );
            let text = diagnostics.render(&sources);
            assert!(
                text.to_lowercase().contains(&expected.to_lowercase()),
                "{name}: expected message to contain {expected}: {text}"
            );
            assert_clean_failure(name, &text);
        }
    });
}

#[test]
fn rejects_invalid_package_names() {
    case("bad-names", || {
        let scratch = Scratch::new("bad-names");
        for name in ["Hello", "9app", "_", "", "app-name", "fn", "APP"] {
            let relative = format!("names/{}/lsaca.toml", sanitize_tag(name));
            scratch.write(
                &relative,
                &format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n"),
            );
            let (parsed, diagnostics, sources) = manifest_diagnostics(&scratch, &relative);
            assert!(!parsed, "package name {name:?} should be rejected");
            assert!(has_code(&diagnostics, "E0001"), "{name:?}: expected E0001");
            let text = diagnostics.render(&sources);
            assert!(
                text.to_lowercase().contains("name"),
                "{name:?}: expected a name diagnostic: {text}"
            );
            assert_clean_failure("invalid package name", &text);
        }
    });
}

#[test]
fn accepts_valid_package_names() {
    case("good-names", || {
        let scratch = Scratch::new("good-names");
        for name in ["app", "a", "a1", "_x", "my_crate2"] {
            let relative = format!("names/{}/lsaca.toml", sanitize_tag(name));
            scratch.write(&relative, &format!("[package]\nname = \"{name}\"\n"));
            let (parsed, diagnostics, sources) = manifest_diagnostics(&scratch, &relative);
            assert!(
                parsed,
                "{name:?} should be accepted: {}",
                diagnostics.render(&sources)
            );
        }
    });
}

#[test]
fn rejects_invalid_versions() {
    case("bad-versions", || {
        let scratch = Scratch::new("bad-versions");
        for version in ["1.2", "abc", "1.2.3.4", "1..2"] {
            let relative = format!("versions/{}/lsaca.toml", sanitize_tag(version));
            scratch.write(
                &relative,
                &format!("[package]\nname = \"app\"\nversion = \"{version}\"\n"),
            );
            let (parsed, diagnostics, sources) = manifest_diagnostics(&scratch, &relative);
            assert!(!parsed, "version {version:?} should be rejected");
            let text = diagnostics.render(&sources);
            assert!(
                text.contains("version"),
                "{version:?}: expected a version diagnostic: {text}"
            );
            assert_clean_failure("invalid version", &text);
        }
    });
}

#[test]
fn manifest_defaults_to_version_zero_one_zero() {
    case("default-version", || {
        let scratch = Scratch::new("default-version");
        let manifest = "[package]\nname = \"app\"\n";
        scratch.write("app/lsaca.toml", manifest);
        let (parsed, diagnostics, sources) = manifest_diagnostics(&scratch, "app/lsaca.toml");
        assert!(
            parsed,
            "minimal manifest should be accepted: {}",
            diagnostics.render(&sources)
        );
        let path = scratch.path("app/lsaca.toml");
        let mut sources = SourceMap::new();
        let mut diagnostics = Diagnostics::new();
        let parsed = package::parse_manifest(&path, &mut sources, &mut diagnostics)
            .expect("minimal manifest should parse");
        assert_eq!(parsed.package.version, "0.1.0");
        assert_eq!(parsed.package.name, "app");
        assert!(parsed.dependencies.is_empty());
    });
}

#[test]
fn rejects_missing_package_table() {
    case("missing-package", || {
        let scratch = Scratch::new("missing-package");
        for (name, manifest) in [("empty", ""), ("only-deps", "[dependencies]\n")] {
            let relative = format!("missing/{name}/lsaca.toml");
            scratch.write(&relative, manifest);
            let (parsed, diagnostics, sources) = manifest_diagnostics(&scratch, &relative);
            assert!(
                !parsed,
                "{name}: manifest without [package] should be rejected"
            );
            assert!(has_code(&diagnostics, "E0001"));
            let text = diagnostics.render(&sources);
            assert!(text.contains("[package]"), "{name}: {text}");
            assert_clean_failure(name, &text);
        }
    });
}

#[test]
fn rejects_missing_package_name() {
    case("missing-name", || {
        let scratch = Scratch::new("missing-name");
        scratch.write("app/lsaca.toml", "[package]\nversion = \"0.1.0\"\n");
        let (parsed, diagnostics, sources) = manifest_diagnostics(&scratch, "app/lsaca.toml");
        assert!(
            !parsed,
            "manifest without a package name should be rejected"
        );
        let text = diagnostics.render(&sources);
        assert!(text.contains("name"), "{text}");
        assert_clean_failure("missing package name", &text);
    });
}

#[test]
fn rejects_malformed_dependency_entries() {
    case("bad-dependencies", || {
        let scratch = Scratch::new("bad-dependencies");
        let cases: &[(&str, &str, &str)] = &[
            (
                "plain-string",
                "[dependencies]\nfoo = \"nope\"\n",
                "malformed dependency",
            ),
            (
                "bad-path",
                "[dependencies]\nfoo = { path = 3 }\n",
                "double-quoted string",
            ),
            (
                "unterminated",
                "[dependencies]\nfoo = { path = \"../dep\" \n",
                "malformed dependency",
            ),
            (
                "unknown-key",
                "[dependencies]\nfoo = { version = \"1.0\" }\n",
                "malformed dependency",
            ),
        ];
        for (name, dependencies, expected) in cases {
            let relative = format!("deps/{name}/lsaca.toml");
            scratch.write(
                &relative,
                &format!("[package]\nname = \"app\"\n\n{dependencies}"),
            );
            let (parsed, diagnostics, sources) = manifest_diagnostics(&scratch, &relative);
            assert!(!parsed, "{name}: malformed dependency should be rejected");
            assert!(has_code(&diagnostics, "E0001"), "{name}: expected E0001");
            let text = diagnostics.render(&sources);
            assert!(
                text.to_lowercase().contains(&expected.to_lowercase()),
                "{name}: expected a message containing {expected}: {text}"
            );
            assert_clean_failure(name, &text);
        }
    });
}

#[test]
fn rejects_duplicate_dependency_declarations() {
    case("duplicate-dep", || {
        let scratch = Scratch::new("duplicate-dep");
        scratch.write(
            "app/lsaca.toml",
            "[package]\nname = \"app\"\n\n[dependencies]\nfoo = { path = \"../foo\" }\nfoo = { path = \"../bar\" }\n",
        );
        let (parsed, diagnostics, sources) = manifest_diagnostics(&scratch, "app/lsaca.toml");
        assert!(
            !parsed,
            "duplicate dependency declarations should be rejected"
        );
        let text = diagnostics.render(&sources);
        assert!(text.contains("duplicate dependency"), "{text}");
        assert_clean_failure("duplicate dependency", &text);
    });
}

#[test]
fn rejects_non_utf8_manifest() {
    case("non-utf8-manifest", || {
        let scratch = Scratch::new("non-utf8-manifest");
        scratch.write_bytes("app/lsaca.toml", &[0xff, 0xfe, 0x00, 0x5b, 0x70]);
        let (parsed, diagnostics, sources) = manifest_diagnostics(&scratch, "app/lsaca.toml");
        assert!(!parsed, "non-UTF-8 manifest should be rejected");
        assert!(has_code(&diagnostics, "E0001"));
        let text = diagnostics.render(&sources);
        assert!(text.contains("could not read package manifest"), "{text}");
        assert_clean_failure("non-utf8 manifest", &text);
    });
}

// ---------------------------------------------------------------------------
// Package loading and dependency graphs
// ---------------------------------------------------------------------------

#[test]
fn rejects_package_without_a_manifest() {
    case("no-manifest", || {
        let scratch = Scratch::new("no-manifest");
        scratch.write("empty/placeholder.txt", "not a package\n");
        let text = package_failure(&scratch, "empty");
        assert!(text.contains("no lsaca.toml found"), "{text}");
        assert_clean_failure("missing manifest", &text);
    });
}

#[test]
fn rejects_package_without_source_roots() {
    case("no-source-roots", || {
        let scratch = Scratch::new("no-source-roots");
        scratch.write("app/lsaca.toml", "[package]\nname = \"app\"\n");
        let text = package_failure(&scratch, "app");
        assert_clean_failure("missing source roots", &text);
    });
}

#[test]
fn rejects_package_with_non_utf8_root_source() {
    case("non-utf8-root", || {
        let scratch = Scratch::new("non-utf8-root");
        scratch.write("app/lsaca.toml", "[package]\nname = \"app\"\n");
        scratch.write_bytes("app/src/main.ls", &[0x66, 0x6e, 0xff]);
        let text = package_failure(&scratch, "app");
        assert!(text.contains("not valid UTF-8"), "{text}");
        assert_clean_failure("non-utf8 package source", &text);
    });
}

#[test]
fn reports_non_utf8_module_inside_a_package() {
    case("non-utf8-package-module", || {
        let scratch = Scratch::new("non-utf8-package-module");
        scratch.write("app/lsaca.toml", "[package]\nname = \"app\"\n");
        scratch.write("app/src/main.ls", "mod helper;\nfn main() { }\n");
        scratch.write_bytes("app/src/helper.ls", &[0x6d, 0xff]);
        let frontend = package_ok(&scratch, "app");
        let diagnostics = frontend_errors(&frontend);
        assert!(
            has_code(&diagnostics, "E0583"),
            "expected E0583: {}",
            messages(&diagnostics)
        );
        let text = rendered(&frontend, &diagnostics);
        assert!(text.contains("could not read module"), "{text}");
        assert_clean_failure("non-utf8 package module", &text);
    });
}

#[test]
fn loads_minimal_binary_package() {
    case("minimal-binary", || {
        let scratch = Scratch::new("minimal-binary");
        binary_package(
            &scratch,
            "app",
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n",
        );
        let frontend = package_ok(&scratch, "app");
        assert_eq!(frontend.graph().crates.len(), 1);
        assert!(
            frontend.has_main(),
            "binary package should expose a main function"
        );
        expect_clean_check(&frontend);
    });
}

#[test]
fn loads_library_package_without_main() {
    case("library-package", || {
        let scratch = Scratch::new("library-package");
        library_package(
            &scratch,
            "lib",
            "[package]\nname = \"lib\"\nversion = \"0.1.0\"\n",
        );
        let frontend = package_ok(&scratch, "lib");
        assert_eq!(frontend.graph().crates.len(), 1);
        assert!(
            !frontend.has_main(),
            "library package should not require main"
        );
        expect_clean_check(&frontend);
    });
}

#[test]
fn rejects_self_dependency() {
    case("self-dependency", || {
        let scratch = Scratch::new("self-dependency");
        binary_package(
            &scratch,
            "app",
            "[package]\nname = \"app\"\n\n[dependencies]\napp = { path = \".\" }\n",
        );
        let text = package_failure(&scratch, "app");
        assert!(text.contains("cannot depend on itself"), "{text}");
        assert_clean_failure("self dependency", &text);
    });
}

#[test]
fn rejects_missing_dependency_directory() {
    case("missing-dep-dir", || {
        let scratch = Scratch::new("missing-dep-dir");
        binary_package(
            &scratch,
            "app",
            "[package]\nname = \"app\"\n\n[dependencies]\ndep = { path = \"../does_not_exist\" }\n",
        );
        let text = package_failure(&scratch, "app");
        assert!(text.contains("does not exist"), "{text}");
        assert_clean_failure("missing dependency directory", &text);
    });
}

#[test]
fn rejects_dependency_without_a_manifest() {
    case("dep-without-manifest", || {
        let scratch = Scratch::new("dep-without-manifest");
        binary_package(
            &scratch,
            "app",
            "[package]\nname = \"app\"\n\n[dependencies]\ndep = { path = \"../dep\" }\n",
        );
        scratch.write("dep/placeholder.txt", "not a package\n");
        let text = package_failure(&scratch, "app");
        assert!(text.contains("lsaca.toml"), "{text}");
        assert_clean_failure("dependency without manifest", &text);
    });
}

#[test]
fn rejects_binary_only_dependency() {
    case("binary-dep", || {
        let scratch = Scratch::new("binary-dep");
        binary_package(
            &scratch,
            "app",
            "[package]\nname = \"app\"\n\n[dependencies]\ndep = { path = \"../dep\" }\n",
        );
        binary_package(&scratch, "dep", "[package]\nname = \"dep\"\n");
        let text = package_failure(&scratch, "app");
        assert!(text.contains("not a library"), "{text}");
        assert_clean_failure("binary-only dependency", &text);
    });
}

#[test]
fn rejects_two_paths_with_the_same_package_name() {
    case("duplicate-package-name", || {
        let scratch = Scratch::new("duplicate-package-name");
        binary_package(
            &scratch,
            "app",
            "[package]\nname = \"app\"\n\n[dependencies]\nfirst = { path = \"../first\" }\nsecond = { path = \"../second\" }\n",
        );
        library_package(&scratch, "first", "[package]\nname = \"same\"\n");
        library_package(&scratch, "second", "[package]\nname = \"same\"\n");
        let text = package_failure(&scratch, "app");
        assert!(text.contains("two different paths"), "{text}");
        assert_clean_failure("duplicate package name", &text);
    });
}

#[test]
fn rejects_cyclic_package_dependencies() {
    case("cyclic-packages", || {
        let scratch = Scratch::new("cyclic-packages");
        binary_package(
            &scratch,
            "app",
            "[package]\nname = \"app\"\n\n[dependencies]\ncycle_a = { path = \"../cycle_a\" }\n",
        );
        library_package(
            &scratch,
            "cycle_a",
            "[package]\nname = \"cycle_a\"\n\n[dependencies]\ncycle_b = { path = \"../cycle_b\" }\n",
        );
        library_package(
            &scratch,
            "cycle_b",
            "[package]\nname = \"cycle_b\"\n\n[dependencies]\ncycle_a = { path = \"../cycle_a\" }\n",
        );
        let text = package_failure(&scratch, "app");
        assert!(text.contains("cyclic package dependency"), "{text}");
        assert_clean_failure("cyclic package dependencies", &text);
    });
}

#[test]
fn loads_diamond_dependency_graph() {
    case("diamond", || {
        let scratch = Scratch::new("diamond");
        binary_package(
            &scratch,
            "app",
            "[package]\nname = \"app\"\n\n[dependencies]\nleft = { path = \"../left\" }\nright = { path = \"../right\" }\n",
        );
        for name in ["left", "right"] {
            library_package(
                &scratch,
                name,
                &format!(
                    "[package]\nname = \"{name}\"\n\n[dependencies]\nbase = {{ path = \"../base\" }}\n"
                ),
            );
        }
        library_package(&scratch, "base", "[package]\nname = \"base\"\n");
        let frontend = package_ok(&scratch, "app");
        let crates = &frontend.graph().crates;
        assert_eq!(
            crates.len(),
            4,
            "base, left, right and app are one crate each"
        );
        assert_eq!(crates[0].name, "base");
        assert_eq!(crates[crates.len() - 1].name, "app");
        for krate in &crates[..3] {
            assert!(krate.is_dependency, "{} should be a dependency", krate.name);
        }
        assert!(!crates[crates.len() - 1].is_dependency);
        let first_order: Vec<String> = crates.iter().map(|krate| krate.name.clone()).collect();
        let second_order: Vec<String> = package_ok(&scratch, "app")
            .graph()
            .crates
            .iter()
            .map(|krate| krate.name.clone())
            .collect();
        assert_eq!(
            first_order, second_order,
            "dependency order must be deterministic"
        );
    });
}

#[test]
fn loads_deep_dependency_chain() {
    case("dependency-chain-40", || {
        let scratch = Scratch::new("dependency-chain-40");
        write_dependency_chain(&scratch, 40);
        let frontend = package_ok(&scratch, "app");
        let crates = &frontend.graph().crates;
        assert_eq!(crates.len(), 41);
        assert_eq!(
            crates[0].name, "dep039",
            "deepest dependency must come first"
        );
        assert_eq!(crates[crates.len() - 1].name, "app");
        // Every crate contributes at least its root source file; manifests are
        // registered in the same source map as well.
        assert!(
            frontend.sources().len() >= crates.len(),
            "expected at least one source per crate, got {}",
            frontend.sources().len()
        );
    });
}

#[test]
fn handles_very_deep_dependency_chain() {
    slow_case("dependency-chain-200", || {
        let scratch = Scratch::new("dependency-chain-200");
        write_dependency_chain(&scratch, 200);
        match Frontend::load_package(&scratch.path("app")) {
            Ok(frontend) => {
                assert_eq!(frontend.graph().crates.len(), 201);
            }
            Err(error) => {
                let text = error.render();
                assert_clean_failure("deep dependency chain", &text);
            }
        }
    });
}

#[test]
fn package_with_both_source_roots_is_handled_cleanly() {
    case("both-source-roots", || {
        // target/v17/DESIGN.md section 6 asks for a dedicated E0001 here; at the
        // time of writing the driver picks src/main.ls. Either behavior is
        // accepted as long as it fails or loads cleanly without a panic.
        let scratch = Scratch::new("both-source-roots");
        write_package(
            &scratch,
            "app",
            "[package]\nname = \"app\"\n",
            Some("fn main() { }\n"),
            Some("pub fn helper() -> i64 { 1 }\n"),
        );
        match Frontend::load_package(&scratch.path("app")) {
            Ok(frontend) => {
                assert_eq!(frontend.graph().crates.len(), 1);
                assert!(!frontend.sources().is_empty());
            }
            Err(error) => {
                let text = error.render();
                assert_clean_failure("both source roots", &text);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Existing module and package fixtures
// ---------------------------------------------------------------------------

#[test]
fn existing_module_fixtures_load_without_crashing() {
    slow_case("module-fixtures", || {
        for name in [
            "math_basic",
            "nested_tree",
            "sibling_paths",
            "name_reuse",
            "reexport",
            "private_field",
            "glob_import",
            "private_fn",
            "panic_location",
        ] {
            let path = repo_path(&format!("tests/programs/modules/{name}/main.ls"));
            let frontend = match Frontend::load(&path) {
                Ok(frontend) => frontend,
                Err(error) => panic!("fixture {name} should load: {error}"),
            };
            assert!(
                frontend.sources().len() >= 2,
                "fixture {name} should load its module files"
            );
            match frontend.check_reporting() {
                Ok(_) => {}
                Err(diagnostics) => {
                    let text = diagnostics.render(frontend.sources());
                    assert_clean_failure(&format!("fixture {name}"), &text);
                }
            }
        }
    });
}

#[test]
fn existing_package_fixtures_load_without_crashing() {
    slow_case("package-fixtures", || {
        for (name, expect_main) in [
            ("binary_hello", Some(true)),
            ("greeting", Some(false)),
            ("with_dep", Some(true)),
            ("library_only", Some(false)),
        ] {
            let path = repo_path(&format!("tests/packages/{name}"));
            let frontend = match Frontend::load_package(&path) {
                Ok(frontend) => frontend,
                Err(error) => panic!("fixture {name} should load: {}", error.render()),
            };
            if let Some(expect_main) = expect_main {
                assert_eq!(frontend.has_main(), expect_main, "fixture {name}");
            }
            match frontend.check_reporting() {
                Ok(_) => {}
                Err(diagnostics) => {
                    let text = diagnostics.render(frontend.sources());
                    assert_clean_failure(&format!("package fixture {name}"), &text);
                }
            }
        }
        for name in ["missing_dep", "bad_manifest", "cycle_a"] {
            let path = repo_path(&format!("tests/packages/{name}"));
            let text = match Frontend::load_package(&path) {
                Ok(_) => panic!("package fixture {name} should fail to load"),
                Err(LoadError::Io(error)) => format!("{error}"),
                Err(LoadError::Diagnostics {
                    diagnostics,
                    sources,
                }) => diagnostics.render(&sources),
            };
            assert_clean_failure(&format!("package fixture {name}"), &text);
        }
    });
}

// ---------------------------------------------------------------------------
// Parser fuzz: malformed mod/use/pub snippets and random token storms
// ---------------------------------------------------------------------------

const MALFORMED_SNIPPETS: &[&str] = &[
    "",
    "mod",
    "mod ;",
    "mod 1;",
    "mod a",
    "mod a b;",
    "mod a;",
    "mod a {",
    "mod a { }",
    "mod a { mod b; }",
    "mod a; mod a;",
    "mod a::b;",
    "mod a;\nmod a;",
    "use",
    "use ;",
    "use ::",
    "use ::a;",
    "use a",
    "use a::",
    "use a::;",
    "use a::*;",
    "use a::{b};",
    "use a::{b,};",
    "use a::{b as c};",
    "use a::b as;",
    "use crate;",
    "use crate::;",
    "use crate::{a, b};",
    "use super::super::x;",
    "use self;",
    "use a::::b;",
    "use a::{::b};",
    "use a::b::{c::{d}};",
    "use {};",
    "use {a};",
    "use *;",
    "pub",
    "pub ;",
    "pub pub fn f() { }",
    "pub mod;",
    "pub use ;",
    "pub(crate) fn f() { }",
    "pub(super) mod a;",
    "pub(in crate::a) fn f() { }",
    "pub mod a { pub use b::c; mod b; }",
    "fn main( { }",
    "mod \u{6a21};",
    "mod a; // trailing",
    "/* mod a; */",
    "use a::b; /* unterminated",
    "mod a { /* nested */ }",
];

#[test]
fn malformed_mod_use_pub_snippets_do_not_panic() {
    case("snippet-fuzz", || {
        for snippet in MALFORMED_SNIPPETS {
            let diagnostics = source_diagnostics("snippet.ls", snippet);
            let text = messages(&diagnostics);
            if !text.is_empty() {
                assert_clean_failure("malformed snippet", &text);
            }
        }
    });
}

fn next_random(state: &mut u64) -> u64 {
    let mut value = *state;
    value ^= value << 13;
    value ^= value >> 7;
    value ^= value << 17;
    *state = value;
    value
}

const STORM_TOKENS: &[&str] = &[
    "mod", "use", "pub", "fn", "main", "crate", "super", "self", "as", "struct", "enum", "trait",
    "impl", "let", "mut", "const", "return", "if", "else", "while", "loop", "match", "type",
    "true", "false", "{", "}", "(", ")", "[", "]", "<", ">", "::", ":", ";", ",", ".", "=", "==",
    "*", "&", "+", "-", "->", "=>", "|", "||", "1", "2", "0", "\"text\"", "'c'", "#", "@", "!",
    "?", "%", "^", "~", "/", "//", "/*", "*/", "\\", "\u{0}",
];

fn random_storm(state: &mut u64, token_count: usize) -> String {
    let mut source = String::new();
    for index in 0..token_count {
        if index > 0 {
            source.push(' ');
        }
        let pick = (next_random(state) as usize) % STORM_TOKENS.len();
        source.push_str(STORM_TOKENS[pick]);
    }
    source
}

#[test]
fn random_token_storms_do_not_panic_in_the_parser() {
    case("storm-parser", || {
        let mut state = 0x2545_F491_4F6C_DD1D_u64;
        for iteration in 0..200 {
            let token_count = 1 + (iteration * 7) % 80;
            let source = random_storm(&mut state, token_count);
            let diagnostics = source_diagnostics("storm.ls", &source);
            let text = messages(&diagnostics);
            if !text.is_empty() {
                assert_clean_failure("token storm", &text);
            }
        }
    });
}

#[test]
fn random_token_storms_do_not_panic_in_the_frontend() {
    slow_case("storm-frontend", || {
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        for iteration in 0..60 {
            let token_count = 1 + (iteration * 13) % 120;
            let source = random_storm(&mut state, token_count);
            let frontend = Frontend::new("storm.ls", source);
            match frontend.check_reporting() {
                Ok(_) => {}
                Err(diagnostics) => {
                    let text = diagnostics.render(frontend.sources());
                    assert_clean_failure("frontend token storm", &text);
                }
            }
        }
    });
}

#[test]
fn in_memory_modules_are_reported_as_missing() {
    case("in-memory-modules", || {
        let frontend = Frontend::new("main.ls", "mod helper;\nfn main() { }\n");
        match frontend.check() {
            Ok(_) => panic!("in-memory module declarations cannot be resolved"),
            Err(diagnostics) => {
                assert!(
                    has_code(&diagnostics, "E0583"),
                    "expected E0583: {}",
                    messages(&diagnostics)
                );
                let text = diagnostics.render(frontend.sources());
                assert!(text.contains("in-memory"), "{text}");
                assert_clean_failure("in-memory modules", &text);
            }
        }
    });
}

#[test]
fn deeply_nested_expressions_are_rejected() {
    case("deep-expressions", || {
        let mut additive = String::from("fn main() { let value = 1");
        for _ in 0..5000 {
            additive.push_str(" + 1");
        }
        additive.push_str("; }\n");
        let diagnostics = source_diagnostics("deep_add.ls", &additive);
        assert!(
            diagnostics.has_errors(),
            "deep additive chain should be rejected"
        );
        assert!(
            has_code(&diagnostics, "E0001"),
            "expected E0001: {}",
            messages(&diagnostics)
        );
        assert!(
            messages(&diagnostics).contains("nested too deeply"),
            "{}",
            messages(&diagnostics)
        );

        let mut parens = String::from("fn main() { let value = ");
        for _ in 0..3000 {
            parens.push('(');
        }
        parens.push('1');
        for _ in 0..3000 {
            parens.push(')');
        }
        parens.push_str("; }\n");
        let diagnostics = source_diagnostics("deep_parens.ls", &parens);
        assert!(
            diagnostics.has_errors(),
            "deep parentheses should be rejected"
        );
        assert_clean_diagnostics("deep parentheses", &diagnostics);
    });
}

#[test]
fn huge_integer_literal_is_rejected_cleanly() {
    case("huge-int", || {
        let mut source = String::from("fn main() { let value = ");
        source.push_str(&"9".repeat(50_000));
        source.push_str("; }\n");
        let diagnostics = source_diagnostics("huge_int.ls", &source);
        assert!(
            has_code(&diagnostics, "E0002"),
            "expected E0002: {}",
            messages(&diagnostics)
        );
        assert!(
            messages(&diagnostics).contains("too large"),
            "{}",
            messages(&diagnostics)
        );
        assert_clean_diagnostics("huge integer literal", &diagnostics);
    });
}

#[test]
fn huge_string_literal_is_handled_within_the_budget() {
    slow_case("huge-string", || {
        let mut source = String::from("fn main() { let value = \"");
        source.push_str(&"x".repeat(1_000_000));
        source.push_str("\"; }\n");
        let diagnostics = source_diagnostics("huge_string.ls", &source);
        let text = messages(&diagnostics);
        if !text.is_empty() {
            assert_clean_failure("huge string literal", &text);
        }
    });
}

#[test]
fn unterminated_constructs_are_reported_cleanly() {
    case("unterminated", || {
        let cases: &[(&str, &str, &str)] = &[
            ("unterminated-string", "\"abc", "E0004"),
            ("unterminated-comment", "fn main() { /*", "E0006"),
            ("unterminated-char", "fn main() { let c = '", "E0004"),
        ];
        for (name, source, expected) in cases {
            let diagnostics = source_diagnostics("unterminated.ls", source);
            assert!(diagnostics.has_errors(), "{name}: expected an error");
            assert!(
                has_code(&diagnostics, expected),
                "{name}: expected {expected}: {}",
                messages(&diagnostics)
            );
            assert_clean_diagnostics(name, &diagnostics);
        }
    });
}

#[test]
fn nul_and_control_characters_are_reported_cleanly() {
    case("control-characters", || {
        let cases = [
            "fn main() { let \u{0} = 1; }",
            "fn main() { let value = 1; }\u{7}",
            "\u{1}\u{2}\u{3}",
        ];
        for source in cases {
            let diagnostics = source_diagnostics("control.ls", source);
            let text = messages(&diagnostics);
            assert!(
                !text.is_empty(),
                "control characters should produce diagnostics"
            );
            assert_clean_failure("control characters", &text);
        }
    });
}

#[test]
fn valid_sources_with_bom_and_crlf_parse_cleanly() {
    case("bom-crlf-parse", || {
        let source = "\u{feff}struct Point {\r\n    x: i64,\r\n    y: i64,\r\n}\r\n\r\nfn main() {\r\n    let point = Point { x: 1, y: 2 };\r\n}\r\n";
        let diagnostics = source_diagnostics("crlf.ls", source);
        assert!(!diagnostics.has_errors(), "{}", messages(&diagnostics));
    });
}

// ---------------------------------------------------------------------------
// v2.1 semantic hardening: integer inference, structs, text and slice views
// Adds clean rejection and acceptance coverage for the rules that changed in
// the 2.1 release, plus parsing robustness for the new surface syntax.
// ---------------------------------------------------------------------------

fn expect_v21_rejected(
    case_name: &'static str,
    source: &'static str,
    expected: &'static str,
    fragment: Option<&'static str>,
) {
    case(case_name, move || {
        let scratch = Scratch::new(case_name);
        scratch.write("main.ls", source);
        let (frontend, diagnostics) = failed_load_diagnostics(&scratch, "main.ls");
        let text = rendered(&frontend, &diagnostics);
        assert!(
            has_code(&diagnostics, expected),
            "{case_name}: expected {expected}, got:\n{text}"
        );
        if let Some(fragment) = fragment {
            assert!(
                messages(&diagnostics).contains(fragment),
                "{case_name}: expected a message containing {fragment:?}, got:\n{text}"
            );
        }
        assert_clean_failure(case_name, &text);
    });
}

fn expect_v21_rejected_message(
    case_name: &'static str,
    source: &'static str,
    fragment: &'static str,
) {
    case(case_name, move || {
        let scratch = Scratch::new(case_name);
        scratch.write("main.ls", source);
        let (frontend, diagnostics) = failed_load_diagnostics(&scratch, "main.ls");
        let text = rendered(&frontend, &diagnostics);
        assert!(
            messages(&diagnostics).contains(fragment),
            "{case_name}: expected a message containing {fragment:?}, got:\n{text}"
        );
        assert_clean_failure(case_name, &text);
    });
}

fn expect_v21_accepted(case_name: &'static str, source: &'static str) {
    case(case_name, move || {
        let scratch = Scratch::new(case_name);
        scratch.write("main.ls", source);
        expect_clean_check(&load_ok(&scratch, "main.ls"));
    });
}

#[test]
fn rejects_mixed_integer_arithmetic() {
    expect_v21_rejected(
        "v21-mixed-arith",
        r#"
fn main() {
    let small: i32 = 1;
    let big: i64 = 2;
    let total = small + big;
    println!("{}", total);
}
"#,
        "E0308",
        None,
    );
}

#[test]
fn rejects_usize_to_i64_assignment() {
    expect_v21_rejected(
        "v21-usize-to-i64",
        r#"
fn main() {
    let index: usize = 2;
    let value: i64 = index;
    println!("{}", value);
}
"#,
        "E0308",
        None,
    );
}

#[test]
fn rejects_mixed_integer_comparison() {
    expect_v21_rejected(
        "v21-mixed-compare",
        r#"
fn main() {
    let index: i32 = 1;
    let values = vec![10, 20, 30];
    if index < values.len() {
        println!("in range");
    }
}
"#,
        "E0308",
        None,
    );
}

#[test]
fn rejects_i32_vector_index() {
    expect_v21_rejected(
        "v21-i32-index",
        r#"
fn main() {
    let values = vec![1, 2];
    let index: i32 = 0;
    println!("{}", values[index]);
}
"#,
        "E0308",
        None,
    );
}

#[test]
fn rejects_suffixed_literal_out_of_range() {
    expect_v21_rejected(
        "v21-i8-range",
        r#"
fn main() {
    let value: i8 = 128;
    println!("{}", value);
}
"#,
        "E0080",
        Some("literal out of range for i8"),
    );
}

#[test]
fn rejects_unsuffixed_literal_out_of_i32_range() {
    expect_v21_rejected(
        "v21-default-overflow",
        r#"
fn main() {
    let value = 3000000000;
    println!("{}", value);
}
"#,
        "E0080",
        Some("literal out of range for i32"),
    );
}

#[test]
fn rejects_tuple_struct_constructor_arity() {
    expect_v21_rejected(
        "v21-tuple-arity",
        r#"
struct Point(i64, i64);

fn main() {
    let point = Point(1);
    println!("{:?}", point);
}
"#,
        "E0061",
        None,
    );
}

#[test]
fn rejects_tuple_struct_field_out_of_bounds() {
    expect_v21_rejected(
        "v21-tuple-field",
        r#"
struct Point(i64, i64);

fn main() {
    let point = Point(1, 2);
    println!("{}", point.2);
}
"#,
        "E0609",
        None,
    );
}

#[test]
fn rejects_unit_struct_constructor_call() {
    expect_v21_rejected(
        "v21-unit-call",
        r#"
struct Marker;

fn main() {
    let marker = Marker();
    println!("{:?}", marker);
}
"#,
        "E0532",
        None,
    );
}

#[test]
fn rejects_tuple_struct_pattern_arity() {
    expect_v21_rejected(
        "v21-tuple-pattern",
        r#"
struct Point(i64, i64);

fn main() {
    let point = Point(1, 2);
    match point {
        Point(x) => println!("{}", x),
    }
}
"#,
        "E0023",
        None,
    );
}

#[test]
fn rejects_mut_self_receiver() {
    expect_v21_rejected_message(
        "v21-mut-self",
        r#"
struct Counter {
    count: i64,
}

impl Counter {
    fn bump(mut self) -> Counter {
        self.count += 1;
        self
    }
}

fn main() {
    let counter = Counter { count: 0 };
    println!("{}", counter.bump().count);
}
"#,
        "cannot be declared 'mut'",
    );
}

#[test]
fn rejects_slice_parameter_by_value() {
    expect_v21_rejected(
        "v21-slice-param",
        r#"
fn total(values: [i64]) -> i64 {
    0
}

fn main() {
}
"#,
        "E0001",
        Some("slices are unsized"),
    );
}

#[test]
fn rejects_str_index() {
    expect_v21_rejected(
        "v21-str-index",
        r#"
fn main() {
    let text: &str = "hello";
    let first = text[0];
    println!("{}", first);
}
"#,
        "E0608",
        None,
    );
}

#[test]
fn rejects_generic_parse_target() {
    expect_v21_rejected(
        "v21-parse-target",
        r#"
fn main() {
    let value = "42".parse::<u8>();
    println!("{:?}", value);
}
"#,
        "E0107",
        Some("unsupported type argument for parse: u8"),
    );
}

#[test]
fn rejects_strong_count_as_i64() {
    expect_v21_rejected(
        "v21-strong-count",
        r#"
fn main() {
    let value = Rc::new(String::from("hello"));
    let count: i64 = value.strong_count();
    println!("{}", count);
}
"#,
        "E0308",
        None,
    );
}

#[test]
fn rejects_shutdown_from_integer() {
    expect_v21_rejected(
        "v21-shutdown",
        r#"
use std::net::Shutdown;

fn shut(how: Shutdown) {
    println!("requested");
}

fn main() {
    shut(0);
}
"#,
        "E0308",
        None,
    );
}

#[test]
fn accepts_slice_view_stored_in_a_struct() {
    expect_v21_accepted(
        "v21-slice-struct",
        r#"
struct Holder<'a> {
    values: &'a [i64],
}

fn main() {
}
"#,
    );
}

#[test]
fn rejects_borrowed_text_concatenation() {
    expect_v21_rejected(
        "v21-str-concat",
        r#"
fn main() {
    let left: &str = "a";
    let right: &str = "b";
    let joined = left + right;
    println!("{}", joined);
}
"#,
        "E0369",
        None,
    );
}

#[test]
fn rejects_owned_string_concatenation() {
    expect_v21_rejected(
        "v21-string-concat",
        r#"
fn main() {
    let left = String::from("a");
    let right = String::from("b");
    let joined = left + right;
    println!("{}", joined);
}
"#,
        "E0277",
        None,
    );
}

#[test]
fn rejects_move_while_a_str_view_is_live() {
    expect_v21_rejected(
        "v21-move-borrowed",
        r#"
fn main() {
    let text = String::from("hello");
    let view: &str = &text;
    let moved = text;
    println!("{} {}", view, moved);
}
"#,
        "E0505",
        None,
    );
}

#[test]
fn rejects_move_out_of_string_field() {
    expect_v21_rejected(
        "v21-move-field",
        r#"
struct Holder {
    text: String,
}

fn main() {
    let holder = Holder { text: String::from("hello") };
    let text = holder.text;
    println!("{}", text);
}
"#,
        "E0507",
        None,
    );
}

#[test]
fn rejects_move_out_of_indexed_string_element() {
    expect_v21_rejected(
        "v21-move-index",
        r#"
fn main() {
    let values: Vec<String> = vec![String::from("a")];
    let first = values[0];
    println!("{}", first);
}
"#,
        "E0507",
        None,
    );
}

#[test]
fn rejects_use_after_move_of_string() {
    expect_v21_rejected(
        "v21-use-after-move",
        r#"
fn main() {
    let first = String::from("a");
    let second = first;
    println!("{}", first);
    println!("{}", second);
}
"#,
        "E0382",
        None,
    );
}

#[test]
fn accepts_clone_keeps_string_source_live() {
    expect_v21_accepted(
        "v21-clone",
        r#"
fn main() {
    let first = String::from("a");
    let second = first.clone();
    println!("{} {}", first, second);
}
"#,
    );
}

#[test]
fn accepts_move_after_last_borrow() {
    expect_v21_accepted(
        "v21-last-borrow",
        r#"
fn main() {
    let text = String::from("hello");
    let view: &str = &text;
    println!("view = {}", view);
    let moved = text;
    println!("moved = {}", moved);
}
"#,
    );
}

#[test]
fn accepts_v21_tuple_structs_and_mut_parameters() {
    expect_v21_accepted(
        "v21-tuple-accept",
        r#"
struct Point(i64, i64);

struct Meters(i64);

struct Marker;

impl Point {
    fn new(x: i64, y: i64) -> Point {
        Point(x, y)
    }

    fn sum(&self) -> i64 {
        self.0 + self.1
    }
}

fn bump(mut value: i64) -> i64 {
    value += 1;
    value
}

fn fill(mut text: String, suffix: String) -> String {
    text += "!";
    text + &suffix
}

fn main() {
    let point = Point::new(1, 2);
    let meters = Meters(3);
    let _marker = Marker;
    let pair = Point(4, 5);
    let total = match pair {
        Point(x, y) => x + y,
    };
    println!(
        "{} {} {} {} {}",
        point.sum(),
        meters.0,
        bump(1),
        fill(String::from("a"), String::from("b")),
        total
    );
}
"#,
    );
}

#[test]
fn accepts_v21_borrowed_text_and_slices() {
    expect_v21_accepted(
        "v21-text-accept",
        r#"
fn greet(name: &str) -> String {
    String::from_str("hello, ") + name
}

fn total(values: &[i64]) -> i64 {
    let mut sum = 0;
    let mut index = 0;
    while index < values.len() {
        sum += values[index];
        index += 1;
    }
    sum
}

fn main() {
    let name = String::from("world");
    let greeting = greet("there");
    let borrowed = greet(&name);
    let literal: &str = "text";
    let values: Vec<i64> = vec![1, 2, 3];
    let view: &[i64] = &values;
    let index = 0;
    println!(
        "{} {} {} {} {}",
        greeting,
        borrowed,
        literal.len(),
        total(&values),
        view[index]
    );
}
"#,
    );
}

#[test]
fn accepts_v21_dyn_trait_objects() {
    expect_v21_accepted(
        "v21-dyn",
        r#"
trait Shape {
    fn area(&self) -> i64;
}

struct Square {
    side: i64,
}

impl Shape for Square {
    fn area(&self) -> i64 {
        self.side * self.side
    }
}

fn measure(shape: &dyn Shape) -> i64 {
    shape.area()
}

fn main() {
    let square = Square { side: 3 };
    let boxed: Box<dyn Shape> = Box::new(Square { side: 4 });
    let counted: Rc<dyn Shape> = Rc::new(Square { side: 5 });
    println!(
        "{} {} {}",
        measure(&square),
        boxed.area(),
        counted.area()
    );
}
"#,
    );
}

#[test]
fn accepts_v21_time_types() {
    expect_v21_accepted(
        "v21-time",
        r#"
use std::time::{Duration, Instant, SystemTime, sleep};

fn main() {
    let duration = Duration::from_secs(2).checked_add(Duration::from_secs(3)).unwrap();
    let start = Instant::now();
    let elapsed = start.elapsed();
    let now = SystemTime::now();
    println!(
        "{} {} {}",
        duration.as_secs(),
        elapsed.as_millis() >= 0,
        now.unix_ms() != 0
    );
    println!("{}", unix_epoch().unix_ms());
    sleep(Duration::from_millis(0));
}
"#,
    );
}

#[test]
fn accepts_v21_fs_env_net_types() {
    expect_v21_accepted(
        "v21-system",
        r#"
use std::fs::FileType;
use std::net::Shutdown;

fn kind(info: &std::fs::Metadata) -> FileType {
    info.file_type()
}

fn shut(how: Shutdown) {
    let _ = how;
    println!("shutdown requested");
}

fn main() {
    let result = std::env::var("LSACA_ROBUSTNESS_MISSING_VARIABLE");
    println!("{}", result.is_ok());
    shut(Shutdown::Both);
    println!("{}", std::env::var("TEMP").is_ok());
}
"#,
    );
}

const V21_MALFORMED_SNIPPETS: &[&str] = &[
    "struct Point(i64, i64",
    "struct Point(i64 i64);",
    "struct Point(i64, i64) { }",
    "struct Marker; fn main() { Marker(1); }",
    "fn take() -> &[i64] { }",
    "fn take(values: [i64; 3]) -> i64 { 0 }",
    "fn take(values: &[i64) -> i64 { 0 }",
    "use std::net::Shutdown",
    "use std::time::{Duration, };",
    "use std::fs::FileType as;",
    "trait Shape { fn area(&self) -> dyn Shape; }",
    "impl Point { fn new(mut self) -> Point { self } }",
    "fn main() { let value = 1i128; }",
    "fn main() { let value = \"abc\".parse::<i64>; }",
    "fn main() { let value: &[i64] = [1, 2]; }",
    "fn main() { let value = vec![1, 2,]; }",
    "fn main() { let value = String::from_str(); }",
    "fn main() { let value = Duration::from_secs(); }",
];

#[test]
fn malformed_v21_snippets_do_not_panic() {
    case("v21-snippets", || {
        for snippet in V21_MALFORMED_SNIPPETS {
            let diagnostics = source_diagnostics("v21_snippet.ls", snippet);
            let text = messages(&diagnostics);
            if !text.is_empty() {
                assert_clean_failure("v2.1 malformed snippet", &text);
            }
        }
    });
}

const V21_STORM_TOKENS: &[&str] = &[
    "struct",
    "Point",
    "i64",
    "i8",
    "u8",
    "usize",
    "i32",
    "(",
    ")",
    "[",
    "]",
    "&",
    "&[",
    "dyn",
    "Shape",
    "trait",
    "impl",
    "fn",
    "main",
    "self",
    "mut",
    "parse",
    "::<",
    ">",
    "::",
    ";",
    "{",
    "}",
    ",",
    ".",
    "=",
    "->",
    "=>",
    "use",
    "std",
    "net",
    "fs",
    "time",
    "Shutdown",
    "FileType",
    "Duration",
    "String",
    "from_str",
    "from_secs",
    "1u8",
    "1i8",
    "0",
    "1.5",
    "\"text\"",
    "'c'",
    "?",
];

#[test]
fn random_v21_token_storms_do_not_panic() {
    case("storm-v21", || {
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        for _ in 0..120 {
            let mut source = String::new();
            for index in 0..160 {
                if index > 0 {
                    source.push(' ');
                }
                let pick = (next_random(&mut state) as usize) % V21_STORM_TOKENS.len();
                source.push_str(V21_STORM_TOKENS[pick]);
            }
            let diagnostics = source_diagnostics("v21_storm.ls", &source);
            let text = messages(&diagnostics);
            assert_clean_failure("v2.1 token storm", &text);
        }
    });
}

#[test]
fn rejects_owned_string_for_str_parameter() {
    expect_v21_rejected(
        "v21-owned-string-param",
        "fn greet(name: &str) -> i64 {\n    name.len()\n}\n\nfn main() {\n    println!(\"{}\", greet(String::from(\"abc\")));\n}\n",
        "E0308",
        None,
    );
}

#[test]
fn rejects_assignment_to_immutable_parameter() {
    expect_v21_rejected(
        "v21-immutable-param-assign",
        "fn bump(value: i64) -> i64 {\n    value += 1;\n    value\n}\n\nfn main() {\n    println!(\"{}\", bump(1));\n}\n",
        "E0596",
        Some("immutable"),
    );
}

#[test]
fn rejects_duration_millis_as_i64() {
    expect_v21_rejected(
        "v21-duration-millis",
        "use std::time::Duration;\n\nfn main() {\n    let millis: i64 = Duration::from_secs(1).as_millis();\n    println!(\"{}\", millis);\n}\n",
        "E0308",
        None,
    );
}

#[test]
fn rejects_env_var_as_string() {
    expect_v21_rejected(
        "v21-env-var-string",
        "fn main() {\n    let value: String = std::env::var(\"LSACA_MISSING\");\n    println!(\"{}\", value);\n}\n",
        "E0308",
        None,
    );
}

// ---------------------------------------------------------------------------
// v2.2 semantic hardening: fixed size arrays and loop labels
// The 2.2 release adds the array type [T; N] with Rust copy and move rules and
// labels on loops. Both are parsed and typed in parallel with this suite, so
// the cases below pin the diagnostics that are part of the frozen contract:
// array lengths must be const evaluated (E0435/E0080), labels must resolve to
// an enclosing loop (E0426), elements of an owned array are places (E0507) and
// owned arrays move (E0382). Acceptance cases pin the new surface syntax so a
// regression in the parser or the move checker is caught here as well.
// ---------------------------------------------------------------------------

fn expect_v22_rejected(
    case_name: &'static str,
    source: &'static str,
    expected: &'static str,
    fragment: Option<&'static str>,
) {
    expect_v21_rejected(case_name, source, expected, fragment);
}

fn expect_v22_accepted(case_name: &'static str, source: &'static str) {
    expect_v21_accepted(case_name, source);
}

#[test]
fn rejects_array_length_that_is_not_const() {
    expect_v22_rejected(
        "v22-nonconst-array-length",
        "fn main() {\n    let n: usize = 3;\n    let values: [i64; n] = [1, 2, 3];\n    println!(\"{}\", values[0]);\n}\n",
        "E0435",
        Some("integer literal or an integer constant"),
    );
}

#[test]
fn rejects_negative_array_length() {
    expect_v22_rejected(
        "v22-negative-array-length",
        "fn main() {\n    let values: [i64; -1] = [];\n    println!(\"{}\", values[0]);\n}\n",
        "E0080",
        Some("array length must be between 0 and 1048576"),
    );
}

#[test]
fn rejects_array_length_above_the_cap() {
    expect_v22_rejected(
        "v22-array-length-cap",
        "fn main() {\n    let values: [i64; 2000000] = [];\n    println!(\"{}\", values[0]);\n}\n",
        "E0080",
        Some("1048576"),
    );
}

#[test]
fn rejects_unknown_loop_label() {
    expect_v22_rejected(
        "v22-unknown-label",
        "fn main() {\n    'outer: loop {\n        break 'missing;\n    }\n}\n",
        "E0426",
        Some("use of undeclared label 'missing'"),
    );
}

#[test]
fn rejects_unknown_continue_label() {
    expect_v22_rejected(
        "v22-unknown-continue-label",
        "fn main() {\n    for i in 0..2 {\n        continue 'missing;\n    }\n}\n",
        "E0426",
        Some("use of undeclared label"),
    );
}

#[test]
fn rejects_loop_label_before_a_statement() {
    expect_v22_rejected(
        "v22-label-before-statement",
        "fn main() {\n    'outer: let x = 1;\n    println!(\"{}\", x);\n}\n",
        "E0001",
        Some("loop label can only precede"),
    );
}

#[test]
fn rejects_break_with_a_value() {
    expect_v22_rejected(
        "v22-break-value",
        "fn main() {\n    loop {\n        break 1;\n    }\n}\n",
        "E0308",
        Some("break with a value is not supported"),
    );
}

#[test]
fn rejects_lifetime_parameter_bounds() {
    expect_v22_rejected(
        "v22-lifetime-bounds",
        "struct View<'a: 'static> { text: &'a str }\n\nfn main() {\n}\n",
        "E0001",
        Some("lifetime parameters cannot have bounds"),
    );
}

#[test]
fn rejects_empty_array_without_type() {
    expect_v22_rejected(
        "v22-empty-array-no-type",
        "fn main() {\n    let values = [];\n    println!(\"{}\", 1);\n}\n",
        "E0282",
        Some("cannot infer the element type of an empty array"),
    );
}

#[test]
fn rejects_non_usize_array_index() {
    expect_v22_rejected(
        "v22-array-index-type",
        "fn main() {\n    let values: [i64; 3] = [1, 2, 3];\n    let index: i32 = 0;\n    println!(\"{}\", values[index]);\n}\n",
        "E0308",
        None,
    );
}

#[test]
fn rejects_moving_an_array_element() {
    expect_v22_rejected(
        "v22-array-element-move",
        "fn main() {\n    let words: [String; 2] = [String::from(\"a\"), String::from(\"b\")];\n    let first: String = words[0];\n    println!(\"{}\", first);\n}\n",
        "E0507",
        Some("indexed element"),
    );
}

#[test]
fn rejects_use_of_a_moved_array() {
    expect_v22_rejected(
        "v22-array-use-after-move",
        "fn main() {\n    let words: [String; 2] = [String::from(\"a\"), String::from(\"b\")];\n    let moved = words;\n    println!(\"{:?}\", words);\n}\n",
        "E0382",
        Some("use of moved value: words"),
    );
}

#[test]
fn rejects_use_of_an_array_after_passing_it_by_value() {
    expect_v22_rejected(
        "v22-array-passed-by-value",
        "fn take(values: [String; 2]) -> usize { values.len() }\n\nfn main() {\n    let words: [String; 2] = [String::from(\"a\"), String::from(\"b\")];\n    let count: usize = take(words);\n    println!(\"{} {:?}\", count, words);\n}\n",
        "E0382",
        Some("use of moved value: words"),
    );
}

#[test]
fn rejects_reference_stored_in_vector() {
    expect_v22_rejected(
        "v22-vector-of-references",
        "fn main() {\n    let value: i64 = 1;\n    let references: Vec<&i64> = Vec::new();\n    println!(\"{} {}\", value, references.len());\n}\n",
        "E0498",
        Some("references cannot be stored in this type"),
    );
}

#[test]
fn accepts_arrays_and_length_sugar() {
    expect_v22_accepted(
        "v22-arrays-accepted",
        "fn main() {\n    let numbers: [i64; 3] = [1, 2, 3];\n    let empty: [i64; 0] = [];\n    println!(\"{} {} {}\", numbers.len(), numbers.is_empty(), empty.is_empty());\n}\n",
    );
}

#[test]
fn accepts_array_repeat_of_copy_values() {
    expect_v22_accepted(
        "v22-array-repeat",
        "fn main() {\n    let zeros: [i64; 4] = [0; 4];\n    let copy = zeros;\n    println!(\"{:?} {:?}\", zeros, copy);\n}\n",
    );
}

#[test]
fn accepts_empty_owned_array_as_copy() {
    expect_v22_accepted(
        "v22-empty-owned-array",
        "fn main() {\n    let words: [String; 0] = [];\n    let first = words;\n    let second = words;\n    println!(\"{} {}\", first.len(), second.len());\n}\n",
    );
}

#[test]
fn accepts_array_of_arrays() {
    expect_v22_accepted(
        "v22-array-of-arrays",
        "fn main() {\n    let matrix: [[i64; 2]; 2] = [[1, 2], [3, 4]];\n    println!(\"{}\", matrix[1][0]);\n}\n",
    );
}

#[test]
fn rejects_array_repeat_of_a_non_copy_value() {
    expect_v22_rejected(
        "v22-array-repeat-noncopy",
        "fn main() {\n    let word = String::from(\"x\");\n    let pair: [String; 2] = [word; 2];\n    println!(\"{:?}\", pair);\n}\n",
        "E0507",
        Some("cannot move out of a value of type String in an array repeat expression"),
    );
}

#[test]
fn accepts_array_repeat_with_a_single_owned_element() {
    expect_v22_accepted(
        "v22-array-repeat-move",
        "fn main() {\n    let word = String::from(\"x\");\n    let single: [String; 1] = [word; 1];\n    println!(\"{:?}\", single);\n}\n",
    );
}

#[test]
fn accepts_empty_array_repeat_of_a_non_copy_value() {
    expect_v22_accepted(
        "v22-array-repeat-empty",
        "fn main() {\n    let empty: [String; 0] = [String::from(\"x\"); 0];\n    println!(\"{}\", empty.len());\n}\n",
    );
}

#[test]
fn accepts_slice_of_an_array() {
    expect_v22_accepted(
        "v22-array-slice",
        "fn main() {\n    let numbers: [i64; 4] = [4, 5, 6, 7];\n    let part: &[i64] = &numbers[1..3];\n    println!(\"{:?}\", part);\n}\n",
    );
}

#[test]
fn accepts_labelled_break_and_continue() {
    expect_v22_accepted(
        "v22-labels-accepted",
        "fn main() {\n    'outer: for i in 0..3 {\n        let mut inner: i64 = 0;\n        'inner: while inner < 2 {\n            inner = inner + 1;\n            if i == 1 { continue 'outer; }\n            if inner == 2 { break 'inner; }\n        }\n        if i == 2 { break 'outer; }\n    }\n    let mut count: i64 = 0;\n    'timer: loop {\n        count = count + 1;\n        if count >= 2 { break 'timer; }\n    }\n    println!(\"{}\", count);\n}\n",
    );
}

#[test]
fn accepts_reference_fields_in_a_struct() {
    expect_v22_accepted(
        "v22-reference-fields",
        "struct Pair<'a> { left: &'a i64, right: &'a i64 }\n\nfn main() {\n    let left: i64 = 1;\n    let right: i64 = 2;\n    let pair: Pair = Pair { left: &left, right: &right };\n    println!(\"{} {}\", pair.left, pair.right);\n}\n",
    );
}

#[test]
fn rejects_reference_field_without_a_lifetime() {
    expect_v22_rejected(
        "v22-missing-lifetime",
        "struct Holder {\n    values: &[i64],\n}\n\nfn main() {\n}\n",
        "E0106",
        Some("missing lifetime specifier"),
    );
}

#[test]
fn accepts_reference_field_with_a_named_lifetime() {
    expect_v22_accepted(
        "v22-named-lifetime",
        "struct View<'a> {\n    text: &'a str,\n}\n\nfn main() {\n    let view: View = View { text: \"hello\" };\n    println!(\"{}\", view.text);\n}\n",
    );
}

#[test]
fn rejects_try_without_a_from_conversion() {
    expect_v22_rejected(
        "v22-try-no-from",
        "use std::error::Error;\n\nstruct AppError {\n    message: String,\n}\n\nimpl Error for AppError {\n    fn message(&self) -> String {\n        return self.message.clone();\n    }\n}\n\nfn parse(text: &str) -> Result<i64, AppError> {\n    let value: i64 = text.parse_i64()?;\n    return Ok(value);\n}\n\nfn main() {\n    let parsed: Result<i64, AppError> = parse(\"42\");\n    println!(\"{:?}\", parsed.is_ok());\n}\n",
        "E0308",
        Some("cannot convert error types"),
    );
}

const V22_MALFORMED_SNIPPETS: &[&str] = &[
    "fn main() { let values: [i64; 3] = [1, 2",
    "fn main() { let values = [];",
    "fn main() { let values = [1,;",
    "fn main() { let values = [1;",
    "fn main() { let values: [i64; ] = [];",
    "fn main() { let values: [; 3] = [];",
    "fn main() { let values: [i64 3] = [];",
    "fn main() { 'outer: }",
    "fn main() { 'outer: 'inner: loop { } }",
    "fn main() { loop 'outer { } }",
    "fn main() { break '; }",
    "fn main() { continue '; }",
    "fn main() { break 'outer }",
    "fn main() { 'a fn inner() { } }",
    "struct Parser<'a { text: &'a str }",
    "struct Parser<'a,> { text: &'a str }",
    "struct Parser<'a: > { text: &'a str }",
    "fn take(view: &'a str) -> 'a i64 { 0 }",
    "fn main() { let value: [&str; 2] = [&'a, &'b]; }",
    "fn main() { let value = 'static; }",
];

#[test]
fn malformed_v22_snippets_do_not_panic() {
    case("v22-snippets", || {
        for snippet in V22_MALFORMED_SNIPPETS {
            let diagnostics = source_diagnostics("v22_snippet.ls", snippet);
            let text = messages(&diagnostics);
            if !text.is_empty() {
                assert_clean_failure("v2.2 malformed snippet", &text);
            }
        }
    });
}

const V22_STORM_TOKENS: &[&str] = &[
    "struct",
    "Parser",
    "View",
    "fn",
    "main",
    "loop",
    "while",
    "for",
    "in",
    "break",
    "continue",
    "if",
    "else",
    "'outer",
    "'inner",
    "'timer",
    "'a",
    "'static",
    "'",
    "0..3",
    "i64",
    "usize",
    "String",
    "[",
    "]",
    "[i64",
    ";",
    "3]",
    "[0",
    "4]",
    "&",
    "&mut",
    "&[",
    "(",
    ")",
    "{",
    "}",
    ",",
    ".",
    "=",
    "==",
    "->",
    "=>",
    ":",
    "::",
    "<",
    ">",
    "\"text\"",
    "'c'",
    "1",
    "0",
    "+",
    "?",
    "struct Parser<'a>",
    "text: &'a str",
    "let",
];

#[test]
fn random_v22_token_storms_do_not_panic() {
    case("storm-v22", || {
        let mut state = 0xDEAD_BEEF_CAFE_F00D_u64;
        for _ in 0..120 {
            let mut source = String::new();
            for index in 0..160 {
                if index > 0 {
                    source.push(' ');
                }
                let pick = (next_random(&mut state) as usize) % V22_STORM_TOKENS.len();
                source.push_str(V22_STORM_TOKENS[pick]);
            }
            let diagnostics = source_diagnostics("v22_storm.ls", &source);
            let text = messages(&diagnostics);
            assert_clean_failure("v2.2 token storm", &text);
        }
    });
}
