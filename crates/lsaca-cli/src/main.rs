//! The lsaca command line interface: compile, build and run source files or
//! packages.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use lsaca::codegen::CodegenOptions;
use lsaca::diag::Diagnostics;
use lsaca::driver::{self, BuildOptions, Frontend, LoadError};
use lsaca::package;
use lsaca::span::{SourceFile, SourceMap};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(code) => ExitCode::from(code),
        Err(Cli::Help) => {
            print_help();
            ExitCode::from(0)
        }
        Err(Cli::Usage(message)) => {
            eprintln!("lsaca: {}", message);
            eprintln!("lsaca: run 'lsaca help' for usage");
            ExitCode::from(2)
        }
        Err(Cli::Failure(message)) => {
            eprintln!("lsaca: {}", message);
            ExitCode::from(1)
        }
    }
}

/// Errors that end a command early.
enum Cli {
    /// Print the usage text and exit successfully (--help).
    Help,
    /// Bad usage: exit code 2.
    Usage(String),
    /// Compile or build failure: exit code 1.
    Failure(String),
}

impl From<io::Error> for Cli {
    fn from(error: io::Error) -> Cli {
        Cli::Failure(error.to_string())
    }
}

type CliResult = Result<u8, Cli>;

fn dispatch(args: &[String]) -> CliResult {
    let Some(first) = args.first() else {
        print_help();
        return Ok(2);
    };
    match first.as_str() {
        "run" => command_run(&args[1..]),
        "build" => command_build(&args[1..]),
        "emit-c" => command_emit_c(&args[1..]),
        "check" => command_check(&args[1..]),
        "fmt" => command_fmt(&args[1..]),
        "new" => command_new(&args[1..]),
        "version" | "--version" | "-V" => {
            reject_extra_args("version", &args[1..])?;
            println!("lsaca {}", VERSION);
            Ok(0)
        }
        "help" | "--help" | "-h" => {
            reject_extra_args("help", &args[1..])?;
            print_help();
            Ok(0)
        }
        other if other.ends_with(".ls") => command_run(args),
        other => Err(Cli::Usage(format!("unknown command '{}'", other))),
    }
}

/// Reject arguments for a command that takes none (version and help).
fn reject_extra_args(command: &str, args: &[String]) -> Result<(), Cli> {
    if let Some(arg) = args.first() {
        return Err(Cli::Usage(format!(
            "unexpected argument '{}' for 'lsaca {}'",
            arg, command
        )));
    }
    Ok(())
}

/// Parsed command line options shared by every command.
///
/// The input file is optional: when it is missing, run/build/emit-c/check
/// operate on the package whose lsaca.toml is the nearest one walking up from
/// the current directory.
struct Options {
    file: Option<PathBuf>,
    release: bool,
    verbose: bool,
    out: Option<PathBuf>,
    keep: bool,
    cmake: bool,
    program_args: Vec<String>,
}

fn parse_options(command: &str, args: &[String]) -> Result<Options, Cli> {
    let mut options = Options {
        file: None,
        release: false,
        verbose: false,
        out: None,
        keep: false,
        cmake: false,
        program_args: Vec::new(),
    };
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--release" | "-r" => options.release = true,
            "--verbose" | "-v" => options.verbose = true,
            "--keep" => options.keep = true,
            "--cmake" => {
                if command != "run" && command != "build" {
                    return Err(Cli::Usage(format!(
                        "unknown option '--cmake' for 'lsaca {}'",
                        command
                    )));
                }
                options.cmake = true;
            }
            "--help" | "-h" => return Err(Cli::Help),
            "-o" | "--out-dir" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(Cli::Usage(String::from("missing value after -o")));
                };
                options.out = Some(PathBuf::from(value));
            }
            "--" => {
                if command != "run" {
                    return Err(Cli::Usage(String::from(
                        "'--' program arguments are only supported by 'lsaca run'",
                    )));
                }
                options
                    .program_args
                    .extend(args[index + 1..].iter().cloned());
                break;
            }
            other if other.starts_with('-') && other != "-" => {
                return Err(Cli::Usage(format!(
                    "unknown option '{}' for 'lsaca {}'",
                    other, command
                )));
            }
            other => {
                if options.file.is_none() {
                    options.file = Some(PathBuf::from(other));
                } else {
                    return Err(Cli::Usage(format!(
                        "unexpected argument '{}': only one source file may be given (did you mean '-- {}'?)",
                        other, other
                    )));
                }
            }
        }
        index += 1;
    }
    Ok(options)
}

fn command_run(args: &[String]) -> CliResult {
    let options = parse_options("run", args)?;
    let frontend = load_input(&options)?;
    require_binary(&options, &frontend)?;
    let program_name = program_name_for(&options, &frontend);
    let generated = compile_to_c(&frontend, &program_name, &options)?;
    let cache_root = build_cache(&options, &frontend)?;
    let build = BuildOptions::new(program_name.clone())
        .release(options.release)
        .verbose(options.verbose)
        .use_cmake(options.cmake);
    let description = input_description(&options, &frontend);
    let built = driver::build_with_cache(&generated, &build, &cache_root)
        .map_err(|error| Cli::Failure(format!("failed to build {}: {}", description, error)))?;
    if options.verbose || options.keep {
        eprintln!("lsaca: build tree: {}", built.build_dir.display());
    }
    run_program(&built.executable, &options.program_args)
}

fn command_build(args: &[String]) -> CliResult {
    let options = parse_options("build", args)?;
    let frontend = load_input(&options)?;
    require_binary(&options, &frontend)?;
    let program_name = program_name_for(&options, &frontend);
    let generated = compile_to_c(&frontend, &program_name, &options)?;
    let cache_root = build_cache(&options, &frontend)?;
    let build = BuildOptions::new(program_name.clone())
        .release(options.release)
        .verbose(options.verbose)
        .use_cmake(options.cmake);
    let description = input_description(&options, &frontend);
    let built = driver::build_with_cache(&generated, &build, &cache_root)
        .map_err(|error| Cli::Failure(format!("failed to build {}: {}", description, error)))?;
    let out_dir = options.out.clone().unwrap_or_else(|| PathBuf::from("."));
    let destination = out_dir.join(executable_file_name(&program_name));
    driver::copy_executable(&built.executable, &destination)?;
    if options.verbose || options.keep {
        eprintln!("lsaca: build tree: {}", built.build_dir.display());
    }
    eprintln!("lsaca: built {}", destination.display());
    Ok(0)
}

/// Resolve the build cache, reporting the input that triggered the build.
fn build_cache(options: &Options, frontend: &Frontend) -> Result<PathBuf, Cli> {
    let cache_root = driver::cache_root().map_err(|error| {
        Cli::Failure(format!(
            "failed to build {}: {}",
            input_description(options, frontend),
            error
        ))
    })?;
    if options.verbose {
        eprintln!("lsaca: build cache: {}", cache_root.display());
    }
    Ok(cache_root)
}

fn command_emit_c(args: &[String]) -> CliResult {
    let options = parse_options("emit-c", args)?;
    let frontend = load_input(&options)?;
    // The generated C project is an executable: a library package has no
    // entry point, so only 'check' applies to it.
    require_binary(&options, &frontend)?;
    let program_name = program_name_for(&options, &frontend);
    let generated = compile_to_c(&frontend, &program_name, &options)?;
    let default_dir = PathBuf::from(format!("{}_c", program_name));
    let out_dir = options.out.clone().unwrap_or(default_dir);
    driver::write_project(&out_dir, &generated)?;
    let description = input_description(&options, &frontend);
    eprintln!(
        "lsaca: wrote C project for {} to {}",
        description,
        out_dir.display()
    );
    eprintln!(
        "lsaca: build it with: cmake -S {} -B {}/build && cmake --build {}/build",
        out_dir.display(),
        out_dir.display(),
        out_dir.display()
    );
    Ok(0)
}

fn command_check(args: &[String]) -> CliResult {
    let options = parse_options("check", args)?;
    let frontend = load_input(&options)?;
    let description = input_description(&options, &frontend);
    match frontend.check_reporting() {
        Ok((_program, warnings)) => {
            print_diagnostics(frontend.sources(), &warnings);
            eprintln!("lsaca: {}: ok", description);
            Ok(0)
        }
        Err(diagnostics) => {
            print_diagnostics(frontend.sources(), &diagnostics);
            Err(Cli::Failure(format!(
                "{}: could not compile {} due to previous error(s)",
                description, description
            )))
        }
    }
}

/// Parsed command line options for 'lsaca fmt'.
struct FmtOptions {
    /// File or directory to format; the current directory when absent.
    target: PathBuf,
    check: bool,
    stdout: bool,
}

fn parse_fmt_options(args: &[String]) -> Result<FmtOptions, Cli> {
    let mut target: Option<PathBuf> = None;
    let mut check = false;
    let mut stdout = false;
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => return Err(Cli::Help),
            "--check" => check = true,
            "--stdout" => stdout = true,
            other if other.starts_with('-') && other != "-" => {
                return Err(Cli::Usage(format!(
                    "unknown option '{}' for 'lsaca fmt'",
                    other
                )));
            }
            other => {
                if target.is_some() {
                    return Err(Cli::Usage(format!(
                        "unexpected argument '{}': 'lsaca fmt' takes one file or directory",
                        other
                    )));
                }
                target = Some(PathBuf::from(other));
            }
        }
    }
    if check && stdout {
        return Err(Cli::Usage(
            "'lsaca fmt' accepts either --check or --stdout, not both".to_string(),
        ));
    }
    Ok(FmtOptions {
        target: target.unwrap_or_else(|| PathBuf::from(".")),
        check,
        stdout,
    })
}

/// Format one source file, or every module of the package containing the target.
///
/// A file is only rewritten when it parses cleanly: a syntax error leaves the
/// original bytes untouched and reports the usual diagnostics. With --check the
/// command reports which files would change and exits with 1 in that case;
/// with --stdout the formatted text goes to standard output instead of the
/// file (single-file mode only).
///
/// When the target is a directory, the nearest lsaca.toml is discovered and
/// every .ls file under that package's src directory is formatted. All modules
/// are validated before anything is written, so a broken module never leaves a
/// package half formatted.
fn command_fmt(args: &[String]) -> CliResult {
    let options = parse_fmt_options(args)?;
    let is_directory = fs::metadata(&options.target)
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false);
    if is_directory {
        if options.stdout {
            return Err(Cli::Usage(
                "'--stdout' formats a single file; a directory formats a whole package".to_string(),
            ));
        }
        return format_package(&options);
    }
    format_single_file(&options)
}

/// Format a single file given on the command line.
fn format_single_file(options: &FmtOptions) -> CliResult {
    let path = &options.target;
    let source = fs::read_to_string(path)
        .map_err(|error| Cli::Failure(format!("failed to read {}: {}", path.display(), error)))?;
    validate_source(path, &source)?;
    let formatted = lsaca::fmt::format_source(&source);
    if options.stdout {
        print!("{}", formatted);
        return Ok(0);
    }
    if formatted == source {
        return Ok(0);
    }
    if options.check {
        eprintln!("lsaca: would format {}", path.display());
        return Ok(1);
    }
    fs::write(path, formatted.as_bytes())
        .map_err(|error| Cli::Failure(format!("failed to write {}: {}", path.display(), error)))?;
    Ok(0)
}

/// Format every module of the package containing the fmt target directory.
fn format_package(options: &FmtOptions) -> CliResult {
    let Some(manifest) = package::discover_manifest(&options.target)? else {
        return Err(Cli::Usage(format!(
            "'lsaca fmt' needs a file or a directory inside a package: no lsaca.toml found in {} or its ancestors",
            options.target.display()
        )));
    };
    let package_dir = manifest
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let source_root = package_dir.join("src");
    let mut files = Vec::new();
    collect_source_files(&source_root, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err(Cli::Failure(format!(
            "no .ls source files found under {}",
            display_relative(&source_root)
        )));
    }
    // Validate every module before touching any file, so a syntax error never
    // leaves the package partially formatted.
    let mut invalid = false;
    for path in &files {
        let source = fs::read_to_string(path).map_err(|error| {
            Cli::Failure(format!("failed to read {}: {}", path.display(), error))
        })?;
        if validate_source(path, &source).is_err() {
            invalid = true;
        }
    }
    if invalid {
        return Err(Cli::Failure(format!(
            "{}: could not be formatted due to previous error(s)",
            display_relative(&package_dir)
        )));
    }
    let mut changed: Vec<(PathBuf, String)> = Vec::new();
    for path in &files {
        let source = fs::read_to_string(path).map_err(|error| {
            Cli::Failure(format!("failed to read {}: {}", path.display(), error))
        })?;
        let formatted = lsaca::fmt::format_source(&source);
        if formatted != source {
            changed.push((path.clone(), formatted));
        }
    }
    if options.check {
        for (path, _) in &changed {
            eprintln!("lsaca: would format {}", display_relative(path));
        }
        return Ok(if changed.is_empty() { 0 } else { 1 });
    }
    for (path, text) in &changed {
        fs::write(path, text.as_bytes()).map_err(|error| {
            Cli::Failure(format!("failed to write {}: {}", path.display(), error))
        })?;
    }
    let noun = if changed.len() == 1 { "file" } else { "files" };
    eprintln!(
        "lsaca: formatted {} {} in {}",
        changed.len(),
        noun,
        display_relative(&package_dir)
    );
    Ok(0)
}

/// Check that a source file lexes and parses before it is reformatted.
fn validate_source(path: &Path, source: &str) -> Result<(), Cli> {
    let file = SourceFile::new(path.display().to_string(), source.to_string());
    let (tokens, mut diagnostics) = lsaca::lexer::tokenize_file(&file);
    let (_, parse_diagnostics) = lsaca::parser::parse(&tokens, &file);
    diagnostics.extend(parse_diagnostics);
    if diagnostics.has_errors() {
        eprint!("{}", diagnostics.render(&file));
        return Err(Cli::Failure(format!(
            "{}: could not be formatted due to previous error(s)",
            path.display()
        )));
    }
    Ok(())
}

/// Collect .ls files below a directory in a deterministic order.
fn collect_source_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), Cli> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(Cli::from(error)),
    };
    let mut paths = Vec::new();
    for entry in entries {
        paths.push(entry.map_err(Cli::from)?.path());
    }
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_source_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "ls") {
            files.push(path);
        }
    }
    Ok(())
}

/// Path shown in fmt messages: relative to the current directory when
/// possible and always with forward slashes.
fn display_relative(path: &Path) -> String {
    let relative = env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(&cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf());
    relative.to_string_lossy().replace('\\', "/")
}
/// Create a new binary package: lsaca.toml plus src/main.ls.
fn command_new(args: &[String]) -> CliResult {
    let mut destination: Option<PathBuf> = None;
    for arg in args {
        match arg.as_str() {
            "--help" | "-h" => return Err(Cli::Help),
            other if other.starts_with('-') && other != "-" => {
                return Err(Cli::Usage(format!(
                    "unknown option '{}' for 'lsaca new'",
                    other
                )));
            }
            other => {
                if destination.is_some() {
                    return Err(Cli::Usage(format!(
                        "unexpected argument '{}': 'lsaca new' takes one directory",
                        other
                    )));
                }
                destination = Some(PathBuf::from(other));
            }
        }
    }
    let Some(destination) = destination else {
        return Err(Cli::Usage(String::from(
            "no directory given for 'lsaca new'",
        )));
    };
    create_package(&destination)?;
    Ok(0)
}

fn create_package(destination: &Path) -> Result<(), Cli> {
    if destination.exists() {
        if !destination.is_dir() {
            return Err(Cli::Failure(format!(
                "cannot create a package at {}: the path exists and is not a directory",
                destination.display()
            )));
        }
        let mut entries = fs::read_dir(destination).map_err(Cli::from)?;
        if entries.next().is_some() {
            return Err(Cli::Failure(format!(
                "cannot create a package at {}: the directory already exists and is not empty",
                destination.display()
            )));
        }
    }
    let raw_name = destination
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| {
            Cli::Failure(format!(
                "cannot derive a package name from {}",
                destination.display()
            ))
        })?;
    let name = package_identifier(&raw_name)?;
    let source_dir = destination.join("src");
    fs::create_dir_all(&source_dir).map_err(|error| {
        Cli::Failure(format!(
            "cannot create {}: {}",
            destination.display(),
            error
        ))
    })?;
    let manifest = format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\n", name);
    let main_source = "fn main() {\n    println!(\"Hello, world!\");\n}\n";
    fs::write(destination.join("lsaca.toml"), manifest)
        .map_err(|error| Cli::Failure(format!("cannot write lsaca.toml: {}", error)))?;
    fs::write(source_dir.join("main.ls"), main_source)
        .map_err(|error| Cli::Failure(format!("cannot write src/main.ls: {}", error)))?;
    eprintln!(
        "lsaca: created binary package '{}' in {}",
        name,
        destination.display()
    );
    eprintln!("lsaca: next: cd {} && lsaca run", destination.display());
    Ok(())
}

/// Turn a directory name into a valid package name.
fn package_identifier(raw: &str) -> Result<String, Cli> {
    let mut name = String::with_capacity(raw.len());
    for (index, character) in raw.trim().chars().enumerate() {
        if character.is_ascii_alphanumeric() || character == '_' {
            if index == 0 && character.is_ascii_digit() {
                name.push('_');
            }
            name.push(character.to_ascii_lowercase());
        } else {
            name.push('_');
        }
    }
    if name.is_empty() {
        return Err(Cli::Failure(format!(
            "cannot derive a package name from '{}'",
            raw
        )));
    }
    if package_identifier_reserved(&name) {
        name.push('_');
    }
    Ok(name)
}

fn package_identifier_reserved(name: &str) -> bool {
    matches!(
        name,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "return"
            | "self"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "use"
            | "while"
    )
}

/// Type check the program and generate its C project.
fn compile_to_c(
    frontend: &Frontend,
    program_name: &str,
    options: &Options,
) -> Result<lsaca::codegen::Generated, Cli> {
    let description = input_description(options, frontend);
    let program = match frontend.check_reporting() {
        Ok((program, warnings)) => {
            print_diagnostics(frontend.sources(), &warnings);
            program
        }
        Err(diagnostics) => {
            print_diagnostics(frontend.sources(), &diagnostics);
            return Err(Cli::Failure(format!(
                "{}: could not compile the program due to previous error(s)",
                description
            )));
        }
    };
    let codegen_options = CodegenOptions {
        program_name: program_name.to_string(),
        debug_checks: !options.release,
        emit_location_info: true,
        verbose: options.verbose,
        sources: Some(frontend.sources().clone()),
    };
    Ok(driver::generate_c(&program, &codegen_options))
}

/// Load the program from the file given on the command line, or from the
/// package discovered in the current directory (or one of its parents).
fn load_input(options: &Options) -> Result<Frontend, Cli> {
    match &options.file {
        Some(path) => Frontend::load(path)
            .map_err(|error| Cli::Failure(format!("cannot read {}: {}", path.display(), error))),
        None => {
            let current_dir = env::current_dir()?;
            Frontend::load_package(&current_dir).map_err(|error| match &error {
                LoadError::Io(io_error) if io_error.kind() == io::ErrorKind::NotFound => {
                    Cli::Usage(io_error.to_string())
                }
                other => Cli::Failure(other.render().trim_end().to_string()),
            })
        }
    }
}

fn input_description(options: &Options, frontend: &Frontend) -> String {
    match &options.file {
        Some(path) => path.display().to_string(),
        None => format!("package '{}'", frontend.graph().root_crate().name),
    }
}

/// run/build/emit-c need an entry point: either a single file or a binary
/// package that defines fn main(). Libraries are only type checked.
fn require_binary(options: &Options, frontend: &Frontend) -> Result<(), Cli> {
    if options.file.is_some() || frontend.has_main() {
        return Ok(());
    }
    let name = &frontend.graph().root_crate().name;
    if frontend.has_binary_root() {
        return Err(Cli::Failure(format!(
            "package '{}' has a src/main.ls but no fn main(): add fn main() or move the crate root to src/lib.ls to make it a library",
            name
        )));
    }
    Err(Cli::Failure(format!(
        "package '{}' is a library package: 'lsaca run', 'lsaca build' and 'lsaca emit-c' need a binary package with src/main.ls (only 'lsaca check' works on a library)",
        name
    )))
}

fn print_diagnostics(sources: &SourceMap, diagnostics: &Diagnostics) {
    eprint!("{}", diagnostics.render(sources));
}

fn run_program(executable: &Path, args: &[String]) -> CliResult {
    let status = Command::new(executable)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| {
            Cli::Failure(format!("failed to run {}: {}", executable.display(), error))
        })?;
    Ok(status.code().unwrap_or(1) as u8)
}

fn program_name_for(options: &Options, frontend: &Frontend) -> String {
    match &options.file {
        Some(path) => program_name(path),
        None => sanitize_program_name(&frontend.graph().root_crate().name),
    }
}

fn program_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| String::from("program"));
    sanitize_program_name(&stem)
}

fn sanitize_program_name(raw: &str) -> String {
    let mut name = String::new();
    for character in raw.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            name.push(character);
        } else {
            name.push('_');
        }
    }
    let starts_with_digit = name
        .chars()
        .next()
        .map(|character| character.is_ascii_digit())
        .unwrap_or(true);
    if name.is_empty() || starts_with_digit {
        name = format!("program_{}", name);
    }
    name
}

fn executable_file_name(program_name: &str) -> String {
    if cfg!(windows) {
        format!("{}.exe", program_name)
    } else {
        program_name.to_string()
    }
}

fn print_help() {
    println!(
        r#"lsaca {} - a small statically typed language with an ORC memory model

USAGE:
    lsaca <COMMAND> [OPTIONS] [FILE.ls]

COMMANDS:
    run [FILE.ls] [-- args...]   compile, build (cached) and run the program
    build [FILE.ls]              compile and build a native executable
    emit-c [FILE.ls]             write the generated C project only
    check [FILE.ls]              run the front end and report diagnostics
    fmt [FILE.ls|DIR] [--check]  format one file or every module of a package
    new <DIR>                    create a new binary package
    version                      print the compiler version
    help                         print this help

When no FILE.ls is given, run/build/emit-c/check operate on the package whose
lsaca.toml is the nearest one walking up from the current directory. Package
dependencies declared in [dependencies] with a path are compiled together.

'lsaca fmt' formats a single file, or - when given a directory (or no path at
all inside a package) - every module file under that package's src directory.

OPTIONS:
    -r, --release                build with optimizations and wrapping arithmetic
    -o, --out-dir <DIR>          output directory for 'build' and 'emit-c'
    -v, --verbose                print the commands executed by the build
        --keep                   print the temporary build tree location
        --cmake                  build via the generated CMake project instead
                                 of the built-in direct C compiler back end
        --stdout                 (fmt) print the formatted file instead of writing it
    -h, --help                   print this help

EXIT CODES:
    0   success
    1   compile or build error
    2   command line usage error
    101 a panic inside the compiled program (propagated from the child)
"#,
        VERSION
    );
}
