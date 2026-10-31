//! End-to-end tests: every case is a complete program that is compiled to C,
//! built with a native C compiler, then executed and compared against the
//! expected output and exit code.
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
            name: "try_option_ok",
            source: include_str!("programs/try_option_ok.ls"),
            args: &[],
            stdout: "some 8\nnone\ndone\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "try_option_none",
            source: include_str!("programs/try_option_none.ls"),
            args: &[],
            stdout: "got 42\nno value\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "try_result_ok",
            source: include_str!("programs/try_result_ok.ls"),
            args: &[],
            stdout: "42\n10\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "try_result_err",
            source: include_str!("programs/try_result_err.ls"),
            args: &[],
            stdout: "ok 42\nerr invalid integer\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "try_chain",
            source: include_str!("programs/try_chain.ls"),
            args: &[],
            stdout: "second 25\ntotal 27\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "combo_map",
            source: include_str!("programs/combo_map.ls"),
            args: &[],
            stdout: "42\n-1\n7\nfalse\ntrue\n22\nerr INVALID INTEGER\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "combo_and_then",
            source: include_str!("programs/combo_and_then.ls"),
            args: &[],
            stdout: "kept lsaca\ndropped\n31\n15\nerr bad\n20\n",
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
            name: "iter_basic",
            source: include_str!("programs/iter_basic.ls"),
            args: &[],
            stdout: "total 10\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "iter_any_all_find",
            source: include_str!("programs/iter_any_all_find.ls"),
            args: &[],
            stdout: "any true\nall true\nfound 9\nlong hello\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "iter_capture",
            source: include_str!("programs/iter_capture.ls"),
            args: &[],
            stdout: "[\"3!\", \"6!\", \"9!\"]\ntotal 60\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "iter_fold_count_sum",
            source: include_str!("programs/iter_fold_count_sum.ls"),
            args: &[],
            stdout: "total 15\njoined ab\ncount 3\nints 6\nfloats 3.75\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "trait_dispatch",
            source: include_str!("programs/trait_dispatch.ls"),
            args: &[],
            stdout: "rex says woof (inherent)\n3\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "option_scalar_fields",
            source: include_str!("programs/option_scalar_fields.ls"),
            args: &[],
            stdout: "5\n1 true\nnone\n7\n8 false\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "fn_in_aggregates",
            source: include_str!("programs/fn_in_aggregates.ls"),
            args: &[],
            stdout: "double 10\ntriple 15\n10\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "self_new",
            source: include_str!("programs/self_new.ls"),
            args: &[],
            stdout: "2 2\n3\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "pub_fields",
            source: include_str!("programs/pub_fields.ls"),
            args: &[],
            stdout: "main 2\nfast\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "generics_closure_infer",
            source: include_str!("programs/generics_closure_infer.ls"),
            args: &[],
            stdout: "12\n5\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "iter_map_filter",
            source: include_str!("programs/iter_map_filter.ls"),
            args: &[],
            stdout: "6\n12\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "iter_collect",
            source: include_str!("programs/iter_collect.ls"),
            args: &[],
            stdout: "[2, 4, 6]\n[\"a!\", \"b!\"]\n[2, 4, 6]\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "iter_take_skip_enumerate",
            source: include_str!("programs/iter_take_skip_enumerate.ls"),
            args: &[],
            stdout: "20\n30\n40\n0 a\n1 b\n2 c\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "iter_chain",
            source: include_str!("programs/iter_chain.ls"),
            args: &[],
            stdout: "0 16\n1 25\n2 36\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "iter_for_user_type",
            source: include_str!("programs/iter_for_user_type.ls"),
            args: &[],
            stdout: "1\n2\n3\n2\n4\n6\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "iter_noncopy",
            source: include_str!("programs/iter_noncopy.ls"),
            args: &[],
            stdout: "[5, 2, 5]\n[2, 1, 3]\n[\"hello\", \"hey\"]\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "iter_lazy_local",
            source: include_str!("programs/iter_lazy_local.ls"),
            args: &[],
            stdout: "8\n10\n12\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "generics_basic",
            source: include_str!("programs/generics_basic.ls"),
            args: &[],
            stdout: "1 2\n2 1\nleft right\n3\n9 0\nhere\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "generics_traits",
            source: include_str!("programs/generics_traits.ls"),
            args: &[],
            stdout: "point(1, 2) + 3\n2\nt:v\n1\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "hashmap_basic",
            source: include_str!("programs/hashmap_basic.ls"),
            args: &[],
            stdout: "2 3\nhas b\nSome(3)\n1 false\ntrue false\nfalse\ntrue 0\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "hashmap_advanced",
            source: include_str!("programs/hashmap_advanced.ls"),
            args: &[],
            stdout: "x=10\nNone\n2\n2\n30\ntrue false\n1 2\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "borrow_hashmap_owner",
            source: include_str!("programs/borrow_hashmap_owner.ls"),
            args: &[],
            stdout: "v=7\na\n1\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "tuples_nested",
            source: include_str!("programs/tuples_nested.ls"),
            args: &[],
            stdout: "((1, 2), (3, 4))\n1 2 3 4\n(1, (2, 3))\n1 2\n3\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "aggregate_equality",
            source: include_str!("programs/aggregate_equality.ls"),
            args: &[],
            stdout: "true false\ntrue false\ntrue false\ntrue false\ntrue false\ntrue false\ntrue\ntrue\nequality ok\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "iterators",
            source: include_str!("../../../examples/iterators.ls"),
            args: &[],
            stdout: "sum of evens: 12\nlengths: [5, 4]\ncountdown sum: 6\njoined: ab\n",
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
        Case {
            name: "refs_basic",
            source: include_str!("programs/refs_basic.ls"),
            args: &[],
            stdout: "10 10\n42\n5\n5\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "refs_functions",
            source: include_str!("programs/refs_functions.ls"),
            args: &[],
            stdout: "42\n3\n(1, 2)\n4\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "refs_struct",
            source: include_str!("programs/refs_struct.ls"),
            args: &[],
            stdout: "10 20\n30\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "refs_match",
            source: include_str!("programs/refs_match.ls"),
            args: &[],
            stdout: "7\n12\n7\n9\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "refs_box_rc",
            source: include_str!("programs/refs_box_rc.ls"),
            args: &[],
            stdout: "15\n42\n1\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "refs_nll",
            source: include_str!("programs/refs_nll.ls"),
            args: &[],
            stdout: "2\n5\n15 3\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "refs_mut_match_ergonomics",
            source: include_str!("programs/refs_mut_match_ergonomics.ls"),
            args: &[],
            stdout: "10 20\n7\nSome(2)\nSome(9)\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "refs_mut_self_returns",
            source: include_str!("programs/refs_mut_self_returns.ls"),
            args: &[],
            stdout: "1\n1\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "refs_mut_heap",
            source: include_str!("programs/refs_mut_heap.ls"),
            args: &[],
            stdout: "hi!\n[1, 2, 3, 4]\n6\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "drop_basic",
            source: include_str!("programs/drop_basic.ls"),
            args: &[],
            stdout: "start\nin block\ndrop inner\nend\ndrop boxed\ndrop v0\ndrop v1\ndrop pair\ndrop first\ndrop second\ndrop outer\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "drop_move",
            source: include_str!("programs/drop_move.ls"),
            args: &[],
            stdout: "took first\ndrop first\nafter take\nmoved to second\nend\ndrop second\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "clone_basic",
            source: include_str!("programs/clone_basic.ls"),
            args: &[],
            stdout: "lsaca lsaca\n[1, 2, 3, 4] [1, 2, 3]\nfirst [7, 8]\nsecond [7, 8, 9]\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "clone_vec",
            source: include_str!("programs/clone_vec.ls"),
            args: &[],
            stdout: "2 2\n[\"alpha\", \"beta\"]\n[\"alpha\", \"beta\"] [\"changed\", \"beta\", \"gamma\"]\n[[1, 2, 9], [3]] [[1, 2], [3]]\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "move_into_fn",
            source: include_str!("programs/move_into_fn.ls"),
            args: &[],
            stdout: "consume first [1, 2, 3]\ndone\nconsume second [4]\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "move_return",
            source: include_str!("programs/move_return.ls"),
            args: &[],
            stdout: "kept [1, 2]\nlsaca\ncreated\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "for_vec_move",
            source: include_str!("programs/for_vec_move.ls"),
            args: &[],
            stdout: "item a\ndrop a\nitem b\ndrop b\ndone\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "closure_capture_move",
            source: include_str!("programs/closure_capture_move.ls"),
            args: &[],
            stdout: "captured lsaca env\ncaptured lsaca env\ncount 3\ndone\ndrop env\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "rc_box_clone",
            source: include_str!("programs/rc_box_clone.ls"),
            args: &[],
            stdout: "true\n20 20\nstrong_count = 2\nchanged hi\nw w 5\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "drop_match",
            source: include_str!("programs/drop_match.ls"),
            args: &[],
            stdout: "plain\nbound a\ndrop a\nguard\nguard passed b\ndrop b\nguard_fail\nfallback c\ndrop c\nwildcard\nignored\ndrop d\noption\noption bound e\ndrop e\ndone\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "logical_ops",
            source: include_str!("programs/logical_ops.ls"),
            args: &[],
            stdout: "true\nfalse\ntrue\nfalse\ntrue counter = 0\nfalse counter = 0\nbranch\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "vec_extend",
            source: include_str!("programs/vec_extend.ls"),
            args: &[],
            stdout: "[1, 2, 3, 100, 200]\n5\n[1.5, 2.5, 3.5]\n[true, false, true]\n[\"alpha\", \"beta\"]\n[\"alpha\", \"beta\", \"gamma\"]\n3\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "result_clone",
            source: include_str!("programs/result_clone.ls"),
            args: &[],
            stdout: "21\n21\n21\n-1\ntrue\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "closure_ref_local",
            source: include_str!("programs/closure_ref_local.ls"),
            args: &[],
            stdout: "lsaca\nalpha\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "borrow_in_loop",
            source: include_str!("programs/borrow_in_loop.ls"),
            args: &[],
            stdout: "alpha\n20 2\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "ref_field_chain",
            source: include_str!("programs/ref_field_chain.ls"),
            args: &[],
            stdout: "2\n2 4\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "char_basics",
            source: include_str!("programs/char_basics.ls"),
            args: &[],
            stdout: "a true 你\n'\\n'\n97\ntrue\nletter a\nsmall digit\nspace\nother\n2 你\nsome z\n你 1\n你\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "string_unicode",
            source: include_str!("programs/string_unicode.ls"),
            args: &[],
            stdout: "13\n8\ntrue false\nSome('h')\nSome('é')\nSome(' ')\n你好\nhéll !\nSTRASSE\nhéllo\n[x]\n0 true\n3\n3\nx \"x\"\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "unicode_debug",
            source: include_str!("programs/unicode_debug.ls"),
            args: &[],
            stdout: "'x'\n'\\n'\n'\\''\n'\\\\'\n'\\u{7}'\n'你'\n\"a\\u{0}b\"\n\"tab\\there\"\n\"\"\n[😀]\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "hashmap_char",
            source: include_str!("programs/hashmap_char.ls"),
            args: &[],
            stdout: "4 5\nSome(2) 3\n0 true\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "hashmap_swiss",
            source: include_str!("programs/hashmap_swiss.ls"),
            args: &[],
            stdout: "2000 5997000\n1000 1000\n1000\n",
            exit: 0,
            stderr_contains: &[],
        },
        Case {
            name: "panic_substring",
            source: include_str!("programs/panic_substring.ls"),
            args: &[],
            stdout: "",
            exit: 101,
            stderr_contains: &[
                "byte index 2 is not a char boundary",
                "it is inside 'é' (bytes 1..3) of `héllo`",
            ],
        },
        Case {
            name: "deep_expression",
            source: include_str!("programs/deep_expression.ls"),
            args: &[],
            stdout: "100\n",
            exit: 0,
            stderr_contains: &[],
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
        ("bad_try_scope", include_str!("programs/bad_try_scope.ls")),
        ("bad_try_type", include_str!("programs/bad_try_type.ls")),
        (
            "bad_borrow_alias",
            include_str!("programs/bad_borrow_alias.ls"),
        ),
        (
            "bad_borrow_shared_mut",
            include_str!("programs/bad_borrow_shared_mut.ls"),
        ),
        (
            "bad_borrow_assign",
            include_str!("programs/bad_borrow_assign.ls"),
        ),
        ("bad_ref_return", include_str!("programs/bad_ref_return.ls")),
        (
            "bad_ref_param_return",
            include_str!("programs/bad_ref_param_return.ls"),
        ),
        (
            "bad_ref_local_lifetime",
            include_str!("programs/bad_ref_local_lifetime.ls"),
        ),
        ("bad_ref_store", include_str!("programs/bad_ref_store.ls")),
        (
            "bad_ref_temporary",
            include_str!("programs/bad_ref_temporary.ls"),
        ),
        ("bad_deref", include_str!("programs/bad_deref.ls")),
        (
            "bad_ambiguous_ref_return",
            include_str!("programs/bad_ambiguous_ref_return.ls"),
        ),
        ("bad_mut_borrow", include_str!("programs/bad_mut_borrow.ls")),
        (
            "bad_deep_nesting",
            include_str!("programs/bad_deep_nesting.ls"),
        ),
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

/// The try operator reports targeted diagnostics: the wrong enclosing return
/// type and a non Option/Result operand both mention what is wrong and how to
/// fix it.
#[test]
fn rejects_try_operator_misuse() {
    let programs: &[(&str, &str, &str)] = &[
        (
            "bad_try_scope",
            include_str!("programs/bad_try_scope.ls"),
            "can only be used in a function that returns Option",
        ),
        (
            "bad_try_type",
            include_str!("programs/bad_try_type.ls"),
            "can only be applied to Option or Result values",
        ),
    ];
    for (name, source, needle) in programs {
        let frontend = Frontend::new(format!("{}.ls", name), *source);
        let diagnostics = match frontend.check() {
            Ok(_) => panic!(
                "{}: expected a compile error, but the program was accepted",
                name
            ),
            Err(diagnostics) => diagnostics,
        };
        let rendered = diagnostics.render(frontend.file());
        assert!(
            rendered.contains("error"),
            "{}: rendered diagnostics do not mention an error:\n{}",
            name,
            rendered
        );
        assert!(
            rendered.contains(needle),
            "{}: expected the diagnostic to mention {:?}:\n{}",
            name,
            needle,
            rendered
        );
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

/// Pathologically deep expressions are rejected with one diagnostic instead of
/// overflowing the parser or type-checker stack; ordinary deep chains compile.
#[test]
fn deeply_nested_expressions_are_rejected() {
    let source = include_str!("programs/bad_deep_nesting.ls");
    let frontend = Frontend::new("bad_deep_nesting.ls", source);
    let diagnostics = match frontend.check() {
        Ok(_) => panic!("expected a nesting error, but the program was accepted"),
        Err(diagnostics) => diagnostics,
    };
    let rendered = diagnostics.render(frontend.file());
    assert!(
        rendered.contains("this code is nested too deeply"),
        "rendered diagnostics do not mention deep nesting:\n{}",
        rendered
    );
    assert_eq!(
        rendered.matches("this code is nested too deeply").count(),
        1,
        "expected exactly one nesting diagnostic:\n{}",
        rendered
    );
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
        (
            "bad_char_operators",
            include_str!("programs/bad_char_operators.ls"),
            &[
                "error[E0308]",
                "cannot cast char as bool",
                "expected char, found {integer}",
            ],
        ),
        (
            "bad_char_format",
            include_str!("programs/bad_char_format.ls"),
            &["the '+' flag of a format specifier is not supported for characters"],
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

/// Reference and borrowing diagnostics are targeted: every rejected program
/// names the error code and explains the violated rule. The checker must not
/// leak internal HIR names such as v1 into user-facing messages.
#[test]
fn borrow_errors_are_reported() {
    let programs: &[(&str, &str, &[&str])] = &[
        (
            "bad_borrow_alias",
            include_str!("programs/bad_borrow_alias.ls"),
            &[
                "error[E0499]",
                "cannot borrow value as mutable more than once at a time",
            ],
        ),
        (
            "bad_borrow_shared_mut",
            include_str!("programs/bad_borrow_shared_mut.ls"),
            &[
                "error[E0502]",
                "cannot borrow value as immutable because it is also borrowed as mutable",
            ],
        ),
        (
            "bad_borrow_assign",
            include_str!("programs/bad_borrow_assign.ls"),
            &[
                "error[E0506]",
                "cannot assign to value because it is borrowed",
            ],
        ),
        (
            "bad_ref_return",
            include_str!("programs/bad_ref_return.ls"),
            &[
                "error[E0106]",
                "missing lifetime specifier",
                "error[E0515]",
                "returns a reference to a local value",
            ],
        ),
        (
            "bad_ref_param_return",
            include_str!("programs/bad_ref_param_return.ls"),
            &["error[E0515]", "not the elided lifetime source"],
        ),
        (
            "bad_ref_local_lifetime",
            include_str!("programs/bad_ref_local_lifetime.ls"),
            &["error[E0597]", "does not live long enough"],
        ),
        (
            "bad_ref_store",
            include_str!("programs/bad_ref_store.ls"),
            &["error[E0498]", "references cannot be stored"],
        ),
        (
            "bad_ref_temporary",
            include_str!("programs/bad_ref_temporary.ls"),
            &[
                "error[E0745]",
                "cannot borrow a temporary value, bind it to a local first",
            ],
        ),
        (
            "bad_deref",
            include_str!("programs/bad_deref.ls"),
            &["error[E0614]", "cannot be dereferenced"],
        ),
        (
            "bad_ambiguous_ref_return",
            include_str!("programs/bad_ambiguous_ref_return.ls"),
            &["error[E0106]", "missing lifetime specifier"],
        ),
        (
            "bad_mut_borrow",
            include_str!("programs/bad_mut_borrow.ls"),
            &["error[E0596]", "the binding is not declared as mutable"],
        ),
        (
            "bad_ref_return_branch",
            include_str!("programs/bad_ref_return_branch.ls"),
            &[
                "error[E0515]",
                "cannot return a reference to local variable x",
            ],
        ),
        (
            "bad_ref_return_match",
            include_str!("programs/bad_ref_return_match.ls"),
            &[
                "error[E0515]",
                "cannot return a reference to local variable x",
            ],
        ),
        (
            "bad_ref_return_call_argument",
            include_str!("programs/bad_ref_return_call_argument.ls"),
            &[
                "error[E0515]",
                "cannot return a reference to local variable x",
            ],
        ),
        (
            "bad_borrow_then_move_in_branch",
            include_str!("programs/bad_borrow_then_move_in_branch.ls"),
            &[
                "error[E0505]",
                "cannot move out of text because it is borrowed",
            ],
        ),
        (
            "bad_match_borrow_then_move",
            include_str!("programs/bad_match_borrow_then_move.ls"),
            &[
                "error[E0505]",
                "cannot move out of maybe because it is borrowed",
            ],
        ),
        (
            "bad_char_cast",
            include_str!("programs/bad_char_cast.ls"),
            &["error[E0308]", "only u8 can be cast as char"],
        ),
    ];
    let mut problems: Vec<String> = Vec::new();
    for (name, source, expected) in programs {
        let frontend = Frontend::new(format!("{}.ls", name), *source);
        let diagnostics = match frontend.check() {
            Ok(_) => {
                problems.push(format!(
                    "{}: expected a compile error, but the program was accepted",
                    name
                ));
                continue;
            }
            Err(diagnostics) => diagnostics,
        };
        let rendered = diagnostics.render(frontend.file());
        for needle in *expected {
            if !rendered.contains(needle) {
                problems.push(format!(
                    "{}: diagnostics do not mention {:?}:\n{}",
                    name, needle, rendered
                ));
            }
        }
    }
    if !problems.is_empty() {
        panic!("{}", problems.join("\n\n"));
    }
}

/// Ownership diagnostics are targeted: moves of moved values, out of
/// projections and out of Drop types, moves while borrowed, and clones of Drop
/// types all name an error code and explain the violated ownership rule.
#[test]
fn move_errors_are_reported() {
    let programs: &[(&str, &str, &[&str])] = &[
        (
            "bad_use_after_move",
            include_str!("programs/bad_use_after_move.ls"),
            &["error[E0382]", "use of moved value"],
        ),
        (
            "bad_move_out_of_field",
            include_str!("programs/bad_move_out_of_field.ls"),
            &["error[E0507]", "cannot move out of a field"],
        ),
        (
            "bad_move_out_of_drop_type",
            include_str!("programs/bad_move_out_of_drop_type.ls"),
            &["error[E0509]", "implements Drop"],
        ),
        (
            "bad_move_while_borrowed",
            include_str!("programs/bad_move_while_borrowed.ls"),
            &["error[E0505]", "because it is borrowed"],
        ),
        (
            "bad_clone_drop_type",
            include_str!("programs/bad_clone_drop_type.ls"),
            &["error[E0277]", "cannot be cloned"],
        ),
    ];
    let mut problems: Vec<String> = Vec::new();
    for (name, source, expected) in programs {
        let frontend = Frontend::new(format!("{}.ls", name), *source);
        let diagnostics = match frontend.check() {
            Ok(_) => {
                problems.push(format!(
                    "{}: expected a compile error, but the program was accepted",
                    name
                ));
                continue;
            }
            Err(diagnostics) => diagnostics,
        };
        let rendered = diagnostics.render(frontend.file());
        for needle in *expected {
            if !rendered.contains(needle) {
                problems.push(format!(
                    "{}: diagnostics do not mention {:?}:\n{}",
                    name, needle, rendered
                ));
            }
        }
    }
    if !problems.is_empty() {
        panic!("{}", problems.join("\n\n"));
    }
}

/// v1.5 features reject ill-formed programs with targeted diagnostics:
/// generic arity, trait conformance (missing/extra/mis-typed members,
/// duplicate impls), trait objects, unsatisfied bounds, HashMap key
/// restrictions, reference storage and iterator adaptor misuse.
#[test]
fn v15_feature_errors_are_reported() {
    let programs: &[(&str, &str, &[&str])] = &[
        (
            "bad_generic_arity",
            include_str!("programs/bad_generic_arity.ls"),
            &[
                "error[E0107]",
                "struct Pair takes 1 type argument but 2 were supplied",
            ],
        ),
        (
            "bad_trait_missing",
            include_str!("programs/bad_trait_missing.ls"),
            &["error[E0046]", "not all trait items implemented"],
        ),
        (
            "bad_trait_extra",
            include_str!("programs/bad_trait_extra.ls"),
            &["error[E0407]"],
        ),
        (
            "bad_trait_type",
            include_str!("programs/bad_trait_type.ls"),
            &["error[E0412]", "trait objects are not supported"],
        ),
        (
            "bad_trait_duplicate",
            include_str!("programs/bad_trait_duplicate.ls"),
            &["error[E0119]"],
        ),
        (
            "bad_trait_signature",
            include_str!("programs/bad_trait_signature.ls"),
            &["error[E0053]"],
        ),
        (
            "bad_generic_bound",
            include_str!("programs/bad_generic_bound.ls"),
            &[
                "error[E0277]",
                "the trait bound (i64: Sized2) is not satisfied",
            ],
        ),
        (
            "bad_hashmap_key",
            include_str!("programs/bad_hashmap_key.ls"),
            &[
                "error[E0277]",
                "HashMap keys must be one of i64, u64, bool, char or String, found f64",
            ],
        ),
        (
            "bad_hashmap_get_with_ref",
            include_str!("programs/bad_hashmap_get_with_ref.ls"),
            &["error[E0498]", "references cannot be stored"],
        ),
        (
            "bad_store_hash_map_value_ref",
            include_str!("programs/bad_store_hash_map_value_ref.ls"),
            &["error[E0498]", "references cannot be stored"],
        ),
        (
            "bad_store_hash_map_key_ref",
            include_str!("programs/bad_store_hash_map_key_ref.ls"),
            &[
                "error[E0277]",
                "HashMap keys must be one of i64, u64, bool, char or String",
            ],
        ),
        (
            "bad_store_vec_ref",
            include_str!("programs/bad_store_vec_ref.ls"),
            &["error[E0498]", "references cannot be stored"],
        ),
        (
            "bad_store_option_ref_field",
            include_str!("programs/bad_store_option_ref_field.ls"),
            &["error[E0498]", "references cannot be stored"],
        ),
        (
            "bad_store_tuple_ref_map_value",
            include_str!("programs/bad_store_tuple_ref_map_value.ls"),
            &["error[E0498]", "references cannot be stored"],
        ),
        (
            "bad_store_generic_struct_ref",
            include_str!("programs/bad_store_generic_struct_ref.ls"),
            &["error[E0498]", "references cannot be stored"],
        ),
        (
            "bad_store_closure_ref_field",
            include_str!("programs/bad_store_closure_ref_field.ls"),
            &["error[E0498]", "references cannot be captured"],
        ),
        (
            "iter_collect_no_type",
            include_str!("programs/iter_collect_no_type.ls"),
            &[
                "error[E0282]",
                ".collect() requires a known Vec<T> result type",
            ],
        ),
        (
            "iter_sum_non_numeric",
            include_str!("programs/iter_sum_non_numeric.ls"),
            &["error[E0277]", ".sum() is only supported"],
        ),
        (
            "bad_trait_impl_target_builtin",
            include_str!("programs/bad_trait_impl_target_builtin.ls"),
            &[
                "error[E0118]",
                "impl target must be a struct or enum type defined in this program, found Vec<i64>",
            ],
        ),
        (
            "bad_method_turbofish",
            include_str!("programs/bad_method_turbofish.ls"),
            &[
                "error[E0107]",
                "method get takes 0 generic arguments but 1 was supplied",
            ],
        ),
        (
            "bad_iter_zip",
            include_str!("programs/bad_iter_zip.ls"),
            &[
                "error[E0599]",
                "no method named zip found for type VecIter<i64>",
            ],
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

/// Build and run a program with the given bytes on standard input.
fn run_program_with_stdin(
    name: &str,
    source: &str,
    stdin: &[u8],
) -> Result<(String, String, i32), String> {
    use std::io::Write;
    use std::process::{Command as StdCommand, Stdio};
    let executable = build_program(name, source, false)?;
    let mut child = StdCommand::new(&executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not run {}: {}", name, error))?;
    {
        let mut pipe = child.stdin.take().expect("piped stdin");
        pipe.write_all(stdin)
            .map_err(|error| format!("could not write stdin of {}: {}", name, error))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("could not wait for {}: {}", name, error))?;
    Ok((
        normalize(&String::from_utf8_lossy(&output.stdout)),
        normalize(&String::from_utf8_lossy(&output.stderr)),
        output.status.code().unwrap_or(-1),
    ))
}

const READ_LINE_PROGRAM: &str = r###"
fn main() {
    let line = read_line().unwrap();
    println!("{}", line);
    let rest = read_line();
    match rest {
        Some(value) => println!("{}", value),
        None => println!("<eof>"),
    }
}
"###;

/// `read_line` must accept valid UTF-8 from standard input and report end of
/// file as `None`; invalid UTF-8 must panic with Rust's message instead of
/// injecting an invalid `String` into the program.
#[test]
fn read_line_validates_stdin_utf8() {
    match run_program_with_stdin("stdin_ok", READ_LINE_PROGRAM, b"hello\nworld\n") {
        Ok((stdout, stderr, exit)) => {
            assert_eq!(stdout, "hello\nworld\n");
            assert_eq!(exit, 0);
            assert!(!stderr.contains("reference-counted allocation"));
        }
        Err(error) => panic!("{}", error),
    }
    match run_program_with_stdin("stdin_bad", READ_LINE_PROGRAM, b"ok\n\xff\xfe\n") {
        Ok((stdout, stderr, exit)) => {
            assert_eq!(stdout, "ok\n", "output before the panic");
            assert_eq!(exit, 101, "a runtime panic exits with 101");
            assert!(
                stderr.contains("stream did not contain valid UTF-8"),
                "stderr should explain the invalid UTF-8: {}",
                stderr
            );
        }
        Err(error) => panic!("{}", error),
    }
}

/// Programs whose emitted user functions exceed the split threshold must be
/// generated as `main.c` plus deterministic extra translation units, and the
/// build driver must write, compile and link every unit.
#[test]
fn large_programs_are_split_into_translation_units() {
    let source = include_str!("../../../benchmarks/programs/compile_large.ls");
    let frontend = Frontend::new("compile_large.ls", source);
    let program = match frontend.check() {
        Ok(program) => program,
        Err(diagnostics) => panic!(
            "compile_large was rejected:\n{}",
            diagnostics.render(frontend.file())
        ),
    };
    let options = CodegenOptions {
        program_name: "compile_large_split".to_string(),
        debug_checks: false,
        emit_location_info: true,
        source: Some(frontend.file().clone()),
        ..CodegenOptions::default()
    };
    let generated = driver::generate_c(&program, &options);
    assert!(
        !generated.units.is_empty(),
        "compile_large should be split into translation units"
    );
    let names: Vec<String> = generated
        .units
        .iter()
        .map(|unit| unit.file_name.clone())
        .collect();
    let expected: Vec<String> = (1..=names.len())
        .map(|index| format!("lsaca_unit_{}.c", index))
        .collect();
    assert_eq!(
        names, expected,
        "unit file names must be contiguous and deterministic"
    );
    assert!(
        generated.main_c.contains("int main("),
        "main.c keeps the C entry point"
    );
    let build = BuildOptions::new("compile_large_split".to_string());
    let built = match driver::build_with_cache(&generated, &build, &test_cache_root()) {
        Ok(built) => built,
        Err(error) => panic!("building the split project failed:\n{}", error),
    };
    let project_dir = built
        .build_dir
        .parent()
        .expect("the build directory is inside the project directory");
    for name in &names {
        assert!(
            project_dir.join(name).is_file(),
            "the split build tree should contain {}",
            name
        );
    }
    let output = Command::new(&built.executable)
        .output()
        .unwrap_or_else(|error| panic!("could not run the split program: {}", error));
    let stdout = normalize(&String::from_utf8_lossy(&output.stdout));
    let stderr = normalize(&String::from_utf8_lossy(&output.stderr));
    assert_eq!(stdout, "checksum=14655037\n");
    assert_eq!(output.status.code(), Some(0));
    assert!(!stderr.contains("reference-counted allocation"));
}
