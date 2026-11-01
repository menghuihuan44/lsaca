//! Integration tests for the lsaca command line tool.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_lsaca")
}

/// Directory used for the build cache of the CLI tests.
///
/// It lives inside the workspace target directory rather than the system
/// temporary directory so that the spawned compiler and CMake processes can
/// write to it in sandboxed build environments.
fn cache_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/cli-test-cache")
}

/// A `lsaca` invocation whose build cache is the test cache directory.
fn lsaca() -> Command {
    let mut command = std::process::Command::new(binary());
    command.env("LSACA_CACHE_DIR", cache_dir());
    command
}

/// A `lsaca` invocation with a private cache directory, so the test always
/// exercises a cold build.
fn lsaca_with_cache(cache: &Path) -> Command {
    let mut command = std::process::Command::new(binary());
    command.env("LSACA_CACHE_DIR", cache);
    command
}

/// Executable file name for a program built on the current platform.
fn executable_name(program_name: &str) -> String {
    if cfg!(windows) {
        format!("{}.exe", program_name)
    } else {
        program_name.to_string()
    }
}

fn unique_dir(name: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/cli-test-tmp");
    path.push(format!(
        "lsaca-cli-test-{}-{}-{}",
        std::process::id(),
        name,
        id
    ));
    fs::create_dir_all(&path).expect("could not create the test directory");
    path
}

fn write_program(dir: &Path, name: &str, source: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, source).expect("could not write the test program");
    path
}

#[test]
fn version_is_printed() {
    let output = lsaca()
        .arg("version")
        .output()
        .expect("could not run lsaca");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "version exited with {:?}",
        output.status.code()
    );
    let expected = format!("lsaca {}", env!("CARGO_PKG_VERSION"));
    assert!(
        stdout.contains(&expected),
        "unexpected version output: {:?}",
        stdout
    );
}

#[test]
fn version_and_help_take_no_arguments() {
    for (command, extra) in [("version", "--cmake"), ("help", "--cmake")] {
        let output = lsaca()
            .arg(command)
            .arg(extra)
            .output()
            .expect("could not run lsaca");
        assert_eq!(
            output.status.code(),
            Some(2),
            "lsaca {} {} was accepted",
            command,
            extra
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("unexpected argument '--cmake'"),
            "unexpected diagnostic for lsaca {} {}: {:?}",
            command,
            extra,
            stderr
        );
    }
}

#[test]
fn help_is_printed_without_a_file() {
    let output = lsaca().arg("help").output().expect("could not run lsaca");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    for expected in [
        "USAGE:",
        "run [FILE.ls]",
        "check [FILE.ls]",
        "new <DIR>",
        "COMMANDS:",
        "EXIT CODES:",
    ] {
        assert!(
            stdout.contains(expected),
            "help output is missing {:?}: {:?}",
            expected,
            stdout
        );
    }
}

#[test]
fn no_arguments_is_a_usage_error() {
    let output = lsaca().output().expect("could not run lsaca");
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("USAGE"),
        "expected usage text, got {:?}",
        stdout
    );
}

#[test]
fn unknown_command_is_a_usage_error() {
    let output = lsaca()
        .arg("frobnicate")
        .output()
        .expect("could not run lsaca");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown command"),
        "expected unknown command error, got {:?}",
        stderr
    );
}

#[test]
fn missing_file_is_an_error() {
    let dir = unique_dir("missing");
    let path = dir.join("does_not_exist.ls");
    let output = lsaca()
        .arg("check")
        .arg(&path)
        .output()
        .expect("could not run lsaca");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot read"),
        "expected a read error, got {:?}",
        stderr
    );
}

#[test]
fn check_accepts_a_valid_program() {
    let dir = unique_dir("valid");
    let path = write_program(
        &dir,
        "ok.ls",
        "fn main() {\n    let value = 1 + 2;\n    println!(\"{}\", value);\n}\n",
    );
    let output = lsaca()
        .arg("check")
        .arg(&path)
        .output()
        .expect("could not run lsaca");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "check failed: {:?} {:?}",
        stdout,
        stderr
    );
}

#[test]
fn check_reports_type_errors() {
    let dir = unique_dir("invalid");
    let path = write_program(
        &dir,
        "bad.ls",
        "fn main() {\n    let wrong: bool = 42;\n}\n",
    );
    let output = lsaca()
        .arg("check")
        .arg(&path)
        .output()
        .expect("could not run lsaca");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error"),
        "expected a type error, got {:?}",
        stderr
    );
    assert!(
        stderr.contains("bad.ls"),
        "the diagnostic should mention the file: {:?}",
        stderr
    );
}

#[test]
fn emit_c_writes_a_cmake_project() {
    let dir = unique_dir("emit");
    let path = write_program(
        &dir,
        "hello.ls",
        "fn main() {\n    println!(\"hello\");\n}\n",
    );
    let out_dir = dir.join("generated");
    let output = lsaca()
        .arg("emit-c")
        .arg(&path)
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("could not run lsaca");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "emit-c failed: {:?}", stderr);
    for name in ["main.c", "CMakeLists.txt"] {
        let generated = out_dir.join(name);
        assert!(generated.is_file(), "emit-c did not write {}", name);
    }
    for (name, _) in lsaca::runtime::sources() {
        let generated = out_dir.join(name);
        assert!(generated.is_file(), "emit-c did not write {}", name);
    }
    let cmake = fs::read_to_string(out_dir.join("CMakeLists.txt"))
        .expect("could not read the generated CMakeLists.txt");
    assert!(cmake.contains("main.c"), "CMakeLists.txt must list main.c");
    for (name, _) in lsaca::runtime::sources() {
        if name.ends_with(".c") {
            assert!(
                cmake.contains(name),
                "CMakeLists.txt does not list the runtime file {}",
                name
            );
        }
    }
}

#[test]
fn check_prints_warnings_for_successful_programs() {
    let dir = unique_dir("warn");
    let path = write_program(
        &dir,
        "warn.ls",
        "fn main() {\n    let value = 3;\n    match value {\n        0 => println!(\"zero\"),\n        _ => println!(\"other\"),\n        1 => println!(\"one\"),\n    }\n}\n",
    );
    let output = lsaca()
        .arg("check")
        .arg(&path)
        .output()
        .expect("could not run lsaca");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "check failed: {:?}", stderr);
    assert!(
        stderr.contains("warning"),
        "expected a warning, got {:?}",
        stderr
    );
    assert!(
        stderr.contains("unreachable"),
        "expected an unreachable-pattern warning, got {:?}",
        stderr
    );
    assert!(
        stderr.contains("ok"),
        "a checked file should still report ok: {:?}",
        stderr
    );
}

#[test]
fn run_builds_and_executes_a_program() {
    let dir = unique_dir("run");
    let path = write_program(
        &dir,
        "hello.ls",
        "fn main() {\n    println!(\"hello from run\");\n}\n",
    );
    let output = lsaca()
        .arg("run")
        .arg(&path)
        .output()
        .expect("could not run lsaca");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "run failed: {:?}", stderr);
    assert_eq!(stdout, "hello from run\n");
}

#[test]
fn run_propagates_panic_exit_codes() {
    let dir = unique_dir("panic");
    let path = write_program(
        &dir,
        "boom.ls",
        "fn main() {\n    let zero = 0;\n    println!(\"{}\", 1 / zero);\n}\n",
    );
    let output = lsaca()
        .arg("run")
        .arg(&path)
        .output()
        .expect("could not run lsaca");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(101),
        "expected panic exit code, got {:?} ({:?})",
        output.status.code(),
        stderr
    );
    assert!(
        stderr.contains("panicked"),
        "expected a panic message, got {:?}",
        stderr
    );
    assert!(
        stderr.contains("divide by zero"),
        "expected a division-by-zero message, got {:?}",
        stderr
    );
}

#[test]
fn build_copies_the_executable_to_the_output_directory() {
    let dir = unique_dir("build");
    let path = write_program(
        &dir,
        "built.ls",
        "fn main() {\n    println!(\"built ok\");\n}\n",
    );
    let out_dir = dir.join("out");
    let output = lsaca()
        .arg("build")
        .arg(&path)
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("could not run lsaca");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "build failed: {:?}", stderr);
    assert!(
        stderr.contains("built"),
        "expected a build confirmation, got {:?}",
        stderr
    );
    let executable = out_dir.join(if cfg!(windows) { "built.exe" } else { "built" });
    assert!(
        executable.is_file(),
        "the executable was not copied to {}",
        executable.display()
    );
    let executed = Command::new(&executable)
        .output()
        .expect("could not execute the built program");
    assert_eq!(String::from_utf8_lossy(&executed.stdout), "built ok\n");
}

#[test]
fn unusable_cache_directory_is_reported() {
    let dir = unique_dir("bad-cache");
    let blocker = dir.join("blocker");
    fs::write(&blocker, b"this is a file, not a directory").expect("could not write the blocker");
    let path = write_program(
        &dir,
        "hello.ls",
        "fn main() {\n    println!(\"hello\");\n}\n",
    );
    let output = lsaca()
        .env("LSACA_CACHE_DIR", &blocker)
        .arg("build")
        .arg(&path)
        .arg("-o")
        .arg(dir.join("out"))
        .output()
        .expect("could not run lsaca");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("LSACA_CACHE_DIR"),
        "expected a cache-directory error, got {:?}",
        stderr
    );
    assert!(
        stderr.contains("hello.ls"),
        "expected the source file to be named, got {:?}",
        stderr
    );
}

#[test]
fn default_build_uses_the_direct_compiler() {
    let dir = unique_dir("direct");
    let path = write_program(
        &dir,
        "direct_prog.ls",
        "fn main() {\n    println!(\"direct ok\");\n}\n",
    );
    let out_dir = dir.join("out");
    let output = lsaca_with_cache(&dir.join("cache"))
        .arg("build")
        .arg("-r")
        .arg("-v")
        .arg(&path)
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("could not run lsaca");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "build failed: {:?}", stderr);
    assert!(
        stderr.contains("direct compiler backend"),
        "the default build should use the direct C compiler backend: {:?}",
        stderr
    );
    assert!(
        !stderr.contains("cmake configure"),
        "the default build must not configure a CMake project: {:?}",
        stderr
    );
    let executable = out_dir.join(executable_name("direct_prog"));
    let executed = Command::new(&executable)
        .output()
        .expect("could not execute the built program");
    assert_eq!(String::from_utf8_lossy(&executed.stdout), "direct ok\n");
}

#[test]
fn cmake_flag_builds_through_the_generated_project() {
    // CMake spawns a build tool (Ninja) that cannot create child processes
    // under some confined developer sandboxes: the children hang instead of
    // failing. The generated project's CMake path is still covered by the
    // dedicated CI job, so such an environment opts out explicitly.
    if std::env::var_os("LSACA_SKIP_CMAKE_TESTS").is_some() {
        eprintln!("skipping the CMake backend test: LSACA_SKIP_CMAKE_TESTS is set");
        return;
    }
    let dir = unique_dir("cmake-backend");
    let path = write_program(
        &dir,
        "cmake_prog.ls",
        "fn main() {\n    println!(\"cmake ok\");\n}\n",
    );
    let out_dir = dir.join("out");
    let output = lsaca_with_cache(&dir.join("cache"))
        .arg("build")
        .arg("--cmake")
        .arg(&path)
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("could not run lsaca");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "the --cmake build failed: {:?}",
        stderr
    );
    let executable = out_dir.join(executable_name("cmake_prog"));
    let executed = Command::new(&executable)
        .output()
        .expect("could not execute the built program");
    assert_eq!(String::from_utf8_lossy(&executed.stdout), "cmake ok\n");
}

#[test]
fn invalid_compiler_is_reported() {
    let dir = unique_dir("bad-compiler");
    let path = write_program(
        &dir,
        "hello.ls",
        "fn main() {\n    println!(\"hello\");\n}\n",
    );
    let missing = dir.join("not-a-compiler.exe");
    let output = lsaca_with_cache(&dir.join("cache"))
        .env("LSACA_CC", &missing)
        .arg("build")
        .arg(&path)
        .arg("-o")
        .arg(dir.join("out"))
        .output()
        .expect("could not run lsaca");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("LSACA_CC"),
        "expected a compiler configuration error, got {:?}",
        stderr
    );
    assert!(
        stderr.contains("not an executable file"),
        "expected an explanatory compiler error, got {:?}",
        stderr
    );
}

/// Create a package directory with a manifest and source files.
fn create_package(dir: &Path, manifest: &str, files: &[(&str, &str)]) {
    fs::create_dir_all(dir).expect("could not create the package directory");
    fs::write(dir.join("lsaca.toml"), manifest).expect("could not write lsaca.toml");
    for (relative, contents) in files {
        let path = dir.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("could not create a source directory");
        }
        fs::write(&path, contents).expect("could not write a package source file");
    }
}

#[test]
fn new_creates_a_runnable_binary_package() {
    let dir = unique_dir("new-package");
    let package = dir.join("hello_pkg");
    let output = lsaca()
        .arg("new")
        .arg(&package)
        .output()
        .expect("could not run lsaca");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "lsaca new failed: {:?}",
        stderr
    );
    assert!(
        stderr.contains("created binary package"),
        "expected a creation message, got {:?}",
        stderr
    );
    assert!(package.join("lsaca.toml").is_file());
    assert!(package.join("src/main.ls").is_file());
    let run = lsaca()
        .arg("run")
        .current_dir(&package)
        .output()
        .expect("could not run lsaca");
    let stdout = String::from_utf8_lossy(&run.stdout);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert_eq!(
        run.status.code(),
        Some(0),
        "package run failed: {:?}",
        stderr
    );
    assert_eq!(stdout, "Hello, world!\n");
    // Creating a package in a non-empty directory must fail cleanly.
    let again = lsaca()
        .arg("new")
        .arg(&package)
        .output()
        .expect("could not run lsaca");
    assert_eq!(again.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&again.stderr);
    assert!(
        stderr.contains("not empty"),
        "unexpected error: {:?}",
        stderr
    );
}

#[test]
fn missing_manifest_is_a_usage_error() {
    let dir = unique_dir("no-manifest");
    let output = lsaca()
        .arg("check")
        .current_dir(&dir)
        .output()
        .expect("could not run lsaca");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no lsaca.toml found"),
        "expected a manifest discovery error, got {:?}",
        stderr
    );
}

#[test]
fn library_packages_check_but_do_not_run() {
    let dir = unique_dir("library-package");
    let package = dir.join("util_lib");
    create_package(
        &package,
        "[package]\nname = \"util_lib\"\nversion = \"0.1.0\"\n",
        &[("src/lib.ls", "pub fn answer() -> i64 { 42 }\n")],
    );
    let check = lsaca()
        .arg("check")
        .current_dir(&package)
        .output()
        .expect("could not run lsaca");
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert_eq!(
        check.status.code(),
        Some(0),
        "a library package should type check: {:?}",
        stderr
    );
    let run = lsaca()
        .arg("run")
        .current_dir(&package)
        .output()
        .expect("could not run lsaca");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert_eq!(run.status.code(), Some(1));
    assert!(
        stderr.contains("library package"),
        "expected a library-package error, got {:?}",
        stderr
    );
    // emit-c and build need an entry point too: a library would otherwise
    // produce a C executable rooted at an arbitrary exported function.
    for command in ["build", "emit-c"] {
        let output = lsaca()
            .arg(command)
            .current_dir(&package)
            .output()
            .expect("could not run lsaca");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "expected '{}' to reject a library package: {:?}",
            command,
            stderr
        );
        assert!(
            stderr.contains("library package"),
            "expected a library-package error for '{}', got {:?}",
            command,
            stderr
        );
    }
}

#[test]
fn binary_packages_without_main_get_a_specific_error() {
    let dir = unique_dir("missing-main-package");
    let package = dir.join("no_main");
    create_package(
        &package,
        "[package]\nname = \"no_main\"\nversion = \"0.1.0\"\n",
        &[("src/main.ls", "pub fn helper() -> i64 { 1 }\n")],
    );
    let run = lsaca()
        .arg("run")
        .current_dir(&package)
        .output()
        .expect("could not run lsaca");
    assert_eq!(run.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("no fn main()"),
        "expected a missing-main error, got {:?}",
        stderr
    );
    // Type checking reports the missing entry point as a diagnostic.
    let check = lsaca()
        .arg("check")
        .current_dir(&package)
        .output()
        .expect("could not run lsaca");
    assert_eq!(check.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        stderr.contains("main"),
        "expected a missing-main diagnostic, got {:?}",
        stderr
    );
}

#[test]
fn package_dependencies_are_built_together() {
    let dir = unique_dir("package-dependency");
    let dependency = dir.join("greeting");
    create_package(
        &dependency,
        "[package]\nname = \"greeting\"\nversion = \"0.1.0\"\n",
        &[(
            "src/lib.ls",
            "pub fn greet(name: String) -> String {\n    String::from(\"hello, \") + name\n}\n",
        )],
    );
    let app = dir.join("app");
    create_package(
        &app,
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\ngreeting = { path = \"../greeting\" }\n",
        &[(
            "src/main.ls",
            "use greeting::greet;\n\nfn main() {\n    println!(\"{}\", greet(String::from(\"package\")));\n}\n",
        )],
    );
    let output = lsaca()
        .arg("run")
        .current_dir(&app)
        .output()
        .expect("could not run lsaca");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "package run failed: {:?}",
        stderr
    );
    assert_eq!(stdout, "hello, package\n");
}
