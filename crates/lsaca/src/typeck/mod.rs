//! Semantic analysis: name resolution, type inference, type checking,
//! pattern exhaustiveness and lowering to the HIR.

mod builtins;
mod check;
mod consts;
mod exhaust;
mod exprs;
mod items;
mod itype;
mod lower;
mod patterns;
mod tree;

use std::path::Path;

use crate::diag::{Diagnostic, Diagnostics, Severity, Span, sort_and_dedup};
use crate::span::SourceFile;
use crate::{ast, hir};

use self::check::Checker;
use self::itype::ITy;

/// Type check a parsed program and lower it to the fully typed HIR.
///
/// This is the original frozen entry point: warnings are only surfaced when
/// the check fails. Use [check_reporting] when warnings from successful
/// checks matter.
pub fn check(program: &ast::Program, file: &SourceFile) -> Result<hir::Program, Diagnostics> {
    check_reporting(program, file).0
}

/// Type check a parsed program and lower it to the fully typed HIR, also
/// returning the warnings that were produced even when checking succeeds.
///
/// The first element behaves exactly like [check]. The second element holds
/// the warnings-only diagnostics (every severity except Error), sorted and
/// deduplicated deterministically.
pub fn check_reporting(
    program: &ast::Program,
    file: &SourceFile,
) -> (Result<hir::Program, Diagnostics>, Diagnostics) {
    let mut checker = Checker::new(file);
    checker.collect_items(program);
    for id in 0..checker.consts.len() {
        checker.ensure_const(id);
    }
    checker.check_all_fns();
    validate_main(&mut checker);

    let collected = std::mem::take(&mut checker.diagnostics);
    let (mut errors, mut warnings) = split_diagnostics(collected);
    if errors.has_errors() {
        // Preserve the historical behaviour of check(): the error result
        // carries every diagnostic, warnings included.
        errors.extend(warnings.clone());
        sort_and_dedup(&mut errors);
        return (Err(errors), warnings);
    }

    let name = program_name(file);
    let program = checker.build_hir(name);

    // Lowering can report additional diagnostics (for example unresolved
    // inference variables), so split whatever the lowering phase produced.
    let lowered = std::mem::take(&mut checker.diagnostics);
    let (late_errors, late_warnings) = split_diagnostics(lowered);
    for diagnostic in late_warnings.into_vec() {
        warnings.push(diagnostic);
    }
    sort_and_dedup(&mut warnings);
    if late_errors.has_errors() {
        let mut reported = late_errors;
        for diagnostic in warnings.clone().into_vec() {
            reported.push(diagnostic);
        }
        sort_and_dedup(&mut reported);
        return (Err(reported), warnings);
    }
    (Ok(program), warnings)
}

/// Split diagnostics into errors and everything else, sorting and
/// deduplicating both halves deterministically. Warnings and notes land in
/// the second half so that notes attached by the checker are not lost.
fn split_diagnostics(diagnostics: Diagnostics) -> (Diagnostics, Diagnostics) {
    let mut errors = Diagnostics::new();
    let mut warnings = Diagnostics::new();
    for diagnostic in diagnostics.into_vec() {
        match diagnostic.severity {
            Severity::Error => errors.push(diagnostic),
            Severity::Warning | Severity::Note => warnings.push(diagnostic),
        }
    }
    sort_and_dedup(&mut errors);
    sort_and_dedup(&mut warnings);
    (errors, warnings)
}

fn validate_main(checker: &mut Checker<'_>) {
    let main_id = match checker.fn_by_name.get("main").copied() {
        Some(main_id) => main_id,
        None => {
            let mut diagnostic =
                Diagnostic::error("main function not found".to_string(), Span::empty(0))
                    .with_code("E0601")
                    .with_note("the program must define fn main()".to_string());
            diagnostic.severity = crate::diag::Severity::Error;
            checker.diagnostics.push(diagnostic);
            return;
        }
    };
    let info = checker.fns[main_id].clone();
    if !info.decl.params.is_empty() {
        checker.error_code(
            format!(
                "main function must not take parameters, but it declares {}",
                info.decl.params.len()
            ),
            info.name_span,
            "E0580",
        );
    }
    let ret = match &info.sig {
        Some(sig) => checker.infer.resolve(&sig.ret),
        None => ITy::Unit,
    };
    if ret != ITy::Unit {
        let found = checker.describe_resolved(&ret);
        checker.error_code(
            format!("main function must return (), but it returns {}", found),
            info.name_span,
            "E0580",
        );
    }
}

fn program_name(file: &SourceFile) -> String {
    let raw = file.name();
    let base = Path::new(raw)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(raw);
    let stem = base.strip_suffix(".ls").unwrap_or(base);
    let trimmed = stem.trim();
    if trimmed.is_empty() {
        "program".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::Severity;
    use crate::hir::{ConstValue, HExprKind, HStmt};
    use crate::types::{FloatTy, IntTy, SelfKind, Ty};

    fn parse_source(source: &str) -> (ast::Program, SourceFile, Diagnostics) {
        let file = SourceFile::new("test.ls", source);
        let (tokens, mut diagnostics) = crate::lexer::tokenize(source);
        let (program, more) = crate::parser::parse(&tokens, &file);
        diagnostics.extend(more);
        (program, file, diagnostics)
    }

    fn messages(diagnostics: &Diagnostics) -> Vec<String> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect()
    }

    fn check_source(source: &str) -> Result<hir::Program, Diagnostics> {
        let (program, file, diagnostics) = parse_source(source);
        assert!(
            !diagnostics.has_errors(),
            "unexpected parse errors: {:?}",
            messages(&diagnostics)
        );
        check(&program, &file)
    }

    fn expect_ok(source: &str) -> hir::Program {
        match check_source(source) {
            Ok(program) => program,
            Err(diagnostics) => panic!(
                "expected the program to type check, got: {:?}",
                messages(&diagnostics)
            ),
        }
    }

    fn expect_errors(source: &str) -> Diagnostics {
        match check_source(source) {
            Ok(_) => panic!("expected type errors, but the program type checked"),
            Err(diagnostics) => diagnostics,
        }
    }

    fn analyze(source: &str) -> Diagnostics {
        let (program, file, parse_diagnostics) = parse_source(source);
        assert!(
            !parse_diagnostics.has_errors(),
            "unexpected parse errors: {:?}",
            messages(&parse_diagnostics)
        );
        let mut checker = Checker::new(&file);
        checker.collect_items(&program);
        for id in 0..checker.consts.len() {
            checker.ensure_const(id);
        }
        checker.check_all_fns();
        validate_main(&mut checker);
        let mut diagnostics = checker.diagnostics;
        sort_and_dedup(&mut diagnostics);
        diagnostics
    }

    fn has_message(diagnostics: &Diagnostics, needle: &str) -> bool {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(needle))
    }

    fn has_code(diagnostics: &Diagnostics, code: &str) -> bool {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == Some(code))
    }

    fn has_warning(diagnostics: &Diagnostics, needle: &str) -> bool {
        diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == Severity::Warning && diagnostic.message.contains(needle)
        })
    }

    fn main_fn(program: &hir::Program) -> &hir::HFn {
        &program.fns[program.main_fn]
    }

    fn find_fn<'a>(program: &'a hir::Program, name: &str) -> Option<&'a hir::HFn> {
        program.fns.iter().find(|function| function.name == name)
    }

    // -----------------------------------------------------------------
    // Item collection and lowering
    // -----------------------------------------------------------------

    #[test]
    fn accepts_minimal_program() {
        let program = expect_ok("fn main() { }");
        assert_eq!(program.fns.len(), 1);
        assert_eq!(main_fn(&program).name, "main");
        assert!(program.structs.is_empty());
        assert!(program.enums.is_empty());
        assert!(program.consts.is_empty());
        assert!(program.closures.is_empty());
    }

    #[test]
    fn collects_items_and_lowers_methods() {
        let program = expect_ok(
            r#"struct Point { x: i64, y: i64 }
enum Shape { Circle(i64), Empty }
impl Point {
    fn sum(&self) -> i64 { self.x + self.y }
}
const LIMIT: i64 = 7;
fn main() {
    let p = Point { x: 1, y: 2 };
    let s = p.sum();
    println!("{} {} {}", s, LIMIT, String::from("ok"));
    let shape = Shape::Empty;
    match shape {
        Shape::Empty => {}
        Shape::Circle(radius) => println!("{}", radius),
    }
}"#,
        );
        assert_eq!(program.structs.len(), 1);
        assert_eq!(program.structs[0].name, "Point");
        assert_eq!(program.structs[0].fields.len(), 2);
        assert_eq!(program.enums.len(), 1);
        assert_eq!(program.enums[0].variants.len(), 2);
        assert_eq!(
            program.enums[0].variants[0].fields,
            vec![Ty::Int(IntTy::I64)]
        );
        assert_eq!(program.consts.len(), 1);
        assert_eq!(program.consts[0].name, "LIMIT");
        assert_eq!(program.consts[0].ty, Ty::Int(IntTy::I64));
        assert_eq!(program.consts[0].value, ConstValue::Int(7));
        let method = find_fn(&program, "Point::sum").expect("method should be lowered");
        assert_eq!(method.self_kind, Some(SelfKind::Ref));
        assert_eq!(method.params.len(), 1);
        assert_eq!(method.params[0].name, "self");
        assert_eq!(method.ret, Ty::Int(IntTy::I64));
    }

    #[test]
    fn hoists_nested_items() {
        let program = expect_ok(
            r#"fn main() {
    struct Local { value: i64 }
    impl Local { fn get(&self) -> i64 { self.value } }
    fn helper(x: i64) -> i64 { x + 1 }
    let local = Local { value: 4 };
    println!("{} {}", local.get(), helper(local.value));
}"#,
        );
        assert_eq!(program.structs.len(), 1);
        assert_eq!(program.structs[0].name, "Local");
        assert!(find_fn(&program, "helper").is_some());
        assert!(find_fn(&program, "Local::get").is_some());
    }

    #[test]
    fn supports_type_aliases() {
        let program = expect_ok(
            r#"type Number = i64;
struct Wrap { value: Number }
fn main() {
    let wrap = Wrap { value: 3 };
    println!("{}", wrap.value);
}"#,
        );
        assert_eq!(program.structs.len(), 1);
        assert_eq!(program.structs[0].fields[0].ty, Ty::Int(IntTy::I64));
    }

    // -----------------------------------------------------------------
    // Inference
    // -----------------------------------------------------------------

    #[test]
    fn infers_local_types() {
        let program = expect_ok(
            r#"fn main() {
    let a = 1 + 2;
    let b = 1.5 * 2.0;
    let c = "text";
    let d = a;
    println!("{} {} {} {}", a, b, c, d);
}"#,
        );
        let body = &main_fn(&program).body;
        match &body.stmts[0] {
            HStmt::Let { pat, init, .. } => {
                assert_eq!(pat.ty, Ty::Int(IntTy::I64));
                let init = init.as_ref().expect("initializer");
                assert_eq!(init.ty, Ty::Int(IntTy::I64));
            }
            _ => panic!("expected a let statement"),
        }
        match &body.stmts[1] {
            HStmt::Let { pat, .. } => assert_eq!(pat.ty, Ty::Float(FloatTy::F64)),
            _ => panic!("expected a let statement"),
        }
        match &body.stmts[2] {
            HStmt::Let { pat, .. } => assert_eq!(pat.ty, Ty::String),
            _ => panic!("expected a let statement"),
        }
    }

    #[test]
    fn requires_annotations_for_unconstrained_collections() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let values = Vec::new();
    println!("{}", values.len());
}"#,
        );
        assert!(has_code(&diagnostics, "E0282"));
    }

    // -----------------------------------------------------------------
    // Name resolution errors
    // -----------------------------------------------------------------

    #[test]
    fn rejects_unknown_names() {
        let diagnostics = expect_errors(
            r#"fn main() {
    println!("{}", later);
    let later = 1;
}"#,
        );
        assert!(has_message(&diagnostics, "cannot find value"));
        let diagnostics = expect_errors("fn main() { let value: Missing = 1; }");
        assert!(has_code(&diagnostics, "E0412"));
        assert!(has_message(&diagnostics, "cannot find type"));
        let diagnostics = expect_errors(
            r#"struct P { x: i64 }
fn main() {
    let p = P { x: 1 };
    println!("{}", p.y);
}"#,
        );
        assert!(has_code(&diagnostics, "E0609"));
        assert!(has_message(&diagnostics, "no field"));
        let diagnostics = expect_errors("fn main() { missing_function(); }");
        assert!(has_code(&diagnostics, "E0425"));
    }

    #[test]
    fn reports_duplicate_definitions() {
        let diagnostics = analyze(
            r#"struct A { x: i64 }
struct A { y: i64 }
fn main() { }"#,
        );
        assert!(has_code(&diagnostics, "E0428"));
        assert!(has_message(&diagnostics, "defined multiple times"));
        let diagnostics = analyze(
            r#"struct B { x: i64, x: i64 }
fn main() { }"#,
        );
        assert!(has_message(&diagnostics, "already declared"));
    }

    // -----------------------------------------------------------------
    // Type checking and mutability
    // -----------------------------------------------------------------

    #[test]
    fn reports_mismatched_types() {
        let diagnostics = expect_errors(r#"fn main() { let x: i64 = "text"; }"#);
        assert!(has_code(&diagnostics, "E0308"));
        assert!(has_message(&diagnostics, "mismatched types"));
    }

    #[test]
    fn rejects_uninitialized_bindings() {
        let diagnostics = expect_errors("fn main() { let x: i64; }");
        assert!(has_message(
            &diagnostics,
            "cannot declare a binding without an initializer"
        ));
    }

    #[test]
    fn assignment_requires_mutable_bindings() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let x = 1;
    x = 2;
}"#,
        );
        assert!(has_code(&diagnostics, "E0596"));
        expect_ok(
            r#"fn main() {
    let mut x = 1;
    x = 2;
    x += 3;
    x *= 2;
    println!("{}", x);
}"#,
        );
    }

    #[test]
    fn mutable_method_receivers_require_mutable_places() {
        let source_prefix = r#"struct Counter { value: i64 }
impl Counter { fn bump(&mut self) { self.value = self.value + 1; } }
fn main() {
    let counter = Counter { value: 0 };
    counter.bump();
}"#;
        let diagnostics = expect_errors(source_prefix);
        assert!(has_code(&diagnostics, "E0596"));
        assert!(has_message(&diagnostics, "is not declared as mutable"));
        expect_ok(
            r#"struct Counter { value: i64 }
impl Counter { fn bump(&mut self) { self.value = self.value + 1; } }
fn main() {
    let mut counter = Counter { value: 0 };
    counter.bump();
    println!("{}", counter.value);
}"#,
        );
    }

    #[test]
    fn checks_binary_operators() {
        let diagnostics = expect_errors(
            r#"struct P { x: i64 }
fn main() {
    let a = P { x: 1 };
    let b = P { x: 2 };
    let equal = a == b;
    println!("{}", equal);
}"#,
        );
        assert!(has_code(&diagnostics, "E0369"));
        let diagnostics = expect_errors(
            r#"fn main() {
    let a: i64 = 1;
    let b: i32 = 2;
    println!("{}", a + b);
}"#,
        );
        assert!(has_message(&diagnostics, "mismatched types"));
        expect_ok(
            r#"fn main() {
    let less = "apple" < "banana";
    let joined = "foo" + "bar";
    println!("{} {}", less, joined);
}"#,
        );
    }

    #[test]
    fn checks_casts() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let text = "x";
    let value = text as i64;
    println!("{}", value);
}"#,
        );
        assert!(has_code(&diagnostics, "E0605"));
        assert!(has_message(&diagnostics, "non-primitive cast"));
        expect_ok(
            r#"fn main() {
    let a: i64 = 300;
    let b: u8 = a as u8;
    let c: f64 = b as f64;
    let d: i32 = c as i32;
    println!("{} {} {} {}", a, b, c, d);
}"#,
        );
    }

    // -----------------------------------------------------------------
    // Patterns and exhaustiveness
    // -----------------------------------------------------------------

    #[test]
    fn match_requires_exhaustive_patterns() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let value: Option<i64> = Some(1);
    match value {
        Some(number) => println!("{}", number),
    }
}"#,
        );
        assert!(has_code(&diagnostics, "E0004"));
        assert!(has_message(&diagnostics, "non-exhaustive patterns"));
        assert!(has_message(&diagnostics, "None"));
        let diagnostics = expect_errors(
            r#"fn main() {
    let flag = true;
    match flag {
        false => println!("no"),
    }
}"#,
        );
        assert!(has_message(&diagnostics, "true"));
        let diagnostics = expect_errors(
            r#"enum Shape { Circle(i64), Empty }
fn main() {
    let shape = Shape::Empty;
    match shape {
        Shape::Circle(radius) => println!("{}", radius),
    }
}"#,
        );
        assert!(has_message(&diagnostics, "Empty"));
        expect_ok(
            r#"fn main() {
    let value: Option<i64> = None;
    match value {
        Some(number) => println!("{}", number),
        None => println!("none"),
    }
}"#,
        );
    }

    #[test]
    fn warns_about_unreachable_arms() {
        let diagnostics = analyze(
            r#"fn main() {
    let value = 1;
    match value {
        _ => println!("any"),
        1 => println!("one"),
    }
}"#,
        );
        assert!(!diagnostics.has_errors());
        assert!(has_warning(&diagnostics, "unreachable pattern"));
    }

    #[test]
    fn check_reporting_returns_warnings_on_success() {
        let (program, file, parse_diagnostics) = parse_source(
            r#"fn main() {
    let value = 1;
    match value {
        _ => println!("any"),
        1 => println!("one"),
    }
}"#,
        );
        assert!(
            !parse_diagnostics.has_errors(),
            "unexpected parse errors: {:?}",
            messages(&parse_diagnostics)
        );
        let (result, warnings) = check_reporting(&program, &file);
        match &result {
            Ok(_) => {}
            Err(diagnostics) => panic!(
                "expected the program to type check, got: {:?}",
                messages(diagnostics)
            ),
        }
        assert!(!warnings.has_errors());
        assert!(!warnings.is_empty());
        assert!(has_warning(&warnings, "unreachable pattern"));
        assert!(check(&program, &file).is_ok());
    }

    #[test]
    fn check_reporting_keeps_errors_and_has_no_warnings() {
        let (program, file, parse_diagnostics) = parse_source(
            r#"fn main() {
    let value: i64 = "text";
}"#,
        );
        assert!(
            !parse_diagnostics.has_errors(),
            "unexpected parse errors: {:?}",
            messages(&parse_diagnostics)
        );
        let (result, warnings) = check_reporting(&program, &file);
        match &result {
            Ok(_) => panic!("expected type errors, but the program type checked"),
            Err(diagnostics) => {
                assert!(diagnostics.has_errors());
                assert!(has_message(diagnostics, "mismatched types"));
            }
        }
        assert!(warnings.is_empty());
        assert!(check(&program, &file).is_err());
    }

    #[test]
    fn rejects_refutable_let_patterns() {
        let diagnostics = expect_errors(r#"fn main() { let Some(value) = Some(1); }"#);
        assert!(has_code(&diagnostics, "E0005"));
        assert!(has_message(&diagnostics, "refutable pattern"));
    }

    #[test]
    fn supports_enum_construction_and_patterns() {
        let program = expect_ok(
            r#"enum Message { Quit, Write(i64), Move(i64, i64) }
enum Color { Red, Blue }
fn describe(message: Message) -> i64 {
    match message {
        Message::Quit => 0,
        Message::Write(code) => code,
        Message::Move(x, y) => x + y,
    }
}
fn main() {
    println!("{}", describe(Message::Move(1, 2)));
    let color = Color::Red;
    match color {
        Red => println!("red"),
        Blue => println!("blue"),
    }
}"#,
        );
        assert_eq!(program.enums.len(), 2);
        assert_eq!(program.enums[0].variants.len(), 3);
        assert_eq!(program.enums[0].variants[2].fields.len(), 2);
        assert_eq!(program.enums[1].variants.len(), 2);
    }

    #[test]
    fn supports_constant_patterns() {
        expect_ok(
            r#"const TWO: i64 = 2;
fn main() {
    let value = 3;
    match value {
        TWO => println!("two"),
        _ => println!("other"),
    }
}"#,
        );
    }

    // -----------------------------------------------------------------
    // Constants
    // -----------------------------------------------------------------

    #[test]
    fn evaluates_constants() {
        let program = expect_ok(
            r#"const BASE: i64 = 2 * 3 + 1;
const MESSAGE: String = format!("value");
fn main() {
    println!("{} {}", BASE, MESSAGE);
}"#,
        );
        assert_eq!(program.consts.len(), 2);
        assert_eq!(program.consts[0].value, ConstValue::Int(7));
        assert_eq!(program.consts[0].ty, Ty::Int(IntTy::I64));
        assert_eq!(
            program.consts[1].value,
            ConstValue::Str("value".to_string())
        );
    }

    #[test]
    fn detects_constant_cycles() {
        let diagnostics = analyze(
            r#"const A: i64 = B;
const B: i64 = A;
fn main() { }"#,
        );
        assert!(has_code(&diagnostics, "E0391"));
        assert!(has_message(&diagnostics, "cycle detected"));
    }

    #[test]
    fn rejects_non_constant_initializers() {
        let diagnostics = analyze(
            r#"const NOW: i64 = args().len();
fn main() { }"#,
        );
        assert!(has_code(&diagnostics, "E0015"));
        assert!(has_message(
            &diagnostics,
            "constant initializer must be a constant expression"
        ));
    }

    // -----------------------------------------------------------------
    // Closures
    // -----------------------------------------------------------------

    #[test]
    fn captures_variables_in_closures() {
        let program = expect_ok(
            r#"fn main() {
    let base = 10;
    let add = |x: i64| x + base;
    println!("{}", add(5));
}"#,
        );
        assert_eq!(program.closures.len(), 1);
        let closure = &program.closures[0];
        assert_eq!(closure.captures.len(), 1);
        assert_eq!(closure.captures[0].name, "base");
        assert_eq!(closure.params.len(), 1);
        assert_eq!(closure.ret, Ty::Int(IntTy::I64));
        let body = &main_fn(&program).body;
        match &body.stmts[1] {
            HStmt::Let {
                init: Some(init), ..
            } => match &init.kind {
                HExprKind::Closure {
                    closure_id,
                    captures,
                } => {
                    assert_eq!(*closure_id, 0);
                    assert_eq!(captures.len(), 1);
                    assert!(matches!(captures[0].kind, HExprKind::Local { .. }));
                }
                _ => panic!("expected a closure expression"),
            },
            _ => panic!("expected a let statement"),
        }
    }

    #[test]
    fn captures_mutable_variables_in_closures() {
        expect_ok(
            r#"fn main() {
    let mut counter = 0;
    let mut bump = || { counter = counter + 1; };
    bump();
    bump();
    println!("{}", counter);
}"#,
        );
    }

    #[test]
    fn supports_function_values() {
        let program = expect_ok(
            r#"fn double(x: i64) -> i64 { x * 2 }
fn apply(function: fn(i64) -> i64, value: i64) -> i64 { function(value) }
fn main() {
    println!("{} {}", apply(double, 4), double(3));
}"#,
        );
        let apply = find_fn(&program, "apply").expect("apply should exist");
        assert_eq!(
            apply.params[0].ty,
            Ty::Fn(vec![Ty::Int(IntTy::I64)], Box::new(Ty::Int(IntTy::I64)))
        );
    }

    // -----------------------------------------------------------------
    // Builtins and macros
    // -----------------------------------------------------------------

    #[test]
    fn checks_builtin_methods() {
        expect_ok(
            r#"fn main() {
    let mut values: Vec<i64> = Vec::new();
    values.push(1);
    values.push(2);
    println!("{} {}", values.len(), values.first().unwrap_or(0));
    let text = String::from("hello");
    println!("{} {}", text.len(), text.contains("ell"));
    let parsed: Result<i64, String> = "42".parse_i64();
    println!("{}", parsed.unwrap_or(0));
    let turbofish: Result<f64, String> = "2.5".parse::<f64>();
    println!("{}", turbofish.unwrap_or(0.0));
    let maybe: Option<i64> = None;
    println!("{}", maybe.unwrap_or(3));
}"#,
        );
    }

    #[test]
    fn checks_format_macros() {
        let program = expect_ok(
            r#"fn main() {
    let name = "world";
    print!("hello ");
    println!("{}, {:?} {{}}", name, name);
    eprint!("x");
    eprintln!("y");
}"#,
        );
        let body = &main_fn(&program).body;
        let mut kinds = Vec::new();
        for stmt in &body.stmts {
            if let HStmt::Expr(expr) = stmt
                && let HExprKind::Macro(call) = &expr.kind
            {
                kinds.push(call.kind);
            }
        }
        assert!(kinds.contains(&crate::hir::HMacroKind::Print));
        assert!(kinds.contains(&crate::hir::HMacroKind::Println));
        assert!(kinds.contains(&crate::hir::HMacroKind::Eprint));
        assert!(kinds.contains(&crate::hir::HMacroKind::Eprintln));
        let diagnostics = expect_errors(r#"fn main() { println!("{}", 1, 2); }"#);
        assert!(has_code(&diagnostics, "E0001"));
        let diagnostics = expect_errors(r#"fn main() { println!("{}"); }"#);
        assert!(has_code(&diagnostics, "E0001"));
    }

    #[test]
    fn checks_assert_and_panic_macros() {
        expect_ok(
            r#"fn main() {
    assert!(true);
    assert_eq!(1 + 1, 2);
    if false { panic!("boom {}", 1); }
}"#,
        );
        let diagnostics = expect_errors("fn main() { assert_eq!(1, 2, 3); }");
        assert!(has_code(&diagnostics, "E0061"));
    }

    #[test]
    fn checks_io_builtins() {
        expect_ok(
            r#"fn main() {
    let line = read_line();
    match line {
        Some(text) => println!("{}", text),
        None => println!("none"),
    }
    let file = read_file("path");
    match file {
        Ok(contents) => println!("{}", contents),
        Err(error) => println!("{}", error),
    }
    let written = write_file("path", "data");
    match written {
        Ok(_) => println!("ok"),
        Err(error) => println!("{}", error),
    }
    let cli = args();
    println!("{}", cli.len());
}"#,
        );
    }

    // -----------------------------------------------------------------
    // Control flow lowering
    // -----------------------------------------------------------------

    #[test]
    fn lowers_control_flow() {
        let program = expect_ok(
            r#"fn main() {
    let mut total = 0;
    for index in 0..5 { total = total + index; }
    for index in 0..=3 { total = total + index; }
    let values = vec![1, 2, 3];
    for value in values { total = total + value; }
    let flag = true;
    if flag { total = total + 1; } else { total = total - 1; }
    while total > 0 { total = total - 1; }
    println!("{}", total);
}"#,
        );
        let body = &main_fn(&program).body;
        let mut for_ranges = 0;
        let mut for_vecs = 0;
        let mut ifs = 0;
        let mut whiles = 0;
        for stmt in &body.stmts {
            if let HStmt::Expr(expr) = stmt {
                match &expr.kind {
                    HExprKind::ForRange { .. } => for_ranges += 1,
                    HExprKind::ForVec { .. } => for_vecs += 1,
                    HExprKind::If { .. } => ifs += 1,
                    HExprKind::While { .. } => whiles += 1,
                    _ => {}
                }
            }
        }
        assert_eq!(for_ranges, 2);
        assert_eq!(for_vecs, 1);
        assert_eq!(ifs, 1);
        assert_eq!(whiles, 1);
    }

    #[test]
    fn lowers_short_circuit_operators_to_conditionals() {
        let program = expect_ok(
            r#"fn main() {
    let a = true;
    let b = false;
    let c = a && b;
    let d = a || b;
    println!("{} {}", c, d);
}"#,
        );
        let body = &main_fn(&program).body;
        for index in [2usize, 3usize] {
            match &body.stmts[index] {
                HStmt::Let {
                    init: Some(init), ..
                } => {
                    assert!(matches!(init.kind, HExprKind::If { .. }));
                }
                _ => panic!("expected a let statement with a conditional initializer"),
            }
        }
    }

    // -----------------------------------------------------------------
    // Program level validation
    // -----------------------------------------------------------------

    #[test]
    fn validates_main_signature() {
        let diagnostics = expect_errors("fn helper() { }");
        assert!(has_code(&diagnostics, "E0601"));
        assert!(has_message(&diagnostics, "main function not found"));
        let diagnostics = expect_errors("fn main(value: i64) { }");
        assert!(has_code(&diagnostics, "E0580"));
        assert!(has_message(&diagnostics, "must not take parameters"));
        let diagnostics = expect_errors("fn main() -> i64 { 1 }");
        assert!(has_code(&diagnostics, "E0580"));
        assert!(has_message(&diagnostics, "must return ()"));
    }

    #[test]
    fn detects_recursive_types() {
        let diagnostics = analyze(
            r#"struct Node { next: Node }
fn main() { }"#,
        );
        assert!(has_message(&diagnostics, "recursive type"));
        assert!(has_message(&diagnostics, "infinite size"));
        let diagnostics = analyze(
            r#"type A = B;
type B = A;
fn main() { }"#,
        );
        assert!(has_message(
            &diagnostics,
            "cycle detected when expanding type alias"
        ));
    }

    #[test]
    fn supports_shadowing_and_block_scopes() {
        let program = expect_ok(
            r#"fn main() {
    let value = 1;
    {
        let value = "shadowed";
        println!("{}", value);
    }
    println!("{}", value);
}"#,
        );
        let body = &main_fn(&program).body;
        assert_eq!(body.stmts.len(), 3);
    }

    // -----------------------------------------------------------------
    // End-to-end fixture corpus
    // -----------------------------------------------------------------

    #[test]
    fn accepts_fixture_programs() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs");
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .expect("fixture directory should exist")
            .map(|entry| entry.expect("fixture entry").path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("ls"))
            .collect();
        paths.sort();
        assert!(
            paths.len() >= 20,
            "expected the e2e fixture corpus, found {} files",
            paths.len()
        );
        for path in paths {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .to_string();
            let source = std::fs::read_to_string(&path).expect("fixture should be readable");
            let file = SourceFile::new(name.clone(), &source);
            let (tokens, lex_diagnostics) = crate::lexer::tokenize(&source);
            let (program, parse_diagnostics) = crate::parser::parse(&tokens, &file);
            let rejected_at_parse = lex_diagnostics.has_errors() || parse_diagnostics.has_errors();
            if name.starts_with("bad_") {
                if rejected_at_parse {
                    continue;
                }
                let accepted = check(&program, &file).is_ok();
                assert!(!accepted, "{} should be rejected by the type checker", name);
                continue;
            }
            let mut parse_messages = messages(&lex_diagnostics);
            parse_messages.extend(messages(&parse_diagnostics));
            assert!(
                !rejected_at_parse,
                "{} should parse cleanly: {:?}",
                name, parse_messages
            );
            if let Err(diagnostics) = check(&program, &file) {
                panic!(
                    "{} should type check, got: {:?}",
                    name,
                    messages(&diagnostics)
                );
            }
        }
    }
}
