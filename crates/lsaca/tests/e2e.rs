//! End-to-end tests: every case is a complete program that is compiled to C,
//! built with CMake and a native C compiler, then executed and compared against
//! the expected output and exit code.
//!
//! The tests are skipped when no C compiler is available, so cargo test works on
//! machines without a C toolchain (the compiler unit tests still run).

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use lsaca::codegen::CodegenOptions;
use lsaca::driver::{self, BuildOptions, Frontend};

struct Case {
    name: &'static str,
    source: &'static str,
    args: &'static [&'static str],
    stdout: &'static str,
    exit: i32,
    stderr_contains: &'static [&'static str],
}

/// Root directory for cached build trees created by the end-to-end tests.
///
/// The tests honour `LSACA_CACHE_DIR` like the CLI does. When it is not set they
/// use a directory inside the workspace target directory instead of the system
/// temporary directory, which keeps the tests runnable in sandboxed build
/// environments where temporary directories are not writable by child processes.
fn test_cache_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("LSACA_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/e2e-build-cache")
}

/// Compile a source program to a native executable. Returns the executable path
/// or a human readable explanation of the first failing stage.
fn build_program(name: &str, source: &str, release: bool) -> Result<PathBuf, String> {
    let frontend = Frontend::new(format!("{}.ls", name), source);
    let program = match frontend.check() {
        Ok(program) => program,
        Err(diagnostics) => {
            return Err(format!(
                "the front end rejected {}:\n{}",
                name,
                diagnostics.render(frontend.file())
            ));
        }
    };
    let mut program_name = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            program_name.push(character);
        } else {
            program_name.push('_');
        }
    }
    let options = CodegenOptions {
        program_name: program_name.clone(),
        debug_checks: !release,
        emit_location_info: true,
        source: Some(frontend.file().clone()),
        ..CodegenOptions::default()
    };
    let generated = driver::generate_c(&program, &options);
    let build = BuildOptions::new(program_name).release(release);
    match driver::build_with_cache(&generated, &build, &test_cache_root()) {
        Ok(built) => Ok(built.executable),
        Err(error) => Err(format!("building {} failed:\n{}", name, error)),
    }
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn run_case(case: &Case) -> Result<(), String> {
    run_case_with_profile(case, false)
}

fn run_case_with_profile(case: &Case, release: bool) -> Result<(), String> {
    let executable = build_program(case.name, case.source, release)?;
    let output = match Command::new(&executable).args(case.args).output() {
        Ok(output) => output,
        Err(error) => return Err(format!("could not run {}: {}", case.name, error)),
    };
    let stdout = normalize(&String::from_utf8_lossy(&output.stdout));
    let stderr = normalize(&String::from_utf8_lossy(&output.stderr));
    let exit = output.status.code().unwrap_or(-1);
    let mut problems: Vec<String> = Vec::new();
    if stdout != case.stdout {
        problems.push(format!(
            "stdout mismatch\n--- expected ---\n{:?}\n--- actual ---\n{:?}",
            case.stdout, stdout
        ));
    }
    if exit != case.exit {
        problems.push(format!(
            "exit code mismatch: expected {}, got {}",
            case.exit, exit
        ));
    }
    for needle in case.stderr_contains {
        if !stderr.contains(needle) {
            problems.push(format!(
                "stderr does not contain {:?}\n--- stderr ---\n{}",
                needle, stderr
            ));
        }
    }
    // Debug builds track reference-counted allocations and report leaks at
    // shutdown. A well formed program must not leak, so any leak report points
    // at a compiler or runtime bug in the ORC implementation.
    if !release && stderr.contains("reference-counted allocation") {
        problems.push(format!(
            "the generated program leaked reference counted allocations\n--- stderr ---\n{}",
            stderr
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!("{}: {}", case.name, problems.join("\n")))
    }
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "hello",
            source: include_str!("programs/hello.ls"),
            args: &[],
            stdout: "hello world!\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "arithmetic",
            source: include_str!("programs/arithmetic.ls"),
            args: &[],
            stdout: "10 4 21 2\n1\n15\n14\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "vars",
            source: include_str!("programs/vars.ls"),
            args: &[],
            stdout: "14\n15\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "strings",
            source: include_str!("programs/strings.ls"),
            args: &[],
            stdout: "hello, world!\n13\nfoobar foo\ntrue\nFOOBAR\n\"foobar\"\nline1\nline2\ttab\\done\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "if_else",
            source: include_str!("programs/if_else.ls"),
            args: &[],
            stdout: "negative zero positive\n10\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "loops",
            source: include_str!("programs/loops.ls"),
            args: &[],
            stdout: "55\n3\n5\n24\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "recursion",
            source: include_str!("programs/recursion.ls"),
            args: &[],
            stdout: "120\n55\ntrue true\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "closures",
            source: include_str!("programs/closures.ls"),
            args: &[],
            stdout: "5 6\n15\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "structs",
            source: include_str!("programs/structs.ls"),
            args: &[],
            stdout: "5\n20 30 50\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "enums",
            source: include_str!("programs/enums.ls"),
            args: &[],
            stdout: "12.56\n12\n0\nsome 7 none\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "option_result",
            source: include_str!("programs/option_result.ls"),
            args: &[],
            stdout: "20\n-1\n42\ntrue\n7\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "vec",
            source: include_str!("programs/vec.ls"),
            args: &[],
            stdout: "4\n9\n2\n1\n3\n9\n3\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "tuples",
            source: include_str!("programs/tuples.ls"),
            args: &[],
            stdout: "1 9\n1 9\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "casts",
            source: include_str!("programs/casts.ls"),
            args: &[],
            stdout: "44\n3\n1\n65\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "const_alias",
            source: include_str!("programs/const_alias.ls"),
            args: &[],
            stdout: "200\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "args",
            source: include_str!("programs/args.ls"),
            args: &["alpha", "beta"],
            stdout: "2 alpha\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "parse_math",
            source: include_str!("programs/parse_math.ls"),
            args: &[],
            stdout: "42\n4\n5\n3\n3.9\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "eprintln",
            source: include_str!("programs/eprintln.ls"),
            args: &[],
            stdout: "out\n",
            exit: 0,
            stderr_contains: &["warn: 1"],
        },
        Case {
            name: "if_let",
            source: include_str!("programs/if_let.ls"),
            args: &[],
            stdout: "got 5\n6\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "example_hello",
            source: include_str!("../../../examples/hello.ls"),
            args: &[],
            stdout: "hello, world!\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "example_fib",
            source: include_str!("../../../examples/fib.ls"),
            args: &[],
            stdout: "0 1 1 2 3 5 8 13 21 34 \n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "example_structs",
            source: include_str!("../../../examples/structs.ls"),
            args: &[],
            stdout: "manhattan distance: 7\nafter scaling: 6 -8\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "example_enums",
            source: include_str!("../../../examples/enums.ls"),
            args: &[],
            stdout: "area = 3.14\narea = 6\narea = 0\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "example_vec",
            source: include_str!("../../../examples/vec.ls"),
            args: &[],
            stdout: "sum of squares: 285\nfirst = 10\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "example_closures",
            source: include_str!("../../../examples/closures.ls"),
            args: &[],
            stdout: "add_base(5) = 15\napply_twice(*2, 3) = 12\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "example_word_count",
            source: include_str!("../../../examples/word_count.ls"),
            args: &[],
            stdout: "0 lines, 0 words\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "rc_basic",
            source: include_str!("programs/rc_basic.ls"),
            args: &[],
            stdout: "10 10\nstrong_count = 2\n99 99\n7\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "rc_cycle",
            source: include_str!("programs/rc_cycle.ls"),
            args: &[],
            stdout: "1\n2\ndone\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "rc_payload",
            source: include_str!("programs/rc_payload.ls"),
            args: &[],
            stdout: "default [1, 2, 3]\n3\nhello\nworld\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "rc_identity",
            source: include_str!("programs/rc_identity.ls"),
            args: &[],
            stdout: "true\nfalse\ntrue\ntrue\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "box_basic",
            source: include_str!("programs/box_basic.ls"),
            args: &[],
            stdout: "10\n20 10\ntrue\n2\n7\nhi [\"hi\"]\nbye\n2\nsecond\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "box_recursive",
            source: include_str!("programs/box_recursive.ls"),
            args: &[],
            stdout: "11\n5\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "format_specs",
            source: include_str!("programs/format_specs.ls"),
            args: &[],
            stdout: "[lsaca]\n[   lsaca]\n[lsaca   ]\n[ lsaca  ]\n[***lsaca]\n[00003.14]\n[+42] [-7] [+0]\n[     3.5] [       2]\n[2.67] [2]\n[0.333]\n[你好]\n[   3.000]\n[\"lsaca\"]\n[  \"hi\"]\n[0005]\n[    1234.568]\n[         7          ]\n[**true***]\n[   42]\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "or_patterns",
            source: include_str!("programs/or_patterns.ls"),
            args: &[],
            stdout: "small\nmedium\nlarge\na\nq\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "or_pattern_bindings",
            source: include_str!("programs/or_pattern_bindings.ls"),
            args: &[],
            stdout: "pair\nsingle\nmid 9\nb 4\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "match_guards",
            source: include_str!("programs/match_guards.ls"),
            args: &[],
            stdout: "1\n2\n3\n4\nsmall\nzero\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "vec_capacity",
            source: include_str!("programs/vec_capacity.ls"),
            args: &[],
            stdout: "0 true\n[1, 2, 3]\n[\"hi\", \"there\"]\n1\n1 4\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "panic_capacity",
            source: include_str!("programs/vec_capacity_negative.ls"),
            args: &[],
            stdout: "",
            exit: 101,
            stderr_contains: &["panicked at", "capacity overflow"],
        },
        Case {
            name: "panic_div",
            source: include_str!("programs/panic_div.ls"),
            args: &[],
            stdout: "",
            exit: 101,
            stderr_contains: &["thread 'main' panicked at", "attempt to divide by zero"],
        },
        Case {
            name: "panic_bounds",
            source: include_str!("programs/panic_bounds.ls"),
            args: &[],
            stdout: "",
            exit: 101,
            stderr_contains: &[
                "panicked at",
                "index out of bounds: the len is 3 but the index is 5",
            ],
        },
        Case {
            name: "panic_unwrap",
            source: include_str!("programs/panic_unwrap.ls"),
            args: &[],
            stdout: "",
            exit: 101,
            stderr_contains: &[
                "panicked at",
                "called \x60Option::unwrap()\x60 on a \x60None\x60 value",
            ],
        },
        Case {
            name: "overflow",
            source: include_str!("programs/overflow.ls"),
            args: &[],
            stdout: "",
            exit: 101,
            stderr_contains: &["panicked at", "attempt to add with overflow"],
        },
    ]
}

#[test]
fn compiled_programs() {
    if let Err(error) = driver::find_c_compiler() {
        eprintln!("skipping end-to-end tests: {}", error);
        return;
    }
    let cases = cases();
    let workers = 4.min(cases.len());
    let next = AtomicUsize::new(0);
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    if index >= cases.len() {
                        break;
                    }
                    if let Err(message) = run_case(&cases[index]) {
                        failures.lock().unwrap().push(message);
                    }
                }
            });
        }
    });
    let mut failures = failures.into_inner().unwrap();
    if !failures.is_empty() {
        failures.sort();
        panic!(
            "{} of {} end-to-end programs failed:\n\n{}",
            failures.len(),
            cases.len(),
            failures.join("\n\n")
        );
    }
}

#[test]
fn release_builds_wrap_overflow() {
    if let Err(error) = driver::find_c_compiler() {
        eprintln!("skipping release build test: {}", error);
        return;
    }
    let case = Case {
        name: "overflow_release",
        source: include_str!("programs/overflow.ls"),
        args: &[],
        stdout: "-9223372036854775808\n",
        exit: 0,
        stderr_contains: &[],
    };
    if let Err(message) = run_case_with_profile(&case, true) {
        panic!("{}", message);
    }
}

#[test]
fn frontend_rejects_invalid_programs() {
    let programs: &[(&str, &str)] = &[
        ("bad_type", include_str!("programs/bad_type.ls")),
        ("bad_name", include_str!("programs/bad_name.ls")),
        ("bad_syntax", include_str!("programs/bad_syntax.ls")),
        ("bad_match", include_str!("programs/bad_match.ls")),
        ("bad_mut", include_str!("programs/bad_mut.ls")),
        ("bad_args", include_str!("programs/bad_args.ls")),
        ("bad_condition", include_str!("programs/bad_condition.ls")),
        ("missing_main", "fn helper() {}\n"),
    ];
    let mut problems: Vec<String> = Vec::new();
    for (name, source) in programs {
        let frontend = Frontend::new(format!("{}.ls", name), *source);
        match frontend.check() {
            Ok(_) => problems.push(format!(
                "{}: expected a compile error, but the program was accepted",
                name
            )),
            Err(diagnostics) => {
                if !diagnostics.has_errors() {
                    problems.push(format!("{}: diagnostics do not contain an error", name));
                }
                let rendered = diagnostics.render(frontend.file());
                if !rendered.contains("error") {
                    problems.push(format!(
                        "{}: rendered diagnostics do not mention an error:\n{}",
                        name, rendered
                    ));
                }
            }
        }
    }
    if !problems.is_empty() {
        panic!("{}", problems.join("\n\n"));
    }
}

/// `if`, `while` and match guards require a `bool`; numeric conditions are
/// rejected with a type error instead of being treated as C truthiness.
#[test]
fn non_bool_conditions_are_rejected() {
    let source = include_str!("programs/bad_condition.ls");
    let frontend = Frontend::new("bad_condition.ls", source);
    let diagnostics = match frontend.check() {
        Ok(_) => panic!("expected condition type errors, but the program was accepted"),
        Err(diagnostics) => diagnostics,
    };
    let rendered = diagnostics.render(frontend.file());
    for missing in ["expected bool, found i64", "expected bool, found {integer}"] {
        assert!(
            rendered.contains(missing),
            "rendered diagnostics do not contain {:?}:\n{}",
            missing,
            rendered
        );
    }
}

/// Format specifications are checked against the argument type at compile time;
/// each invalid combination has to name the offending construct.
#[test]
fn format_specifier_errors_are_reported() {
    let programs: &[(&str, &str, &[&str])] = &[
        (
            "bad_format",
            include_str!("programs/bad_format.ls"),
            &[
                "unsupported format specifier '{:q}' in format string",
                "precision is only supported for floats and strings",
                "the '0' flag is only supported for numeric values",
            ],
        ),
        (
            "bad_format_unsigned",
            include_str!("programs/bad_format_unsigned.ls"),
            &["the '+' sign flag is not supported for unsigned integers"],
        ),
    ];
    let mut problems: Vec<String> = Vec::new();
    for (name, source, expected) in programs {
        let frontend = Frontend::new(format!("{}.ls", name), *source);
        match frontend.check() {
            Ok(_) => problems.push(format!(
                "{}: expected a compile error, but the program was accepted",
                name
            )),
            Err(diagnostics) => {
                let rendered = diagnostics.render(frontend.file());
                for message in *expected {
                    if !rendered.contains(message) {
                        problems.push(format!(
                            "{}: diagnostics do not mention {:?}:\n{}",
                            name, message, rendered
                        ));
                    }
                }
            }
        }
    }
    if !problems.is_empty() {
        panic!("{}", problems.join("\n\n"));
    }
}
