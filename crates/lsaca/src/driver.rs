//! Pipeline orchestration: source text to a checked HIR, then to a C project
//! and (optionally) a native executable.
//!
//! The compiler emits a regular, self-contained C project (main.c, the runtime
//! sources and CMakeLists.txt). By default the project is built by invoking the
//! C compiler directly: every runtime translation unit is compiled once per
//! (runtime content, compiler, flags) combination into a shared object cache
//! and then linked with main.c. The generated CMake project remains available
//! through [`BuildOptions::use_cmake`] (`lsaca --cmake`) and is what `lsaca
//! emit-c` writes. Build trees are content-addressed and cached between runs.
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
use std::time::Instant;

use crate::codegen::{self, CodegenOptions, Generated};
use crate::diag::Diagnostics;
use crate::hir;
use crate::lexer;
use crate::parser;
use crate::runtime;
use crate::span::{SourceFile, Span};
use crate::typeck;

/// Stack size of the worker thread that runs the recursive compiler passes.
///
/// The parser rejects syntax nested deeper than 128 AST levels, and every later
/// pass walks the tree recursively. The type checker needs roughly 10 KiB of
/// stack per level in a debug build, so on the default 1 MiB main thread a legal
/// program with about 99 nested expressions used to abort with a stack overflow
/// (exit code 0xC00000FD) instead of compiling. A 64 MiB thread keeps every
/// accepted program safe (128 levels need well under 2 MiB); the main thread
/// stack is left untouched for the rest of the process.
const COMPILER_STACK_SIZE: usize = 64 * 1024 * 1024;

/// Run one recursive compiler pass on a worker thread with a large stack.
///
/// Returns the panic payload when the thread could not be spawned or when the
/// pass panicked. Both are internal compiler errors: the type checker reports
/// them as a diagnostic, while the code generator re-raises the panic.
fn run_on_compiler_stack<T, F>(work: F) -> Result<T, Box<dyn std::any::Any + Send>>
where
    T: Send,
    F: FnOnce() -> T + Send,
{
    std::thread::scope(|scope| {
        match std::thread::Builder::new()
            .name("lsaca-compiler".to_string())
            .stack_size(COMPILER_STACK_SIZE)
            .spawn_scoped(scope, work)
        {
            Ok(handle) => handle.join(),
            Err(error) => Err(
                Box::new(format!("could not start the compiler thread: {error}"))
                    as Box<dyn std::any::Any + Send>,
            ),
        }
    })
}

/// Describe a panic payload passed through run_on_compiler_stack.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        return (*message).to_string();
    }
    "internal compiler error".to_string()
}

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
        // The standard-library prelude is embedded in the compiler and parsed
        // once per process. Its spans are already zeroed, so prelude-origin
        // diagnostics never point into the file being checked.
        let prelude = match crate::prelude::program() {
            Ok(program) => Some(program),
            Err(errors) => {
                diagnostics.extend(errors);
                return Err(diagnostics);
            }
        };
        // Type checking and HIR lowering recurse once per AST level, so they
        // run on the large-stack worker thread: the parser already guarantees
        // that accepted syntax is at most 128 levels deep, which overflows the
        // default main-thread stack in debug builds.
        let checked = run_on_compiler_stack(|| {
            typeck::check_reporting_with_prelude(&ast, prelude, &self.file)
        });
        let (result, type_diagnostics) = match checked {
            Ok(checked) => checked,
            Err(payload) => {
                diagnostics.error(
                    format!(
                        "the compiler crashed while checking this program ({})",
                        panic_message(&*payload)
                    ),
                    Span::default(),
                );
                return Err(diagnostics);
            }
        };
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
///
/// Code generation walks the HIR recursively, so it runs on the large-stack
/// worker thread as well. A failure here is an internal compiler bug (the input
/// has already been type checked), so the panic is propagated unchanged.
pub fn generate_c(program: &hir::Program, options: &CodegenOptions) -> Generated {
    match run_on_compiler_stack(|| codegen::generate(program, options)) {
        Ok(generated) => generated,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// Write the generated C project (main.c, the extra translation units of a
/// split program, every runtime source published by [`runtime::sources`] and
/// CMakeLists.txt) into a directory, creating it if needed.
pub fn write_project(dir: &Path, generated: &Generated) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    write_atomic(&dir.join("main.c"), &generated.main_c)?;
    for unit in &generated.units {
        write_atomic(&dir.join(&unit.file_name), &unit.contents)?;
    }
    write_runtime_sources(dir)?;
    write_atomic(&dir.join("CMakeLists.txt"), &generated.cmake)?;
    Ok(())
}

/// Write the runtime sources (headers and translation units) next to main.c.
fn write_runtime_sources(dir: &Path) -> io::Result<()> {
    for (name, contents) in runtime::sources() {
        write_atomic(&dir.join(name), contents)?;
    }
    Ok(())
}

/// Write a file without ever exposing a partially written file to concurrent
/// readers: the contents go to a unique temporary file in the same directory
/// that is then renamed into place. Build trees are content-addressed, so
/// several threads may legitimately write the same file with the same bytes;
/// losing that race is not an error.
fn write_atomic(path: &Path, contents: &str) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let temp = temporary_path(parent, name);
    fs::write(&temp, contents)?;
    match fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temp);
            if is_non_empty_file(path) {
                Ok(())
            } else {
                Err(io::Error::new(
                    error.kind(),
                    format!("could not write {}: {}", path.display(), error),
                ))
            }
        }
    }
}

/// A collision-free temporary path in the same directory as the final file.
fn temporary_path(dir: &Path, name: &str) -> PathBuf {
    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    dir.join(format!(".{}.{}.{}.tmp", name, process::id(), id))
}

/// Move a freshly built file to its final content-addressed location.
///
/// Concurrent builds may race to install the same file. Each build compiles to
/// its own temporary path, so a rename that loses the race can be discarded as
/// soon as the destination is a complete file.
fn install_built_file(temp: &Path, destination: &Path, what: &str) -> io::Result<()> {
    for attempt in 0..5 {
        match fs::rename(temp, destination) {
            Ok(()) => return Ok(()),
            Err(error) => {
                if is_non_empty_file(destination) {
                    let _ = fs::remove_file(temp);
                    return Ok(());
                }
                if attempt == 4 {
                    let _ = fs::remove_file(temp);
                    return Err(io::Error::new(
                        error.kind(),
                        format!(
                            "could not install the {} at {}: {}",
                            what,
                            destination.display(),
                            error
                        ),
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(10 * (attempt + 1)));
            }
        }
    }
    Err(io::Error::other(format!(
        "could not install the {} at {}",
        what,
        destination.display()
    )))
}

/// Result of a successful native build.
#[derive(Debug, Clone)]
pub struct BuiltExecutable {
    /// Directory containing the build tree (the CMake build directory or the
    /// direct-build object and executable directory).
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
    /// Build through the generated CMake project instead of invoking the C
    /// compiler directly. The default is the direct backend, which avoids the
    /// CMake configure step.
    pub use_cmake: bool,
}

impl Default for BuildOptions {
    fn default() -> BuildOptions {
        BuildOptions {
            release: false,
            program_name: String::from("program"),
            verbose: false,
            use_cmake: false,
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

    /// Select the CMake back end instead of the default direct compiler back
    /// end. `lsaca --cmake` sets this.
    pub fn use_cmake(mut self, use_cmake: bool) -> BuildOptions {
        self.use_cmake = use_cmake;
        self
    }
}

/// Build a generated C project.
///
/// The project directory must already contain main.c, the runtime sources and
/// CMakeLists.txt (see [write_project]). By default the C compiler is invoked
/// directly; when [BuildOptions::use_cmake] is set the generated CMake project
/// is configured and built instead. The build tree is created in
/// project_dir/build.
pub fn build_project(project_dir: &Path, options: &BuildOptions) -> io::Result<BuiltExecutable> {
    let cache_root = project_dir.parent().unwrap_or(project_dir);
    if options.use_cmake {
        build_project_with_cmake(project_dir, options)
    } else {
        build_project_direct(project_dir, cache_root, options)
    }
}

/// Configure and build a generated C project with CMake.
///
/// This is the optional back end selected by [BuildOptions::use_cmake]
/// ("lsaca --cmake"). The default back end invokes the compiler directly. The
/// build tree is created in project_dir/build.
fn build_project_with_cmake(
    project_dir: &Path,
    options: &BuildOptions,
) -> io::Result<BuiltExecutable> {
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
    if debug_rc_requested() {
        // The generated project defines a CMake option for leak tracking in
        // every configuration; expose it through the environment so users do
        // not have to edit the generated build system.
        configure.arg("-DLSACA_DEBUG_RC=ON");
    }
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

/// Compile and link a generated project by invoking the C compiler directly.
///
/// Runtime translation units are compiled once into a shared object cache under
/// cache_root/_runtime/<key>/, so the runtime cost is paid only when its
/// sources, the compiler or the flags change. Missing runtime objects are
/// compiled in parallel and the generated main.c is compiled at the same time;
/// once both are ready everything is linked into project_dir/build.
fn build_project_direct(
    project_dir: &Path,
    cache_root: &Path,
    options: &BuildOptions,
) -> io::Result<BuiltExecutable> {
    let compiler = find_c_compiler()?;
    let build_dir = project_dir.join("build");
    fs::create_dir_all(&build_dir)?;
    let tmp_dir = project_dir.join("tmp");
    fs::create_dir_all(&tmp_dir)?;
    let flags = compiler_flags(options);
    let started = Instant::now();

    write_runtime_sources(project_dir)?;

    let runtime_sources = runtime_c_sources();
    let object_dir = runtime_object_dir(cache_root, &compiler, &flags);
    fs::create_dir_all(&object_dir)?;

    let mut missing: Vec<(String, PathBuf)> = Vec::new();
    let mut reused = 0usize;
    for source in &runtime_sources {
        let object = object_dir.join(runtime_object_name(source));
        if is_non_empty_file(&object) {
            reused += 1;
        } else {
            missing.push(((*source).to_string(), object));
        }
    }
    // The generated sources to compile: main.c plus the extra translation
    // units of a split project (discovered on disk, because [write_project]
    // wrote them into the project directory).
    let generated_sources = generated_source_files(project_dir);
    let generated_objects: Vec<PathBuf> = generated_sources
        .iter()
        .map(|name| build_dir.join(object_name_for_source(name)))
        .collect();

    let (runtime_result, generated_result) = std::thread::scope(|scope| {
        let runtime_handle = scope.spawn(|| {
            compile_runtime_objects(&compiler, project_dir, &tmp_dir, &flags, options, &missing)
        });
        let generated_handle = scope.spawn(|| {
            compile_generated_objects(
                &compiler,
                project_dir,
                &tmp_dir,
                &flags,
                options,
                &generated_sources,
                &generated_objects,
            )
        });
        (
            join_compilation(runtime_handle),
            join_compilation(generated_handle),
        )
    });
    runtime_result?;
    generated_result?;

    let executable = build_dir.join(executable_name(&options.program_name));
    let temp_executable = temporary_path(&build_dir, &executable_name(&options.program_name));
    let mut link = Command::new(&compiler);
    for object in &generated_objects {
        link.arg(object);
    }
    for source in &runtime_sources {
        link.arg(object_dir.join(runtime_object_name(source)));
    }
    link.arg("-o").arg(&temp_executable);
    if cfg!(unix) {
        link.arg("-lm");
    }
    link.current_dir(project_dir);
    prepare_child(&mut link, compiler.parent(), &tmp_dir);
    run_tool(&mut link, "link executable", options.verbose)?;
    install_built_file(&temp_executable, &executable, "executable")?;
    for object in &generated_objects {
        let _ = fs::remove_file(object);
    }

    if options.verbose {
        eprintln!(
            "lsaca: build: direct compiler backend, {} runtime object(s) reused, {} compiled, {} generated translation unit(s), {:.3}s total",
            reused,
            missing.len(),
            generated_sources.len(),
            started.elapsed().as_secs_f64()
        );
    }

    if !executable.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "the C compiler reported success but no executable named {} was found in {}",
                options.program_name,
                build_dir.display()
            ),
        ));
    }
    Ok(BuiltExecutable {
        build_dir,
        executable,
    })
}

/// The generated translation units of a project: main.c followed by the extra
/// units of a split program, in deterministic lexical order.
fn generated_source_files(project_dir: &Path) -> Vec<String> {
    let mut sources = vec![String::from("main.c")];
    let Ok(entries) = fs::read_dir(project_dir) else {
        return sources;
    };
    let mut units: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("lsaca_unit_") && name.ends_with(".c") {
            units.push(name);
        }
    }
    units.sort();
    sources.extend(units);
    sources
}

/// Object file name for one generated C source (`main.c` -> `main.o`).
fn object_name_for_source(source: &str) -> String {
    let stem = source.strip_suffix(".c").unwrap_or(source);
    format!("{}.o", stem)
}

/// Compile every generated translation unit, using up to one thread per job.
fn compile_generated_objects(
    compiler: &Path,
    project_dir: &Path,
    tmp_dir: &Path,
    flags: &[String],
    options: &BuildOptions,
    sources: &[String],
    objects: &[PathBuf],
) -> io::Result<()> {
    if sources.is_empty() {
        return Ok(());
    }
    let threads = build_parallelism().min(sources.len()).max(1);
    let verbose = options.verbose;
    let mut results: Vec<io::Result<()>> = Vec::new();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for worker in 0..threads {
            handles.push(scope.spawn(move || -> io::Result<()> {
                for index in (worker..sources.len()).step_by(threads) {
                    compile_generated_object(
                        compiler,
                        project_dir,
                        tmp_dir,
                        flags,
                        &sources[index],
                        &objects[index],
                        verbose,
                    )?;
                }
                Ok(())
            }));
        }
        for handle in handles {
            results.push(
                handle
                    .join()
                    .unwrap_or_else(|_| Err(io::Error::other("a compilation thread panicked"))),
            );
        }
    });
    for result in results {
        result?;
    }
    Ok(())
}

/// Compile one generated translation unit into an object file.
fn compile_generated_object(
    compiler: &Path,
    project_dir: &Path,
    tmp_dir: &Path,
    flags: &[String],
    source: &str,
    object: &Path,
    verbose: bool,
) -> io::Result<()> {
    let mut command = Command::new(compiler);
    command
        .arg("-c")
        .arg(project_dir.join(source))
        .args(flags)
        .arg("-I")
        .arg(project_dir)
        .arg("-o")
        .arg(object);
    command.current_dir(project_dir);
    prepare_child(&mut command, compiler.parent(), tmp_dir);
    run_tool(&mut command, &format!("compile {}", source), verbose)
}

/// Wait for one compilation thread and turn a thread panic into an IO error.
fn join_compilation(handle: std::thread::ScopedJoinHandle<'_, io::Result<()>>) -> io::Result<()> {
    match handle.join() {
        Ok(result) => result,
        Err(_) => Err(io::Error::other("a compilation thread panicked")),
    }
}

/// Compile the runtime translation units that are not in the object cache yet,
/// using up to one thread per job for the missing files.
fn compile_runtime_objects(
    compiler: &Path,
    project_dir: &Path,
    tmp_dir: &Path,
    flags: &[String],
    options: &BuildOptions,
    missing: &[(String, PathBuf)],
) -> io::Result<()> {
    if missing.is_empty() {
        return Ok(());
    }
    let threads = build_parallelism().min(missing.len()).max(1);
    let verbose = options.verbose;
    let mut results: Vec<io::Result<()>> = Vec::new();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for worker in 0..threads {
            handles.push(scope.spawn(move || -> io::Result<()> {
                for index in (worker..missing.len()).step_by(threads) {
                    let (source, object) = &missing[index];
                    // Compile to a private temporary file and install it
                    // atomically: parallel test threads (and parallel
                    // compiler invocations) may miss the same object and
                    // race to build it.
                    let temp = temporary_path(tmp_dir, &runtime_object_name(source));
                    let mut command = Command::new(compiler);
                    command
                        .arg("-c")
                        .arg(project_dir.join(source))
                        .args(flags)
                        .arg("-I")
                        .arg(project_dir)
                        .arg("-o")
                        .arg(&temp);
                    command.current_dir(project_dir);
                    prepare_child(&mut command, compiler.parent(), tmp_dir);
                    run_tool(
                        &mut command,
                        &format!("compile runtime source {}", source),
                        verbose,
                    )?;
                    install_built_file(&temp, object, "runtime object")?;
                }
                Ok(())
            }));
        }
        for handle in handles {
            results.push(handle.join().unwrap_or_else(|_| {
                Err(io::Error::other("a runtime compilation thread panicked"))
            }));
        }
    });
    for result in results {
        result?;
    }
    Ok(())
}

/// Runtime translation units, in the order published by [runtime::sources].
fn runtime_c_sources() -> Vec<&'static str> {
    runtime::sources()
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| name.ends_with(".c"))
        .collect()
}

/// Object file name for one runtime source file.
fn runtime_object_name(source: &str) -> String {
    let stem = Path::new(source)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(source);
    format!("{}.o", stem)
}

/// Directory of the shared runtime object cache for one compiler and flag set.
fn runtime_object_dir(cache_root: &Path, compiler: &Path, flags: &[String]) -> PathBuf {
    let mut hash = FNV_OFFSET_BASIS;
    for (name, contents) in runtime::sources() {
        for bytes in [name.as_bytes(), contents.as_bytes()] {
            hash = fnv1a(hash, bytes);
            hash = fnv1a(hash, &[0xff]);
        }
    }
    hash = fnv1a(hash, compiler.as_os_str().to_string_lossy().as_bytes());
    hash = fnv1a(hash, &[0xff]);
    for flag in flags {
        hash = fnv1a(hash, flag.as_bytes());
        hash = fnv1a(hash, &[0xff]);
    }
    cache_root.join("_runtime").join(format!("{:016x}", hash))
}

/// Flags shared by every translation unit of a build.
fn compiler_flags(options: &BuildOptions) -> Vec<String> {
    let mut flags: Vec<String> = ["-std=c17", "-Wall", "-Wextra"]
        .iter()
        .map(|flag| (*flag).to_string())
        .collect();
    if options.release {
        flags.push(String::from("-O2"));
        flags.push(String::from("-DNDEBUG"));
    } else {
        flags.push(String::from("-g"));
        flags.push(String::from("-O0"));
        flags.push(String::from("-DLSACA_DEBUG"));
    }
    if debug_rc_requested() {
        flags.push(String::from("-DLSACA_DEBUG_RC"));
    }
    flags
}

fn is_non_empty_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

/// Executable file name for the current platform.
fn executable_name(program_name: &str) -> String {
    if cfg!(windows) {
        format!("{}.exe", program_name)
    } else {
        program_name.to_string()
    }
}

/// Number of concurrent compiler invocations to use.
fn build_parallelism() -> usize {
    jobs().parse::<usize>().unwrap_or(4).clamp(1, 64)
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
    let result = if options.use_cmake {
        build_project_with_cmake(&project_dir, options)
    } else {
        build_project_direct(&project_dir, cache_root, options)
    };
    result.map_err(|error| {
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
        compiler.as_os_str().to_string_lossy().as_bytes(),
        options.program_name.as_bytes(),
    ] {
        hash = fnv1a(hash, bytes);
        hash = fnv1a(hash, &[0xff]);
    }
    for unit in &generated.units {
        for bytes in [unit.file_name.as_bytes(), unit.contents.as_bytes()] {
            hash = fnv1a(hash, bytes);
            hash = fnv1a(hash, &[0xff]);
        }
    }
    for (name, contents) in runtime::sources() {
        for bytes in [name.as_bytes(), contents.as_bytes()] {
            hash = fnv1a(hash, bytes);
            hash = fnv1a(hash, &[0xff]);
        }
    }
    hash = fnv1a(
        hash,
        &[
            if options.release { 1 } else { 0 },
            if options.use_cmake { 1 } else { 0 },
            if debug_rc_requested() { 1 } else { 0 },
        ],
    );
    hash
}

/// Whether the environment asks for reference-count leak tracking in every
/// build configuration (`LSACA_DEBUG_RC`).
fn debug_rc_requested() -> bool {
    std::env::var("LSACA_DEBUG_RC")
        .map(|value| debug_rc_enabled(&value))
        .unwrap_or(false)
}

/// Parse the boolean spelling accepted by `LSACA_DEBUG_RC`.
fn debug_rc_enabled(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on" | "yes"
    )
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
    use crate::codegen::GeneratedUnit;

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

    fn sample_generated() -> Generated {
        Generated {
            main_c: String::from("int main(void) { return 0; }\n"),
            cmake: String::from("project(sample)\n"),
            units: Vec::new(),
        }
    }

    #[test]
    fn discovers_split_translation_units_in_order() {
        let dir = scratch_dir("units");
        for name in ["main.c", "lsaca_unit_2.c", "lsaca_unit_1.c", "notes.txt"] {
            fs::write(dir.join(name), b"/* generated */\n").expect("could not write a source file");
        }
        let sources = generated_source_files(&dir);
        assert_eq!(
            sources,
            vec![
                String::from("main.c"),
                String::from("lsaca_unit_1.c"),
                String::from("lsaca_unit_2.c"),
            ]
        );
        assert_eq!(object_name_for_source("main.c"), "main.o");
        assert_eq!(object_name_for_source("lsaca_unit_1.c"), "lsaca_unit_1.o");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_key_covers_split_translation_units() {
        let compiler = PathBuf::from("cc");
        let options = BuildOptions::new("sample");
        let single = sample_generated();
        let plain_key = cache_key(&single, &options, &compiler);
        let mut split = single.clone();
        split.units.push(GeneratedUnit {
            file_name: String::from("lsaca_unit_1.c"),
            contents: String::from("void lsaca_fn_extra(void) {}\n"),
        });
        let split_key = cache_key(&split, &options, &compiler);
        assert_ne!(plain_key, split_key);
        split.units[0].contents.push_str("/* changed */\n");
        assert_ne!(split_key, cache_key(&split, &options, &compiler));
    }

    #[test]
    fn write_project_writes_every_translation_unit() {
        let dir = scratch_dir("project");
        let mut generated = sample_generated();
        generated.units.push(GeneratedUnit {
            file_name: String::from("lsaca_unit_1.c"),
            contents: String::from("void lsaca_fn_extra(void) {}\n"),
        });
        write_project(&dir, &generated).expect("the project should be written");
        assert!(dir.join("main.c").is_file());
        assert!(dir.join("lsaca_unit_1.c").is_file());
        assert_eq!(
            fs::read_to_string(dir.join("lsaca_unit_1.c")).expect("unit should be readable"),
            "void lsaca_fn_extra(void) {}\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_the_debug_rc_switch() {
        for enabled in ["1", "true", "ON", " yes "] {
            assert!(
                debug_rc_enabled(enabled),
                "{enabled:?} should enable tracking"
            );
        }
        for disabled in ["", "0", "false", "off", "no", "2"] {
            assert!(
                !debug_rc_enabled(disabled),
                "{disabled:?} should disable tracking"
            );
        }
    }
}
