//! Semantic analysis: name resolution, type inference, type checking,
//! pattern exhaustiveness and lowering to the HIR.

mod builtins;
mod check;
mod consts;
mod dyn_traits;
mod exhaust;
mod exprs;
mod generics;
mod hash_map;
mod items;
mod iterators;
mod itype;
mod lower;
mod modules;
mod ownership;
mod patterns;
mod refs;
mod slices;
mod traits;
mod tree;

use std::path::Path;

use crate::diag::{Diagnostic, Diagnostics, Severity, Span, sort_and_dedup};
use crate::span::SourceLookup;
use crate::{ast, hir};

use self::check::Checker;
use self::itype::ITy;

/// Options that control a crate-level type check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckOptions {
    /// Whether the root crate must define a `main` entry point. Library
    /// crates and dependency crates are checked with this set to `false`.
    pub require_main: bool,
}

impl CheckOptions {
    /// Options for a normal application: `main` is required.
    pub fn new() -> CheckOptions {
        CheckOptions { require_main: true }
    }

    /// Set whether the checked program must define `main`.
    pub fn require_main(mut self, require_main: bool) -> CheckOptions {
        self.require_main = require_main;
        self
    }
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// Type check a parsed program and lower it to the fully typed HIR.
///
/// This is the original frozen entry point: warnings are only surfaced when
/// the check fails. Use [check_reporting] when warnings from successful
/// checks matter.
pub fn check(
    program: &ast::Program,
    sources: &dyn SourceLookup,
) -> Result<hir::Program, Diagnostics> {
    check_reporting(program, sources).0
}

/// Type check a parsed program and lower it to the fully typed HIR, also
/// returning the warnings that were produced even when checking succeeds.
///
/// The first element behaves exactly like [check]. The second element holds
/// the warnings-only diagnostics (every severity except Error), sorted and
/// deduplicated deterministically.
pub fn check_reporting(
    program: &ast::Program,
    sources: &dyn SourceLookup,
) -> (Result<hir::Program, Diagnostics>, Diagnostics) {
    check_reporting_with_prelude(program, None, sources)
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
    sources: &dyn SourceLookup,
) -> Result<hir::Program, Diagnostics> {
    check_reporting_with_prelude(program, prelude, sources).0
}

/// Like [check_with_prelude] but also returns the warnings of a successful
/// check.
pub fn check_reporting_with_prelude(
    program: &ast::Program,
    prelude: Option<&ast::Program>,
    sources: &dyn SourceLookup,
) -> (Result<hir::Program, Diagnostics>, Diagnostics) {
    check_reporting_with_options(program, prelude, sources, &CheckOptions::default())
}

/// Like [check_reporting_with_prelude] but with explicit options.
fn check_reporting_with_options(
    program: &ast::Program,
    prelude: Option<&ast::Program>,
    sources: &dyn SourceLookup,
    options: &CheckOptions,
) -> (Result<hir::Program, Diagnostics>, Diagnostics) {
    let name = program_name(sources);
    check_program_reporting(program, prelude, sources, name, options.require_main)
}

/// Type check a whole crate graph, lowering the root crate to HIR.
pub fn check_crate(
    graph: &ast::CrateGraph,
    sources: &dyn SourceLookup,
) -> Result<hir::Program, Diagnostics> {
    check_crate_reporting_with_options(graph, None, sources, &CheckOptions::default()).0
}

/// Type check a crate graph together with the standard-library prelude.
pub fn check_crate_with_prelude(
    graph: &ast::CrateGraph,
    prelude: Option<&ast::Program>,
    sources: &dyn SourceLookup,
) -> Result<hir::Program, Diagnostics> {
    check_crate_reporting_with_options(graph, prelude, sources, &CheckOptions::default()).0
}

/// Like [check_crate_with_prelude] but also returns warnings and honours
/// [CheckOptions].
pub fn check_crate_reporting_with_options(
    graph: &ast::CrateGraph,
    prelude: Option<&ast::Program>,
    sources: &dyn SourceLookup,
    options: &CheckOptions,
) -> (Result<hir::Program, Diagnostics>, Diagnostics) {
    let mut checker = Checker::new(sources);
    // Dependency crates are registered first: their items are visible to the
    // root crate through the extern prelude (uniform paths), but only the root
    // crate is lowered to HIR.
    for (index, krate) in graph.crates.iter().enumerate() {
        if index == graph.root {
            continue;
        }
        let crate_id = checker.begin_crate(&krate.name, true);
        let root_module = checker.crate_roots[crate_id];
        checker.collect_program_items(root_module, &krate.program.items);
    }
    let root = graph.root_crate();
    let root_id = checker.begin_crate(&root.name, root.is_dependency);
    checker.root_crate = root_id;
    let root_module = checker.crate_roots[root_id];
    checker.collect_program_items(root_module, &root.program.items);
    collect_prelude(&mut checker, prelude);
    finish_program_check(checker, root.name.clone(), options.require_main)
}

/// Shared implementation of the single-program and crate entry points.
fn check_program_reporting(
    program: &ast::Program,
    prelude: Option<&ast::Program>,
    sources: &dyn SourceLookup,
    name: String,
    require_main: bool,
) -> (Result<hir::Program, Diagnostics>, Diagnostics) {
    let mut checker = Checker::new(sources);
    let crate_id = checker.begin_crate(&name, false);
    checker.root_crate = crate_id;
    let root_module = checker.crate_roots[crate_id];
    checker.collect_program_items(root_module, &program.items);
    collect_prelude(&mut checker, prelude);
    finish_program_check(checker, name, require_main)
}

/// Register the embedded standard library as a separate crate. Its items are
/// generic and only reach the HIR when instantiated.
fn collect_prelude(checker: &mut Checker<'_>, prelude: Option<&ast::Program>) {
    let Some(prelude) = prelude else {
        return;
    };
    let crate_id = checker.begin_prelude_crate("std");
    let root_module = checker.crate_roots[crate_id];
    checker.collect_program_items(root_module, &prelude.items);
}

/// Run the shared pipeline over every collected crate and lower the root crate.
fn finish_program_check(
    mut checker: Checker<'_>,
    name: String,
    require_main: bool,
) -> (Result<hir::Program, Diagnostics>, Diagnostics) {
    checker.register_collected_items();
    checker.process_imports();
    checker.register_pending_impls();
    checker.resolve_all_types();
    for id in 0..checker.consts.len() {
        checker.ensure_const(id);
    }
    checker.check_all_fns();
    validate_main(&mut checker, require_main);

    let collected = std::mem::take(&mut checker.diagnostics);
    let (mut errors, mut warnings) = split_diagnostics(collected);
    if errors.has_errors() {
        // Preserve the historical behaviour of check(): the error result
        // carries every diagnostic, warnings included.
        errors.extend(warnings.clone());
        sort_and_dedup(&mut errors);
        return (Err(errors), warnings);
    }

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

fn validate_main(checker: &mut Checker<'_>, require_main: bool) {
    if !require_main {
        return;
    }
    let main_id = match checker.root_main_fn() {
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

fn program_name(sources: &dyn SourceLookup) -> String {
    let Some(file) = sources.primary_file() else {
        return "program".to_string();
    };
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
    use crate::hir::{
        ConstValue, HBuiltinAssoc, HBuiltinMethod, HExpr, HExprKind, HPatKind, HStmt,
    };
    use crate::span::{SourceFile, SourceMap};
    use crate::typeck::generics::DefOrigin;
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
                "expected the program to type check, got: {:#?}",
                diagnostics
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

    /// Type check a user program against a hand-written standard library.
    fn check_source_with_stdlib(
        source: &str,
        stdlib_source: &str,
    ) -> Result<hir::Program, Diagnostics> {
        let (program, file, diagnostics) = parse_source(source);
        assert!(
            !diagnostics.has_errors(),
            "unexpected parse errors: {:?}",
            messages(&diagnostics)
        );
        let stdlib_file = SourceFile::new("stdlib.ls", stdlib_source);
        let (tokens, mut stdlib_diagnostics) = crate::lexer::tokenize(stdlib_source);
        let (stdlib, more) = crate::parser::parse(&tokens, &stdlib_file);
        stdlib_diagnostics.extend(more);
        assert!(
            !stdlib_diagnostics.has_errors(),
            "unexpected standard library parse errors: {:?}",
            messages(&stdlib_diagnostics)
        );
        check_with_prelude(&program, Some(&stdlib), &file)
    }

    /// Type check against the embedded standard library, exactly like the
    /// compiler driver does.
    fn expect_errors_with_stdlib_program(source: &str) -> Diagnostics {
        let (program, file, diagnostics) = parse_source(source);
        assert!(
            !diagnostics.has_errors(),
            "unexpected parse errors: {:?}",
            messages(&diagnostics)
        );
        let prelude = crate::stdlib::program().expect("the embedded standard library must parse");
        match check_with_prelude(&program, Some(prelude), &file) {
            Ok(_) => panic!("expected type errors, but the program type checked"),
            Err(diagnostics) => diagnostics,
        }
    }
    fn expect_ok_with_stdlib(source: &str, stdlib_source: &str) -> hir::Program {
        match check_source_with_stdlib(source, stdlib_source) {
            Ok(program) => program,
            Err(diagnostics) => panic!(
                "expected the program to type check, got: {:?}",
                messages(&diagnostics)
            ),
        }
    }

    fn expect_errors_with_stdlib(source: &str, stdlib_source: &str) -> Diagnostics {
        match check_source_with_stdlib(source, stdlib_source) {
            Ok(_) => panic!("expected type errors, but the program type checked"),
            Err(diagnostics) => diagnostics,
        }
    }

    fn expect_ok_with_prelude(source: &str) -> hir::Program {
        match check_source_with_prelude(source) {
            Ok(program) => program,
            Err(diagnostics) => panic!(
                "expected the program to type check, got: {:#?}",
                diagnostics
            ),
        }
    }

    fn expect_errors_with_prelude(source: &str) -> Diagnostics {
        match check_source_with_prelude(source) {
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
        let crate_id = checker.begin_crate("program", false);
        checker.root_crate = crate_id;
        checker.collect_items_only(&program, DefOrigin::User);
        checker.register_collected_items();
        checker.process_imports();
        checker.register_pending_impls();
        checker.resolve_all_types();
        for id in 0..checker.consts.len() {
            checker.ensure_const(id);
        }
        checker.check_all_fns();
        validate_main(&mut checker, true);
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
                assert_eq!(pat.ty, Ty::Int(IntTy::I32));
                let init = init.as_ref().expect("initializer");
                assert_eq!(init.ty, Ty::Int(IntTy::I32));
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
    fn checks_assert_ne_macro() {
        let program = expect_ok(
            r#"fn main() {
    assert_ne!(1, 2);
    assert_ne!(String::from("a"), String::from("b"));
    assert_ne!(true, false);
    assert_ne!(1, 2, "numbers must differ");
    assert_ne!(1, 2, "expected {} and {}", 3, 4);
}"#,
        );
        let body = &main_fn(&program).body;
        let mut assert_ne_calls = 0;
        for stmt in &body.stmts {
            if let HStmt::Expr(expr) = stmt
                && let HExprKind::Macro(call) = &expr.kind
                && call.kind == crate::hir::HMacroKind::AssertNe
            {
                assert_ne_calls += 1;
                assert_eq!(call.args.len(), 2);
            }
        }
        assert_eq!(assert_ne_calls, 5);
    }

    #[test]
    fn rejects_malformed_assert_ne_macro() {
        let diagnostics = expect_errors("fn main() { assert_ne!(1); }");
        assert!(
            has_code(&diagnostics, "E0061"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "assert_ne! takes 2 arguments"),
            "{:?}",
            messages(&diagnostics)
        );

        let diagnostics = expect_errors("fn main() { assert_ne!(1, true); }");
        assert!(
            has_code(&diagnostics, "E0308"),
            "{:?}",
            messages(&diagnostics)
        );

        let diagnostics = expect_errors(
            r#"fn first() -> i64 { 1 }
fn second() -> i64 { 2 }
fn main() {
    assert_ne!(first, second);
}"#,
        );
        assert!(
            has_message(&diagnostics, "assert_ne! requires a type that supports !="),
            "{:?}",
            messages(&diagnostics)
        );
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
    fn try_operator_unwraps_the_operand_payload_independently_of_the_return_type() {
        // `?` unwraps the operand and yields that operand's Ok/Some payload; the
        // Ok/Some type of the enclosing function is unrelated to it.
        expect_ok(
            r#"fn greeting() -> Result<String, String> {
    return Ok(String::from("hello"));
}

fn collect_values() -> Result<Vec<i64>, String> {
    return Ok(vec![1, 2, 3]);
}

struct Payload {
    id: i64,
}

fn payload() -> Result<Payload, String> {
    return Ok(Payload { id: 4 });
}

fn combine(text_value: Option<String>, number: Option<i64>) -> Option<i64> {
    let text: String = text_value?;
    let value: i64 = number?;
    Some(text.len() as i64 + value)
}

fn main() {
    let text: String = greeting().unwrap();
    let values: Vec<i64> = collect_values().unwrap();
    let item: Payload = payload().unwrap();
    let sum: i64 = combine(Some(text), Some(item.id)).unwrap();
    println!("{} {} {} {}", values.len(), item.id, sum, values[0]);
}"#,
        );
    }

    #[test]
    fn reports_try_operator_error_type_mismatches() {
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
        assert_eq!(init.ty, Ty::Option(Box::new(Ty::Int(IntTy::I32))));
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
        assert_eq!(init.ty, Ty::Int(IntTy::I32));
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
        assert_eq!(filter_init.ty, Ty::Option(Box::new(Ty::Int(IntTy::I32))));
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
            Ty::Result(Box::new(Ty::Int(IntTy::I32)), Box::new(Ty::String))
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
        let expected = shared_ref(shared_ref(Ty::Int(IntTy::I32)));
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
        assert_eq!(let_init(&main.body, 2).ty, shared_ref(Ty::Int(IntTy::I32)));
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
        assert_eq!(reference.ty, shared_ref(Ty::Int(IntTy::I32)));
        match &reference.kind {
            HExprKind::Ref { mutable, expr } => {
                assert!(!*mutable);
                assert!(matches!(&expr.kind, HExprKind::Local { name, .. } if name == "value"));
            }
            other => panic!("expected a borrow expression, got {:?}", other),
        }
        let copy = let_init(body, 2);
        assert_eq!(copy.ty, Ty::Int(IntTy::I32));
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
        assert_eq!(reference.ty, mutable_ref(Ty::Int(IntTy::I32)));
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
            r#"fn length(values: &&Vec<i64>) -> usize {
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
    fn checks_recursive_types_in_linear_time() {
        // Regression guard: type_contains_ref_at used to walk recursive
        // struct types without a cycle guard, so a Node with two
        // Option<Box<Node>> children took exponential time (the 30 line
        // refs_box_rc fixture needed over 100 seconds).
        use std::time::{Duration, Instant};
        let started = Instant::now();
        expect_ok(
            r#"struct Node {
    value: i64,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

fn leaf(value: i64) -> Node {
    Node {
        value: value,
        left: None,
        right: None,
    }
}

fn sum(tree: &Option<Box<Node>>) -> i64 {
    match tree {
        Some(boxed) => {
            let current: &Node = boxed;
            current.value + sum(&current.left) + sum(&current.right)
        }
        None => 0,
    }
}

fn main() {
    let tree = leaf(1);
    println!("{}", sum(&Some(Box::new(tree))));
}"#,
        );
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "type checking recursive types took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn requires_a_lifetime_on_reference_struct_fields() {
        // Since v2.2 a struct field may hold one shared reference, but the
        // lifetime must be spelled out: the field would otherwise dangle.
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
            has_code(&diagnostics, "E0106"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "missing lifetime specifier"));
    }

    #[test]
    fn allows_shared_reference_struct_fields_with_lifetime() {
        expect_ok(
            r#"struct Holder<'a> {
    value: &'a i64,
}

fn main() {
    let number = 1;
    let holder = Holder { value: &number };
    println!("{}", holder.value);
}"#,
        );
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
    fn allows_references_stored_in_optional_values() {
        // Option payloads are value positions: Option<&T> is the return type
        // of the borrowed collection accessors, so it must stay legal.
        expect_ok(
            r#"fn main() {
    let number = 1;
    let wrapped: Option<&i64> = Some(&number);
    println!("{}", *wrapped.unwrap());
}"#,
        );
    }

    #[test]
    fn allows_references_stored_in_tuples() {
        // Tuples are value positions too; the borrow checker keeps the loan
        // alive for as long as the tuple (or one of its fields) is used.
        expect_ok(
            r#"fn main() {
    let number = 1;
    let pair: (&i64, i64) = (&number, 2);
    println!("{}", *pair.0);
}"#,
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
        assert!(
            has_code(&diagnostics, "E0119"),
            "the derived-trait rejection must carry a diagnostic code: {:?}",
            messages(&diagnostics)
        );
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
    fn allows_moving_drop_values_out_of_tuple_patterns() {
        // A tuple does not implement Drop, so its parts may be moved out of it
        // by a pattern even when a part has its own Drop impl.
        expect_ok(
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
    }

    #[test]
    fn allows_moving_drop_fields_out_of_non_drop_structs() {
        expect_ok(
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
    }

    #[test]
    fn allows_moving_drop_payloads_out_of_variant_patterns() {
        // Option does not implement Drop: the whole Tracked payload moves out.
        expect_ok(
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

    #[test]
    fn rejects_destructuring_a_drop_enum_by_value_with_non_copy_payloads() {
        let diagnostics = expect_errors(
            r#"enum Payload {
    Text(String),
    Number(i64),
}

impl Drop for Payload {
    fn drop(&mut self) {}
}

fn main() {
    let value = Payload::Text(String::from("x"));
    match value {
        Payload::Text(text) => println!("{}", text),
        Payload::Number(number) => println!("{}", number),
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
    fn rejects_destructuring_a_drop_enum_by_value_in_match_arms() {
        let diagnostics = expect_errors(
            r#"enum Payload {
    Text(String),
    Number(i64),
}

impl Drop for Payload {
    fn drop(&mut self) {}
}

fn main() {
    let value = Payload::Text(String::from("x"));
    match value {
        Payload::Text(text) => println!("{}", text),
        Payload::Number(number) => println!("{}", number),
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
    fn rejects_destructuring_a_drop_struct_by_value() {
        let diagnostics = expect_errors(
            r#"struct Payload {
    text: String,
}

impl Drop for Payload {
    fn drop(&mut self) {}
}

fn main() {
    let value = Payload {
        text: String::from("x"),
    };
    let Payload { text } = value;
    println!("{}", text);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0509"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn allows_copying_fields_out_of_drop_types_by_value() {
        expect_ok(
            r#"struct Payload {
    number: i64,
}

impl Drop for Payload {
    fn drop(&mut self) {}
}

fn main() {
    let value = Payload { number: 3 };
    let Payload { number } = value;
    println!("{}", number);
}"#,
        );
    }

    #[test]
    fn allows_copying_payloads_out_of_drop_enums_by_value() {
        expect_ok(
            r#"enum Payload {
    Number(i64),
    Text(String),
}

impl Drop for Payload {
    fn drop(&mut self) {}
}

fn main() {
    let value = Payload::Number(3);
    match value {
        Payload::Number(number) => println!("{}", number),
        Payload::Text(_) => {}
    }
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
    fn allows_mutable_references_to_user_structs_containing_heap_values() {
        // Every sized user struct, enum and tuple is a valid mutable target:
        // the borrow checker enforces exclusive access.
        expect_ok(
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
    }

    #[test]
    fn allows_mutable_references_in_tuples() {
        // v2.2 allows shared references in struct/enum fields and both shared
        // and mutable references in tuples, Option and Result payloads. Only
        // mutable struct/enum *fields* are still rejected (see
        // validate_field_type).
        expect_ok(
            r#"fn main() {
    let mut text = String::from("hi");
    let pair: (&mut String, i64) = (&mut text, 1);
    println!("{}", pair.1);
}"#,
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
    fn allows_if_let_bindings_of_drop_glue_payloads() {
        expect_ok(
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
        return format!("{}:{}", self.label, extra);
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

    /// A minimal standard library that declares the builtin marker types and
    /// implements methods for them, mirroring the real stdlib layout.
    const STANDARD_LIBRARY: &str = r#"pub struct Vec<T> { internal: () }
pub struct String { internal: () }
pub struct Option<T> { internal: () }
pub struct Result<T, E> { internal: () }
pub struct Rc<T> { internal: () }
pub struct Box<T> { internal: () }
pub struct HashMap<K, V> { internal: () }

impl<T> Vec<T> {
    pub fn push(&mut self, value: T) {
        return;
    }

    pub fn len(&self) -> i64 {
        return 0;
    }
}

impl String {
    pub fn is_empty(&self) -> bool {
        return false;
    }
}

impl char {
    pub fn is_digit(&self) -> bool {
        return false;
    }
}

impl<T> Option<T> {
    pub fn present(&self) -> bool {
        return true;
    }
}

impl<T, E> Result<T, E> {
    pub fn failed(&self) -> bool {
        return false;
    }
}

impl<T> Rc<T> {
    pub fn rc_count(&self) -> i64 {
        return 1;
    }
}

impl<T> Box<T> {
    pub fn box_count(&self) -> i64 {
        return 1;
    }
}

impl<K, V> HashMap<K, V> {
    pub fn entry_count(&self) -> i64 {
        return 0;
    }
}"#;

    /// A minimal standard library with a trait implemented for a builtin type.
    const STANDARD_LIBRARY_WITH_TRAITS: &str = r#"pub struct Vec<T> { internal: () }
pub trait Countable {
    fn count_things(&self) -> i64;
}
impl<T> Countable for Vec<T> {
    fn count_things(&self) -> i64 {
        return 0;
    }
}"#;

    #[test]
    fn stdlib_impls_for_builtin_markers_resolve() {
        expect_ok_with_stdlib(
            r#"fn main() {
    let mut values: Vec<i64> = Vec::new();
    values.push(1);
    let count = values.len();
    let name = String::new();
    let empty = name.is_empty();
    let digit = '7'.is_digit();
    let maybe: Option<i64> = None;
    let present = maybe.present();
    let result: Result<i64, String> = Err(String::new());
    let failed = result.failed();
    let shared: Rc<i64> = Rc::new(1);
    let shared_count = shared.rc_count();
    let boxed: Box<i64> = Box::new(2);
    let box_count = boxed.box_count();
    let mut map: HashMap<i64, i64> = HashMap::new();
    map.insert(1, 2);
    let entries = map.entry_count();
    println!("{} {} {}", count, digit, shared_count);
    println!("{} {}", box_count, entries);
    if empty || present || failed {
        println!("flags");
    }
}"#,
            STANDARD_LIBRARY,
        );
    }

    #[test]
    fn stdlib_trait_impls_for_builtin_markers_resolve() {
        expect_ok_with_stdlib(
            r#"fn main() {
    let values: Vec<i64> = Vec::new();
    println!("{}", values.count_things());
}"#,
            STANDARD_LIBRARY_WITH_TRAITS,
        );
    }

    #[test]
    fn stdlib_constructors_are_used_instead_of_legacy_builtins() {
        let program = expect_ok_with_stdlib(
            r#"fn main() {
    let values: Vec<i64> = Vec::new();
    let mut capacity: Vec<i64> = Vec::with_capacity(4);
    capacity.push(1);
    let names: Vec<String> = Vec::<String>::new();
    let buffer = String::new();
    let text = String::from(&buffer);
    let shared: Rc<i64> = Rc::new(1);
    let boxed: Box<i64> = Box::new(2);
    let mut map: HashMap<i64, String> = HashMap::new();
    println!("{}", buffer.is_empty());
}"#,
            STANDARD_LIBRARY_WITH_CONSTRUCTORS,
        );
        // Instances are named after the display form, e.g. `Vec<i64>::new`.
        let concrete: Vec<&str> = program
            .fns
            .iter()
            .filter(|function| !function.intrinsic)
            .map(|function| function.name.as_str())
            .collect();
        assert!(
            concrete
                .iter()
                .any(|name| name.ends_with("::with_capacity")),
            "the stdlib `Vec::with_capacity` impl must be instantiated, got {:?}",
            concrete
        );
        assert!(
            concrete.iter().any(|name| name.ends_with("::from")),
            "the stdlib `String::from` impl must be instantiated, got {:?}",
            concrete
        );
        let new_instances = concrete
            .iter()
            .filter(|name| name.ends_with("::new"))
            .count();
        assert_eq!(
            new_instances, 6,
            "every marker constructor must resolve through its stdlib impl, got {:?}",
            concrete
        );
        let intrinsics: Vec<&str> = program
            .fns
            .iter()
            .filter(|function| function.intrinsic)
            .map(|function| function.name.as_str())
            .collect();
        assert!(
            intrinsics.iter().any(|name| name.starts_with("__vec_new")),
            "the constructor impl must call the frozen intrinsic, got {:?}",
            intrinsics
        );
    }

    #[test]
    fn stdlib_constructors_infer_from_use_after_the_call() {
        let program = expect_ok_with_stdlib(
            r#"fn main() {
    let mut values = Vec::new();
    values.push(1);
    let mut names = Vec::new();
    names.push(String::new());
}"#,
            STANDARD_LIBRARY_WITH_CONSTRUCTORS,
        );
        assert!(
            program
                .fns
                .iter()
                .any(|function| function.name.ends_with("::new") && !function.intrinsic),
            "the deferred constructor instance must be checked and lowered, got {:?}",
            program
                .fns
                .iter()
                .map(|function| function.name.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn stdlib_constructor_errors_are_not_hidden_by_legacy_builtins() {
        let diagnostics = expect_errors_with_stdlib(
            r#"fn main() {
    let values: Vec<String> = Vec::new(1);
}"#,
            STANDARD_LIBRARY_WITH_CONSTRUCTORS,
        );
        assert!(
            has_code(&diagnostics, "E0061"),
            "a stdlib constructor with the wrong signature must report its own error, got {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn stdlib_constructors_check_struct_arguments_in_the_caller_module() {
        // The argument expression belongs to the call site: a struct literal
        // and a local function name must resolve in the caller's module, not in
        // the standard-library impl module the constructor was declared in.
        expect_ok_with_stdlib(
            r#"struct S { v: i64 }

fn main() {
    let boxed = Box::new(S { v: 1 });
}"#,
            STANDARD_LIBRARY_WITH_CONSTRUCTORS,
        );
    }

    #[test]
    fn stdlib_constructors_check_function_arguments_in_the_caller_module() {
        expect_ok_with_stdlib(
            r#"fn node() -> i64 {
    return 1;
}

fn main() {
    let boxed = Box::new(node());
}"#,
            STANDARD_LIBRARY_WITH_CONSTRUCTORS,
        );
    }

    #[test]
    fn stdlib_constructors_infer_tuple_arguments_in_the_caller_module() {
        expect_ok_with_stdlib(
            r#"fn main() {
    let pair_boxed = Box::new((8, false));
}"#,
            STANDARD_LIBRARY_WITH_CONSTRUCTORS,
        );
    }

    #[test]
    fn stdlib_constructors_with_a_hint_check_arguments_in_the_caller_module() {
        expect_ok_with_stdlib(
            r#"struct S { v: i64 }

fn main() {
    let boxed: Box<S> = Box::new(S { v: 2 });
    let shared = Rc::new(S { v: 3 });
}"#,
            STANDARD_LIBRARY_WITH_CONSTRUCTORS,
        );
    }

    /// A minimal standard library whose marker impls provide constructors and
    /// call the frozen intrinsic ABI.
    const STANDARD_LIBRARY_WITH_CONSTRUCTORS: &str = r#"pub struct Vec<T> { internal: () }
pub struct String { internal: () }
pub struct Rc<T> { internal: () }
pub struct Box<T> { internal: () }
pub struct HashMap<K, V> { internal: () }

extern fn __vec_new<T>() -> Vec<T>;
extern fn __vec_with_capacity<T>(capacity: i64) -> Vec<T>;
extern fn __string_new() -> String;
extern fn __string_from_str(value: &String) -> String;
extern fn __rc_new<T>(value: T) -> Rc<T>;
extern fn __box_new<T>(value: T) -> Box<T>;
extern fn __map_new<K, V>() -> HashMap<K, V>;

impl<T> Vec<T> {
    pub fn new() -> Vec<T> {
        return __vec_new::<T>();
    }

    pub fn with_capacity(capacity: i64) -> Vec<T> {
        return __vec_with_capacity::<T>(capacity);
    }

    pub fn push(&mut self, value: T) {
        return;
    }
}

impl String {
    pub fn new() -> String {
        return __string_new();
    }

    pub fn from(value: &String) -> String {
        return __string_from_str(value);
    }

    pub fn is_empty(&self) -> bool {
        return false;
    }
}

impl<T> Rc<T> {
    pub fn new(value: T) -> Rc<T> {
        return __rc_new::<T>(value);
    }
}

impl<T> Box<T> {
    pub fn new(value: T) -> Box<T> {
        return __box_new::<T>(value);
    }
}

impl<K, V> HashMap<K, V> {
    pub fn new() -> HashMap<K, V> {
        return __map_new::<K, V>();
    }
}"#;

    #[test]
    fn stdlib_marker_reexports_from_modules_work() {
        let stdlib = r#"
mod vec {
    pub struct Vec<T> {
        internal: (),
    }
    impl<T> Vec<T> {
        pub fn push(&mut self, value: T) {
            return;
        }
        pub fn len(&self) -> i64 {
            return 0;
        }
    }
}
mod ops {
    use super::vec::Vec;
    impl<T> Vec<T> {
        pub fn peek(&self) -> i64 {
            return 3;
        }
    }
}
pub use vec::Vec;
"#;
        let program = expect_ok_with_stdlib(
            r#"fn main() {
    let mut values: Vec<i64> = Vec::new();
    values.push(1);
    println!("{}", values.len());
    println!("{}", values.peek());
}"#,
            stdlib,
        );
        assert!(
            program.structs.is_empty(),
            "builtin marker structs must not lower to HIR structs"
        );
    }

    #[test]
    fn stdlib_crate_paths_resolve() {
        let stdlib = r#"
pub mod vec {
    pub struct Vec<T> {
        internal: (),
    }
    impl<T> Vec<T> {
        pub fn push(&mut self, value: T) {
            return;
        }
        pub fn len(&self) -> i64 {
            return 0;
        }
    }
}
pub mod collections {
    pub struct HashMap<K, V> {
        internal: (),
    }
    pub use crate::vec::Vec;
}
pub use vec::Vec;
"#;
        expect_ok_with_stdlib(
            r#"use std::vec::Vec;
use std::collections::HashMap;
fn main() {
    let mut values: Vec<i64> = Vec::new();
    values.push(1);
    println!("{}", values.len());
    let mut map: HashMap<i64, i64> = HashMap::new();
    map.insert(1, 2);
    println!("{}", map.len());
}"#,
            stdlib,
        );
    }

    #[test]
    fn inherent_methods_take_priority_over_trait_methods() {
        expect_ok(
            r#"trait Describe { fn describe(&self) -> i64; }
struct Boxed<T> { inner: T }
impl<T> Boxed<T> {
    fn describe(&self) -> i64 { return 1; }
}
impl<T> Describe for Boxed<T> {
    fn describe(&self) -> i64 { return 2; }
}
fn main() {
    let boxed: Boxed<i64> = Boxed { inner: 5 };
    println!("{}", boxed.describe());
}"#,
        );
    }

    #[test]
    fn reports_builtin_marker_arity_errors() {
        let diagnostics =
            expect_errors_with_stdlib("fn main() {}", "pub struct Vec { internal: () }");
        assert!(has_code(&diagnostics, "E0107"));
        assert!(has_message(
            &diagnostics,
            "builtin type Vec takes 1 type argument but 0 were supplied"
        ));
    }

    #[test]
    fn rejects_user_redefinition_of_builtin_markers() {
        let diagnostics = expect_errors_with_stdlib(
            r#"struct Vec<T> { inner: T }
struct String { text: i64 }
fn main() {}"#,
            STANDARD_LIBRARY,
        );
        assert!(has_code(&diagnostics, "E0428"));
        assert!(has_message(
            &diagnostics,
            "the name 'Vec' is defined multiple times"
        ));
        assert!(has_message(
            &diagnostics,
            "the name 'String' is defined multiple times"
        ));
        // The names stay reserved even without a standard library loaded.
        let diagnostics = expect_errors(
            r#"struct Vec<T> { inner: T }
fn main() {}"#,
        );
        assert!(has_code(&diagnostics, "E0428"));
    }

    #[test]
    fn user_crates_cannot_impl_builtin_markers() {
        let diagnostics = expect_errors_with_stdlib(
            r#"impl<T> Vec<T> {
    pub fn extra(&self) -> i64 { return 1; }
}
fn main() {}"#,
            STANDARD_LIBRARY,
        );
        assert!(has_code(&diagnostics, "E0118"));
        assert!(has_message(&diagnostics, "found Vec<T>"));
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
            "HashMap keys must be one of i64, u64, isize, usize, bool, char or String, found Vec<i64>"
        ));
        assert!(has_note(
            &diagnostics,
            "only the listed key types are supported; use a struct field or a String key instead"
        ));
        let key_errors = messages(&diagnostics)
            .iter()
            .filter(|message| {
                message.contains(
                    "HashMap keys must be one of i64, u64, isize, usize, bool, char or String",
                )
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
    let iterator = values.iter();
    let total = iterator.flatten();
    let _ = total;
    println!("done");
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
            Ok(_) => panic!(".flatten() must be rejected as an unsupported iterator method"),
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
    // Version 1.6: char and UTF-8 String API
    // -----------------------------------------------------------------

    #[test]
    fn accepts_char_values_in_all_positions() {
        let program = expect_ok(
            r#"struct Glyph { symbol: char, weight: i64 }
enum Token { Letter(char), Digit(char) }
fn identify(symbol: char) -> char {
    return symbol;
}
fn pick(flag: bool) -> Option<char> {
    if flag {
        return Some('x');
    }
    return None;
}
fn main() {
    let symbol: char = 'a';
    let copied = symbol;
    println!("{}", copied);
    let mut mutation = 'a';
    mutation = 'b';
    println!("{}", mutation);
    let glyph = Glyph { symbol: 'b', weight: 1 };
    println!("{}", glyph.symbol);
    let letters: Vec<char> = vec!['a', 'b'];
    println!("{}", letters.len());
    let picked: Option<char> = pick(true);
    println!("{}", picked.is_some());
    let result: Result<char, String> = Ok('z');
    println!("{}", result.is_ok());
    let boxed: Box<char> = Box::new('q');
    let shared: Rc<char> = Rc::new('r');
    let tuple: (char, i64) = ('t', 2);
    println!("{}", tuple.0);
    let token: Token = Token::Letter('k');
    match token {
        Token::Letter(letter) => println!("{}", letter),
        Token::Digit(digit) => println!("{}", digit),
    }
    println!("{}", identify('m'));
}"#,
        );
        assert_eq!(program.structs[0].fields[0].ty, Ty::Char);
        assert_eq!(program.enums[0].variants[0].fields[0], Ty::Char);
        let mut saw_box_char = false;
        let mut saw_rc_char = false;
        for stmt in &main_fn(&program).body.stmts {
            if let HStmt::Let {
                init: Some(init), ..
            } = stmt
            {
                if init.ty == Ty::Box(Box::new(Ty::Char)) {
                    saw_box_char = true;
                }
                if init.ty == Ty::Rc(Box::new(Ty::Char)) {
                    saw_rc_char = true;
                }
            }
        }
        assert!(saw_box_char, "missing a Box<char> binding");
        assert!(saw_rc_char, "missing a Rc<char> binding");
    }

    #[test]
    fn accepts_char_comparisons() {
        expect_ok(
            r#"fn rank(a: char, b: char) -> i64 {
    if a == b {
        return 0;
    }
    if a != b {
        return 1;
    }
    if a < b {
        return 2;
    }
    if a <= b {
        return 3;
    }
    if a > b {
        return 4;
    }
    if a >= b {
        return 5;
    }
    return 6;
}
fn main() {
    println!("{}", rank('a', 'b'));
}"#,
        );
    }

    #[test]
    fn rejects_char_arithmetic_and_bitwise_operations() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let left = 'a';
    let right = 'b';
    let _sum = left + right;
    let _difference = left - right;
    let _product = left * right;
    let _quotient = left / right;
    let _remainder = left % right;
    let _negated = -left;
    let _inverted = !left;
    let _anded = left & right;
    let _ored = left | right;
    let _xored = left ^ right;
    let _mixed = left + 1;
}"#,
        );
        let rendered = messages(&diagnostics);
        let arithmetic = rendered
            .iter()
            .filter(|message| message.contains("cannot apply arithmetic to values of type char"))
            .count();
        assert_eq!(
            arithmetic, 6,
            "arithmetic on char must be rejected for + - * / % and unary -: {:?}",
            rendered
        );
        let bitwise = rendered
            .iter()
            .filter(|message| {
                message.contains("cannot apply a bitwise operation to values of type char")
            })
            .count();
        assert_eq!(
            bitwise, 4,
            "bitwise operators ! & | ^ must be rejected for char: {:?}",
            rendered
        );
        assert!(has_code(&diagnostics, "E0308"), "{:?}", rendered);
        assert!(
            has_message(&diagnostics, "mismatched types"),
            "{:?}",
            rendered
        );
    }

    #[test]
    fn accepts_char_casts() {
        let program = expect_ok(
            r#"fn main() {
    let symbol = 'A';
    let as_i8: i8 = symbol as i8;
    let as_i16: i16 = symbol as i16;
    let as_i32: i32 = symbol as i32;
    let as_i64: i64 = symbol as i64;
    let as_u8: u8 = symbol as u8;
    let as_u16: u16 = symbol as u16;
    let as_u32: u32 = symbol as u32;
    let as_u64: u64 = symbol as u64;
    let as_f32: f32 = symbol as f32;
    let as_f64: f64 = symbol as f64;
    let byte: u8 = 65;
    let restored: char = byte as char;
    let same: char = symbol as char;
    println!("{}", as_i64);
    println!("{}", as_f64);
    println!("{}", restored);
    println!("{}", same);
}"#,
        );
        assert!(!main_fn(&program).body.stmts.is_empty());
    }

    #[test]
    fn rejects_invalid_char_casts() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let wide: i64 = 65;
    let narrowed = wide as char;
    let small: i32 = 66;
    let also_narrowed = small as char;
    let truth = 'a' as bool;
    println!("{}", narrowed);
    println!("{}", also_narrowed);
    println!("{}", truth);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0308"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "only u8 can be cast as char"));
        assert!(has_message(&diagnostics, "cannot cast char as bool"));
    }

    #[test]
    fn accepts_char_constants() {
        let program = expect_ok(
            r#"const FIRST: char = 'a';
const SECOND: char = 'b';
const CODE: i64 = 'a' as i64;
const LETTER: char = (97 as u8) as char;
const ORDERED: bool = 'a' < 'b';
fn main() {
    println!("{}", FIRST == SECOND);
    println!("{}", FIRST < SECOND);
    println!("{}", CODE);
    println!("{}", LETTER);
    let symbol: char = FIRST;
    println!("{}", symbol);
}"#,
        );
        let constant = |name: &str| {
            program
                .consts
                .iter()
                .find(|constant| constant.name == name)
                .unwrap_or_else(|| panic!("missing constant {}", name))
        };
        assert_eq!(program.consts.len(), 5);
        assert_eq!(constant("FIRST").ty, Ty::Char);
        assert_eq!(constant("FIRST").value, ConstValue::Char('a'));
        assert_eq!(constant("SECOND").value, ConstValue::Char('b'));
        assert_eq!(constant("CODE").ty, Ty::Int(IntTy::I64));
        assert_eq!(constant("CODE").value, ConstValue::Int(97));
        assert_eq!(constant("LETTER").ty, Ty::Char);
        assert_eq!(constant("LETTER").value, ConstValue::Char('a'));
        assert_eq!(constant("ORDERED").ty, Ty::Bool);
        assert_eq!(constant("ORDERED").value, ConstValue::Bool(true));
        assert_eq!(main_fn(&program).body.stmts.len(), 6);
    }

    #[test]
    fn rejects_integer_literal_character_constants() {
        // Numeric -> char casts are only legal from a value already known to
        // be u8, so integer literals in char constants fail the cast rule.
        // The old E0080 "not a valid character" folding path is consequently
        // unreachable from source (u8 values are always valid scalar values);
        // it is kept in the constant folder as a defensive check.
        let diagnostics = expect_errors(
            r#"const BROKEN: char = 1114112 as char;
const SURROGATE: char = 55296 as char;
const LETTER: char = 97 as char;
fn main() {
    println!("{}{}{}", BROKEN, SURROGATE, LETTER);
}"#,
        );
        let cast_errors = messages(&diagnostics)
            .into_iter()
            .filter(|message| message.contains("only u8 can be cast as char"))
            .count();
        assert_eq!(cast_errors, 3, "{:?}", messages(&diagnostics));
        // The constant folder still evaluates the rejected cast expression and
        // reports the invalid scalar as a secondary E0080 diagnostic for the
        // two out-of-range literals; source-level programs without constants
        // only see the E0308 cast error.
        let value_errors = messages(&diagnostics)
            .into_iter()
            .filter(|message| message.contains("not a valid character"))
            .count();
        assert_eq!(value_errors, 2, "{:?}", messages(&diagnostics));
        assert!(
            has_code(&diagnostics, "E0308"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(
            has_code(&diagnostics, "E0080"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_unsuffixed_integer_literals_cast_as_char() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let thousand = 1000 as char;
    let letter = 65 as char;
    let three_hundred = 300 as char;
    println!("{}{}{}", thousand, letter, three_hundred);
}"#,
        );
        let cast_errors = messages(&diagnostics)
            .into_iter()
            .filter(|message| message.contains("only u8 can be cast as char"))
            .count();
        assert_eq!(cast_errors, 3, "{:?}", messages(&diagnostics));
        assert!(
            has_code(&diagnostics, "E0308"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_float_casts_as_char() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let literal = 1.5 as char;
    let typed: f64 = 2.5;
    let narrowed = typed as char;
    println!("{}{}", literal, narrowed);
}"#,
        );
        let cast_errors = messages(&diagnostics)
            .into_iter()
            .filter(|message| message.contains("non-primitive cast"))
            .count();
        assert_eq!(cast_errors, 2, "{:?}", messages(&diagnostics));
        assert!(
            has_code(&diagnostics, "E0605"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn accepts_u8_to_char_casts_and_folds_them() {
        let program = expect_ok(
            r#"const LETTER: char = (97 as u8) as char;
fn main() {
    let byte: u8 = 65;
    let restored: char = byte as char;
    let literal: char = (66 as u8) as char;
    println!("{}{}{}", LETTER, restored, literal);
}"#,
        );
        let letter = program
            .consts
            .iter()
            .find(|constant| constant.name == "LETTER")
            .unwrap_or_else(|| panic!("missing constant LETTER"));
        assert_eq!(letter.ty, crate::types::Ty::Char);
        assert_eq!(letter.value, ConstValue::Char('a'));
    }

    #[test]
    fn rejects_non_exhaustive_char_matches() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let symbol = 'a';
    match symbol {
        'a' => println!("a"),
        'b' => println!("b"),
    }
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0004"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "not covered"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn accepts_char_matches_with_catch_all_or_alternatives() {
        expect_ok(
            r#"fn describe(symbol: char) -> i64 {
    let mut result = 0;
    match symbol {
        'a' | 'b' => result = 1,
        _ => result = 2,
    }
    return result;
}
fn main() {
    println!("{}", describe('a'));
}"#,
        );
    }

    #[test]
    fn accepts_string_character_methods() {
        let program = expect_ok(
            r#"fn main() {
    let mut text: String = String::from("hello");
    let count = text.char_count();
    let chars: Vec<char> = text.chars();
    let second = text.char_at(1);
    let boundary = text.is_char_boundary(1);
    text.push('!');
    let last = text.pop();
    println!("{}", count);
    println!("{}", chars.len());
    println!("{}", second.is_some());
    println!("{}", boundary);
    println!("{}", last.is_some());
}"#,
        );
        let expected = [
            (1usize, HBuiltinMethod::StrCharCount, Ty::Int(IntTy::Usize)),
            (2, HBuiltinMethod::StrChars, Ty::Vec(Box::new(Ty::Char))),
            (3, HBuiltinMethod::StrCharAt, Ty::Option(Box::new(Ty::Char))),
            (4, HBuiltinMethod::StrIsCharBoundary, Ty::Bool),
        ];
        for (index, method, ty) in expected {
            let init = let_init(&main_fn(&program).body, index);
            assert_eq!(init.ty, ty, "result type of {:?}", method);
            match &init.kind {
                HExprKind::BuiltinMethod { method: found, .. } => {
                    assert_eq!(*found, method, "statement {}", index)
                }
                other => panic!("expected a builtin method call, got {:?}", other),
            }
        }
        match &main_fn(&program).body.stmts[5] {
            HStmt::Expr(expr) => match &expr.kind {
                HExprKind::BuiltinMethod {
                    method: HBuiltinMethod::StrPush,
                    args,
                    ..
                } => {
                    assert_eq!(args.len(), 1);
                    assert_eq!(args[0].ty, Ty::Char);
                }
                other => panic!("expected String::push, got {:?}", other),
            },
            other => panic!("expected an expression statement, got {:?}", other),
        }
        let pop_init = let_init(&main_fn(&program).body, 6);
        assert_eq!(pop_init.ty, Ty::Option(Box::new(Ty::Char)));
        match &pop_init.kind {
            HExprKind::BuiltinMethod {
                method: HBuiltinMethod::StrPop,
                ..
            } => {}
            other => panic!("expected String::pop, got {:?}", other),
        }
    }

    #[test]
    fn accepts_string_from_char() {
        let program = expect_ok(
            r#"fn main() {
    let text: String = String::from_char('x');
    println!("{}", text);
}"#,
        );
        let init = let_init(&main_fn(&program).body, 0);
        assert_eq!(init.ty, Ty::String);
        match &init.kind {
            HExprKind::BuiltinAssoc {
                func: HBuiltinAssoc::StringFromChar,
                args,
                ..
            } => {
                assert_eq!(args.len(), 1);
                assert_eq!(args[0].ty, Ty::Char);
            }
            other => panic!("expected String::from_char, got {:?}", other),
        }
    }

    #[test]
    fn accepts_char_format_specifiers() {
        expect_ok(
            r#"fn main() {
    let symbol = 'x';
    println!("{}", symbol);
    println!("{:?}", symbol);
    println!("{:>4}", symbol);
    println!("{:<4}", symbol);
    println!("{:^5}", symbol);
    println!("{:*>5}", symbol);
    println!("{:>4?}", symbol);
    println!("{:?} {}", symbol, symbol);
}"#,
        );
    }

    #[test]
    fn rejects_unsupported_char_format_flags() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let symbol = 'x';
    println!("{:.2}", symbol);
    println!("{:+}", symbol);
    println!("{:05}", symbol);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0001"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(
            &diagnostics,
            "the precision of a format specifier is not supported for characters"
        ));
        assert!(has_message(
            &diagnostics,
            "the '+' flag of a format specifier is not supported for characters"
        ));
        assert!(has_message(
            &diagnostics,
            "the '0' flag of a format specifier is not supported for characters"
        ));
    }

    #[test]
    fn accepts_char_hash_map_keys() {
        expect_ok(
            r#"fn main() {
    let mut map: HashMap<char, i64> = HashMap::new();
    map.insert('a', 1);
    map.insert('b', 2);
    println!("{}", map.contains_key('a'));
    println!("{}", map.get('b').is_some());
    println!("{}", map.len());
    let keys: Vec<char> = map.keys();
    println!("{}", keys.len());
    map.remove('a');
}"#,
        );
    }

    #[test]
    fn reports_hash_map_key_message_with_char() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let mut map: HashMap<f64, i64> = HashMap::new();
    map.insert(1.0, 2);
    println!("{}", map.len());
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0277"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(
            &diagnostics,
            "HashMap keys must be one of i64, u64, isize, usize, bool, char or String, found f64"
        ));
    }

    // -----------------------------------------------------------------
    // Reference-storage holes (v1.6 memory safety audit)
    // -----------------------------------------------------------------

    #[test]
    fn rejects_references_inside_owning_constructor_payloads() {
        // Since v2.2 the value positions Option and Result accept borrowing
        // payloads, while the owning heap pointers Box and Rc still reject
        // them: an inferred reference payload would dangle once its owner is
        // released (borrower audit F1).
        let diagnostics = expect_errors(
            r#"fn main() {
    let text = String::from("hello");
    let some = Some(&text);
    let boxed = Box::new(&text);
    let shared = Rc::new(&text);
    let nested = Some(Some(&text));
    let ok = Ok(&text);
    println!("{} {} {} {} {}", some.is_some(), boxed.get().len(), shared.strong_count(), nested.is_some(), ok.is_ok());
}"#,
        );
        let detected = messages(&diagnostics)
            .iter()
            .filter(|message| message.contains("references cannot be stored in this type"))
            .count();
        assert!(
            detected >= 2,
            "expected a Box and an Rc diagnostic, got {:?}",
            messages(&diagnostics)
        );
        assert!(
            has_code(&diagnostics, "E0498"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_references_escaping_blocks_through_option_payloads() {
        // Option payloads may borrow since v2.2, so the borrow checker (not
        // the storage guard) reports the escape as a lifetime error.
        let diagnostics = expect_errors(
            r#"fn main() {
    let escaped = {
        let value: i64 = 1;
        Some(&value)
    };
    println!("{}", escaped.is_some());
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0597"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "value does not live long enough"));
    }

    #[test]
    fn rejects_references_pushed_or_inserted_into_vectors() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let text = String::from("hi");
    let mut values = Vec::new();
    values.push(&text);
    values.insert(0, &text);
    let moved = text;
    println!("{} {}", values.len(), moved);
}"#,
        );
        let detected = messages(&diagnostics)
            .iter()
            .filter(|message| message.contains("references cannot be stored in this type"))
            .count();
        assert!(
            detected >= 2,
            "expected a diagnostic for push and insert, got {:?}",
            messages(&diagnostics)
        );
        assert!(
            has_code(&diagnostics, "E0498"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_reference_elements_in_vec_literals_with_resolved_message() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let text = String::from("hi");
    let values = vec![&text];
    println!("{}", values.len());
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0498"),
            "{:?}",
            messages(&diagnostics)
        );
        // The message must describe the resolved element type, not the
        // unresolved inference variable (borrower audit F7).
        assert!(
            has_message(
                &diagnostics,
                "references cannot be stored in this type (found &String)"
            ),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(
            !has_message(&diagnostics, "(found _)"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_reference_map_value_payloads() {
        // Result payloads are value positions and accept borrows; HashMap
        // values are owned storage and still reject them.
        let diagnostics = expect_errors(
            r#"fn main() {
    let text = String::from("hi");
    let mut map: HashMap<i64, &String> = HashMap::new();
    map.insert(1, &text);
    println!("{}", map.len());
}"#,
        );
        let detected = messages(&diagnostics)
            .iter()
            .filter(|message| message.contains("references cannot be stored in this type"))
            .count();
        assert!(
            detected >= 1,
            "expected a diagnostic for HashMap::insert, got {:?}",
            messages(&diagnostics)
        );
        assert!(
            has_code(&diagnostics, "E0498"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn reports_reference_map_keys_once_without_standard_library_spans() {
        let diagnostics = expect_errors_with_stdlib_program(
            r#"fn main() {
    let text = String::from("key");
    let mut map: HashMap<&String, i64> = HashMap::new();
    map.insert(&text, 1);
    println!("{}", map.len());
}"#,
        );
        // The instantiated standard-library signature reports the same
        // problem at its own declaration; only the user-facing diagnostic may
        // survive.
        let reference_storage = messages(&diagnostics)
            .iter()
            .filter(|message| message.contains("references cannot be stored in this type"))
            .count();
        assert_eq!(
            reference_storage,
            1,
            "expected exactly one user-facing diagnostic, got {:?}",
            messages(&diagnostics)
        );
        assert!(
            !messages(&diagnostics)
                .iter()
                .any(|message| message.contains("internal standard library error")),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn accepts_owned_constructor_and_container_payloads() {
        expect_ok(
            r#"fn main() {
    let boxed = Box::new(String::from("box"));
    let shared = Rc::new(String::from("rc"));
    let some = Some(String::from("some"));
    let values: Vec<String> = vec![String::from("a")];
    let mut list: Vec<String> = Vec::new();
    list.push(String::from("b"));
    list.insert(0, String::from("c"));
    let result: Result<i64, String> = Err(String::from("bad"));
    println!("{} {} {} {} {} {}", boxed.get().len(), shared.get().len(), some.is_some(), values.len(), list.len(), result.is_err());
}"#,
        );
    }

    // -----------------------------------------------------------------
    // Integer/float to bool casts (borrower audit F2)
    // -----------------------------------------------------------------

    #[test]
    fn rejects_numeric_casts_to_bool() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let literal = 1 as bool;
    let folded = (1 + 1) as bool;
    let float = 1.5 as bool;
    let number: i64 = 1;
    let typed = number as bool;
    let real: f64 = 1.0;
    let other = real as bool;
    println!("{} {} {} {} {} {}", literal, folded, float, typed, other, true);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0605"),
            "{:?}",
            messages(&diagnostics)
        );
        let detected = messages(&diagnostics)
            .iter()
            .filter(|message| message.contains("non-primitive cast"))
            .count();
        assert_eq!(
            detected,
            5,
            "every numeric to bool cast must be rejected: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn accepts_bool_to_integer_casts() {
        expect_ok(
            r#"fn main() {
    let flag = true;
    let number = flag as i64;
    let again = (number > 0) as i64;
    println!("{} {}", number, again);
}"#,
        );
    }

    // -----------------------------------------------------------------
    // Negative integer literal range folding (borrower audit F5/F8)
    // -----------------------------------------------------------------

    #[test]
    fn accepts_i64_min_literal() {
        expect_ok(
            r#"fn main() {
    let explicit: i64 = -9223372036854775808;
    let inferred: i64 = -9223372036854775808;
    let narrowed: i32 = -2147483648;
    println!("{} {} {}", explicit, inferred, narrowed);
}"#,
        );
    }

    #[test]
    fn rejects_positive_and_double_negated_out_of_range_literals() {
        let diagnostics = expect_errors(
            r#"fn main() {
    let too_big: i64 = 9223372036854775808;
    println!("{}", too_big);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0080"),
            "{:?}",
            messages(&diagnostics)
        );
        let diagnostics = expect_errors(
            r#"fn main() {
    let double: i64 = -(-9223372036854775808);
    println!("{}", double);
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0080"),
            "double negation must overflow: {:?}",
            messages(&diagnostics)
        );
    }

    // -----------------------------------------------------------------
    // Modules, use paths and visibility
    // -----------------------------------------------------------------

    fn parse_program(source: &str) -> ast::Program {
        let (program, _file, diagnostics) = parse_source(source);
        assert!(
            !diagnostics.has_errors(),
            "unexpected parse errors: {:?}",
            messages(&diagnostics)
        );
        program
    }

    fn check_crate_programs(
        crates: &[(&str, &str)],
        root: usize,
        require_main: bool,
    ) -> Result<hir::Program, Diagnostics> {
        let programs: Vec<ast::Program> = crates
            .iter()
            .map(|(_name, source)| parse_program(source))
            .collect();
        let mut sources = SourceMap::new();
        for (name, source) in crates {
            sources.add_file(*name, *source);
        }
        let graph = ast::CrateGraph {
            crates: programs
                .into_iter()
                .enumerate()
                .map(|(index, program)| ast::Crate {
                    name: crates[index].0.to_string(),
                    program,
                    is_dependency: index != root,
                })
                .collect(),
            root,
        };
        let options = CheckOptions::new().require_main(require_main);
        check_crate_reporting_with_options(&graph, None, &sources, &options).0
    }

    fn expect_crate_ok(crates: &[(&str, &str)], root: usize) -> hir::Program {
        match check_crate_programs(crates, root, true) {
            Ok(program) => program,
            Err(diagnostics) => panic!(
                "expected the crate graph to type check, got: {:?}",
                messages(&diagnostics)
            ),
        }
    }

    fn expect_crate_errors(crates: &[(&str, &str)], root: usize) -> Diagnostics {
        match check_crate_programs(crates, root, true) {
            Ok(_) => panic!("expected crate errors, but the graph type checked"),
            Err(diagnostics) => diagnostics,
        }
    }

    #[test]
    fn inline_module_paths_resolve() {
        expect_ok(
            r#"
mod math {
    pub fn add(left: i64, right: i64) -> i64 { left + right }
    pub const SCALE: i64 = 10;
    pub struct Meter { pub value: i64 }
    pub enum State { On, Off }
}
fn main() {
    let total = math::add(math::SCALE, 5);
    let meter = math::Meter { value: total };
    let state = math::State::On;
    match state {
        math::State::On => { println!("{}", meter.value); }
        math::State::Off => {}
    }
}
"#,
        );
    }

    #[test]
    fn nested_modules_and_super_paths_resolve() {
        expect_ok(
            r#"
mod outer {
    fn hidden() -> i64 { 1 }
    pub mod inner {
        pub fn call() -> i64 { super::hidden() + 1 }
        pub fn root_call() -> i64 { crate::top_level() }
    }
}
pub fn top_level() -> i64 { 3 }
fn main() {
    println!("{}", outer::inner::call());
    println!("{}", outer::inner::root_call());
}
"#,
        );
    }

    #[test]
    fn sibling_module_paths_resolve() {
        expect_ok(
            r#"
mod first {
    pub fn value() -> i64 { 1 }
}
mod second {
    pub fn value() -> i64 { super::first::value() + 1 }
}
fn main() { println!("{}", second::value()); }
"#,
        );
    }

    #[test]
    fn use_paths_import_items_and_aliases() {
        expect_ok(
            r#"
mod math {
    pub fn add(left: i64, right: i64) -> i64 { left + right }
    pub fn multiply(left: i64, right: i64) -> i64 { left * right }
    pub const SCALE: i64 = 2;
    pub struct Point { pub x: i64, pub y: i64 }
}
use math::{add, multiply as mul, SCALE, Point};
fn main() {
    let total = add(SCALE, mul(2, 3));
    let point = Point { x: total, y: 1 };
    println!("{} {}", point.x, point.y);
}
"#,
        );
    }

    #[test]
    fn use_imports_child_modules_and_reexports() {
        expect_ok(
            r#"
mod inner {
    pub fn answer() -> i64 { 42 }
}
mod api {
    pub use crate::inner::answer;
}
use api::answer;
fn main() {
    println!("{}", answer());
}
"#,
        );
    }

    #[test]
    fn use_imports_modules_directly() {
        expect_ok(
            r#"
mod holder {
    pub mod inner {
        pub fn value() -> i64 { 5 }
    }
}
use holder::inner;
fn main() { println!("{}", inner::value()); }
"#,
        );
    }

    #[test]
    fn imported_generic_functions_are_callable() {
        // A generic function belongs to the value namespace: importing it must
        // bind it where lookup_fn/lookup_generic_fn expect it, both in the root
        // module and inside nested modules.
        expect_ok(
            r#"
mod m {
    pub fn plain() -> i64 { 7 }
    pub fn g<T>(value: T) -> T { value }
}
use m::plain;
use m::g;
mod nested {
    use crate::m::g;
    pub fn call() -> i64 { g(2) }
}
fn main() {
    println!("{} {}", plain(), g(3));
    println!("{}", nested::call());
}
"#,
        );
    }

    #[test]
    fn imported_generic_functions_are_callable_from_the_standard_library_root() {
        // The standard library root re-exports its generic helper functions
        // with "pub use"; the prelude lookup must find them as values.
        expect_ok_with_stdlib(
            r#"
fn main() {
    println!("{}", identity(3));
    println!("{}", identity("text"));
}
"#,
            r#"
mod utils {
    pub fn identity<T>(value: T) -> T { value }
}
pub use utils::identity;
"#,
        );
    }

    #[test]
    fn imported_generic_function_conflicts_with_a_value_binding() {
        let diagnostics = expect_errors(
            r#"
mod a { pub fn g<T>(value: T) -> T { value } }
fn g() -> i64 { 1 }
use a::g;
fn main() { println!("{}", g()); }
"#,
        );
        assert!(
            has_code(&diagnostics, "E0255"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn use_self_and_super_paths_resolve() {
        expect_ok(
            r#"
mod outer {
    pub fn top() -> i64 { 7 }
    pub mod child {
        use super::top;
        pub fn call() -> i64 { top() }
    }
}
mod self_import {
    use self::helper::value;
    mod helper { pub fn value() -> i64 { 9 } }
    pub fn call() -> i64 { value() }
}
fn main() {
    println!("{}", outer::child::call());
    println!("{}", self_import::call());
}
"#,
        );
    }

    #[test]
    fn private_items_are_not_visible_through_module_paths() {
        let diagnostics = expect_errors(
            r#"
mod helper {
    fn hidden() -> i64 { 1 }
    pub fn visible() -> i64 { 2 }
}
fn main() { println!("{}", helper::hidden()); }
"#,
        );
        assert!(
            has_code(&diagnostics, "E0603"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "private"));
    }

    #[test]
    fn private_nested_modules_are_not_visible_outside() {
        let diagnostics = expect_errors(
            r#"
mod outer {
    mod inner {
        pub fn value() -> i64 { 1 }
    }
}
fn main() { println!("{}", outer::inner::value()); }
"#,
        );
        assert!(
            has_code(&diagnostics, "E0603"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn private_items_are_visible_to_descendant_modules() {
        expect_ok(
            r#"
mod outer {
    fn hidden() -> i64 { 1 }
    pub mod inner {
        pub fn call() -> i64 { super::hidden() }
    }
}
fn main() { println!("{}", outer::inner::call()); }
"#,
        );
    }

    #[test]
    fn unresolved_use_paths_report_e0432() {
        let diagnostics = expect_errors(
            r#"
mod helper {
    pub fn visible() -> i64 { 1 }
}
use helper::missing;
fn main() { println!("{}", missing()); }
"#,
        );
        assert!(
            has_code(&diagnostics, "E0432"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "unresolved import"));
    }

    #[test]
    fn super_at_the_crate_root_reports_e0433() {
        let diagnostics = expect_errors(
            r#"
pub fn value() -> i64 { 1 }
use super::value;
fn main() { println!("{}", value()); }
"#,
        );
        assert!(
            has_code(&diagnostics, "E0433"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "super"));
    }

    #[test]
    fn conflicting_imports_report_e0252() {
        let diagnostics = expect_errors(
            r#"
mod a { pub fn f() -> i64 { 1 } }
mod b { pub fn f() -> i64 { 2 } }
use a::f;
use b::f;
fn main() { println!("{}", f()); }
"#,
        );
        assert!(
            has_code(&diagnostics, "E0252"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn imports_conflicting_with_local_items_report_e0255() {
        let diagnostics = expect_errors(
            r#"
mod a { pub fn f() -> i64 { 1 } }
fn f() -> i64 { 2 }
use a::f;
fn main() { println!("{}", f()); }
"#,
        );
        assert!(
            has_code(&diagnostics, "E0255"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn duplicate_module_names_report_e0428() {
        let diagnostics = expect_errors(
            r#"
mod duplicated {}
mod duplicated {}
fn main() {}
"#,
        );
        assert!(
            has_code(&diagnostics, "E0428"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn glob_imports_are_rejected() {
        let diagnostics = expect_errors(
            r#"
mod a { pub fn f() -> i64 { 1 } }
use a::*;
fn main() { println!("{}", f()); }
"#,
        );
        assert!(
            has_code(&diagnostics, "E0432"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "glob"));
    }

    #[test]
    fn module_type_and_enum_paths_resolve() {
        expect_ok(
            r#"
mod types {
    pub struct Meter { pub value: i64 }
    pub enum State { On, Off }
    pub type Pair = (i64, i64);
}
fn measure(meter: types::Meter) -> i64 { meter.value }
fn classify(state: types::State) -> i64 {
    match state {
        types::State::On => 1,
        types::State::Off => 2,
    }
}
fn pair() -> types::Pair { (1, 2) }
fn main() {
    let meter = types::Meter { value: pair().0 };
    println!("{} {}", measure(meter), classify(types::State::On));
}
"#,
        );
    }

    #[test]
    fn private_fields_report_e0616_outside_their_module() {
        let diagnostics = expect_errors(
            r#"
mod point {
    pub struct Point { x: i64, y: i64 }
    pub fn make() -> Point { Point { x: 1, y: 2 } }
}
fn main() {
    let point = point::make();
    println!("{}", point.x);
}
"#,
        );
        assert!(
            has_code(&diagnostics, "E0616"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "private"));
    }

    #[test]
    fn public_fields_are_visible_across_modules() {
        expect_ok(
            r#"
mod point {
    pub struct Point { pub x: i64, pub y: i64 }
    pub fn make() -> Point { Point { x: 3, y: 4 } }
}
fn main() {
    let point = point::make();
    println!("{} {}", point.x, point.y);
}
"#,
        );
    }

    #[test]
    fn private_fields_in_struct_literals_report_e0451() {
        let diagnostics = expect_errors(
            r#"
mod point {
    pub struct Point { x: i64 }
    pub fn make() -> Point { Point { x: 1 } }
}
fn main() {
    let point = point::Point { x: 2 };
    println!("{}", point.x);
}
"#,
        );
        assert!(
            has_code(&diagnostics, "E0451"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn private_fields_in_patterns_report_e0451() {
        let diagnostics = expect_errors(
            r#"
mod point {
    pub struct Point { x: i64 }
    pub fn make() -> Point { Point { x: 1 } }
}
fn main() {
    let point = point::make();
    let point::Point { x } = point;
    println!("{}", x);
}
"#,
        );
        assert!(
            has_code(&diagnostics, "E0451"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn private_methods_report_e0624() {
        let diagnostics = expect_errors(
            r#"
mod counter {
    pub struct Counter { pub value: i64 }
    impl Counter {
        pub fn new(value: i64) -> Counter { Counter { value } }
        fn secret(&self) -> i64 { self.value }
    }
    pub fn make() -> Counter { Counter::new(1) }
}
fn main() {
    let counter = counter::make();
    println!("{}", counter.secret());
}
"#,
        );
        assert!(
            has_code(&diagnostics, "E0624"),
            "{:?}",
            messages(&diagnostics)
        );
        assert!(has_message(&diagnostics, "private"));
    }

    #[test]
    fn private_associated_functions_report_e0624() {
        let diagnostics = expect_errors(
            r#"
mod counter {
    pub struct Counter { pub value: i64 }
    impl Counter {
        fn new() -> Counter { Counter { value: 1 } }
        pub fn zero() -> Counter { Counter { value: 0 } }
    }
}
fn main() {
    let counter = counter::Counter::new();
    println!("{}", counter.value);
}
"#,
        );
        assert!(
            has_code(&diagnostics, "E0624"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn widening_item_reexports_report_e0364() {
        let diagnostics = expect_errors(
            r#"
mod helper {
    pub(crate) fn only_in_crate() -> i64 { 1 }
}
pub use helper::only_in_crate;
fn main() { println!("{}", only_in_crate()); }
"#,
        );
        assert!(
            has_code(&diagnostics, "E0364"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn widening_module_reexports_report_e0365() {
        let diagnostics = expect_errors(
            r#"
mod holder {
    pub(crate) mod inner {
        pub fn value() -> i64 { 1 }
    }
}
pub use holder::inner;
fn main() { println!("{}", inner::value()); }
"#,
        );
        assert!(
            has_code(&diagnostics, "E0365"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn deep_paths_into_missing_modules_report_e0433() {
        let diagnostics = expect_errors(
            r#"
mod a { pub fn f() -> i64 { 1 } }
fn main() { println!("{}", a::b::c::f()); }
"#,
        );
        assert!(
            has_code(&diagnostics, "E0433"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    // -----------------------------------------------------------------
    // Crate graphs, cross-crate visibility and entry points
    // -----------------------------------------------------------------

    #[test]
    fn dependency_crate_items_are_visible_to_the_root() {
        let program = expect_crate_ok(
            &[
                (
                    "greeting",
                    r#"
pub fn greet(name: String) -> String { String::from("hello, ") + &name }
pub const ANSWER: i64 = 42;
pub struct Counter { pub value: i64 }
pub fn counter() -> Counter { Counter { value: 7 } }
"#,
                ),
                (
                    "app",
                    r#"
use greeting::{counter, greet, ANSWER, Counter};
fn main() {
    let counted: Counter = counter();
    println!("{}", counted.value);
    println!("{}", ANSWER);
    println!("{}", greet(String::from("crate")));
}
"#,
                ),
            ],
            1,
        );
        assert!(find_fn(&program, "main").is_some());
    }

    #[test]
    fn pub_crate_items_are_not_visible_across_crates() {
        let diagnostics = expect_crate_errors(
            &[
                (
                    "hidden_api",
                    r#"
pub(crate) fn hidden() -> i64 { 1 }
pub fn visible() -> i64 { 2 }
"#,
                ),
                (
                    "app",
                    r#"
use hidden_api::hidden;
fn main() { println!("{}", hidden()); }
"#,
                ),
            ],
            1,
        );
        assert!(
            has_code(&diagnostics, "E0603"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn private_fields_of_dependency_structs_report_e0616() {
        let diagnostics = expect_crate_errors(
            &[
                (
                    "greeting",
                    r#"
pub struct Counter { value: i64 }
pub fn counter() -> Counter { Counter { value: 7 } }
"#,
                ),
                (
                    "app",
                    r#"
use greeting::counter;
fn main() {
    let counted = counter();
    println!("{}", counted.value);
}
"#,
                ),
            ],
            1,
        );
        assert!(
            has_code(&diagnostics, "E0616"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn public_fields_of_dependency_structs_are_visible() {
        let program = expect_crate_ok(
            &[
                (
                    "greeting",
                    r#"
pub struct Counter { pub value: i64 }
pub fn counter() -> Counter { Counter { value: 7 } }
"#,
                ),
                (
                    "app",
                    r#"
use greeting::counter;
fn main() {
    let counted = counter();
    println!("{}", counted.value);
}
"#,
                ),
            ],
            1,
        );
        assert!(find_fn(&program, "main").is_some());
    }

    #[test]
    fn library_crates_check_without_main_when_not_required() {
        let crates: &[(&str, &str)] = &[(
            "library",
            r#"
pub fn double(value: i64) -> i64 { value * 2 }
pub struct Config { pub level: i64 }
"#,
        )];
        let result = check_crate_programs(crates, 0, false);
        match result {
            Ok(program) => assert_eq!(program.main_fn, 0),
            Err(diagnostics) => panic!(
                "expected the library crate to type check, got: {:?}",
                messages(&diagnostics)
            ),
        }
    }

    #[test]
    fn crate_mode_requires_a_main_function_by_default() {
        let crates: &[(&str, &str)] = &[(
            "library",
            "pub fn double(value: i64) -> i64 { value * 2 }\n",
        )];
        let diagnostics = expect_crate_errors(crates, 0);
        assert!(
            has_code(&diagnostics, "E0601"),
            "{:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn the_root_crate_name_is_used_for_the_hir_program() {
        let program = expect_crate_ok(
            &[
                ("dep", "pub fn value() -> i64 { 1 }\n"),
                (
                    "named_root",
                    r#"
use dep::value;
fn main() { println!("{}", value()); }
"#,
                ),
            ],
            1,
        );
        assert_eq!(program.name, "named_root");
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
        // The fixture gate runs the same front-end entry point as the driver:
        // the embedded standard library is the prelude crate, so fixtures that
        // use slice/str/trait-object APIs resolve here exactly as they do in
        // a real compilation.
        let prelude = match crate::stdlib::program() {
            Ok(prelude) => prelude,
            Err(diagnostics) => panic!(
                "the embedded standard library must parse: {:?}",
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
                    "{} should type check, got: {:#?}",
                    name, diagnostics
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

    // -- v2.0 standard library migration regressions -------------------------

    #[test]
    fn mutable_references_to_hash_maps_are_allowed() {
        const STDLIB: &str = r#"
pub struct String { internal: () }
pub struct HashMap<K, V> { internal: () }
"#;
        expect_ok_with_stdlib(
            r#"
fn fill(map: &mut HashMap<String, i64>) {
    let _ = map;
}

fn main() {
    let mut map: HashMap<String, i64> = HashMap::new();
    fill(&mut map);
}
"#,
            STDLIB,
        );
    }

    #[test]
    fn local_bindings_shadow_prelude_generic_functions() {
        const STDLIB: &str = r#"
pub extern fn max<T>(a: T, b: T) -> T;
pub extern fn min<T>(a: T, b: T) -> T;
pub extern fn abs<T>(value: T) -> T;
pub extern fn sqrt<T>(value: T) -> T;
"#;
        expect_ok_with_stdlib(
            r#"
fn main() {
    let max: i64 = 9223372036854775807;
    println!("{}", max + 1);
    let min: i64 = -9223372036854775807;
    println!("{}", min - 1);
    let abs: i64 = 7;
    println!("{}", abs * 2);
    let sqrt: f64 = 9.0;
    println!("{}", sqrt);
}
"#,
            STDLIB,
        );
    }

    #[test]
    fn module_items_shadow_prelude_generic_functions() {
        const STDLIB: &str = r#"
pub extern fn max<T>(a: T, b: T) -> T;
"#;
        expect_ok_with_stdlib(
            r#"
fn max(a: i64, b: i64) -> i64 {
    if a > b {
        return a;
    }
    return b;
}

fn main() {
    println!("{}", max(1, 2));
}
"#,
            STDLIB,
        );
    }
    #[test]
    fn mutable_references_to_user_structs_allow_field_mutation() {
        let source = r#"
struct Person {
    age: i64,
    height: i64,
}

fn birthday(person: &mut Person) {
    person.age = person.age + 1;
}

fn grow(person: &mut Person, amount: i64) {
    person.height = person.height + amount;
}

fn main() {
    let mut person = Person { age: 36, height: 170 };
    birthday(&mut person);
    grow(&mut person, 2);
    println!("{} {}", person.age, person.height);
}
"#;
        expect_ok(source);
    }

    #[test]
    fn mutable_references_to_enums_and_tuples_are_allowed() {
        let source = r#"
enum State {
    Idle,
    Running(i64),
}

fn start(state: &mut State) {
    *state = State::Running(1);
}

fn reset(pair: &mut (i64, i64)) {
    pair.0 = 0;
}

fn main() {
    let mut state = State::Idle;
    start(&mut state);
    let mut pair = (1, 2);
    reset(&mut pair);
    println!("{} {}", pair.0, pair.1);
}
"#;
        expect_ok(source);
    }

    #[test]
    fn mutable_reference_targets_keep_the_place_rules() {
        let source = r#"
struct Holder {
    value: i64,
}

fn mutate(holder: &mut Holder) {
    holder.value = 1;
}

fn main() {
    let holder = Holder { value: 0 };
    mutate(&mut holder);
}
"#;
        let diagnostics = expect_errors(source);
        assert!(
            has_code(&diagnostics, "E0596"),
            "expected E0596 for a mutable borrow of an immutable binding: {:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "not declared as mutable"),
            "expected the mutable-borrow message: {:?}",
            messages(&diagnostics)
        );
    }

    // -- v2.0 phase B: trait objects, slices and str -------------------------

    const PHASE_B_STDLIB: &str = r#"pub struct Vec<T> { internal: () }
pub struct String { internal: () }
pub struct Rc<T> { internal: () }
pub struct Box<T> { internal: () }
pub struct HashMap<K, V> { internal: () }

extern fn __vec_new<T>() -> Vec<T>;
extern fn __vec_push<T>(values: &mut Vec<T>, value: T);
extern fn __vec_as_slice<Z>(values: &Vec<Z>) -> &[Z];
extern fn __slice_len<Z>(values: &[Z]) -> i64;
extern fn __slice_get<Z>(values: &[Z], index: i64) -> Option<Z>;
extern fn __string_new() -> String;
extern fn __string_from_str(value: String) -> String;
extern fn __string_as_str(value: &String) -> &str;
extern fn __rc_new<T>(value: T) -> Rc<T>;
extern fn __rc_get<T>(cell: &Rc<T>) -> T;
extern fn __box_new<T>(value: T) -> Box<T>;
extern fn __box_get<T>(cell: &Box<T>) -> T;
extern fn __map_new<K, V>() -> HashMap<K, V>;
extern fn __str_len(value: &str) -> i64;
extern fn __str_contains(value: &str, needle: &str) -> bool;

impl<T> Vec<T> {
    pub fn new() -> Vec<T> {
        return __vec_new::<T>();
    }
    pub fn push(&mut self, value: T) {
        return __vec_push::<T>(self, value);
    }
    pub fn len(&self) -> i64 {
        return __slice_len(__vec_as_slice(self));
    }
}

impl<T> [T] {
    pub fn len(&self) -> i64 {
        return __slice_len(self);
    }
    pub fn get(&self, index: i64) -> Option<T> {
        return __slice_get(self, index);
    }
}

impl str {
    pub fn len(&self) -> i64 {
        return __str_len(self);
    }
    pub fn contains(&self, needle: String) -> bool {
        return __str_contains(self, &needle);
    }
}

impl String {
    pub fn new() -> String {
        return __string_new();
    }
    pub fn from(value: String) -> String {
        return __string_from_str(value);
    }
    pub fn as_str(&self) -> &str {
        return __string_as_str(self);
    }
}

impl<T> Rc<T> {
    pub fn new(value: T) -> Rc<T> {
        return __rc_new::<T>(value);
    }
    pub fn get(&self) -> T {
        return __rc_get(self);
    }
}

impl<T> Box<T> {
    pub fn new(value: T) -> Box<T> {
        return __box_new::<T>(value);
    }
    pub fn get(&self) -> T {
        return __box_get(self);
    }
}

impl<K, V> HashMap<K, V> {
    pub fn new() -> HashMap<K, V> {
        return __map_new::<K, V>();
    }
}
"#;

    fn pb_walk_block<'a>(block: &'a crate::hir::HBlock, visit: &mut impl FnMut(&'a HExpr)) {
        for stmt in &block.stmts {
            match stmt {
                HStmt::Let { init, .. } => {
                    if let Some(init) = init {
                        pb_walk_expr(init, visit);
                    }
                }
                HStmt::Expr(expr) => pb_walk_expr(expr, visit),
            }
        }
        if let Some(tail) = &block.tail {
            pb_walk_expr(tail, visit);
        }
    }

    fn pb_walk_expr<'a>(expr: &'a HExpr, visit: &mut impl FnMut(&'a HExpr)) {
        visit(expr);
        match &expr.kind {
            HExprKind::Block(block) => pb_walk_block(block, visit),
            HExprKind::Ref { expr, .. } | HExprKind::Deref { expr } | HExprKind::Cast { expr } => {
                pb_walk_expr(expr, visit);
            }
            HExprKind::Unary { expr, .. } => pb_walk_expr(expr, visit),
            HExprKind::Binary { lhs, rhs, .. } => {
                pb_walk_expr(lhs, visit);
                pb_walk_expr(rhs, visit);
            }
            HExprKind::Assign { target, value, .. } => {
                pb_walk_expr(target, visit);
                pb_walk_expr(value, visit);
            }
            HExprKind::Call { callee, args } => {
                pb_walk_expr(callee, visit);
                for arg in args {
                    pb_walk_expr(arg, visit);
                }
            }
            HExprKind::DirectCall { receiver, args, .. } => {
                if let Some(receiver) = receiver {
                    pb_walk_expr(receiver, visit);
                }
                for arg in args {
                    pb_walk_expr(arg, visit);
                }
            }
            HExprKind::BuiltinMethod { receiver, args, .. } => {
                pb_walk_expr(receiver, visit);
                for arg in args {
                    pb_walk_expr(arg, visit);
                }
            }
            HExprKind::BuiltinAssoc { args, .. } | HExprKind::BuiltinCall { args, .. } => {
                for arg in args {
                    pb_walk_expr(arg, visit);
                }
            }
            HExprKind::Field { base, .. } => pb_walk_expr(base, visit),
            HExprKind::Index { base, index } => {
                pb_walk_expr(base, visit);
                pb_walk_expr(index, visit);
            }
            HExprKind::StructLit { fields, .. } => {
                for (_, value) in fields {
                    pb_walk_expr(value, visit);
                }
            }
            HExprKind::Tuple(elems) | HExprKind::VecLit { elems } => {
                for elem in elems {
                    pb_walk_expr(elem, visit);
                }
            }
            HExprKind::VecRepeat { value, count } => {
                pb_walk_expr(value, visit);
                pb_walk_expr(count, visit);
            }
            HExprKind::EnumLit { args, .. } => {
                for arg in args {
                    pb_walk_expr(arg, visit);
                }
            }
            HExprKind::OptionSome(value)
            | HExprKind::ResultOk(value)
            | HExprKind::ResultErr(value)
            | HExprKind::Return(Some(value)) => pb_walk_expr(value, visit),
            HExprKind::Format { parts } => {
                for part in parts {
                    if let crate::hir::HFormatPartKind::Arg { expr, .. } = &part.kind {
                        pb_walk_expr(expr, visit);
                    }
                }
            }
            HExprKind::Macro(call) => {
                for part in &call.parts {
                    if let crate::hir::HFormatPartKind::Arg { expr, .. } = &part.kind {
                        pb_walk_expr(expr, visit);
                    }
                }
                for arg in &call.args {
                    pb_walk_expr(arg, visit);
                }
            }
            HExprKind::DynCoerce { value, .. } => pb_walk_expr(value, visit),
            HExprKind::DynCall { object, args, .. } => {
                pb_walk_expr(object, visit);
                for arg in args {
                    pb_walk_expr(arg, visit);
                }
            }
            HExprKind::Slice {
                base, start, end, ..
            } => {
                pb_walk_expr(base, visit);
                if let Some(start) = start {
                    pb_walk_expr(start, visit);
                }
                if let Some(end) = end {
                    pb_walk_expr(end, visit);
                }
            }
            _ => {}
        }
    }

    fn pb_find_fn<'a>(program: &'a hir::Program, name: &str) -> &'a crate::hir::HFn {
        program
            .fns
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("missing function {}", name))
    }

    fn pb_exprs<'a>(
        program: &'a hir::Program,
        function: &str,
        mut want: impl FnMut(&HExpr) -> bool,
    ) -> Vec<&'a HExpr> {
        let target = pb_find_fn(program, function);
        let mut found = Vec::new();
        pb_walk_block(&target.body, &mut |expr| {
            if want(expr) {
                found.push(expr);
            }
        });
        found
    }

    fn pb_ref_ty(inner: Ty) -> Ty {
        Ty::Ref {
            mutable: false,
            inner: Box::new(inner),
        }
    }

    fn pb_mut_ref_ty(inner: Ty) -> Ty {
        Ty::Ref {
            mutable: true,
            inner: Box::new(inner),
        }
    }

    fn pb_box_ty(inner: Ty) -> Ty {
        Ty::Box(Box::new(inner))
    }

    fn pb_rc_ty(inner: Ty) -> Ty {
        Ty::Rc(Box::new(inner))
    }

    fn pb_i64() -> Ty {
        Ty::Int(IntTy::I64)
    }

    fn pb_slice_ty() -> Ty {
        Ty::Slice(Box::new(pb_i64()))
    }

    #[test]
    fn dyn_trait_objects_register_once_and_share_a_vtable() {
        let program = expect_ok_with_stdlib(
            r#"
trait Shape {
    fn area(&self) -> i64;
    fn label(&self) -> String;
}

struct Rect {
    width: i64,
    height: i64,
}

impl Shape for Rect {
    fn area(&self) -> i64 {
        return self.width * self.height;
    }
    fn label(&self) -> String {
        return String::from("rect");
    }
}

fn describe(shape: &dyn Shape) -> String {
    return format!("{}={}", shape.label(), shape.area());
}

fn main() {
    let rect = Rect { width: 2, height: 5 };
    println!("{}", describe(&rect));
    let boxed: Box<dyn Shape> = Box::new(Rect { width: 3, height: 4 });
    println!("{}", boxed.area());
}
"#,
            PHASE_B_STDLIB,
        );
        assert_eq!(program.trait_objects.len(), 1);
        let object = &program.trait_objects[0];
        assert_eq!(object.name, "Shape");
        assert!(object.args.is_empty());
        assert!(
            object.c_name.starts_with("LsVTable_Shape"),
            "unexpected vtable C name: {}",
            object.c_name
        );
        assert_eq!(object.methods.len(), 2);
        assert_eq!(object.methods[0].name, "area");
        assert_eq!(object.methods[0].receiver, SelfKind::Ref);
        assert!(object.methods[0].params.is_empty());
        assert_eq!(object.methods[0].ret, pb_i64());
        assert_eq!(object.methods[1].name, "label");
        assert_eq!(object.methods[1].ret, Ty::String);
        assert_eq!(program.vtables.len(), 1);
        assert_eq!(program.vtables[0].trait_object, 0);
        assert_eq!(program.vtables[0].self_ty, Ty::Struct(0));
        assert_eq!(program.vtables[0].methods.len(), 2);
        let coercions = pb_exprs(&program, "main", |expr| {
            matches!(expr.kind, HExprKind::DynCoerce { .. })
        });
        assert_eq!(coercions.len(), 2, "expected two trait-object coercions");
        let HExprKind::DynCoerce { value, vtable } = &coercions[0].kind else {
            unreachable!()
        };
        assert_eq!(*vtable, 0);
        assert_eq!(coercions[0].ty, pb_ref_ty(Ty::Dyn(0)));
        assert_eq!(value.ty, pb_ref_ty(Ty::Struct(0)));
        let HExprKind::DynCoerce { value, vtable } = &coercions[1].kind else {
            unreachable!()
        };
        assert_eq!(*vtable, 0);
        assert_eq!(coercions[1].ty, pb_box_ty(Ty::Dyn(0)));
        assert_eq!(value.ty, pb_box_ty(Ty::Struct(0)));
        let calls = pb_exprs(&program, "describe", |expr| {
            matches!(expr.kind, HExprKind::DynCall { .. })
        });
        let mut slots: Vec<usize> = calls
            .iter()
            .map(|expr| match &expr.kind {
                HExprKind::DynCall { slot, .. } => *slot,
                _ => unreachable!(),
            })
            .collect();
        slots.sort_unstable();
        assert_eq!(slots, vec![0, 1]);
        assert_eq!(calls[0].ty, Ty::String);
    }

    #[test]
    fn dyn_method_calls_select_slots_in_declaration_order() {
        let program = expect_ok_with_stdlib(
            r#"
trait Grow {
    fn grow(&mut self);
    fn value(&self) -> i64;
}

struct Cell {
    value: i64,
}

impl Grow for Cell {
    fn grow(&mut self) {
        self.value = self.value + 1;
    }
    fn value(&self) -> i64 {
        return self.value;
    }
}

fn bump(cell: &mut dyn Grow) {
    cell.grow();
}

fn read(cell: &dyn Grow) -> i64 {
    return cell.value();
}

fn main() {
    let mut cell = Cell { value: 1 };
    bump(&mut cell);
    println!("{}", read(&cell));
}
"#,
            PHASE_B_STDLIB,
        );
        assert_eq!(program.trait_objects.len(), 1);
        assert_eq!(program.trait_objects[0].methods[0].name, "grow");
        assert_eq!(
            program.trait_objects[0].methods[0].receiver,
            SelfKind::RefMut
        );
        assert_eq!(program.trait_objects[0].methods[1].receiver, SelfKind::Ref);
        let bump_calls = pb_exprs(&program, "bump", |expr| {
            matches!(expr.kind, HExprKind::DynCall { .. })
        });
        assert_eq!(bump_calls.len(), 1);
        let HExprKind::DynCall {
            object,
            trait_object,
            slot,
            args,
        } = &bump_calls[0].kind
        else {
            unreachable!()
        };
        assert_eq!(*trait_object, 0);
        assert_eq!(*slot, 0);
        assert!(args.is_empty());
        assert_eq!(object.ty, pb_mut_ref_ty(Ty::Dyn(0)));
        assert_eq!(bump_calls[0].ty, Ty::Unit);
        let read_calls = pb_exprs(&program, "read", |expr| {
            matches!(expr.kind, HExprKind::DynCall { .. })
        });
        assert_eq!(read_calls.len(), 1);
        let HExprKind::DynCall { slot, object, .. } = &read_calls[0].kind else {
            unreachable!()
        };
        assert_eq!(*slot, 1);
        assert_eq!(object.ty, pb_ref_ty(Ty::Dyn(0)));
        assert_eq!(read_calls[0].ty, pb_i64());
    }

    #[test]
    fn dyn_box_and_rc_handles_share_one_vtable() {
        let program = expect_ok_with_stdlib(
            r#"
trait Speak {
    fn say(&self) -> String;
}

struct Dog {
    name: String,
}

impl Speak for Dog {
    fn say(&self) -> String {
        return String::from("woof");
    }
}

fn main() {
    let boxed: Box<dyn Speak> = Box::new(Dog { name: String::from("rex") });
    let shared: Rc<dyn Speak> = Rc::new(Dog { name: String::from("rex") });
    println!("{} {}", boxed.say(), shared.say());
}
"#,
            PHASE_B_STDLIB,
        );
        assert_eq!(program.vtables.len(), 1);
        assert_eq!(program.vtables[0].self_ty, Ty::Struct(0));
        assert_eq!(program.vtables[0].methods.len(), 1);
        let coercions = pb_exprs(&program, "main", |expr| {
            matches!(expr.kind, HExprKind::DynCoerce { .. })
        });
        assert_eq!(coercions.len(), 2);
        let HExprKind::DynCoerce { value, .. } = &coercions[0].kind else {
            unreachable!()
        };
        assert_eq!(coercions[0].ty, pb_box_ty(Ty::Dyn(0)));
        assert_eq!(value.ty, pb_box_ty(Ty::Struct(0)));
        let HExprKind::DynCoerce { value, .. } = &coercions[1].kind else {
            unreachable!()
        };
        assert_eq!(coercions[1].ty, pb_rc_ty(Ty::Dyn(0)));
        assert_eq!(value.ty, pb_rc_ty(Ty::Struct(0)));
        let calls = pb_exprs(&program, "main", |expr| {
            matches!(expr.kind, HExprKind::DynCall { .. })
        });
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls
                .iter()
                .map(|expr| match &expr.kind {
                    HExprKind::DynCall { object, .. } => object.ty.clone(),
                    _ => unreachable!(),
                })
                .collect::<Vec<Ty>>(),
            vec![pb_box_ty(Ty::Dyn(0)), pb_rc_ty(Ty::Dyn(0))]
        );
    }

    #[test]
    fn explicit_slice_expressions_lower_to_reference_slice_nodes() {
        let program = expect_ok_with_stdlib(
            r#"
fn total(values: &[i64]) -> i64 {
    return values.len();
}

fn main() {
    let values = vec![10, 20, 30, 40, 50];
    println!("{}", total(&values[1..4]));
    println!("{}", total(&values[2..]));
    println!("{}", total(&values[..]));
    println!("{}", total(&values[..=1]));
}
"#,
            PHASE_B_STDLIB,
        );
        let refs = pb_exprs(&program, "main", |expr| {
            matches!(
                &expr.kind,
                HExprKind::Ref { expr, .. } if matches!(expr.kind, HExprKind::Slice { .. })
            )
        });
        assert_eq!(
            refs.len(),
            4,
            "expected four reference-to-slice expressions"
        );
        let mut shapes = Vec::new();
        for expr in &refs {
            assert_eq!(expr.ty, pb_ref_ty(pb_slice_ty()));
            let HExprKind::Ref {
                mutable,
                expr: inner,
            } = &expr.kind
            else {
                unreachable!()
            };
            assert!(!mutable);
            assert_eq!(inner.ty, pb_slice_ty());
            let HExprKind::Slice {
                base,
                start,
                end,
                inclusive,
            } = &inner.kind
            else {
                unreachable!()
            };
            assert_eq!(base.ty, Ty::Vec(Box::new(pb_i64())));
            shapes.push((start.is_some(), end.is_some(), *inclusive));
        }
        assert_eq!(
            shapes,
            vec![
                (true, true, false),
                (true, false, false),
                (false, false, false),
                (false, true, true),
            ]
        );
    }

    #[test]
    fn borrows_of_vectors_and_strings_coerce_to_slices_and_str() {
        let program = expect_ok_with_stdlib(
            r#"
fn main() {
    let values = vec![1, 2, 3];
    let text = String::from("hello");
    let part: &[i64] = &values;
    let view: &str = &text;
    println!("{} {}", part.len(), view.len());
}
"#,
            PHASE_B_STDLIB,
        );
        let slices = pb_exprs(&program, "main", |expr| {
            matches!(
                &expr.kind,
                HExprKind::Ref { expr, .. } if matches!(expr.kind, HExprKind::Slice { start: None, end: None, .. })
            )
        });
        assert_eq!(slices.len(), 2);
        let HExprKind::Ref { expr: part, .. } = &slices[0].kind else {
            unreachable!()
        };
        assert_eq!(slices[0].ty, pb_ref_ty(pb_slice_ty()));
        assert_eq!(part.ty, pb_slice_ty());
        let HExprKind::Slice { base, .. } = &part.kind else {
            unreachable!()
        };
        assert_eq!(base.ty, Ty::Vec(Box::new(pb_i64())));
        let HExprKind::Ref { expr: view, .. } = &slices[1].kind else {
            unreachable!()
        };
        assert_eq!(slices[1].ty, pb_ref_ty(Ty::Str));
        assert_eq!(view.ty, Ty::Str);
        let HExprKind::Slice { base, .. } = &view.kind else {
            unreachable!()
        };
        assert_eq!(base.ty, Ty::String);
        let direct = pb_exprs(&program, "main", |expr| {
            matches!(expr.kind, HExprKind::DirectCall { .. })
        });
        let names: Vec<String> = direct
            .iter()
            .map(|expr| match &expr.kind {
                HExprKind::DirectCall { fn_id, .. } => program.fns[*fn_id].name.clone(),
                _ => unreachable!(),
            })
            .collect();
        assert!(
            names.iter().any(|name| name.ends_with("::len")),
            "expected a slice/str len call, got {:?}",
            names
        );
    }

    #[test]
    fn trait_object_coercions_without_an_impl_are_rejected() {
        let diagnostics = expect_errors_with_stdlib(
            r#"
trait Named {
    fn name(&self) -> String;
}

struct Plain {
    value: i64,
}

fn main() {
    let value: Box<dyn Named> = Box::new(Plain { value: 1 });
}
"#,
            PHASE_B_STDLIB,
        );
        assert!(has_code(&diagnostics, "E0277"));
        assert!(
            has_message(&diagnostics, "is not implemented for"),
            "expected a missing-impl diagnostic: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn bare_trait_object_and_slice_values_are_unsized_placements() {
        let diagnostics = expect_errors_with_stdlib(
            r#"
trait Shape {
    fn area(&self) -> i64;
}

struct Rect {
    width: i64,
}

impl Shape for Rect {
    fn area(&self) -> i64 {
        return self.width;
    }
}

fn take_shape(shape: dyn Shape) -> i64 {
    return 0;
}

fn main() {
    let values = vec![1, 2, 3];
    let text = String::from("hello");
    let slice = values[1..2];
    let part = text[..2];
}
"#,
            PHASE_B_STDLIB,
        );
        assert!(
            has_message(&diagnostics, "trait objects are unsized"),
            "expected the trait-object unsized diagnostic: {:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "slices are unsized"),
            "expected the slice unsized diagnostic: {:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "str is unsized"),
            "expected the str unsized diagnostic: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn object_unsafe_traits_are_reported_only_when_used() {
        let diagnostics = expect_errors_with_stdlib(
            r#"
trait ByValue {
    fn consume(self) -> i64;
}

struct Thing {
    value: i64,
}

impl ByValue for Thing {
    fn consume(self) -> i64 {
        return self.value;
    }
}

fn static_use() -> i64 {
    let thing = Thing { value: 1 };
    return thing.consume();
}

fn main() {
    println!("{}", static_use());
    let boxed: Box<dyn ByValue> = Box::new(Thing { value: 2 });
    println!("{}", boxed.consume());
}
"#,
            PHASE_B_STDLIB,
        );
        assert!(
            has_code(&diagnostics, "E0038"),
            "expected the object-safety diagnostic: {:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "cannot be made into an object"),
            "expected the object-safety message: {:?}",
            messages(&diagnostics)
        );
        assert!(
            has_note(&diagnostics, "by value"),
            "expected the object-safety note: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn mutating_through_a_shared_trait_object_is_rejected() {
        let diagnostics = expect_errors_with_stdlib(
            r#"
trait Grow {
    fn grow(&mut self);
}

struct Cell {
    value: i64,
}

impl Grow for Cell {
    fn grow(&mut self) {
        self.value = self.value + 1;
    }
}

fn touch(cell: &dyn Grow) {
    cell.grow();
}

fn main() {
    let mut cell = Cell { value: 1 };
    touch(&cell);
}
"#,
            PHASE_B_STDLIB,
        );
        assert!(has_code(&diagnostics, "E0596"));
        assert!(
            has_message(&diagnostics, "cannot mutate through a shared reference"),
            "expected the shared-receiver diagnostic: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn equality_on_trait_objects_is_rejected() {
        let diagnostics = expect_errors_with_stdlib(
            r#"
trait Same {
    fn value(&self) -> i64;
}

struct Item {
    value: i64,
}

impl Same for Item {
    fn value(&self) -> i64 {
        return self.value;
    }
}

fn same(left: &dyn Same, right: &dyn Same) -> bool {
    return left == right;
}

fn main() {
    let item = Item { value: 1 };
    println!("{}", same(&item, &item));
}
"#,
            PHASE_B_STDLIB,
        );
        assert!(has_code(&diagnostics, "E0369"));
        assert!(
            has_message(&diagnostics, "binary operation =="),
            "expected the equality diagnostic: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn user_crates_cannot_implement_slice_or_str() {
        let diagnostics = expect_errors(
            r#"
impl [i64] {
    fn first(&self) -> i64 {
        return 0;
    }
}

impl str {
    fn first_char(&self) -> char {
        return 'a';
    }
}

fn main() {
}
"#,
        );
        assert!(
            has_message(&diagnostics, "found [i64]"),
            "expected the slice impl-target diagnostic: {:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "found str"),
            "expected the str impl-target diagnostic: {:?}",
            messages(&diagnostics)
        );
        assert!(has_code(&diagnostics, "E0118"));
    }

    #[test]
    fn slice_bounds_must_be_i64() {
        let diagnostics = expect_errors_with_stdlib(
            r#"
fn main() {
    let values = vec![1, 2, 3];
    let part = &values[0..true];
}
"#,
            PHASE_B_STDLIB,
        );
        assert!(has_code(&diagnostics, "E0308"));
        assert!(
            has_message(&diagnostics, "mismatched types"),
            "expected the slice-bound diagnostic: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn vec_push_coerces_concrete_boxes_into_trait_objects() {
        let program = expect_ok_with_stdlib(
            r#"
trait Shape {
    fn area(&self) -> i64;
}

struct Rect {
    width: i64,
    height: i64,
}

impl Shape for Rect {
    fn area(&self) -> i64 {
        return self.width * self.height;
    }
}

fn main() {
    let mut shapes: Vec<Box<dyn Shape>> = Vec::<Box<dyn Shape>>::new();
    shapes.push(Box::new(Rect { width: 2, height: 3 }));
    shapes.push(Box::new(Rect { width: 4, height: 5 }));
    println!("{}", shapes[0].area());
}
"#,
            PHASE_B_STDLIB,
        );
        assert_eq!(program.vtables.len(), 1);
        assert_eq!(program.vtables[0].self_ty, Ty::Struct(0));
        let coercions = pb_exprs(&program, "main", |expr| {
            matches!(expr.kind, HExprKind::DynCoerce { .. })
        });
        assert_eq!(coercions.len(), 2);
        for coercion in &coercions {
            let HExprKind::DynCoerce { value, vtable } = &coercion.kind else {
                unreachable!()
            };
            assert_eq!(*vtable, 0);
            assert_eq!(coercion.ty, pb_box_ty(Ty::Dyn(0)));
            assert_eq!(value.ty, pb_box_ty(Ty::Struct(0)));
        }
        let calls = pb_exprs(&program, "main", |expr| {
            matches!(expr.kind, HExprKind::DynCall { .. })
        });
        assert_eq!(calls.len(), 1);
        let HExprKind::DynCall { slot, object, .. } = &calls[0].kind else {
            unreachable!()
        };
        assert_eq!(*slot, 0);
        assert_eq!(object.ty, pb_box_ty(Ty::Dyn(0)));
        assert_eq!(calls[0].ty, pb_i64());
    }

    // -- v2.0 phase B: standard library module origins ----------------------

    #[test]
    fn stdlib_extern_functions_inside_modules_are_accepted() {
        let stdlib = r#"
pub mod vec {
    pub struct Vec<T> {
        internal: (),
    }
    pub extern fn __vec_new<T>() -> Vec<T>;
    impl<T> Vec<T> {
        pub fn new() -> Vec<T> {
            return __vec_new::<T>();
        }
    }
}
pub use vec::Vec;
"#;
        let program = expect_ok_with_stdlib(
            r#"fn main() {
    let values: Vec<i64> = Vec::new();
    println!("{}", values.len());
}"#,
            stdlib,
        );
        assert!(
            program.structs.is_empty(),
            "builtin marker structs must not lower to HIR structs"
        );
    }

    #[test]
    fn user_crates_cannot_declare_extern_functions_in_modules() {
        let diagnostics = expect_errors(
            r#"mod inner {
    extern fn __fake_abs(value: i64) -> i64;
}
fn main() {}"#,
        );
        assert!(has_code(&diagnostics, "E0001"));
        assert!(has_message(
            &diagnostics,
            "extern function declarations are only allowed in the standard library"
        ));
        assert!(has_note(
            &diagnostics,
            "extern functions belong to the standard library"
        ));
    }

    #[test]
    fn allows_moving_drop_payloads_out_of_result_values() {
        // Phase C standard library pattern: `match File::open(path) { Ok(file) => ... }`.
        // Result does not implement Drop, so the whole File value moves out of
        // the payload and File's own destructor still runs exactly once.
        expect_ok(
            r#"struct File {
    handle: i64,
}

impl Drop for File {
    fn drop(&mut self) {}
}

fn open() -> Result<File, String> {
    return Ok(File { handle: 1 });
}

fn main() {
    match open() {
        Ok(file) => { println!("{}", file.handle); }
        Err(error) => { println!("{}", error); }
    }
    if let Ok(other) = open() {
        println!("{}", other.handle);
    }
}"#,
        );
    }

    // -- v2.0 task-72: deferred standard-library instances -------------------

    /// Minimal standard library whose constructors are generic impl methods.
    /// A call without type information defers instantiation until every user
    /// body has been checked.
    const DEFERRED_E0282_STDLIB: &str = r#"
pub struct Vec<T> { internal: () }
pub struct Box<T> { internal: () }
pub struct String { internal: () }

pub extern fn __vec_new<T>() -> Vec<T>;
pub extern fn __box_new<T>(value: T) -> Box<T>;
pub extern fn __string_new() -> String;

impl<T> Vec<T> {
    pub fn new() -> Vec<T> {
        return __vec_new::<T>();
    }
}

impl<T> Box<T> {
    pub fn new(value: T) -> Box<T> {
        return __box_new::<T>(value);
    }
}

impl String {
    pub fn new() -> String {
        return __string_new();
    }

    pub fn buggy() -> String {
        let value: i64 = __string_new();
        let _ = value;
        return String::new();
    }
}
"#;

    #[test]
    fn unconstrained_stdlib_constructor_reports_a_user_e0282() {
        let source = r#"fn main() {
    let v = Vec::new();
    println!("{}", 1);
}"#;
        let diagnostics = expect_errors_with_stdlib(source, DEFERRED_E0282_STDLIB);
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .collect();
        assert_eq!(
            errors.len(),
            1,
            "exactly one error expected, got {:?}",
            messages(&diagnostics)
        );
        assert_eq!(errors[0].code, Some("E0282"));
        assert_eq!(errors[0].message, "type annotations needed");
        let start = source.find("Vec::new()").expect("call site") as u32;
        assert_eq!(errors[0].span().start, start);
        assert_eq!(errors[0].span().end, start + "Vec::new()".len() as u32);
        assert!(
            !messages(&diagnostics)
                .iter()
                .any(|message| message.contains("internal standard library error")),
            "a user inference failure must not be reported as an internal error"
        );
    }

    #[test]
    fn annotated_stdlib_constructor_calls_are_accepted() {
        expect_ok_with_stdlib(
            r#"fn main() {
    let v: Vec<i64> = Vec::new();
    let _ = v;
}"#,
            DEFERRED_E0282_STDLIB,
        );
    }

    #[test]
    fn concrete_stdlib_constructor_arguments_are_accepted() {
        expect_ok_with_stdlib(
            r#"fn main() {
    let b = Box::new(5);
    let _ = b;
}"#,
            DEFERRED_E0282_STDLIB,
        );
    }

    #[test]
    fn late_annotations_resolve_deferred_constructor_instances() {
        expect_ok_with_stdlib(
            r#"fn main() {
    let v = Vec::new();
    let _w: Vec<i64> = v;
}"#,
            DEFERRED_E0282_STDLIB,
        );
    }

    #[test]
    fn genuine_stdlib_body_errors_remain_internal_errors() {
        let diagnostics = expect_errors_with_stdlib(
            r#"fn main() {
    let value = String::buggy();
    let _ = value;
}"#,
            DEFERRED_E0282_STDLIB,
        );
        assert!(
            has_message(&diagnostics, "internal standard library error"),
            "stdlib body errors must keep the internal wrapper: {:?}",
            messages(&diagnostics)
        );
        assert!(has_code(&diagnostics, "E0308"));
    }

    // -----------------------------------------------------------------
    // v2.1 numeric, tuple-struct, and text-context semantics
    // -----------------------------------------------------------------

    #[test]
    fn accepts_tuple_and_unit_structs() {
        let program = expect_ok(
            r#"struct Point(i64, i64);
struct Meters(i64);
struct Unit;
impl Point {
    fn sum(&self) -> i64 { self.0 + self.1 }
    fn set_x(&mut self, x: i64) { self.0 = x; }
}
fn main() {
    let mut p = Point(3, 4);
    p.set_x(10);
    let total = p.sum();
    let m = Meters(7);
    let raw = m.0;
    let first = Unit;
    let second = Unit;
    match p {
        Point(x, y) => { println!("{}", x + y); }
    }
    println!("{} {} {:?} {:?}", total, raw, first, second);
}"#,
        );
        let point = program
            .structs
            .iter()
            .find(|structure| structure.name.starts_with("Point"))
            .expect("Point must be lowered");
        assert_eq!(point.fields.len(), 2);
        assert_eq!(point.fields[0].name, "0");
        assert_eq!(point.fields[1].name, "1");
        assert_eq!(point.fields[0].ty, Ty::Int(IntTy::I64));
        let meters = program
            .structs
            .iter()
            .find(|structure| structure.name.starts_with("Meters"))
            .expect("Meters must be lowered");
        assert_eq!(meters.fields.len(), 1);
        assert_eq!(meters.fields[0].name, "0");
        let unit = program
            .structs
            .iter()
            .find(|structure| structure.name.starts_with("Unit"))
            .expect("unit struct must be lowered");
        assert!(unit.fields.is_empty());
        assert!(find_fn(&program, "Point::set_x").is_some());
    }

    #[test]
    fn rejects_tuple_struct_constructor_arity_mismatch() {
        let diagnostics = expect_errors(
            "struct Point(i64, i64);
fn main() { let p = Point(1); }",
        );
        assert!(
            has_code(&diagnostics, "E0061"),
            "expected an arity diagnostic: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_tuple_struct_pattern_arity_mismatch() {
        let diagnostics = expect_errors(
            r#"struct Point(i64, i64);
fn main() {
    let p = Point(1, 2);
    match p {
        Point(x) => { println!("{}", x); }
    }
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0023"),
            "expected a pattern arity diagnostic: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn unsuffixed_integer_literals_default_to_i32() {
        // An unsuffixed literal adapts to every integer context it meets, and
        // only falls back to its default type when no context constrains it.
        expect_ok(
            r#"fn take_i32(value: i32) -> i32 { value }
fn take_i64(value: i64) -> i64 { value }
fn main() {
    let small = 1 + 2;
    let wide = 3;
    println!("{} {}", take_i32(small), take_i64(wide));
}"#,
        );
        // The default is i32: an unconstrained literal that does not fit i32 is
        // rejected even though i64 could hold it.
        let diagnostics =
            expect_errors(r#"fn main() { let value = 3000000000; println!("{}", value); }"#);
        assert!(
            has_code(&diagnostics, "E0080"),
            "an unconstrained literal must be range checked against i32: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn suffixed_integer_literals_use_their_suffix_and_range() {
        expect_ok(
            r#"fn main() {
    let small: u8 = 255u8;
    let low: i8 = -128i8;
    let wide = 3000000000i64;
    println!("{} {} {}", small, low, wide);
}"#,
        );
        let diagnostics =
            expect_errors(r#"fn main() { let value = 128i8; println!("{}", value); }"#);
        assert!(
            has_code(&diagnostics, "E0080"),
            "a suffixed literal must be range checked: {:?}",
            messages(&diagnostics)
        );
        let diagnostics =
            expect_errors(r#"fn main() { let value: i8 = 128; println!("{}", value); }"#);
        assert!(
            has_code(&diagnostics, "E0080"),
            "a contextually typed literal must be range checked: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn vector_indices_require_usize() {
        expect_ok(
            r#"fn main() {
    let values = vec![10i64, 20i64, 30i64];
    let index = 1;
    println!("{}", values[index]);
}"#,
        );
        let diagnostics = expect_errors(
            r#"fn main() {
    let values = vec![10i64, 20i64, 30i64];
    let index: i32 = 1;
    println!("{}", values[index]);
}"#,
        );
        assert!(
            has_message(&diagnostics, "mismatched types"),
            "an i32 index must not be accepted: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn string_literals_adapt_to_str_parameters() {
        expect_ok(
            r#"fn greet(name: &str) -> i64 { 1 }
fn main() {
    let text = String::from("x");
    println!("{} {}", greet("world"), text);
}"#,
        );
        let diagnostics = expect_errors(
            r#"fn greet(name: &str) -> i64 { 1 }
fn main() {
    let text = String::from("x");
    println!("{}", greet(text));
}"#,
        );
        assert!(
            has_message(&diagnostics, "mismatched types"),
            "an owned String must not implicitly become &str: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn string_concatenation_requires_a_borrowed_text_operand() {
        expect_ok(
            r#"fn main() {
    let mut text = String::from("a");
    text += "b";
    let joined = text + "c";
    println!("{}", joined);
}"#,
        );
        let diagnostics = expect_errors(
            r#"fn main() { let joined = String::from("a") + String::from("b"); println!("{}", joined); }"#,
        );
        assert!(
            has_code(&diagnostics, "E0277"),
            "String + String must be rejected: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn mutable_parameters_are_assignable_in_the_body() {
        expect_ok(
            r#"fn bump(mut value: i64) -> i64 { value += 1; value }
fn main() { println!("{}", bump(1)); }"#,
        );
        let diagnostics = expect_errors(
            r#"fn bump(value: i64) -> i64 { value += 1; value }
fn main() { println!("{}", bump(1)); }"#,
        );
        assert!(
            has_code(&diagnostics, "E0596"),
            "assigning an immutable parameter must be rejected: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn rejects_generic_bound_violation_even_when_the_bound_is_unused() {
        let diagnostics = expect_errors(
            r#"trait Sized2 { fn size(&self) -> i64; }
fn accepts<T: Sized2>(value: &T) -> i64 { return 0; }
fn main() { let number = 3; println!("{}", accepts(&number)); }"#,
        );
        assert!(
            has_code(&diagnostics, "E0277"),
            "an unsatisfied bound must be reported at the call site: {:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(
                &diagnostics,
                "the trait bound (i32: Sized2) is not satisfied"
            ),
            "the diagnostic must name the violating type and the bound: {:?}",
            messages(&diagnostics)
        );
        assert!(
            !has_code(&diagnostics, "E0599"),
            "the bound must not be discovered inside the monomorphized body: {:?}",
            messages(&diagnostics)
        );
        // The regression fixture checks the same rule when the body calls the
        // bound method: the body is legal under the declared bound and the
        // violation is reported at the instantiation, not as E0599 inside it.
        let diagnostics = check_source_with_embedded_stdlib(
            r#"trait Sized2 { fn size(&self) -> i64; }
fn measure<T: Sized2>(value: &T) -> i64 { return value.size(); }
fn main() { let number = 3; println!("{}", measure(&number)); }"#,
        )
        .expect_err("an unsatisfied bound must be rejected at the call site");
        assert!(
            has_code(&diagnostics, "E0277"),
            "the bound violation must be E0277: {:?}",
            messages(&diagnostics)
        );
        assert!(
            !has_code(&diagnostics, "E0599"),
            "the body must not report a missing method: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn accepts_satisfied_generic_bounds() {
        expect_ok(
            r#"trait Doubles { fn double(&self) -> i64; }
struct Number { value: i64 }
impl Doubles for Number { fn double(&self) -> i64 { return self.value * 2; } }
fn apply<T: Doubles>(value: &T) -> i64 { let doubled = value.double(); return doubled; }
fn main() { let number = Number { value: 3 }; println!("{}", apply(&number)); }"#,
        );
    }

    #[test]
    fn rejects_a_missing_bound_when_only_some_are_satisfied() {
        let diagnostics = expect_errors(
            r#"trait First2 { fn first(&self) -> i64; }
trait Second2 { fn second(&self) -> i64; }
impl First2 for i64 { fn first(&self) -> i64 { return 1; } }
fn both<T: First2 + Second2>(value: &T) -> i64 { return 0; }
fn main() { let number: i64 = 3; println!("{}", both(&number)); }"#,
        );
        assert!(
            has_message(
                &diagnostics,
                "the trait bound (i64: Second2) is not satisfied"
            ),
            "the missing bound must be reported even when another one is satisfied: {:?}",
            messages(&diagnostics)
        );
    }

    /// Type check a user program against the embedded standard library, exactly
    /// like the compiler driver does.
    fn check_source_with_embedded_stdlib(source: &str) -> Result<hir::Program, Diagnostics> {
        let (program, file, diagnostics) = parse_source(source);
        assert!(
            !diagnostics.has_errors(),
            "unexpected parse errors: {:?}",
            messages(&diagnostics)
        );
        let stdlib = crate::stdlib::program().expect("the embedded standard library must parse");
        check_with_prelude(&program, Some(stdlib), &file)
    }

    #[test]
    fn generic_parse_uses_the_typed_parse_errors() {
        let program = check_source_with_embedded_stdlib(
            r#"fn main() {
    let direct: Result<i64, ParseIntError> = "42".parse_i64();
    let from_generic = "42".parse::<i64>();
    let typed: Result<i64, ParseIntError> = from_generic;
    let float: Result<f64, ParseFloatError> = "2.5".parse::<f64>();
    println!("{} {}", direct.unwrap_or(0), typed.unwrap_or(0));
    println!("{}", float.unwrap_or(0.0));
}"#,
        )
        .unwrap_or_else(|diagnostics| {
            panic!(
                "parse::<T> must return the typed error of the stdlib method: {:#?}",
                diagnostics
            )
        });
        assert!(find_fn(&program, "main").is_some());
        let diagnostics = check_source_with_embedded_stdlib(
            r#"fn main() {
    let wrong: Result<i64, String> = "42".parse::<i64>();
    println!("{}", wrong);
}"#,
        )
        .expect_err("a String error type must not match ParseIntError");
        assert!(
            has_message(&diagnostics, "mismatched types"),
            "parse::<i64> must type the error as ParseIntError: {:?}",
            messages(&diagnostics)
        );
        let diagnostics = check_source_with_embedded_stdlib(
            r#"fn main() {
    let unsupported = "42".parse::<i32>();
    println!("{:?}", unsupported);
}"#,
        )
        .expect_err("only i64 and f64 parse targets are supported");
        assert!(
            has_code(&diagnostics, "E0107"),
            "other parse targets must stay rejected: {:?}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn lowers_v21_iterator_adapters_to_prelude_calls() {
        expect_ok_with_prelude(
            r#"fn main() {
    let zipped: Vec<(i64, i64)> = vec![1i64, 2i64, 3i64]
        .iter()
        .zip(vec![4i64, 5i64, 6i64].iter())
        .collect();
    let chained: Vec<i64> = vec![1i64, 2i64]
        .iter()
        .chain(vec![3i64, 4i64].iter())
        .collect();
    let filtered: Vec<(i64, i64)> = vec![1i64, 2i64, 3i64]
        .iter()
        .filter(|value| *value > 1)
        .map(|value| value * 2)
        .zip(vec![9i64, 8i64].iter())
        .collect();
    let mapped: Vec<i64> = vec![1i64, 2i64, 3i64, 4i64]
        .iter()
        .filter_map(|value| {
            if value % 2 == 0 {
                return Some(value * 10);
            }
            return None;
        })
        .collect();
    let prefix: Vec<i64> = vec![1i64, 2i64, 3i64, 4i64]
        .iter()
        .take_while(|value| *value < 3)
        .collect();
    let rest: Vec<i64> = vec![1i64, 2i64, 3i64, 4i64]
        .iter()
        .skip_while(|value| *value < 3)
        .collect();
    let stepped: Vec<i64> = vec![1i64, 2i64, 3i64, 4i64, 5i64]
        .iter()
        .step_by(2)
        .collect();
    let inspected: Vec<i64> = vec![1i64, 2i64]
        .iter()
        .inspect(|value| {
            println!("{}", *value);
        })
        .collect();
    vec![1i64, 2i64, 3i64].iter().for_each(|value| {
        println!("{}", value);
    });
    let position: Option<usize> = vec![1i64, 2i64, 3i64]
        .iter()
        .position(|value| value == 3i64);
    let last: Option<i64> = vec![1i64, 2i64, 3i64].iter().last();
    let third: Option<i64> = vec![1i64, 2i64, 3i64].iter().nth(2);
    let reduced: Option<i64> = vec![1i64, 2i64, 3i64]
        .iter()
        .reduce(|acc, value| acc + value);
    println!(
        "{} {} {} {} {} {} {} {}",
        zipped.len(),
        chained.len(),
        filtered.len(),
        mapped.len(),
        prefix.len(),
        rest.len(),
        stepped.len(),
        inspected.len()
    );
    println!(
        "{} {:?} {:?} {:?}",
        position.unwrap_or(0),
        last,
        third,
        reduced
    );
}"#,
        );
    }

    #[test]
    fn numeric_terminal_methods_use_their_specialized_helpers() {
        check_source_with_embedded_stdlib(
            r#"fn main() {
    let sum_int: i32 = vec![1, 2, 3].iter().sum();
    let product_int: i32 = vec![1, 2, 3].iter().product();
    let sum_wide: i64 = vec![1i64, 2i64, 3i64].iter().sum();
    let product_wide: i64 = vec![1i64, 2i64, 3i64].iter().product();
    let sum_count: usize = vec![1usize, 2usize, 3usize].iter().sum();
    let product_count: usize = vec![1usize, 2usize, 3usize].iter().product();
    let sum_float: f64 = vec![1.5, 2.5].iter().sum();
    let product_float: f64 = vec![1.5, 2.5].iter().product();
    println!(
        "{} {} {} {} {} {} {} {}",
        sum_int,
        product_int,
        sum_wide,
        product_wide,
        sum_count,
        product_count,
        sum_float,
        product_float
    );
}"#,
        )
        .unwrap_or_else(|diagnostics| {
            panic!(
                "sum/product must dispatch on the item type: {:#?}",
                diagnostics
            )
        });
    }

    #[test]
    fn rejects_unsupported_iterator_sugar_uses() {
        let diagnostics = expect_errors_with_prelude(
            r#"fn main() {
    let words: Vec<String> = vec![String::from("a")];
    let total = words.iter().sum();
    let product = words.iter().product();
    let _ = total;
    let _ = product;
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0277"),
            "sum/product must reject non-numeric items: {:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(
                &diagnostics,
                "only supported for iterators of i32, i64, usize or f64"
            ),
            "the diagnostic must list the supported item types: {:?}",
            messages(&diagnostics)
        );
        let diagnostics = expect_errors_with_prelude(
            r#"fn main() {
    let values: Vec<i64> = vec![1, 2, 3];
    let zipped = values.iter().zip(5);
    let _ = zipped;
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0277"),
            "zip needs an iterator as its second argument: {:?}",
            messages(&diagnostics)
        );
        assert!(
            has_message(&diagnostics, "needs an iterator argument"),
            "the zip diagnostic must explain the argument requirement: {:?}",
            messages(&diagnostics)
        );
        let diagnostics = expect_errors_with_prelude(
            r#"fn main() {
    let values: Vec<i64> = vec![1, 2, 3];
    let other: Vec<u8> = vec![4u8];
    let chained = values.iter().chain(other.iter());
    let _ = chained;
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0308"),
            "chain requires matching element types: {:?}",
            messages(&diagnostics)
        );
        let diagnostics = expect_errors_with_prelude(
            r#"fn main() {
    let values: Vec<i64> = vec![1, 2, 3];
    let zipped = values.iter().zip();
    let _ = zipped;
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0061"),
            "zip takes exactly one argument: {:?}",
            messages(&diagnostics)
        );
        let diagnostics = expect_errors_with_prelude(
            r#"fn main() {
    let values: Vec<i64> = vec![1, 2, 3];
    let zipped = values.iter().zip::<i64>(values.iter());
    let _ = zipped;
}"#,
        );
        assert!(
            has_code(&diagnostics, "E0107"),
            "only collect accepts a type argument: {:?}",
            messages(&diagnostics)
        );
    }
}
