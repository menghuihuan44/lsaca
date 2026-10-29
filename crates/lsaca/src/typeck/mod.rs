//! Semantic analysis: name resolution, type inference, type checking,
//! pattern exhaustiveness and lowering to the HIR.

mod builtins;
mod check;
mod consts;
mod exhaust;
mod exprs;
mod generics;
mod hash_map;
mod items;
mod iterators;
mod itype;
mod lower;
mod ownership;
mod patterns;
mod refs;
mod traits;
mod tree;

use std::path::Path;

use crate::diag::{Diagnostic, Diagnostics, Severity, Span, sort_and_dedup};
use crate::span::SourceFile;
use crate::{ast, hir};

use self::check::Checker;
use self::generics::DefOrigin;
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
    check_reporting_with_prelude(program, None, file)
}

/// Type check a user program together with the standard-library prelude.
///
/// The prelude (when present) is registered as generic items only: it is never
/// type checked unless a user expression instantiates something from it, and
/// unused prelude items never reach the HIR. Diagnostics produced while
/// checking an instantiated prelude body are reported as internal
/// standard-library errors.
pub fn check_with_prelude(
    program: &ast::Program,
    prelude: Option<&ast::Program>,
    file: &SourceFile,
) -> Result<hir::Program, Diagnostics> {
    check_reporting_with_prelude(program, prelude, file).0
}

/// Like [check_with_prelude] but also returns the warnings of a successful
/// check.
pub fn check_reporting_with_prelude(
    program: &ast::Program,
    prelude: Option<&ast::Program>,
    file: &SourceFile,
) -> (Result<hir::Program, Diagnostics>, Diagnostics) {
    let mut checker = Checker::new(file);
    checker.collect_items_only(program, DefOrigin::User);
    if let Some(prelude) = prelude {
        checker.collect_items_only(prelude, DefOrigin::Prelude);
    }
    checker.register_pending_impls();
    checker.resolve_all_types();
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
    // The borrow checker and the move checker are read-only post-lowering
    // passes: they validate the fully typed HIR and never modify it.
    let mut post_diagnostics = crate::borrow::check(&program);
    post_diagnostics.extend(crate::moveck::check(&program));
    if post_diagnostics.has_errors() {
        let mut reported = post_diagnostics;
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
    use crate::hir::{ConstValue, HBuiltinMethod, HExpr, HExprKind, HPatKind, HStmt};
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

    fn check_source_with_prelude(source: &str) -> Result<hir::Program, Diagnostics> {
        let (program, file, diagnostics) = parse_source(source);
        assert!(
            !diagnostics.has_errors(),
            "unexpected parse errors: {:?}",
            messages(&diagnostics)
        );
        let prelude = crate::prelude::program().expect("the embedded prelude must parse");
        check_with_prelude(&program, Some(prelude), &file)
    }

    fn expect_ok_with_prelude(source: &str) -> hir::Program {
        match check_source_with_prelude(source) {
            Ok(program) => program,
            Err(diagnostics) => panic!(
                "expected the program to type check, got: {:?}",
                messages(&diagnostics)
            ),
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
        checker.collect_items_only(&program, DefOrigin::User);
        checker.register_pending_impls();
        checker.resolve_all_types();
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

    fn has_note(diagnostics: &Diagnostics, needle: &str) -> bool {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.notes.iter().any(|note| note.contains(needle)))
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
    fn conditions_must_have_type_bool() {
        // An annotated integer is rejected.
        let diagnostics =
            expect_errors(r#"fn main() { let n: i64 = 3; if n { println!("yes"); } }"#);
        assert!(has_code(&diagnostics, "E0308"));
        assert!(has_note(&diagnostics, "expected bool, found i64"));
        // So is an unannotated binding whose type is still an integer
        // inference variable, and a bare numeric literal.
        let diagnostics = expect_errors("fn main() { let n = 3; if n { } }");
        assert!(has_code(&diagnostics, "E0308"));
        assert!(has_note(&diagnostics, "expected bool, found {integer}"));
        let diagnostics = expect_errors("fn main() { while 3 { } }");
        assert!(has_code(&diagnostics, "E0308"));
        // Logical operators and match guards use the same rule.
        let diagnostics = expect_errors("fn main() { let n = 1; let x = n && true; }");
        assert!(has_code(&diagnostics, "E0308"));
        let diagnostics = expect_errors(
            r#"fn main() {
    let n = 4;
    let result = match n { n if n => 1, _ => 0 };
    println!("{}", result);
}"#,
        );
        assert!(has_code(&diagnostics, "E0308"));
        // Genuine boolean conditions still type check, including one
        // whose type is inferred as `bool`.
        expect_ok(
            r#"fn main() {
    let flag = true;
    if flag { println!("yes"); }
    let mut n = 0;
    while n < 3 { n += 1; }
    match flag { b if b => println!("{}", b), _ => { } }
}"#,
        );
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
        // User structs get a derived field-wise == in v1.5 (DESIGN §5.4).
        expect_ok(
            r#"struct P { x: i64 }
fn main() {
    let a = P { x: 1 };
    let b = P { x: 2 };
    let equal = a == b;
    println!("{}", equal);
}"#,
        );
        // Function values have no derived equality.
        let diagnostics = expect_errors(
            r#"fn add(value: i64) -> i64 { return value + 1; }
fn main() {
    let first = add;
    let second = add;
    let equal = first == second;
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

    #[test]
    fn lowers_try_operator_on_option() {
        let program = expect_ok(
            r#"fn first(values: Vec<i64>) -> Option<i64> {
    let value = values.first()?;
    Some(value + 1)
}

fn main() {
    println!("{}", first(vec![1, 2]).unwrap());
}"#,
        );
        let function = find_fn(&program, "first").expect("first should be lowered");
        assert_eq!(function.ret, Ty::Option(Box::new(Ty::Int(IntTy::I64))));
        match &function.body.stmts[0] {
            HStmt::Let {
                init: Some(init), ..
            } => {
                assert_eq!(init.ty, Ty::Int(IntTy::I64));
                match &init.kind {
                    HExprKind::Match { arms, .. } => {
                        assert_eq!(arms.len(), 2);
                        assert!(matches!(arms[0].pat.kind, HPatKind::OptionSome(_)));
                        assert!(matches!(arms[1].pat.kind, HPatKind::OptionNone));
                        assert!(matches!(
                            &arms[1].body.kind,
                            HExprKind::Return(Some(value))
                                if matches!(value.kind, HExprKind::OptionNone)
                        ));
                    }
                    other => panic!("expected the lowered try match, got {:?}", other),
                }
            }
            other => panic!("expected a let statement, got {:?}", other),
        }
    }

    #[test]
    fn lowers_try_operator_on_result() {
        let program = expect_ok(
            r#"fn parse_plus(text: String) -> Result<i64, String> {
    let value = text.parse_i64()?;
    Ok(value + 1)
}

fn main() {
    println!("{}", parse_plus("1").unwrap());
}"#,
        );
        let function = find_fn(&program, "parse_plus").expect("parse_plus should be lowered");
        assert_eq!(
            function.ret,
            Ty::Result(Box::new(Ty::Int(IntTy::I64)), Box::new(Ty::String))
        );
        match &function.body.stmts[0] {
            HStmt::Let {
                init: Some(init), ..
            } => match &init.kind {
                HExprKind::Match { arms, .. } => {
                    assert_eq!(arms.len(), 2);
                    assert!(matches!(arms[0].pat.kind, HPatKind::ResultOk(_)));
                    assert!(matches!(arms[1].pat.kind, HPatKind::ResultErr(_)));
                    assert!(matches!(
                        &arms[1].body.kind,
                        HExprKind::Return(Some(value))
                            if matches!(value.kind, HExprKind::ResultErr(_))
                    ));
                }
                other => panic!("expected the lowered try match, got {:?}", other),
            },
            other => panic!("expected a let statement, got {:?}", other),
        }
    }

    #[test]
    fn checks_try_operator_payload_types() {
        let diagnostics = expect_errors(
            r#"fn first(opt: Option<i64>) -> Option<String> {
    let value = opt?;
    Some(value)
}

fn main() {
    let _ = first(Some(1));
}"#,
        );
        assert!(has_code(&diagnostics, "E0308"));
        assert!(has_message(&diagnostics, "mismatched types"));

        let diagnostics = expect_errors(
            r#"fn convert(res: Result<i64, i64>) -> Result<i64, String> {
    let value = res?;
    Ok(value)
}

fn main() {
    let _ = convert(Ok(1));
}"#,
        );
        assert!(has_code(&diagnostics, "E0308"));
        assert!(has_message(&diagnostics, "cannot convert error types"));
    }

    #[test]
    fn reports_try_operator_scope_errors() {
        let diagnostics = expect_errors(
            r#"fn maybe(value: i64) -> Option<i64> {
    Some(value)
}

fn main() {
    let value = maybe(1)?;
    println!("{}", value);
}"#,
        );
        assert!(has_code(&diagnostics, "E0277"));
        assert!(has_message(
            &diagnostics,
            "can only be used in a function that returns Option"
        ));
        assert!(has_note(
            &diagnostics,
            "change the function to return Option"
        ));

        let diagnostics = expect_errors(
            r#"fn compute(value: i64) -> Option<i64> {
    let number = value?;
    Some(number)
}

fn main() {
    let _ = compute(1);
}"#,
        );
        assert!(has_code(&diagnostics, "E0277"));
        assert!(has_message(
            &diagnostics,
            "can only be applied to Option or Result values"
        ));
    }

    #[test]
    fn checks_try_operator_inside_closures() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let opt: Option<i64> = Some(1);
    let bump = || {
        let value = opt?;
        value + 1
    };
    println!("{}", bump());
}"#,
        );
        assert!(has_code(&diagnostics, "E0277"));
        assert!(has_message(
            &diagnostics,
            "requires the enclosing closure to declare an Option return type"
        ));

        let program = expect_ok(
            r#"fn first(opt: Option<i64>) -> i64 {
    let bump = |value: Option<i64>| -> Option<i64> {
        let inner = value?;
        Some(inner + 1)
    };
    bump(opt).unwrap()
}

fn main() {
    println!("{}", first(Some(41)));
}"#,
        );
        assert_eq!(program.closures.len(), 1);
        let closure = &program.closures[0];
        assert_eq!(closure.ret, Ty::Option(Box::new(Ty::Int(IntTy::I64))));
        let inner_block = match closure.body.tail.as_deref() {
            Some(HExpr {
                kind: HExprKind::Block(block),
                ..
            }) => block,
            other => panic!("expected the closure block, got {:?}", other),
        };
        match &inner_block.stmts[0] {
            HStmt::Let {
                init: Some(init), ..
            } => assert!(matches!(init.kind, HExprKind::Match { .. })),
            other => panic!("expected the lowered try match, got {:?}", other),
        }
    }

    #[test]
    fn try_operator_chains_through_calls_and_methods() {
        let program = expect_ok(
            r#"struct Holder {
    items: Vec<i64>,
}

impl Holder {
    fn second(&self) -> Option<i64> {
        let first = self.items.first()?;
        let second = self.items.get(1)?;
        Some(first + second)
    }
}

fn main() {
    let holder = Holder { items: vec![1, 2] };
    println!("{}", holder.second().unwrap());
}"#,
        );
        let method = find_fn(&program, "Holder::second").expect("Holder::second should exist");
        assert_eq!(method.ret, Ty::Option(Box::new(Ty::Int(IntTy::I64))));
        assert_eq!(method.body.stmts.len(), 2);
        for stmt in &method.body.stmts {
            match stmt {
                HStmt::Let {
                    init: Some(init), ..
                } => assert!(matches!(init.kind, HExprKind::Match { .. })),
                other => panic!("expected a lowered try statement, got {:?}", other),
            }
        }
    }

    #[test]
    fn lowers_option_map_combinator() {
        let program = expect_ok(
            r#"fn main() {
    let mapped = Some(4).map(|value| value + 1);
    println!("{}", mapped.unwrap_or(0));
}"#,
        );
        let body = &main_fn(&program).body;
        assert_eq!(body.stmts.len(), 2);
        let init = match &body.stmts[0] {
            HStmt::Let {
                init: Some(init), ..
            } => init,
            other => panic!("expected a let statement, got {:?}", other),
        };
        assert_eq!(init.ty, Ty::Option(Box::new(Ty::Int(IntTy::I64))));
        match &init.kind {
            HExprKind::Match { scrutinee, arms } => {
                assert!(matches!(&scrutinee.kind, HExprKind::OptionSome(_)));
                assert_eq!(arms.len(), 2);
                assert!(matches!(&arms[0].pat.kind, HPatKind::OptionSome(_)));
                assert!(matches!(&arms[1].pat.kind, HPatKind::OptionNone));
                match &arms[0].body.kind {
                    HExprKind::OptionSome(inner) => {
                        assert!(matches!(&inner.kind, HExprKind::Call { .. }))
                    }
                    other => panic!("expected Some(call), got {:?}", other),
                }
                assert!(matches!(&arms[1].body.kind, HExprKind::OptionNone));
            }
            other => panic!("expected the lowered map match, got {:?}", other),
        }
    }

    #[test]
    fn lowers_option_map_or_combinator() {
        let program = expect_ok(
            r#"fn main() {
    let total = Some(2).map_or(7, |value| value * 3);
    println!("{}", total);
}"#,
        );
        let body = &main_fn(&program).body;
        let init = match &body.stmts[0] {
            HStmt::Let {
                init: Some(init), ..
            } => init,
            other => panic!("expected a let statement, got {:?}", other),
        };
        assert_eq!(init.ty, Ty::Int(IntTy::I64));
        match &init.kind {
            HExprKind::Block(block) => {
                assert_eq!(block.stmts.len(), 2);
                let tail = block.tail.as_deref().expect("map_or block needs a tail");
                match &tail.kind {
                    HExprKind::Match { arms, .. } => {
                        assert_eq!(arms.len(), 2);
                        assert!(matches!(&arms[0].body.kind, HExprKind::Call { .. }));
                        assert!(matches!(
                            &arms[1].body.kind,
                            HExprKind::Local { name, .. } if name == "fallback"
                        ));
                    }
                    other => panic!("expected the map_or match, got {:?}", other),
                }
            }
            other => panic!("expected a block expression, got {:?}", other),
        }
    }

    #[test]
    fn lowers_option_and_then_and_filter() {
        let program = expect_ok(
            r#"fn main() {
    let chained = Some(3).and_then(|value| {
        if value > 0 { Some(value * 2) } else { None }
    });
    let filtered = chained.filter(|value| *value > 4);
    println!("{}", filtered.unwrap_or(0));
}"#,
        );
        let body = &main_fn(&program).body;
        let chained_init = match &body.stmts[0] {
            HStmt::Let {
                init: Some(init), ..
            } => init,
            other => panic!("expected a let statement, got {:?}", other),
        };
        match &chained_init.kind {
            HExprKind::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
                assert!(matches!(&arms[0].body.kind, HExprKind::Call { .. }));
                assert!(matches!(&arms[1].body.kind, HExprKind::OptionNone));
            }
            other => panic!("expected the and_then match, got {:?}", other),
        }
        let filter_init = match &body.stmts[1] {
            HStmt::Let {
                init: Some(init), ..
            } => init,
            other => panic!("expected a let statement, got {:?}", other),
        };
        match &filter_init.kind {
            HExprKind::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
                match &arms[0].body.kind {
                    HExprKind::Block(block) => {
                        assert_eq!(block.stmts.len(), 1);
                        match &block.stmts[0] {
                            HStmt::Let {
                                pat,
                                init: Some(init),
                                ..
                            } => {
                                assert!(matches!(&pat.kind, HPatKind::Bind { .. }));
                                assert!(matches!(
                                    &init.kind,
                                    HExprKind::Ref { mutable: false, .. }
                                ));
                            }
                            other => panic!("expected the borrow statement, got {:?}", other),
                        }
                        let tail = block.tail.as_deref().expect("filter arm needs a value");
                        match &tail.kind {
                            HExprKind::If {
                                cond,
                                then_block,
                                else_expr,
                            } => {
                                assert!(matches!(&cond.kind, HExprKind::Call { .. }));
                                let then_tail =
                                    then_block.tail.as_deref().expect("if needs a then value");
                                assert!(matches!(&then_tail.kind, HExprKind::OptionSome(_)));
                                let else_expr =
                                    else_expr.as_deref().expect("if needs an else value");
                                assert!(matches!(&else_expr.kind, HExprKind::OptionNone));
                            }
                            other => {
                                panic!("expected a conditional in the filter arm, got {:?}", other)
                            }
                        }
                    }
                    other => panic!("expected a block in the filter arm, got {:?}", other),
                }
            }
            other => panic!("expected the filter match, got {:?}", other),
        }
        assert_eq!(filter_init.ty, Ty::Option(Box::new(Ty::Int(IntTy::I64))));
    }

    #[test]
    fn lowers_option_ok_or_combinator() {
        let program = expect_ok(
            r#"fn main() {
    let converted = Some(5).ok_or("missing");
    match converted {
        Ok(value) => println!("ok {}", value),
        Err(error) => println!("err {}", error),
    }
}"#,
        );
        let body = &main_fn(&program).body;
        let init = match &body.stmts[0] {
            HStmt::Let {
                init: Some(init), ..
            } => init,
            other => panic!("expected a let statement, got {:?}", other),
        };
        assert_eq!(
            init.ty,
            Ty::Result(Box::new(Ty::Int(IntTy::I64)), Box::new(Ty::String))
        );
        match &init.kind {
            HExprKind::Block(block) => {
                let tail = block.tail.as_deref().expect("ok_or block needs a tail");
                match &tail.kind {
                    HExprKind::Match { arms, .. } => {
                        assert_eq!(arms.len(), 2);
                        assert!(matches!(&arms[0].pat.kind, HPatKind::OptionSome(_)));
                        assert!(matches!(&arms[1].pat.kind, HPatKind::OptionNone));
                        assert!(matches!(&arms[0].body.kind, HExprKind::ResultOk(_)));
                        assert!(matches!(&arms[1].body.kind, HExprKind::ResultErr(_)));
                    }
                    other => panic!("expected the ok_or match, got {:?}", other),
                }
            }
            other => panic!("expected a block expression, got {:?}", other),
        }
    }

    #[test]
    fn lowers_result_map_and_map_err() {
        let program = expect_ok(
            r#"fn compute() -> Result<i64, String> {
    Ok(5)
}

fn main() {
    let mapped = compute().map(|value| value + 1);
    let relabeled = compute().map_err(|error| error + "!");
    println!("{}", mapped.unwrap_or(0));
    println!("{}", relabeled.is_err());
}"#,
        );
        let body = &main_fn(&program).body;
        let map_init = match &body.stmts[0] {
            HStmt::Let {
                init: Some(init), ..
            } => init,
            other => panic!("expected a let statement, got {:?}", other),
        };
        match &map_init.kind {
            HExprKind::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
                assert!(matches!(&arms[0].pat.kind, HPatKind::ResultOk(_)));
                assert!(matches!(&arms[1].pat.kind, HPatKind::ResultErr(_)));
                assert!(matches!(&arms[0].body.kind, HExprKind::ResultOk(_)));
                assert!(matches!(&arms[1].body.kind, HExprKind::ResultErr(_)));
            }
            other => panic!("expected the Result map match, got {:?}", other),
        }
        let map_err_init = match &body.stmts[1] {
            HStmt::Let {
                init: Some(init), ..
            } => init,
            other => panic!("expected a let statement, got {:?}", other),
        };
        match &map_err_init.kind {
            HExprKind::Match { arms, .. } => {
                assert!(matches!(&arms[0].body.kind, HExprKind::ResultOk(_)));
                match &arms[1].body.kind {
                    HExprKind::ResultErr(inner) => {
                        assert!(matches!(&inner.kind, HExprKind::Call { .. }))
                    }
                    other => panic!("expected Err(call), got {:?}", other),
                }
            }
            other => panic!("expected the map_err match, got {:?}", other),
        }
    }

    #[test]
    fn rejects_combinator_misuse() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let mapped = Some(1).map();
    println!("{}", mapped.unwrap_or(0));
}"#,
        );
        assert!(has_code(&diagnostics, "E0061"));
        assert!(has_message(
            &diagnostics,
            "the method map takes 1 argument(s) but 0"
        ));

        let diagnostics = expect_errors(
            r#"fn main() {
    let mapped = Some(1).map_or(0);
    println!("{}", mapped);
}"#,
        );
        assert!(has_code(&diagnostics, "E0061"));

        let diagnostics = expect_errors(
            r#"fn main() {
    let base: Result<i64, String> = Ok(1);
    let mapped = base.map_or(0);
    println!("{}", mapped);
}"#,
        );
        assert!(has_code(&diagnostics, "E0599"));
        assert!(has_message(&diagnostics, "no method named map_or"));

        let diagnostics = expect_errors(
            r#"fn main() {
    let bad = Some(1).filter(2);
    println!("{}", bad.is_some());
}"#,
        );
        assert!(has_code(&diagnostics, "E0308"));
    }

    #[test]
    fn infers_combinator_types_through_parse_calls() {
        let program = expect_ok(
            r#"fn main() {
    let doubled = "21".parse_i64().map(|value| value * 2).unwrap_or(0);
    println!("{}", doubled);
}"#,
        );
        let body = &main_fn(&program).body;
        let init = match &body.stmts[0] {
            HStmt::Let {
                init: Some(init), ..
            } => init,
            other => panic!("expected a let statement, got {:?}", other),
        };
        assert_eq!(init.ty, Ty::Int(IntTy::I64));
        assert!(matches!(&init.kind, HExprKind::BuiltinMethod { .. }));
    }

    #[test]
    fn user_methods_take_precedence_over_combinators() {
        let program = expect_ok(
            r#"struct Counter {
    value: i64,
}

impl Counter {
    fn map(&self) -> i64 {
        self.value + 1
    }
}

fn main() {
    let counter = Counter { value: 4 };
    let mapped = counter.map();
    println!("{}", mapped);
}"#,
        );
        let body = &main_fn(&program).body;
        let init = match &body.stmts[1] {
            HStmt::Let {
                init: Some(init), ..
            } => init,
            other => panic!("expected a let statement, got {:?}", other),
        };
        match &init.kind {
            HExprKind::DirectCall { receiver, .. } => {
                assert!(
                    matches!(receiver.as_deref(), Some(HExpr { kind: HExprKind::Local { name, .. }, .. }) if name == "counter")
                );
            }
            other => panic!("expected a direct method call, got {:?}", other),
        }
    }
    // -----------------------------------------------------------------
    // References (v1.3)
    // -----------------------------------------------------------------

    fn let_init(block: &hir::HBlock, index: usize) -> &HExpr {
        match block.stmts.get(index) {
            Some(HStmt::Let {
                init: Some(init), ..
            }) => init,
            other => panic!(
                "expected a let statement with an initializer, got {:?}",
                other
            ),
        }
    }

    fn shared_ref(inner: Ty) -> Ty {
        Ty::Ref {
            mutable: false,
            inner: Box::new(inner),
        }
    }

    fn mutable_ref(inner: Ty) -> Ty {
        Ty::Ref {
            mutable: true,
            inner: Box::new(inner),
        }
    }

    // ----- borrowing and dereferencing -----

    #[test]
    fn lifetime_diagnostics_use_source_names() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let reference = {
        let value = 5;
        &value
    };
    println!("{}", *reference);
}"#,
        );
        let rendered = messages(&diagnostics).join(" | ");
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == Some("E0597"))
                .count(),
            1,
            "{}",
            rendered
        );
        assert!(
            has_message(&diagnostics, "value does not live long enough"),
            "{}",
            rendered
        );
        for message in messages(&diagnostics) {
            for token in message.split_whitespace() {
                let looks_like_c_name = token.len() > 1
                    && (token.starts_with('v') || token.starts_with('p') || token.starts_with('t'))
                    && token[1..].chars().all(|digit| digit.is_ascii_digit());
                assert!(
                    !looks_like_c_name,
                    "diagnostic must use source names, got: {}",
                    message
                );
            }
        }
    }

    #[test]
    fn bindings_of_references_keep_their_reference_types() {
        // Match ergonomics must not peel a plain binding: a binding initialised
        // with a reference keeps that reference as its own type.
        let program = expect_ok(
            r#"fn main() {
    let value = 10;
    let first = &value;
    let second = first;
    let nested = &second;
    println!("{} {}", *first, **nested);
}"#,
        );
        let main = main_fn(&program);
        let expected = shared_ref(shared_ref(Ty::Int(IntTy::I64)));
        match &main.body.stmts[3] {
            HStmt::Let {
                pat,
                init: Some(init),
                ..
            } => {
                assert_eq!(pat.ty, expected);
                assert_eq!(init.ty, expected);
            }
            other => panic!("expected let nested = &second, got {:?}", other),
        }
        assert_eq!(let_init(&main.body, 2).ty, shared_ref(Ty::Int(IntTy::I64)));
    }

    #[test]
    fn accepts_shared_borrows_and_reads_through_deref() {
        let program = expect_ok(
            r#"fn main() {
    let value = 5;
    let reference = &value;
    let copy = *reference;
    println!("{}", copy);
}"#,
        );
        let body = &main_fn(&program).body;
        let reference = let_init(body, 1);
        assert_eq!(reference.ty, shared_ref(Ty::Int(IntTy::I64)));
        match &reference.kind {
            HExprKind::Ref { mutable, expr } => {
                assert!(!*mutable);
                assert!(matches!(&expr.kind, HExprKind::Local { name, .. } if name == "value"));
            }
            other => panic!("expected a borrow expression, got {:?}", other),
        }
        let copy = let_init(body, 2);
        assert_eq!(copy.ty, Ty::Int(IntTy::I64));
        assert!(matches!(
            &copy.kind,
            HExprKind::Deref { expr }
                if matches!(&expr.kind, HExprKind::Local { name, .. } if name == "reference")
        ));
    }

    #[test]
    fn writes_through_mutable_borrows() {
        let program = expect_ok(
            r#"fn main() {
    let mut value = 1;
    let reference = &mut value;
    *reference = *reference + 1;
    println!("{}", value);
}"#,
        );
        let body = &main_fn(&program).body;
        let reference = let_init(body, 1);
        assert_eq!(reference.ty, mutable_ref(Ty::Int(IntTy::I64)));
        match body.stmts.get(2) {
            Some(HStmt::Expr(HExpr {
                kind: HExprKind::Assign { target, .. },
                ..
            })) => assert!(matches!(&target.kind, HExprKind::Deref { .. })),
            other => panic!("expected an assignment, got {:?}", other),
        }
    }

    #[test]
    fn mutable_parameters_write_through_references() {
        let program = expect_ok(
            r#"fn bump(value: &mut i64) {
    *value = *value + 1;
}

fn main() {
    let mut count = 1;
    bump(&mut count);
    println!("{}", count);
}"#,
        );
        let bump = find_fn(&program, "bump").expect("bump should be lowered");
        assert_eq!(bump.params[0].ty, mutable_ref(Ty::Int(IntTy::I64)));
        match bump.body.stmts.first() {
            Some(HStmt::Expr(HExpr {
                kind: HExprKind::Assign { target, .. },
                ..
            })) => assert!(matches!(&target.kind, HExprKind::Deref { .. })),
            other => panic!("expected an assignment, got {:?}", other),
        }
    }

    #[test]
    fn allows_reborrowing_through_a_mutable_parameter() {
        let program = expect_ok(
            r#"fn increment(value: &mut i64) {
    let inner = &mut *value;
    *inner = *inner + 1;
}

fn main() {
    let mut count = 1;
    increment(&mut count);
    println!("{}", count);
}"#,
        );
        let increment = find_fn(&program, "increment").expect("increment should be lowered");
        assert_eq!(increment.params[0].ty, mutable_ref(Ty::Int(IntTy::I64)));
    }

    #[test]
    fn allows_forwarding_reference_parameters() {
        let program = expect_ok(
            r#"fn forward(value: &i64) -> &i64 {
    value
}

fn main() {
    let number = 3;
    println!("{}", *forward(&number));
}"#,
        );
        assert!(find_fn(&program, "forward").is_some());
    }

    #[test]
    fn allows_returning_a_mutable_reference_parameter() {
        let program = expect_ok(
            r#"fn passthrough(value: &mut i64) -> &mut i64 {
    value
}

fn main() {
    let mut number = 1;
    let reference = passthrough(&mut number);
    *reference = 2;
    println!("{}", number);
}"#,
        );
        assert!(find_fn(&program, "passthrough").is_some());
    }

    #[test]
    fn auto_derefs_fields_through_multiple_reference_levels() {
        let program = expect_ok(
            r#"struct Point {
    x: i64,
}

fn read(point: &&Point) -> i64 {
    point.x
}

fn main() {
    let point = Point { x: 1 };
    let reference = &point;
    println!("{}", read(&reference));
}"#,
        );
        assert!(find_fn(&program, "read").is_some());
    }

    // ----- auto-deref for fields, methods and indexing (R4) -----

    #[test]
    fn auto_derefs_field_access_and_method_receivers() {
        let program = expect_ok(
            r#"struct Point {
    x: i64,
    y: i64,
}

impl Point {
    fn sum(&self) -> i64 {
        self.x + self.y
    }
}

fn distance(point: &Point) -> i64 {
    point.x + point.y + point.sum()
}

fn main() {
    let point = Point { x: 1, y: 2 };
    println!("{}", distance(&point));
}"#,
        );
        let distance = find_fn(&program, "distance").expect("distance should be lowered");
        match &distance.body.tail {
            Some(tail) => match &tail.kind {
                HExprKind::Binary { rhs, .. } => match &rhs.kind {
                    HExprKind::DirectCall {
                        fn_id, receiver, ..
                    } => {
                        assert_eq!(program.fns[*fn_id].name, "Point::sum");
                        match receiver.as_deref() {
                            Some(HExpr {
                                kind: HExprKind::Deref { expr },
                                ..
                            }) => assert!(
                                matches!(&expr.kind, HExprKind::Local { name, .. } if name == "point")
                            ),
                            other => panic!("expected a deref receiver, got {:?}", other),
                        }
                    }
                    other => panic!("expected a method call, got {:?}", other),
                },
                other => panic!("expected an addition, got {:?}", other),
            },
            None => panic!("distance should have a tail expression"),
        }
    }

    #[test]
    fn auto_derefs_fields_through_rc_and_box_references() {
        let program = expect_ok(
            r#"struct Node {
    value: i64,
}

fn read_shared(node: &Rc<Node>) -> i64 {
    node.value
}

fn read_owned(node: &Box<Node>) -> i64 {
    node.value
}

fn main() {
    let shared = Rc::new(Node { value: 7 });
    let owned = Box::new(Node { value: 8 });
    println!("{} {}", read_shared(&shared), read_owned(&owned));
}"#,
        );
        assert!(find_fn(&program, "read_shared").is_some());
        assert!(find_fn(&program, "read_owned").is_some());
    }

    #[test]
    fn auto_derefs_len_and_index_on_reference_vectors() {
        let program = expect_ok(
            r#"fn sum(values: &Vec<i64>) -> i64 {
    let mut total = 0;
    for index in 0..values.len() {
        total = total + values[index];
    }
    total
}

fn main() {
    let values = vec![1, 2, 3];
    println!("{}", sum(&values));
}"#,
        );
        assert!(find_fn(&program, "sum").is_some());
    }

    #[test]
    fn auto_derefs_multiple_reference_levels_for_methods() {
        let program = expect_ok(
            r#"fn length(values: &&Vec<i64>) -> i64 {
    values.len()
}

fn main() {
    let values = vec![1, 2, 3];
    let reference = &values;
    println!("{}", length(&reference));
}"#,
        );
        assert!(find_fn(&program, "length").is_some());
    }

    #[test]
    fn auto_derefs_mutable_reference_receivers() {
        let program = expect_ok(
            r#"struct Counter {
    value: i64,
}

impl Counter {
    fn increment(&mut self) {
        self.value = self.value + 1;
    }
}

fn main() {
    let mut counter = Counter { value: 0 };
    counter.increment();
    println!("{}", counter.value);
}"#,
        );
        assert!(find_fn(&program, "Counter::increment").is_some());
    }

    // ----- coercions (R5) -----

    #[test]
    fn coerces_mutable_borrows_to_shared_borrows_at_arguments() {
        let program = expect_ok(
            r#"fn read(value: &i64) -> i64 {
    *value
}

fn main() {
    let mut value = 1;
    let copy = read(&mut value);
    println!("{}", copy);
}"#,
        );
        let body = &main_fn(&program).body;
        let copy = let_init(body, 1);
        match &copy.kind {
            HExprKind::DirectCall { args, .. } => {
                assert_eq!(args.len(), 1);
                match &args[0].kind {
                    HExprKind::Ref {
                        mutable: false,
                        expr,
                    } => {
                        assert!(matches!(&expr.kind, HExprKind::Ref { mutable: true, .. }));
                    }
                    other => panic!("expected a coercion wrapper, got {:?}", other),
                }
            }
            other => panic!("expected a call, got {:?}", other),
        }
    }

    #[test]
    fn coerces_mutable_borrows_to_shared_borrows_at_returns() {
        let program = expect_ok(
            r#"fn as_shared(value: &mut i64) -> &i64 {
    value
}

fn main() {
    let mut number = 1;
    println!("{}", *as_shared(&mut number));
}"#,
        );
        let as_shared = find_fn(&program, "as_shared").expect("as_shared should be lowered");
        match &as_shared.body.tail {
            Some(tail) => assert!(matches!(&tail.kind, HExprKind::Ref { mutable: false, .. })),
            None => panic!("as_shared should have a tail expression"),
        }
    }

    #[test]
    fn coerces_rc_and_box_borrows_to_payload_references() {
        let program = expect_ok(
            r#"fn read(value: &i64) -> i64 {
    *value
}

fn main() {
    let shared = Rc::new(5);
    let owned = Box::new(7);
    println!("{} {}", read(&shared), read(&owned));
}"#,
        );
        assert!(find_fn(&program, "read").is_some());
    }

    // ----- match ergonomics -----

    #[test]
    fn match_ergonomics_bind_shared_references() {
        let program = expect_ok(
            r#"fn describe(value: &Option<i64>) -> i64 {
    match value {
        Some(inner) => *inner,
        None => 0,
    }
}

fn main() {
    let option = Some(5);
    println!("{}", describe(&option));
}"#,
        );
        let describe = find_fn(&program, "describe").expect("describe should be lowered");
        match &describe.body.tail {
            Some(tail) => match &tail.kind {
                HExprKind::Match { arms, .. } => {
                    assert_eq!(arms.len(), 2);
                    match &arms[0].pat.kind {
                        HPatKind::OptionSome(inner) => {
                            assert_eq!(inner.ty, shared_ref(Ty::Int(IntTy::I64)));
                        }
                        other => panic!("expected a Some pattern, got {:?}", other),
                    }
                }
                other => panic!("expected a match expression, got {:?}", other),
            },
            None => panic!("describe should have a tail expression"),
        }
    }

    #[test]
    fn match_ergonomics_bind_mutable_references() {
        let program = expect_ok(
            r#"fn bump(value: &mut Option<i64>) {
    match value {
        Some(inner) => {
            *inner = *inner + 1;
        }
        None => {}
    }
}

fn main() {
    let mut option = Some(1);
    bump(&mut option);
    println!("{}", option.unwrap_or(0));
}"#,
        );
        let bump = find_fn(&program, "bump").expect("bump should be lowered");
        let mut found = false;
        if let Some(tail) = &bump.body.tail
            && let HExprKind::Match { arms, .. } = &tail.kind
            && let HPatKind::OptionSome(inner) = &arms[0].pat.kind
        {
            assert_eq!(inner.ty, mutable_ref(Ty::Int(IntTy::I64)));
            found = true;
        }
        assert!(found, "expected a Some pattern bound by mutable reference");
    }

    #[test]
    fn match_ergonomics_bind_struct_fields() {
        let program = expect_ok(
            r#"struct Point {
    x: i64,
    y: i64,
}

fn magnitude(point: &Point) -> i64 {
    match point {
        Point { x, y } => *x + *y,
    }
}

fn main() {
    let point = Point { x: 1, y: 2 };
    println!("{}", magnitude(&point));
}"#,
        );
        let magnitude = find_fn(&program, "magnitude").expect("magnitude should be lowered");
        match &magnitude.body.tail {
            Some(tail) => match &tail.kind {
                HExprKind::Match { arms, .. } => match &arms[0].pat.kind {
                    HPatKind::Struct { fields, .. } => {
                        assert_eq!(fields.len(), 2);
                        for (_, field) in fields {
                            assert_eq!(field.ty, shared_ref(Ty::Int(IntTy::I64)));
                        }
                    }
                    other => panic!("expected a struct pattern, got {:?}", other),
                },
                other => panic!("expected a match expression, got {:?}", other),
            },
            None => panic!("magnitude should have a tail expression"),
        }
    }

    // ----- borrowing rules (R1, R2, R3, R7) -----

    #[test]
    fn rejects_borrows_of_temporary_values() {
        let diagnostics = analyze(
            r#"fn main() {
    let reference = &5;
    println!("{}", *reference);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0745"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "cannot borrow a temporary value"));
    }

    #[test]
    fn mutable_borrows_require_mutable_bindings() {
        let diagnostics = analyze(
            r#"fn main() {
    let value = 1;
    let reference = &mut value;
    println!("{}", *reference);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0596"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn cannot_mutate_through_shared_references() {
        let diagnostics = analyze(
            r#"fn main() {
    let mut value = 1;
    let reference = &value;
    *reference = 2;
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0596"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_dereferencing_non_reference_values() {
        let diagnostics = analyze(
            r#"fn main() {
    let value = 1;
    let bad = *value;
    println!("{}", bad);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0614"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "cannot be dereferenced"));
    }

    #[test]
    fn allows_mutable_references_to_heap_strings() {
        expect_ok(
            r#"fn main() {
    let mut text = String::from("hi");
    let reference = &mut text;
    reference.push_str("!");
    println!("{}", reference.len());
}"#,
        );
    }

    #[test]
    fn allows_mutable_references_to_heap_vectors() {
        expect_ok(
            r#"fn main() {
    let mut values = vec![1, 2];
    let reference = &mut values;
    reference.push(3);
    println!("{}", reference.len());
}"#,
        );
    }

    #[test]
    fn allows_mutable_reference_parameters_to_heap_values() {
        expect_ok(
            r#"fn trim(value: &mut String) {
    value.push_str("!");
}

fn main() {
    let mut text = String::from("hi");
    trim(&mut text);
    println!("{}", text);
}"#,
        );
    }

    #[test]
    fn rejects_references_stored_in_struct_fields() {
        let diagnostics = analyze(
            r#"struct Holder {
    value: &i64,
}

fn main() {
    let number = 1;
    let holder = Holder { value: &number };
    println!("{}", holder.value);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0498"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(
            &diagnostics,
            "references cannot be stored in this type"
        ));
    }

    #[test]
    fn rejects_references_stored_in_vector_elements() {
        let diagnostics = analyze(
            r#"fn main() {
    let number = 1;
    let values: Vec<&i64> = vec![&number];
    println!("{}", values.len());
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0498"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_references_stored_in_optional_values() {
        let diagnostics = analyze(
            r#"fn main() {
    let number = 1;
    let wrapped: Option<&i64> = Some(&number);
    println!("{}", *wrapped.unwrap());
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0498"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_references_stored_in_tuples() {
        let diagnostics = analyze(
            r#"fn main() {
    let number = 1;
    let pair: (&i64, i64) = (&number, 2);
    println!("{}", *pair.0);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0498"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_references_captured_by_closures() {
        let diagnostics = analyze(
            r#"fn main() {
    let value = 1;
    let reference = &value;
    let read = || *reference;
    println!("{}", read());
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0498"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(
            &diagnostics,
            "references cannot be captured by closures"
        ));
    }

    // ----- lifetimes and elision (R6) -----

    #[test]
    fn returns_borrow_derived_from_parameter() {
        let program = expect_ok(
            r#"fn first(values: &Vec<i64>) -> &i64 {
    &values[0]
}

fn main() {
    let values = vec![4, 5];
    let head = first(&values);
    println!("{}", *head);
}"#,
        );
        let first = find_fn(&program, "first").expect("first should be lowered");
        assert_eq!(first.ret, shared_ref(Ty::Int(IntTy::I64)));
        assert!(matches!(
            &first.body.tail,
            Some(tail) if matches!(&tail.kind, HExprKind::Ref { mutable: false, .. })
        ));
    }

    #[test]
    fn methods_return_references_to_their_receiver_fields() {
        let program = expect_ok(
            r#"struct Point {
    x: i64,
    y: i64,
}

impl Point {
    fn x_ref(&self) -> &i64 {
        &self.x
    }
}

fn main() {
    let point = Point { x: 3, y: 4 };
    let x = point.x_ref();
    println!("{}", *x);
}"#,
        );
        let x_ref = find_fn(&program, "Point::x_ref").expect("x_ref should be lowered");
        assert_eq!(x_ref.ret, shared_ref(Ty::Int(IntTy::I64)));
    }

    #[test]
    fn reference_returns_need_a_lifetime_source() {
        let diagnostics = analyze(
            r#"fn constant() -> &i64 {
    let value = 1;
    &value
}

fn main() {
    println!("{}", *constant());
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0106"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "missing lifetime specifier"));
        assert!(has_note(&diagnostics, "no reference input"));
    }

    #[test]
    fn reference_returns_need_an_unambiguous_lifetime_source() {
        let diagnostics = analyze(
            r#"fn choose(first: &i64, second: &i64) -> &i64 {
    first
}

fn main() {
    let a = 1;
    let b = 2;
    println!("{}", *choose(&a, &b));
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0106"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_note(&diagnostics, "which parameter"));
    }

    #[test]
    fn rejects_returning_references_to_locals() {
        let diagnostics = analyze(
            r#"fn pick(value: &i64) -> &i64 {
    let local = *value;
    &local
}

fn main() {
    let number = 1;
    println!("{}", *pick(&number));
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0515"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(
            &diagnostics,
            "returns a reference to a local value"
        ));
    }

    #[test]
    fn reference_returns_must_borrow_from_the_elided_parameter() {
        let diagnostics = analyze(
            r#"struct Holder {
    value: i64,
}

impl Holder {
    fn pick(&self, other: &i64) -> &i64 {
        other
    }
}

fn main() {
    let holder = Holder { value: 1 };
    let number = 2;
    println!("{}", *holder.pick(&number));
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0515"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn mutable_reference_returns_require_a_mutable_source() {
        let diagnostics = analyze(
            r#"fn borrow(value: &i64) -> &mut i64 {
    value
}

fn main() {
    let number = 1;
    println!("{}", *borrow(&number));
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0596"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_iterating_over_references_to_vectors() {
        let diagnostics = analyze(
            r#"fn main() {
    let values = vec![1, 2, 3];
    for value in &values {
        println!("{}", value);
    }
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0277"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(
            &diagnostics,
            "cannot iterate over a value of type &Vec"
        ));
    }

    // -----------------------------------------------------------------
    // Version 1.4 ownership: Drop registration
    // -----------------------------------------------------------------

    #[test]
    fn records_drop_hook_for_struct() {
        let program = expect_ok(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(&mut self) {
        println!("{}", self.value);
    }
}

struct Wrapper {
    inner: Tracked,
}

struct Boxed {
    items: Vec<Tracked>,
}

fn main() {
    let wrapper = Wrapper {
        inner: Tracked { value: 1 },
    };
    println!("{}", wrapper.inner.value);
}"#,
        );
        let tracked = program
            .structs
            .iter()
            .position(|item| item.name == "Tracked")
            .expect("Tracked struct is lowered");
        let wrapper = program
            .structs
            .iter()
            .position(|item| item.name == "Wrapper")
            .expect("Wrapper struct is lowered");
        let boxed = program
            .structs
            .iter()
            .position(|item| item.name == "Boxed")
            .expect("Boxed struct is lowered");
        let hook = program.drop_structs[tracked].expect("drop hook is recorded");
        assert_eq!(program.fns[hook].name, "Tracked::drop");
        assert_eq!(program.fns[hook].self_kind, Some(SelfKind::RefMut));
        assert!(!program.copy_structs[tracked], "drop type must not be Copy");
        assert!(!program.is_copy(&Ty::Struct(tracked)));
        assert!(program.has_drop_glue(&Ty::Struct(tracked)));
        assert!(program.has_drop_glue(&Ty::Struct(wrapper)));
        assert!(program.has_drop_glue(&Ty::Struct(boxed)));
        assert!(!program.copy_structs[wrapper]);
    }

    #[test]
    fn records_drop_hook_for_enum() {
        let program = expect_ok(
            r#"enum State {
    Idle,
    Busy(i64),
}

impl Drop for State {
    fn drop(&mut self) {
        println!("dropping");
    }
}

fn main() {
    let state = State::Busy(1);
}"#,
        );
        let state = program
            .enums
            .iter()
            .position(|item| item.name == "State")
            .expect("State enum is lowered");
        let hook = program.drop_enums[state].expect("drop hook is recorded");
        assert_eq!(program.fns[hook].name, "State::drop");
        assert_eq!(program.fns[hook].self_kind, Some(SelfKind::RefMut));
        assert!(!program.copy_enums[state], "drop enum must not be Copy");
        assert!(!program.is_copy(&Ty::Enum(state)));
        assert!(program.has_drop_glue(&Ty::Enum(state)));
        assert!(program.has_drop_glue(&Ty::Option(Box::new(Ty::Enum(state)))));
    }

    #[test]
    fn drop_hook_body_can_mutate_fields_through_mutable_self() {
        expect_ok(
            r#"struct Buffer {
    text: String,
    count: i64,
}

impl Drop for Buffer {
    fn drop(&mut self) {
        self.text.push_str("!");
        self.count = 0;
        println!("{} {}", self.text.len(), self.count);
    }
}

fn main() {
    let buffer = Buffer {
        text: String::from("x"),
        count: 1,
    };
}"#,
        );
    }

    // -----------------------------------------------------------------
    // Version 1.4 ownership: Drop validation
    // -----------------------------------------------------------------

    #[test]
    fn rejects_drop_impl_with_wrong_method_name() {
        let diagnostics = expect_errors(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn release(&mut self) {}
}

fn main() {}"#,
        );
        assert!(
            has_message(&diagnostics, "exactly one method named 'drop'"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_drop_impl_with_shared_receiver() {
        let diagnostics = expect_errors(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(&self) {}
}

fn main() {}"#,
        );
        assert!(
            has_message(&diagnostics, "'&mut self'"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_drop_impl_with_by_value_receiver() {
        let diagnostics = expect_errors(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(self) {}
}

fn main() {}"#,
        );
        assert!(
            has_message(&diagnostics, "'&mut self'"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_drop_impl_with_extra_parameters() {
        let diagnostics = expect_errors(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(&mut self, extra: i64) {}
}

fn main() {}"#,
        );
        assert!(
            has_message(&diagnostics, "'&mut self'"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_drop_impl_with_return_value() {
        let diagnostics = expect_errors(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(&mut self) -> i64 {
        1
    }
}

fn main() {}"#,
        );
        assert!(
            has_message(&diagnostics, "must return unit"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_duplicate_drop_impls() {
        let diagnostics = expect_errors(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(&mut self) {}
}

impl Drop for Tracked {
    fn drop(&mut self) {}
}

fn main() {}"#,
        );
        assert!(
            has_code(&diagnostics, "E0119"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(
            &diagnostics,
            "conflicting implementations of trait 'Drop'"
        ));
    }

    #[test]
    fn rejects_copy_and_clone_impls_as_derived() {
        let diagnostics = expect_errors(
            r#"struct Point {
    x: i64,
}

impl Copy for Point {}

impl Clone for Point {}

fn main() {}"#,
        );
        let derived = messages(&diagnostics)
            .iter()
            .filter(|message| message.contains("derived automatically"))
            .count();
        assert_eq!(derived, 2, "{:?}", messages(&diagnostics));
    }

    #[test]
    fn rejects_unknown_trait_impls() {
        let diagnostics = expect_errors(
            r#"struct Point {
    x: i64,
}

impl Display for Point {}

fn main() {}"#,
        );
        assert!(has_code(&diagnostics, "E0405"));
        assert!(
            has_message(&diagnostics, "cannot find trait 'Display'"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_inherent_methods_named_drop_or_clone() {
        let diagnostics = expect_errors(
            r#"struct Thing {
    value: i64,
}

impl Thing {
    fn drop(&mut self) {}

    fn clone(&self) -> Thing {
        Thing { value: self.value }
    }
}

fn main() {}"#,
        );
        assert!(
            has_message(&diagnostics, "reserved for the Drop trait"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "clone is derived"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn drop_impl_registers_no_callable_method() {
        let diagnostics = expect_errors(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(&mut self) {}
}

fn main() {
    let tracked = Tracked { value: 1 };
    tracked.drop();
    println!("{}", tracked.value);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0599"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "no method named drop"));
    }

    // -----------------------------------------------------------------
    // Version 1.4 ownership: .clone() resolution
    // -----------------------------------------------------------------

    #[test]
    fn derives_clone_for_move_only_values() {
        expect_ok(
            r#"struct Pair {
    left: Vec<i64>,
    right: String,
}

fn main() {
    let pair = Pair {
        left: vec![1],
        right: String::from("x"),
    };
    let pair_copy = pair.clone();
    let text = String::from("hi");
    let text_copy = text.clone();
    let values = vec![1, 2];
    let values_copy = values.clone();
    let number = 7;
    let number_copy = number.clone();
    println!(
        "{} {} {} {} {}",
        pair_copy.right,
        text_copy,
        values_copy.len() + values.len(),
        number_copy,
        pair_copy.left.len()
    );
}"#,
        );
    }

    #[test]
    fn clone_through_references_strips_all_reference_layers() {
        let program = expect_ok(
            r#"fn clone_shared(value: &String) -> String {
    value.clone()
}

fn clone_mut(value: &mut String) -> String {
    value.clone()
}

fn clone_nested(value: &String) -> String {
    let nested = &value;
    nested.clone()
}

fn main() {
    let mut text = String::from("hi");
    let copy = text.clone();
    let shared = clone_shared(&text);
    let mutable = clone_mut(&mut text);
    println!("{} {} {}", copy, shared, mutable);
}"#,
        );
        let shared = find_fn(&program, "clone_shared").expect("clone_shared is lowered");
        let mutable = find_fn(&program, "clone_mut").expect("clone_mut is lowered");
        let nested = find_fn(&program, "clone_nested").expect("clone_nested is lowered");
        assert_eq!(shared.ret, Ty::String);
        assert_eq!(mutable.ret, Ty::String);
        assert_eq!(nested.ret, Ty::String);
        let main = main_fn(&program);
        let clone_ty = main
            .body
            .stmts
            .iter()
            .find_map(|statement| match statement {
                HStmt::Let {
                    init: Some(init), ..
                } if matches!(
                    &init.kind,
                    HExprKind::BuiltinMethod {
                        method: HBuiltinMethod::Clone,
                        ..
                    }
                ) =>
                {
                    Some(init.ty.clone())
                }
                _ => None,
            })
            .expect("the nested .clone() call is lowered");
        assert_eq!(clone_ty, Ty::String);
    }

    #[test]
    fn rejects_clone_of_type_with_drop_glue() {
        let diagnostics = expect_errors(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(&mut self) {}
}

fn main() {
    let tracked = Tracked { value: 1 };
    let copy = tracked.clone();
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0277"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "implements Drop and cannot be cloned"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_clone_with_arguments() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let text = String::from("hi");
    let copy = text.clone(1);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0061"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    // -----------------------------------------------------------------
    // Version 1.4 ownership: E0509 pattern bindings
    // -----------------------------------------------------------------

    #[test]
    fn rejects_drop_glue_bindings_in_tuple_patterns() {
        let diagnostics = expect_errors(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(&mut self) {}
}

fn main() {
    let pair = (Tracked { value: 1 }, Tracked { value: 2 });
    let (left, right) = pair;
    println!("{} {}", left.value, right.value);
}"#,
        );
        let count = diagnostics
            .as_slice()
            .iter()
            .filter(|diagnostic| diagnostic.code == Some("E0509"))
            .count();
        assert_eq!(count, 2, "{:?}", messages(&diagnostics));
    }

    #[test]
    fn rejects_drop_glue_bindings_in_struct_patterns() {
        let diagnostics = expect_errors(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(&mut self) {}
}

struct Holder {
    tracker: Tracked,
    count: i64,
}

fn main() {
    let holder = Holder {
        tracker: Tracked { value: 1 },
        count: 2,
    };
    let Holder { tracker, count } = holder;
    println!("{}", tracker.value + count);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0509"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "implements Drop"));
    }

    #[test]
    fn rejects_drop_glue_bindings_in_variant_patterns() {
        let diagnostics = expect_errors(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(&mut self) {}
}

fn main() {
    let wrapped = Some(Tracked { value: 1 });
    match wrapped {
        Some(inner) => println!("{}", inner.value),
        None => println!("none"),
    }
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0509"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn allows_top_level_bindings_of_drop_types() {
        expect_ok(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(&mut self) {}
}

fn main() {
    let tracked = Tracked { value: 1 };
    let moved = tracked;
    let rebound = match moved {
        value => value,
    };
    println!("{}", rebound.value);
}"#,
        );
    }

    #[test]
    fn allows_compound_patterns_without_drop_glue() {
        expect_ok(
            r#"struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let point = Point { x: 1, y: 2 };
    let Point { x, y } = point;
    let pair = (x, y);
    let (a, b) = pair;
    println!("{} {}", a, b);
}"#,
        );
    }

    #[test]
    fn allows_top_level_let_binding_of_drop_field_values() {
        expect_ok(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(&mut self) {}
}

fn main() {
    let tracked = Tracked { value: 1 };
    let number = tracked.value;
    println!("{}", number);
}"#,
        );
    }

    // -----------------------------------------------------------------
    // Version 1.4 ownership: mutable references to heap values
    // -----------------------------------------------------------------

    #[test]
    fn allows_mutable_references_to_heap_rc_and_box_values() {
        expect_ok(
            r#"fn main() {
    let mut shared = Rc::new(1);
    let reference = &mut shared;
    *reference = Rc::new(2);
    let mut boxed = Box::new(3);
    let boxed_reference = &mut boxed;
    *boxed_reference = Box::new(4);
}"#,
        );
    }

    #[test]
    fn rejects_mutable_references_to_non_heap_user_values() {
        let diagnostics = expect_errors(
            r#"struct Packet {
    body: String,
}

fn touch(packet: &mut Packet) {
    packet.body.push_str("!");
}

fn main() {
    let mut packet = Packet {
        body: String::from("x"),
    };
    touch(&mut packet);
    println!("{}", packet.body);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0498"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "mutable references to"));
    }

    #[test]
    fn rejects_storing_mutable_heap_references_in_aggregates() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let mut text = String::from("hi");
    let pair: (&mut String, i64) = (&mut text, 1);
    println!("{}", pair.1);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0498"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    // -----------------------------------------------------------------
    // Version 1.4 ownership: post-lowering diagnostics (moveck wiring)
    // -----------------------------------------------------------------

    #[test]
    fn reports_post_lowering_diagnostics() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let mut x = 1;
    let a = &mut x;
    let b = &mut x;
    *a = 2;
    *b = 3;
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0499"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn wires_move_errors_into_post_lowering_diagnostics() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let a = String::from("hi");
    let b = a;
    println!("{}", a);
    println!("{}", b);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0382"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "use of moved value: a"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_moving_out_of_fields_after_ownership_change() {
        let diagnostics = expect_errors(
            r#"struct Holder {
    text: String,
}

fn main() {
    let holder = Holder { text: String::from("hi") };
    let text = holder.text;
    println!("{}", text);
    println!("{}", holder.text);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0507"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "cannot move out of a field of type String"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_cloning_containers_of_drop_glue_values() {
        let diagnostics = expect_errors(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(&mut self) {
        println!("{}", self.value);
    }
}

fn main() {
    let tracked = Tracked { value: 1 };
    let items = vec![tracked];
    let copies = items.clone();
    println!("{}", copies.len());
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0277"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "implements Drop and cannot be cloned"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_if_let_bindings_of_drop_glue_payloads() {
        let diagnostics = expect_errors(
            r#"struct Tracked {
    value: i64,
}

impl Drop for Tracked {
    fn drop(&mut self) {
        println!("{}", self.value);
    }
}

fn main() {
    let opt = Some(Tracked { value: 1 });
    if let Some(tracked) = opt {
        println!("{}", tracked.value);
    }
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0509"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "implements Drop"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn filter_borrows_non_copy_payloads() {
        // Option::filter takes Rust's `FnOnce(&T) -> bool`: the predicate
        // borrows the payload and the returned `Some` moves it exactly once.
        let program = expect_ok(
            r#"fn main() {
    let name: Option<String> = Some(String::from("lsaca"));
    match name.filter(|text| text.len() > 2) {
        Some(text) => println!("kept {}", text),
        None => println!("dropped"),
    }
    let short: Option<String> = Some(String::from("a"));
    let kept = short.filter(|text| text.len() > 2);
    println!("{}", kept.is_some());
}"#,
        );
        let body = &main_fn(&program).body;
        let filtered = match &body.stmts[1] {
            HStmt::Expr(expr) => expr,
            other => panic!("expected the filter match statement, got {:?}", other),
        };
        let comb = match &filtered.kind {
            HExprKind::Match { scrutinee, .. } => scrutinee,
            other => panic!("expected the outer match, got {:?}", other),
        };
        let arms = match &comb.kind {
            HExprKind::Match { arms, .. } => arms,
            other => panic!("expected the filter match, got {:?}", other),
        };
        let value_name = match &arms[0].pat.kind {
            HPatKind::OptionSome(inner) => match &inner.kind {
                HPatKind::Bind { c_name, .. } => c_name.clone(),
                other => panic!("expected the payload binding, got {:?}", other),
            },
            other => panic!("expected a Some pattern, got {:?}", other),
        };
        // The arm body is `{ let reference = &value; if predicate(reference) {
        // Some(value) } else { None } }`: the borrow is confined to its own
        // statement so the payload can be moved afterwards.
        let block = match &arms[0].body.kind {
            HExprKind::Block(block) => block,
            other => panic!("expected a block in the filter arm, got {:?}", other),
        };
        let (reference_name, borrow_of) = match &block.stmts[0] {
            HStmt::Let {
                pat,
                init: Some(init),
                ..
            } => {
                let reference_name = match &pat.kind {
                    HPatKind::Bind { c_name, .. } => c_name.clone(),
                    other => panic!("expected a reference binding, got {:?}", other),
                };
                let borrow_of = match &init.kind {
                    HExprKind::Ref { mutable, expr } => {
                        assert!(!*mutable, "filter borrows the payload immutably");
                        match &expr.kind {
                            HExprKind::Local { c_name, .. } => c_name.clone(),
                            other => panic!("expected a local borrow, got {:?}", other),
                        }
                    }
                    other => panic!("expected a reference initializer, got {:?}", other),
                };
                (reference_name, borrow_of)
            }
            other => panic!("expected the filter borrow statement, got {:?}", other),
        };
        assert_eq!(
            borrow_of, value_name,
            "the predicate borrows the matched payload"
        );
        let condition_expr = block.tail.as_deref().expect("filter needs a conditional");
        let (cond, then_block) = match &condition_expr.kind {
            HExprKind::If {
                cond, then_block, ..
            } => (cond, then_block),
            other => panic!("expected a conditional in the filter arm, got {:?}", other),
        };
        let arg = match &cond.kind {
            HExprKind::Call { args, .. } => &args[0],
            other => panic!("expected the predicate call, got {:?}", other),
        };
        match &arg.kind {
            HExprKind::Local { c_name, .. } => assert_eq!(
                *c_name, reference_name,
                "the predicate reads the borrowed payload"
            ),
            other => panic!(
                "expected the predicate to read the reference binding, got {:?}",
                other
            ),
        }
        let keep = then_block
            .tail
            .as_deref()
            .expect("filter needs a then value");
        match &keep.kind {
            HExprKind::OptionSome(inner) => match &inner.kind {
                HExprKind::Local { c_name, .. } => assert_eq!(
                    *c_name, value_name,
                    "the kept payload is the moved match binding"
                ),
                other => panic!("expected the kept payload local, got {:?}", other),
            },
            other => panic!("expected Some in the filter success path, got {:?}", other),
        }
    }

    #[test]
    fn chains_non_copy_payloads_without_moves() {
        expect_ok(
            r#"fn main() {
    let name: Option<String> = Some(String::from("lsaca"));
    let upper = name.map(|text| text.len());
    println!("{}", upper.unwrap());
    let back: Option<String> = Some(String::from("x")).and_then(|text| Some(text));
    println!("{}", back.unwrap());
    let result: Result<String, String> = Ok(String::from("pos"));
    let mapped = result.map(|text| text.len());
    println!("{}", mapped.unwrap());
    let failed: Result<String, String> = Err(String::from("bad"));
    let recovered = failed.map_err(|message| message.len());
    println!("{}", recovered.is_err());
    let chained: Result<String, String> = Ok(String::from("v"));
    let next = chained.and_then(|value| Ok(value));
    println!("{}", next.unwrap());
    let optional: Option<String> = Some(String::from("z"));
    let converted = optional.ok_or(String::from("missing"));
    println!("{}", converted.unwrap());
    let defaulted: Option<String> = None;
    let total = defaulted.map_or(String::from("none"), |value| value);
    println!("{}", total);
}"#,
        );
    }

    // -----------------------------------------------------------------
    // Version 1.5 inference and equality regressions
    // -----------------------------------------------------------------

    #[test]
    fn infers_closure_return_types_for_comparisons() {
        // The binary operator result type (bool) must not leak into the
        // operands of a comparison: a closure body compared inside a context
        // expecting bool used to unify its operands with bool (E0308).
        expect_ok(
            r#"fn accepts(predicate: fn(i64) -> bool) -> bool {
    return predicate(4);
}
fn main() {
    println!("{}", accepts(|value: i64| value > 2));
    let total = accepts(|value: i64| value + 1 > 0);
    println!("{}", total);
}"#,
        );
        // Comparison of a reference parameter stays inferred as well.
        expect_ok(
            r#"fn keeps(predicate: fn(&String) -> bool, text: &String) -> bool {
    return predicate(text);
}
fn main() {
    let name = String::from("lsaca");
    println!("{}", keeps(|text: &String| text.len() > 2, &name));
}"#,
        );
    }

    #[test]
    fn derives_equality_for_aggregates() {
        expect_ok(
            r#"struct Point { x: i64, y: i64 }
enum Kind { Empty, Value(i64) }
fn main() {
    let left = vec![1, 2];
    let right = vec![1, 2];
    println!("{}", left == right);
    let some: Option<i64> = Some(3);
    println!("{}", some == Some(3));
    let ok: Result<i64, String> = Ok(1);
    println!("{}", ok == Ok(1));
    let pair = (1, true);
    println!("{}", pair == (1, true));
    let first = Point { x: 1, y: 2 };
    let second = Point { x: 1, y: 2 };
    println!("{}", first == second);
    println!("{}", Kind::Empty == Kind::Empty);
    let text = String::from("same");
    println!("{}", &text == &text);
    println!("{}", Box::new(5) == Box::new(5));
    let map: HashMap<String, i64> = HashMap::new();
    println!("{}", map == map);
}"#,
        );
    }

    #[test]
    fn derives_to_string_alias_for_strings() {
        let program = expect_ok(
            r#"fn main() {
    let text = String::from("lsaca");
    let copy = text.to_string();
    println!("{}", copy);
}"#,
        );
        let main = main_fn(&program);
        let HStmt::Let {
            init: Some(init), ..
        } = &main.body.stmts[1]
        else {
            panic!("expected the second statement to be a let");
        };
        assert!(
            matches!(
                init.kind,
                HExprKind::BuiltinMethod {
                    method: HBuiltinMethod::Clone,
                    ..
                }
            ),
            "String::to_string() should lower to the derived clone, got {:?}",
            init.kind
        );
    }

    #[test]
    fn rejects_hash_map_get_with_reference_results() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let mut map: HashMap<String, String> = HashMap::new();
    map.insert(String::from("key"), String::from("value"));
    let borrowed = map.get_with(String::from("key"), |value: &String| value);
    println!("{:?}", borrowed);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0498"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "references cannot be stored"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn reports_nested_tuple_pattern_mismatch() {
        // A valid nested tuple pattern is decomposed element-wise.
        expect_ok(
            r#"fn main() {
    let pair = (1, (2, 3));
    let (first, (inner, last)) = pair;
    println!("{} {} {}", first, inner, last);
}"#,
        );
        // A mis-typed nested pattern reports one mismatch instead of letting
        // every binding below degrade into an E0282 cascade.
        let diagnostics = expect_errors(
            r#"fn main() {
    let triple = ((1, (2, 3)), 4);
    let (first, (inner, last)) = triple;
    println!("{} {} {}", first, inner, last);
}"#,
        );
        assert!(
            has_message(&diagnostics, "mismatched types"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn generic_parameters_are_in_scope_in_signatures() {
        expect_ok(
            r#"struct Pair<T> { first: T, second: T }
impl<T> Pair<T> {
    fn new(first: T, second: T) -> Pair<T> {
        return Pair { first: first, second: second };
    }
    fn swapped(self) -> Self {
        return Self { first: self.second, second: self.first };
    }
}
fn identity<T>(value: T) -> T {
    return value;
}
fn main() {
    let pair = Pair::new(1, 2);
    let swapped = pair.swapped();
    println!("{} {}", swapped.first, swapped.second);
    println!("{}", identity(7));
}"#,
        );
    }

    #[test]
    fn validates_trait_impls_against_their_own_self_type() {
        expect_ok(
            r#"trait Describe<T> { fn describe(&self, extra: T) -> String; }
struct Point { x: i64, y: i64 }
struct Tag { label: String }
impl Describe<i64> for Point {
    fn describe(&self, extra: i64) -> String {
        return format!("point({}, {}) + {}", self.x, self.y, extra);
    }
}
impl Describe<String> for Tag {
    fn describe(&self, extra: String) -> String {
        return self.label + ":" + extra;
    }
}
fn main() {
    let point = Point { x: 1, y: 2 };
    println!("{}", point.describe(3));
    let tag = Tag { label: String::from("t") };
    println!("{}", tag.describe(String::from("v")));
}"#,
        );
    }

    #[test]
    fn accepts_user_iterator_implementations() {
        expect_ok_with_prelude(
            r#"struct Counter { value: i64 }
impl Iterator<i64> for Counter {
    fn next(&mut self) -> Option<i64> {
        if self.value >= 3 {
            return None;
        }
        self.value = self.value + 1;
        return Some(self.value);
    }
}
fn main() {
    let first = Counter { value: 0 };
    for value in first {
        println!("{}", value);
    }
    let second = Counter { value: 0 };
    for value in second.map(|number| number * 2) {
        println!("{}", value);
    }
}"#,
        );
    }

    #[test]
    fn rejects_trait_impls_for_builtin_targets() {
        // A trait can only be implemented for a user-defined struct or enum:
        // builtin types cannot gain trait methods.
        let diagnostics = expect_errors(
            r#"trait Tag { fn tag(&self) -> i64; }
impl Tag for Vec<i64> {
    fn tag(&self) -> i64 { return 1; }
}
impl Tag for i64 {
    fn tag(&self) -> i64 { return 2; }
}
impl Tag for (i64, i64) {
    fn tag(&self) -> i64 { return 3; }
}
fn main() {}"#,
        );
        assert!(has_code(&diagnostics, "E0118"));
        assert!(has_message(
            &diagnostics,
            "impl target must be a struct or enum type defined in this program, found Vec<i64>"
        ));
        assert!(has_message(
            &diagnostics,
            "impl target must be a struct or enum type defined in this program, found i64"
        ));
        assert!(has_message(
            &diagnostics,
            "impl target must be a struct or enum type defined in this program, found (i64, i64)"
        ));
    }

    #[test]
    fn rejects_generic_impls_for_builtin_targets() {
        let diagnostics = expect_errors(
            r#"impl<T> Vec<T> {
    fn weird(&self) -> i64 { return 0; }
}
impl i64 {
    fn odd(&self) -> i64 { return 1; }
}
fn main() {}"#,
        );
        assert!(has_code(&diagnostics, "E0118"));
        assert!(has_message(&diagnostics, "found Vec<T>"));
        assert!(has_message(
            &diagnostics,
            "impl target must be a struct or enum type defined in this program, found i64"
        ));
    }

    #[test]
    fn rejected_trait_impls_do_not_provide_methods() {
        let diagnostics = expect_errors(
            r#"trait Tag { fn tag(&self) -> i64; }
impl Tag for i64 {
    fn tag(&self) -> i64 { return 1; }
}
fn main() {
    let value = 2;
    println!("{}", value.tag());
}"#,
        );
        assert!(has_code(&diagnostics, "E0118"));
        assert!(has_code(&diagnostics, "E0599"));
        assert!(has_message(&diagnostics, "no method named tag"));
    }

    #[test]
    fn accepts_generic_impls_for_user_types() {
        expect_ok(
            r#"trait Describe { fn describe(&self) -> i64; }
struct Boxed<T> { inner: T }
impl<T> Describe for Boxed<T> {
    fn describe(&self) -> i64 { return 7; }
}
impl<T> Boxed<T> {
    fn value_of(&self) -> T { return self.inner.clone(); }
}
fn main() {
    let boxed: Boxed<i64> = Boxed { inner: 5 };
    println!("{}", boxed.describe());
    println!("{}", boxed.value_of());
}"#,
        );
    }

    #[test]
    fn rejects_turbofish_on_non_generic_methods() {
        let diagnostics = expect_errors(
            r#"trait Shape { fn area(&self) -> i64; }
struct Point { x: i64 }
struct Square { side: i64 }
impl Point {
    fn get(&self) -> i64 { return self.x; }
}
impl Shape for Square {
    fn area(&self) -> i64 { return self.side * self.side; }
}
fn main() {
    let point = Point { x: 1 };
    let _value = point.get::<i64>();
    let square = Square { side: 2 };
    let _area = square.area::<i64>();
    let values = vec![1, 2];
    let _length = values.len::<i64>();
}"#,
        );
        assert!(has_code(&diagnostics, "E0107"));
        assert!(has_message(
            &diagnostics,
            "method get takes 0 generic arguments but 1 was supplied"
        ));
        assert!(has_message(
            &diagnostics,
            "method area takes 0 generic arguments but 1 was supplied"
        ));
        assert!(has_message(
            &diagnostics,
            "method len takes 0 generic arguments but 1 was supplied"
        ));
    }

    #[test]
    fn keeps_turbofish_for_parse_and_collect() {
        expect_ok_with_prelude(
            r#"fn main() {
    let parsed = String::from("42").parse::<i64>();
    println!("{}", parsed);
    let doubled: Vec<i64> = vec![1, 2, 3].iter().map(|value| value * 2).collect::<Vec<i64>>();
    println!("{:?}", doubled);
}"#,
        );
    }

    #[test]
    fn reports_hash_map_key_errors_once() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let mut map: HashMap<Vec<i64>, i64> = HashMap::new();
    map.insert(vec![1], 2);
    println!("{}", map.len());
}"#,
        );
        assert!(has_code(&diagnostics, "E0277"));
        assert!(has_message(
            &diagnostics,
            "HashMap keys must be one of i64, u64, bool or String, found Vec<i64>"
        ));
        assert!(has_note(
            &diagnostics,
            "only the listed key types are supported; use a struct field or a String key instead"
        ));
        let key_errors = messages(&diagnostics)
            .iter()
            .filter(|message| {
                message.contains("HashMap keys must be one of i64, u64, bool or String")
            })
            .count();
        assert_eq!(
            key_errors,
            1,
            "the invalid key type must be reported exactly once: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn infers_unannotated_closure_arguments_for_callable_bounds() {
        expect_ok(
            r#"fn apply_twice<F: Fn(i64) -> i64>(f: F, value: i64) -> i64 {
    return f(f(value));
}
fn apply_value<T, F: Fn(T) -> T>(value: T, f: F) -> T {
    return f(value);
}
fn apply_predicate<F: Fn(i64) -> bool>(f: F, value: i64) -> bool {
    return f(value);
}
fn main() {
    println!("{}", apply_twice(|x| x * 2, 3));
    println!("{}", apply_value(4, |x| x + 1));
    println!("{}", apply_twice(|x: i64| x - 1, 5));
    println!("{}", apply_predicate(|n| n > 1, 7));
}"#,
        );
    }

    #[test]
    fn diagnostics_do_not_leak_prelude_names() {
        // Unsupported adapter methods are rejected with a message naming the
        // receiver type. The receiver may be a prelude adapter, whose internal
        // `__lsaca_` prefix is an implementation detail that must never reach
        // a user-facing diagnostic.
        let (program, file, parse_diagnostics) = parse_source(
            r#"fn main() {
    let values: Vec<i64> = vec![1, 2, 3];
    let zipped = values.iter().zip(vec![4, 5, 6].iter());
    println!("{}", zipped.count());
}"#,
        );
        assert!(
            !parse_diagnostics.has_errors(),
            "unexpected parse errors: {:?}",
            messages(&parse_diagnostics)
        );
        let prelude = crate::prelude::program().expect("the embedded prelude must parse");
        let (result, _warnings) = check_reporting_with_prelude(&program, Some(prelude), &file);
        let diagnostics = match result {
            Ok(_) => panic!(".zip() must be rejected as an unsupported iterator method"),
            Err(diagnostics) => diagnostics,
        };
        assert!(
            has_code(&diagnostics, "E0599"),
            "{:?}",
            messages(&diagnostics)
        );
        let rendered = diagnostics.render(&file);
        assert!(
            !rendered.contains("__lsaca_"),
            "diagnostics must not leak internal prelude identifiers:\n{}",
            rendered
        );
        assert!(
            rendered.contains("VecIter<i64>"),
            "the public adapter name should be shown instead of the internal one:\n{}",
            rendered
        );
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
        let prelude = match crate::prelude::program() {
            Ok(prelude) => prelude,
            Err(diagnostics) => panic!(
                "the embedded prelude must parse: {:?}",
                messages(&diagnostics)
            ),
        };
        // Fixtures that intentionally exercise a rejected program but keep a
        // descriptive name: the gate asserts both the rejection and the exact
        // diagnostic the fixture demonstrates, so it cannot silently start
        // passing for the wrong reason.
        const EXPECTED_REJECT: [(&str, &str); 2] = [
            (
                "iter_collect_no_type.ls",
                ".collect() requires a known Vec<T> result type",
            ),
            (
                "iter_sum_non_numeric.ls",
                "cannot sum an iterator of type String",
            ),
        ];
        let mut failures: Vec<String> = Vec::new();
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
                let accepted = check_with_prelude(&program, Some(prelude), &file).is_ok();
                assert!(!accepted, "{} should be rejected by the type checker", name);
                continue;
            }
            if let Some((_, expected_message)) =
                EXPECTED_REJECT.iter().find(|(fixture, _)| *fixture == name)
            {
                if rejected_at_parse {
                    continue;
                }
                let diagnostics = match check_with_prelude(&program, Some(prelude), &file) {
                    Ok(_) => panic!("{} should be rejected by the type checker", name),
                    Err(diagnostics) => diagnostics,
                };
                assert!(
                    has_message(&diagnostics, expected_message),
                    "{} should report a diagnostic containing {:?}, got: {:?}",
                    name,
                    expected_message,
                    messages(&diagnostics)
                );
                continue;
            }
            let mut parse_messages = messages(&lex_diagnostics);
            parse_messages.extend(messages(&parse_diagnostics));
            if rejected_at_parse {
                failures.push(format!(
                    "{} should parse cleanly: {:?}",
                    name, parse_messages
                ));
                continue;
            }
            if let Err(diagnostics) = check_with_prelude(&program, Some(prelude), &file) {
                failures.push(format!(
                    "{} should type check, got: {:?}",
                    name,
                    messages(&diagnostics)
                ));
            }
        }
        // Report every stale fixture at once so the fixture migration (owned by
        // the Lead) can be done in one pass instead of one panic at a time.
        assert!(
            failures.is_empty(),
            "{} fixture(s) do not satisfy the static ownership rules:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    // -----------------------------------------------------------------
    // Logical operators
    // -----------------------------------------------------------------

    #[test]
    fn logical_operators_lower_to_short_circuit_conditionals() {
        // a || b must lower to `if a { true } else { b }` and a && b to
        // `if a { b } else { false }`. Swapping the branches silently turns
        // || into its negation, so pin the lowered shape here.
        let program =
            expect_ok("fn main() { let a = true; let b = false; let c = a || b; let d = a && b; }");
        let body = &main_fn(&program).body;
        let mut lowered = Vec::new();
        for stmt in &body.stmts {
            if let HStmt::Let {
                init: Some(init), ..
            } = stmt
                && let HExprKind::If {
                    cond,
                    then_block,
                    else_expr,
                } = &init.kind
            {
                lowered.push((cond, then_block, else_expr.as_deref()));
            }
        }
        assert_eq!(
            lowered.len(),
            2,
            "both logical operators must lower to conditionals"
        );
        let (or_cond, or_then, or_else) = &lowered[0];
        assert!(
            matches!(or_cond.kind, HExprKind::Local { .. }),
            "the left operand is the condition"
        );
        let or_taken = or_then.tail.as_deref().expect("|| needs a taken value");
        assert!(
            matches!(or_taken.kind, HExprKind::Lit(crate::hir::HLit::Bool(true))),
            "|| must yield true when the left operand is true, got {:?}",
            or_taken.kind
        );
        let or_skipped = or_else.expect("|| needs a skipped value");
        assert!(
            matches!(or_skipped.kind, HExprKind::Local { .. }),
            "|| evaluates the right operand only when the left operand is false"
        );
        let (and_cond, and_then, and_else) = &lowered[1];
        assert!(
            matches!(and_cond.kind, HExprKind::Local { .. }),
            "the left operand is the condition"
        );
        let and_taken = and_then.tail.as_deref().expect("&& needs a taken value");
        assert!(
            matches!(and_taken.kind, HExprKind::Local { .. }),
            "&& evaluates the right operand only when the left operand is true"
        );
        let and_skipped = and_else.expect("&& needs a skipped value");
        assert!(
            matches!(
                and_skipped.kind,
                HExprKind::Lit(crate::hir::HLit::Bool(false))
            ),
            "&& must yield false when the left operand is false"
        );
    }
}
