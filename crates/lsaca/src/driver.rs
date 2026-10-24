//! Pipeline orchestration: source text to a checked HIR, then to a C project
//! and (optionally) a native executable built with CMake.
//!
//! The build strategy mirrors what a C toolchain does best: the compiler emits
//! a regular, self-contained C project (main.c, the runtime, CMakeLists.txt)
//! and CMake + a C compiler turn it into a native executable. Build trees are
//! content-addressed and cached between runs.
//!
//! The cache root is chosen defensively, because toolchains dislike non-ASCII
//! paths and locked-down environments (sandboxes, restricted CI runners) only
//! allow writes inside the workspace: the system temporary directory is used
//! when it has an ASCII path and is actually writable, and
//! `.lsaca-build-cache` in the working directory is the fallback. The compiled
//! executable is copied out of the cache, so workspace paths may contain any
//! characters.

use std::collections::VecDeque;
use std::env;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::{self, Command};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::codegen::{self, CodegenOptions, Generated};
use crate::diag::Diagnostics;
use crate::hir;
use crate::lexer;
use crate::parser;
use crate::runtime;
use crate::span::SourceFile;
use crate::typeck;

/// Front-end pipeline: lex, parse and type check one source file.
pub struct Frontend {
    file: SourceFile,
}

impl Frontend {
    pub fn new(file_name: impl Into<String>, source: impl Into<String>) -> Frontend {
        Frontend {
            file: SourceFile::new(file_name, source),
        }
    }

    pub fn file(&self) -> &SourceFile {
        &self.file
    }

    /// Run the front-end. On success the returned HIR is ready for code
    /// generation. On failure the diagnostics explain why the program was
    /// rejected (they may contain several independent errors).
    pub fn check(&self) -> Result<hir::Program, Diagnostics> {
        self.check_reporting().map(|(program, _warnings)| program)
    }

    /// Run the front-end, keeping non-fatal diagnostics from every stage.
    ///
    /// On success the first element holds the HIR and the second holds the
    /// warnings that were reported while checking (unreachable match arms and
    /// similar). On failure the errors are returned as the error value, and
    /// they already include any warnings emitted before the failure.
    pub fn check_reporting(&self) -> Result<(hir::Program, Diagnostics), Diagnostics> {
        let (tokens, mut diagnostics) = lexer::tokenize(self.file.text());
        let (ast, parse_diagnostics) = parser::parse(&tokens, &self.file);
        diagnostics.extend(parse_diagnostics);
        if diagnostics.has_errors() {
            return Err(diagnostics);
        }
        let (result, type_diagnostics) = typeck::check_reporting(&ast, &self.file);
        match result {
            Ok(program) => {
                diagnostics.extend(type_diagnostics);
                Ok((program, diagnostics))
            }
            Err(errors) => {
                diagnostics.extend(errors);
                diagnostics.extend(type_diagnostics);
                Err(diagnostics)
            }
        }
    }
}

/// Generate the C project for a checked program.
pub fn generate_c(program: &hir::Program, options: &CodegenOptions) -> Generated {
    codegen::generate(program, options)
}

/// Write the generated C project (main.c, lsaca_rt.h, lsaca_rt.c,
/// CMakeLists.txt) into a directory, creating it if needed.
pub fn write_project(dir: &Path, generated: &Generated) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    fs::write(dir.join("main.c"), &generated.main_c)?;
    fs::write(dir.join("lsaca_rt.h"), runtime::RUNTIME_H)?;
    fs::write(dir.join("lsaca_rt.c"), runtime::RUNTIME_C)?;
    fs::write(dir.join("CMakeLists.txt"), &generated.cmake)?;
    Ok(())
}

/// Result of a successful native build.
#[derive(Debug, Clone)]
pub struct BuiltExecutable {
    /// Directory containing the CMake build tree.
    pub build_dir: PathBuf,
    /// Path to the built executable.
    pub executable: PathBuf,
}

/// Options controlling one native build.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// Build with optimizations and wrapping arithmetic instead of debug
    /// overflow checks.
    pub release: bool,
    /// Name of the generated executable (without a platform suffix).
    pub program_name: String,
    /// Print the commands that are executed.
    pub verbose: bool,
}

impl Default for BuildOptions {
    fn default() -> BuildOptions {
        BuildOptions {
            release: false,
            program_name: String::from("program"),
            verbose: false,
        }
    }
}

impl BuildOptions {
    pub fn new(program_name: impl Into<String>) -> BuildOptions {
        BuildOptions {
            program_name: program_name.into(),
            ..BuildOptions::default()
        }
    }

    pub fn release(mut self, release: bool) -> BuildOptions {
        self.release = release;
        self
    }

    pub fn verbose(mut self, verbose: bool) -> BuildOptions {
        self.verbose = verbose;
        self
    }
}

/// Configure and build a generated C project with CMake.
///
/// The project directory must already contain main.c, lsaca_rt.h, lsaca_rt.c
/// and CMakeLists.txt (see [write_project]). The build tree is created in
/// project_dir/build.
pub fn build_project(project_dir: &Path, options: &BuildOptions) -> io::Result<BuiltExecutable> {
    let cmake = find_tool("cmake", &[".exe"])?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "CMake was not found on PATH; install CMake 3.16 or newer to build lsaca programs",
        )
    })?;
    let compiler = find_c_compiler()?;
    let generator = find_generator()?;

    let build_dir = project_dir.join("build");
    fs::create_dir_all(&build_dir)?;
    // Point the child toolchain at a scratch directory inside the project, which
    // is known to be writable because the generated sources were just written
    // there. This keeps CMake, the compiler and the generator from depending on
    // the system temporary directory, which is missing in some sandboxes.
    let tmp_dir = project_dir.join("tmp");
    fs::create_dir_all(&tmp_dir)?;

    let mut configure = Command::new(&cmake);
    configure
        .arg("-S")
        .arg(project_dir)
        .arg("-B")
        .arg(&build_dir)
        .arg("-G")
        .arg(&generator)
        .arg(format!("-DCMAKE_C_COMPILER={}", compiler.display()))
        .arg(format!(
            "-DCMAKE_BUILD_TYPE={}",
            if options.release { "Release" } else { "Debug" }
        ));
    configure.current_dir(project_dir);
    prepare_child(&mut configure, compiler.parent(), &tmp_dir);
    run_tool(&mut configure, "cmake configure", options.verbose)?;

    let mut build = Command::new(&cmake);
    build
        .arg("--build")
        .arg(&build_dir)
        .arg("--parallel")
        .arg(jobs());
    build.current_dir(project_dir);
    prepare_child(&mut build, compiler.parent(), &tmp_dir);
    run_tool(&mut build, "cmake build", options.verbose)?;

    let executable = locate_executable(&build_dir, &options.program_name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "the C compiler reported success but no executable named {} was found in {}",
                options.program_name,
                build_dir.display()
            ),
        )
    })?;

    Ok(BuiltExecutable {
        build_dir,
        executable,
    })
}

/// Build a generated project inside a content-addressed cache.
///
/// The cache key covers every generated file, the runtime sources, the compiler
/// identity and the build profile, so a cached executable can be reused for an
/// identical compilation. The cache lives under cache_root and is safe to
/// delete at any time.
pub fn build_with_cache(
    generated: &Generated,
    options: &BuildOptions,
    cache_root: &Path,
) -> io::Result<BuiltExecutable> {
    let compiler = find_c_compiler()?;
    let key = cache_key(generated, options, &compiler);
    let project_dir = cache_root.join(format!("{:016x}", key));
    let build_dir = project_dir.join("build");
    if let Some(executable) = locate_executable(&build_dir, &options.program_name) {
        return Ok(BuiltExecutable {
            build_dir,
            executable,
        });
    }
    write_project(&project_dir, generated).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "could not write the generated C project to {}: {}",
                project_dir.display(),
                error
            ),
        )
    })?;
    build_project(&project_dir, options).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "could not build the generated C project in {}: {}",
                project_dir.display(),
                error
            ),
        )
    })
}

/// Compute the content address of one generated build. Exposed for tests and
/// for the CLI, which caches builds across invocations.
pub fn cache_key(generated: &Generated, options: &BuildOptions, compiler: &Path) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for bytes in [
        generated.main_c.as_bytes(),
        generated.cmake.as_bytes(),
        runtime::RUNTIME_H.as_bytes(),
        runtime::RUNTIME_C.as_bytes(),
        compiler.as_os_str().to_string_lossy().as_bytes(),
        options.program_name.as_bytes(),
    ] {
        hash = fnv1a(hash, bytes);
        hash = fnv1a(hash, &[0xff]);
    }
    hash = fnv1a(hash, &[if options.release { 1 } else { 0 }]);
    hash
}

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Copy a built executable to destination, creating parent directories.
pub fn copy_executable(executable: &Path, destination: &Path) -> io::Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(executable, destination)?;
    Ok(())
}

/// Directory for cached build trees and intermediates.
///
/// The returned directory is created when necessary and is guaranteed to be
/// writable. The candidates are, in order:
///
/// 1. `LSACA_CACHE_DIR`, when set. An unusable value is an error instead of a
///    silent fallback, because the user asked for it explicitly;
/// 2. `lsaca-builds` under the system temporary directory, when that path is
///    ASCII-only and writable;
/// 3. `.lsaca-build-cache` in the current directory. This keeps builds working
///    in sandboxes that only allow writes inside the workspace, and when the
///    system temporary directory is not writable.
pub fn cache_root() -> io::Result<PathBuf> {
    if let Some(configured) = env::var_os("LSACA_CACHE_DIR") {
        let dir = PathBuf::from(configured);
        return match probe_cache_dir(&dir) {
            Ok(()) => Ok(dir),
            Err(error) => Err(io::Error::new(
                error.kind(),
                format!(
                    "LSACA_CACHE_DIR ({}) is not usable as a build cache: {}",
                    dir.display(),
                    error
                ),
            )),
        };
    }
    let mut failures: Vec<String> = Vec::new();
    for candidate in cache_root_candidates() {
        match probe_cache_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) => failures.push(format!("{}: {}", candidate.display(), error)),
        }
    }
    Err(io::Error::other(format!(
        "no writable build cache directory found ({}); set LSACA_CACHE_DIR to a writable directory",
        failures.join("; ")
    )))
}

fn cache_root_candidates() -> Vec<PathBuf> {
    let base = env::temp_dir();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if is_ascii_path(&base) {
        candidates.push(base.join("lsaca-builds"));
    }
    if let Ok(cwd) = env::current_dir() {
        let local = cwd.join(".lsaca-build-cache");
        if !candidates.iter().any(|candidate| candidate == &local) {
            candidates.push(local);
        }
    }
    candidates
}

/// Check that `dir` can be created and that new files can be written in it.
fn probe_cache_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    static PROBE_COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let probe = dir.join(format!(
        ".lsaca-write-probe-{}-{}-{}",
        process::id(),
        nanos,
        PROBE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map(|_| ());
    let _ = fs::remove_file(&probe);
    result
}

fn is_ascii_path(path: &Path) -> bool {
    path.to_string_lossy().is_ascii()
}

fn jobs() -> String {
    match env::var("LSACA_JOBS") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => env::var("NUMBER_OF_PROCESSORS").unwrap_or_else(|_| String::from("4")),
    }
}

fn run_tool(command: &mut Command, what: &str, verbose: bool) -> io::Result<()> {
    if verbose {
        eprintln!("lsaca: running {}: {:?}", what, command);
    }
    let output = command.output().map_err(|error| {
        io::Error::new(error.kind(), format!("failed to start {}: {}", what, error))
    })?;
    if verbose {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stdout.lines().chain(stderr.lines()) {
            eprintln!("lsaca:   {}", line);
        }
    }
    if output.status.success() {
        return Ok(());
    }
    let mut message = format!("{} failed with {}", what, output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.trim().is_empty() {
        message.push_str("\n--- stdout ---\n");
        message.push_str(stdout.trim_end());
    }
    if !stderr.trim().is_empty() {
        message.push_str("\n--- stderr ---\n");
        message.push_str(stderr.trim_end());
    }
    Err(io::Error::other(message))
}

/// Give child processes a writable temporary directory and a PATH that can find
/// the C compiler even when it was located through one of the well-known
/// fallback locations.
fn prepare_child(command: &mut Command, compiler_dir: Option<&Path>, tmp_dir: &Path) {
    command
        .env("TEMP", tmp_dir)
        .env("TMP", tmp_dir)
        .env("TMPDIR", tmp_dir);
    let Some(compiler_dir) = compiler_dir else {
        return;
    };
    let mut paths: Vec<PathBuf> = vec![compiler_dir.to_path_buf()];
    if let Some(existing) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing));
    }
    if let Ok(joined) = env::join_paths(paths) {
        command.env("PATH", joined);
    }
}

/// Find the C compiler used for native builds.
///
/// Order: LSACA_CC, gcc/cc/clang on PATH, then the usual MSYS2 installation
/// directories. The returned path is absolute whenever the compiler was found
/// through a fallback location.
pub fn find_c_compiler() -> io::Result<PathBuf> {
    if let Some(configured) = env::var_os("LSACA_CC") {
        let path = PathBuf::from(configured);
        if which_path(&path).is_some() {
            return Ok(path);
        }
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "LSACA_CC points at {}, which is not an executable file",
                path.display()
            ),
        ));
    }
    for name in ["gcc", "cc", "clang"] {
        if let Some(found) = find_tool(name, &[".exe"])? {
            return Ok(found);
        }
    }
    let mut fallbacks: Vec<PathBuf> = Vec::new();
    for root in [
        r"C:\msys64\ucrt64\bin",
        r"C:\msys64\mingw64\bin",
        r"C:\msys64\clang64\bin",
        r"C:\mingw64\bin",
        r"C:\ProgramData\chocolatey\bin",
    ] {
        fallbacks.push(PathBuf::from(root).join("gcc.exe"));
        fallbacks.push(PathBuf::from(root).join("clang.exe"));
    }
    for candidate in fallbacks {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no C compiler found: install GCC/Clang (for example with MSYS2) and put it on PATH, or set LSACA_CC to the compiler executable",
    ))
}

fn find_generator() -> io::Result<String> {
    if find_tool("ninja", &[".exe"])?.is_some() {
        return Ok(String::from("Ninja"));
    }
    if cfg!(windows) {
        if find_tool("mingw32-make", &[".exe"])?.is_some() {
            return Ok(String::from("MinGW Makefiles"));
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no build generator found: install Ninja (recommended) or mingw32-make and put it on PATH",
        ))
    } else {
        // POSIX systems ship a `make` compatible generator with CMake itself,
        // so Ninja is a performance choice rather than a requirement.
        Ok(String::from("Unix Makefiles"))
    }
}

/// Locate a tool by name on PATH. The returned path is absolute.
pub fn find_tool(name: &str, extensions: &[&str]) -> io::Result<Option<PathBuf>> {
    let mut names: Vec<String> = vec![name.to_string()];
    if Path::new(name).extension().is_none() {
        for extension in extensions {
            names.push(format!("{}{}", name, extension));
        }
    }
    let Some(path_var) = env::var_os("PATH") else {
        return Ok(None);
    };
    for dir in env::split_paths(&path_var) {
        for candidate in &names {
            let path = dir.join(candidate);
            if path.is_file() {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}

fn which_path(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    None
}

/// Find the executable produced by the build, tolerating the directory layouts
/// used by single-config and multi-config CMake generators.
pub fn locate_executable(build_dir: &Path, program_name: &str) -> Option<PathBuf> {
    if !build_dir.is_dir() {
        return None;
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    let suffixes = if cfg!(windows) {
        [".exe", ""]
    } else {
        ["", ".exe"]
    };
    for suffix in suffixes {
        candidates.push(build_dir.join(format!("{}{}", program_name, suffix)));
        candidates.push(
            build_dir
                .join("bin")
                .join(format!("{}{}", program_name, suffix)),
        );
        candidates.push(
            build_dir
                .join("Debug")
                .join(format!("{}{}", program_name, suffix)),
        );
        candidates.push(
            build_dir
                .join("Release")
                .join(format!("{}{}", program_name, suffix)),
        );
    }
    for candidate in &candidates {
        if candidate.is_file() {
            return Some(candidate.clone());
        }
    }
    // Last resort: breadth-first search up to four levels deep. CMake sometimes
    // adds configuration subdirectories for multi-config generators.
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((build_dir.to_path_buf(), 0));
    while let Some((dir, depth)) = queue.pop_front() {
        if depth >= 4 {
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                queue.push_back((path, depth + 1));
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name == program_name || name == format!("{}.exe", program_name))
                .unwrap_or(false)
            {
                return Some(path);
            }
        }
    }
    None
}

/// Make a path relative to the current directory when that is shorter and
/// unambiguous, purely for nicer diagnostics.
pub fn display_path(path: &Path) -> String {
    path.components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect::<PathBuf>()
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scratch directory under the workspace target directory.
    ///
    /// The tests must not depend on the system temporary directory: the
    /// compiler supports environments where child processes cannot write there,
    /// so its own tests are written to work in exactly those environments too.
    fn scratch_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/driver-test-tmp")
            .join(format!("{}-{}", process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("could not create the test directory");
        dir
    }

    #[test]
    fn probe_accepts_writable_directory() {
        let dir = scratch_dir("writable");
        probe_cache_dir(&dir).expect("a fresh temporary directory should be writable");
        assert!(dir.is_dir());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn probe_rejects_path_below_a_file() {
        let dir = scratch_dir("blocked");
        let file = dir.join("blocker");
        fs::write(&file, b"not a directory").expect("could not write the test file");
        assert!(probe_cache_dir(&file.join("nested")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_candidates_end_with_workspace_fallback() {
        let candidates = cache_root_candidates();
        let cwd = env::current_dir().expect("the current directory should be available");
        assert_eq!(candidates.last(), Some(&cwd.join(".lsaca-build-cache")));
    }
}
