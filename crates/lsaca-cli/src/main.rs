//! The lsaca command line interface: compile, build and run source files.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use lsaca::codegen::CodegenOptions;
use lsaca::diag::Diagnostics;
use lsaca::driver::{self, BuildOptions, Frontend};
use lsaca::span::SourceFile;

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
        "version" | "--version" | "-V" => {
            println!("lsaca {}", VERSION);
            Ok(0)
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(0)
        }
        other if other.ends_with(".ls") => command_run(args),
        other => Err(Cli::Usage(format!("unknown command '{}'", other))),
    }
}

/// Parsed command line options shared by every command.
struct Options {
    file: PathBuf,
    release: bool,
    verbose: bool,
    out: Option<PathBuf>,
    keep: bool,
    program_args: Vec<String>,
}

fn parse_options(command: &str, args: &[String]) -> Result<Options, Cli> {
    let mut options = Options {
        file: PathBuf::new(),
        release: false,
        verbose: false,
        out: None,
        keep: false,
        program_args: Vec::new(),
    };
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--release" | "-r" => options.release = true,
            "--verbose" | "-v" => options.verbose = true,
            "--keep" => options.keep = true,
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
                if options.file.as_os_str().is_empty() {
                    options.file = PathBuf::from(other);
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
    if options.file.as_os_str().is_empty() {
        return Err(Cli::Usage(format!(
            "no input file given for 'lsaca {}'",
            command
        )));
    }
    Ok(options)
}

fn command_run(args: &[String]) -> CliResult {
    let options = parse_options("run", args)?;
    let frontend = read_source(&options.file)?;
    let program_name = program_name(&options.file);
    let generated = compile_to_c(&frontend, &program_name, &options)?;
    let cache_root = build_cache(&options)?;
    let build = BuildOptions::new(program_name.clone())
        .release(options.release)
        .verbose(options.verbose);
    let built = driver::build_with_cache(&generated, &build, &cache_root).map_err(|error| {
        Cli::Failure(format!(
            "failed to build {}: {}",
            options.file.display(),
            error
        ))
    })?;
    if options.verbose || options.keep {
        eprintln!("lsaca: build tree: {}", built.build_dir.display());
    }
    run_program(&built.executable, &options.program_args)
}

fn command_build(args: &[String]) -> CliResult {
    let options = parse_options("build", args)?;
    let frontend = read_source(&options.file)?;
    let program_name = program_name(&options.file);
    let generated = compile_to_c(&frontend, &program_name, &options)?;
    let cache_root = build_cache(&options)?;
    let build = BuildOptions::new(program_name.clone())
        .release(options.release)
        .verbose(options.verbose);
    let built = driver::build_with_cache(&generated, &build, &cache_root).map_err(|error| {
        Cli::Failure(format!(
            "failed to build {}: {}",
            options.file.display(),
            error
        ))
    })?;
    let out_dir = options.out.clone().unwrap_or_else(|| PathBuf::from("."));
    let destination = out_dir.join(executable_file_name(&program_name));
    driver::copy_executable(&built.executable, &destination)?;
    if options.verbose || options.keep {
        eprintln!("lsaca: build tree: {}", built.build_dir.display());
    }
    eprintln!("lsaca: built {}", destination.display());
    Ok(0)
}

/// Resolve the build cache, reporting the source file that triggered the build.
fn build_cache(options: &Options) -> Result<PathBuf, Cli> {
    let cache_root = driver::cache_root().map_err(|error| {
        Cli::Failure(format!(
            "failed to build {}: {}",
            options.file.display(),
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
    let frontend = read_source(&options.file)?;
    let program_name = program_name(&options.file);
    let generated = compile_to_c(&frontend, &program_name, &options)?;
    let default_dir = PathBuf::from(format!("{}_c", program_name));
    let out_dir = options.out.clone().unwrap_or(default_dir);
    driver::write_project(&out_dir, &generated)?;
    eprintln!(
        "lsaca: wrote C project for {} to {}",
        options.file.display(),
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
    let frontend = read_source(&options.file)?;
    match frontend.check_reporting() {
        Ok((_program, warnings)) => {
            print_diagnostics(frontend.file(), &warnings);
            eprintln!("lsaca: {}: ok", options.file.display());
            Ok(0)
        }
        Err(diagnostics) => {
            print_diagnostics(frontend.file(), &diagnostics);
            Err(Cli::Failure(format!(
                "{}: could not compile {} due to previous error(s)",
                options.file.display(),
                options.file.display()
            )))
        }
    }
}

/// Type check a source file and generate its C project.
fn compile_to_c(
    frontend: &Frontend,
    program_name: &str,
    options: &Options,
) -> Result<lsaca::codegen::Generated, Cli> {
    let program = match frontend.check_reporting() {
        Ok((program, warnings)) => {
            print_diagnostics(frontend.file(), &warnings);
            program
        }
        Err(diagnostics) => {
            print_diagnostics(frontend.file(), &diagnostics);
            return Err(Cli::Failure(format!(
                "{}: could not compile the program due to previous error(s)",
                options.file.display()
            )));
        }
    };
    let codegen_options = CodegenOptions {
        program_name: program_name.to_string(),
        debug_checks: !options.release,
        emit_location_info: true,
        verbose: options.verbose,
        source: Some(frontend.file().clone()),
    };
    Ok(driver::generate_c(&program, &codegen_options))
}

fn read_source(path: &Path) -> Result<Frontend, Cli> {
    let source = fs::read_to_string(path)
        .map_err(|error| Cli::Failure(format!("cannot read {}: {}", path.display(), error)))?;
    Ok(Frontend::new(display_name(path), source))
}

fn print_diagnostics(file: &SourceFile, diagnostics: &Diagnostics) {
    eprint!("{}", diagnostics.render(file));
}

fn display_name(path: &Path) -> String {
    path.to_string_lossy().into_owned()
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

fn program_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| String::from("program"));
    let mut name = String::new();
    for character in stem.chars() {
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
    lsaca <COMMAND> [OPTIONS]

COMMANDS:
    run <file.ls> [-- args...]   compile, build (cached) and run the program
    build <file.ls>              compile and build a native executable
    emit-c <file.ls>             write the generated C project only
    check <file.ls>              run the front end and report diagnostics
    version                      print the compiler version
    help                         print this help

OPTIONS:
    -r, --release                build with optimizations and wrapping arithmetic
    -o, --out-dir <DIR>          output directory for 'build' and 'emit-c'
    -v, --verbose                print the commands executed by the build
        --keep                   print the temporary build tree location
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
