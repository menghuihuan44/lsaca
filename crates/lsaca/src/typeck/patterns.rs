//! Pattern checking: binding resolution, or-pattern consistency and
//! irrefutability.

use std::collections::HashMap;

use super::check::Checker;
use super::items::TypeKey;
use super::itype::ITy;
use super::tree::{TLit, TPat, TPatKind};
use crate::ast::{self, Pattern, PatternKind};
use crate::diag::Diagnostic;
use crate::span::Span;

/// A binding introduced by a pattern.
#[derive(Clone, Debug)]
struct BindingInfo {
    c_name: String,
    ty: ITy,
    mutable: bool,
}

impl<'a> Checker<'a> {
    /// Check an irrefutable pattern (let binding, for loop head, closure param).
    pub fn check_irrefutable_pattern(
        &mut self,
        pat: &Pattern,
        expected: &ITy,
        _span: Span,
    ) -> TPat {
        self.pattern_names.push(HashMap::new());
        let checked = self.check_pattern_inner(pat, expected, None);
        self.pattern_names.pop();
        if !self.pattern_irrefutable(&checked) {
            let diag = Diagnostic::error(
                "refutable pattern in local binding: patterns in this position must be irrefutable"
                    .to_string(),
                checked.span,
            )
            .with_code("E0005")
            .with_span_label(checked.span)
            .with_note(
                "only wildcards, bindings and single-variant destructuring are allowed here"
                    .to_string(),
            );
            self.diagnostics.push(diag);
        }
        checked
    }

    /// Check a pattern against an expected type.
    pub fn check_pattern(&mut self, pat: &Pattern, expected: &ITy) -> TPat {
        self.pattern_names.push(HashMap::new());
        let checked = self.check_pattern_inner(pat, expected, None);
        self.pattern_names.pop();
        checked
    }

    fn check_pattern_inner(
        &mut self,
        pat: &Pattern,
        expected: &ITy,
        overrides: Option<&HashMap<String, BindingInfo>>,
    ) -> TPat {
        let expected = self.infer.resolve(expected);
        match &pat.kind {
            PatternKind::Wild => TPat::new(expected, TPatKind::Wild, pat.span),
            PatternKind::Binding { name, mutable } => {
                self.check_binding_pattern(name, *mutable, &expected, pat.span, overrides)
            }
            PatternKind::Lit(lit) => self.check_literal_pattern(lit, &expected, pat.span),
            PatternKind::Tuple(pats) => {
                self.check_tuple_pattern(pats, &expected, pat.span, overrides)
            }
            PatternKind::Struct {
                path,
                fields,
                has_rest,
            } => self.check_struct_pattern(path, fields, *has_rest, &expected, pat.span, overrides),
            PatternKind::Path { path, args } => {
                self.check_path_pattern(path, args, &expected, pat.span, overrides)
            }
            PatternKind::Or(alts) => self.check_or_pattern(alts, &expected, pat.span),
        }
    }

    fn check_binding_pattern(
        &mut self,
        name: &str,
        mutable: bool,
        expected: &ITy,
        span: Span,
        overrides: Option<&HashMap<String, BindingInfo>>,
    ) -> TPat {
        if let ITy::Enum(enum_id) = expected
            && let Some(entries) = self.unit_variants.get(name).cloned()
            && let Some((_, variant)) = entries.iter().find(|(id, _)| id == enum_id)
        {
            return TPat::new(
                expected.clone(),
                TPatKind::Enum {
                    enum_id: *enum_id,
                    variant: *variant,
                    args: Vec::new(),
                },
                span,
            );
        }
        if let Some(&const_id) = self.const_by_name.get(name) {
            return self.check_const_pattern(const_id, expected, span);
        }
        if let Some(overrides) = overrides
            && let Some(info) = overrides.get(name)
        {
            self.unify_or_report(&info.ty, expected, span);
            return TPat::new(
                expected.clone(),
                TPatKind::Bind {
                    name: name.to_string(),
                    c_name: info.c_name.clone(),
                    mutable: info.mutable,
                },
                span,
            );
        }
        let previous = self
            .pattern_names
            .last()
            .and_then(|names| names.get(name).copied());
        if let Some(previous) = previous {
            let diag = Diagnostic::error(
                format!(
                    "identifier {} is bound more than once in this pattern",
                    name
                ),
                span,
            )
            .with_code("E0416")
            .with_span_label(span)
            .with_label(previous, "first bound here".to_string());
            self.diagnostics.push(diag);
        } else if let Some(names) = self.pattern_names.last_mut() {
            names.insert(name.to_string(), span);
        }
        let c_name = self.declare_local(name, expected.clone(), mutable, span);
        TPat::new(
            expected.clone(),
            TPatKind::Bind {
                name: name.to_string(),
                c_name,
                mutable,
            },
            span,
        )
    }

    fn check_const_pattern(&mut self, const_id: usize, expected: &ITy, span: Span) -> TPat {
        self.ensure_const(const_id);
        let (ty, value) = match self.const_value(const_id) {
            Some(pair) => pair,
            None => return TPat::new(expected.clone(), TPatKind::Wild, span),
        };
        let const_ty = ITy::from_ty(&ty);
        self.unify_or_report(expected, &const_ty, span);
        TPat::new(expected.clone(), TPatKind::Lit(value.to_lit()), span)
    }

    fn check_literal_pattern(&mut self, lit: &ast::Lit, expected: &ITy, span: Span) -> TPat {
        let checked_lit = match lit {
            ast::Lit::Int { value, raw } => {
                let ty = self.infer.int_literal(*value, raw.clone(), span);
                self.unify_or_report(expected, &ty, span);
                TLit::Int(*value)
            }
            ast::Lit::Float { value, raw } => {
                let ty = self.infer.float_literal(*value, raw.clone(), span);
                self.unify_or_report(expected, &ty, span);
                TLit::Float(*value)
            }
            ast::Lit::Bool(value) => {
                self.unify_or_report(expected, &ITy::bool_(), span);
                TLit::Bool(*value)
            }
            ast::Lit::Str(value) => {
                self.unify_or_report(expected, &ITy::string(), span);
                TLit::Str(value.clone())
            }
        };
        TPat::new(expected.clone(), TPatKind::Lit(checked_lit), span)
    }

    fn check_tuple_pattern(
        &mut self,
        pats: &[Pattern],
        expected: &ITy,
        span: Span,
        overrides: Option<&HashMap<String, BindingInfo>>,
    ) -> TPat {
        let mut expected = self.infer.resolve(expected);
        if pats.is_empty() {
            self.unify_or_report(&expected, &ITy::Unit, span);
            expected = ITy::Unit;
        } else if let ITy::Tuple(elems) = &expected {
            if elems.len() != pats.len() {
                self.error_code(
                    format!(
                        "pattern expects {} element(s), found {}",
                        elems.len(),
                        pats.len()
                    ),
                    span,
                    "E0023",
                );
            }
        } else if matches!(expected, ITy::Var(_)) {
            let elems: Vec<ITy> = pats.iter().map(|_| self.infer.fresh_var()).collect();
            let tuple_ty = ITy::tuple(elems);
            let _ = self.infer.unify(&expected, &tuple_ty);
            expected = tuple_ty;
        } else {
            let found_elems: Vec<ITy> = pats.iter().map(|_| self.infer.fresh_var()).collect();
            let found = ITy::tuple(found_elems);
            self.report_mismatch(&expected, &found, span);
        }
        let child_types = match &self.infer.resolve(&expected) {
            ITy::Tuple(elems) if elems.len() == pats.len() => elems.clone(),
            _ => pats.iter().map(|_| self.infer.fresh_var()).collect(),
        };
        let children: Vec<TPat> = pats
            .iter()
            .zip(child_types.iter())
            .map(|(pat, ty)| self.check_pattern_inner(pat, ty, overrides))
            .collect();
        TPat::new(expected, TPatKind::Tuple(children), span)
    }

    fn check_struct_pattern(
        &mut self,
        path: &ast::PathExpr,
        fields: &[ast::FieldPat],
        has_rest: bool,
        expected: &ITy,
        span: Span,
        overrides: Option<&HashMap<String, BindingInfo>>,
    ) -> TPat {
        let struct_id = match self.resolve_pattern_struct(path, span) {
            Some(id) => id,
            None => return TPat::new(expected.clone(), TPatKind::Wild, span),
        };
        self.ensure_struct_fields(struct_id);
        let struct_ty = ITy::Struct(struct_id);
        self.unify_or_report(expected, &struct_ty, span);
        let struct_fields = self.struct_fields(struct_id);
        let mut assigned: Vec<Option<TPat>> = struct_fields
            .iter()
            .map(|_| None)
            .collect::<Vec<Option<TPat>>>();
        let mut seen: Vec<bool> = vec![false; struct_fields.len()];
        for field in fields {
            let index = struct_fields
                .iter()
                .position(|(name, _, _)| *name == field.name);
            match index {
                Some(index) => {
                    if seen[index] {
                        self.error_code(
                            format!(
                                "field {} is bound more than once in this pattern",
                                field.name
                            ),
                            field.name_span,
                            "E0025",
                        );
                        continue;
                    }
                    seen[index] = true;
                    let field_ty = struct_fields[index].2.clone();
                    let checked = self.check_pattern_inner(&field.pat, &field_ty, overrides);
                    assigned[index] = Some(checked);
                }
                None => {
                    let names: Vec<&str> = struct_fields
                        .iter()
                        .map(|(name, _, _)| name.as_str())
                        .collect();
                    let mut diag = Diagnostic::error(
                        format!(
                            "struct {} does not have a field named {}",
                            self.struct_name(struct_id),
                            field.name
                        ),
                        field.name_span,
                    )
                    .with_code("E0026")
                    .with_span_label(field.name_span);
                    if let Some(suggestion) = super::items::closest_name(&field.name, &names) {
                        diag = diag.with_note(format!("did you mean {}?", suggestion));
                    }
                    self.diagnostics.push(diag);
                }
            }
        }
        if !has_rest {
            for (index, (name, _, _)) in struct_fields.iter().enumerate() {
                if !seen[index] {
                    self.error_code(
                        format!(
                            "pattern does not mention field {} of struct {}",
                            name,
                            self.struct_name(struct_id)
                        ),
                        span,
                        "E0027",
                    );
                }
            }
        }
        let mut checked_fields = Vec::new();
        for (index, sub) in assigned.into_iter().enumerate() {
            if let Some(sub) = sub {
                checked_fields.push((index, sub));
            }
        }
        TPat::new(
            struct_ty,
            TPatKind::Struct {
                struct_id,
                fields: checked_fields,
                has_rest,
            },
            span,
        )
    }

    fn check_path_pattern(
        &mut self,
        path: &ast::PathExpr,
        args: &[Pattern],
        expected: &ITy,
        span: Span,
        overrides: Option<&HashMap<String, BindingInfo>>,
    ) -> TPat {
        if path
            .segments
            .iter()
            .any(|segment| !segment.generics.is_empty())
        {
            self.error_code(
                "patterns do not take generic arguments".to_string(),
                span,
                "E0107",
            );
        }
        let simple = path.simple_name().map(|name| name.to_string());
        match simple.as_deref() {
            Some("Some") => self.check_option_some(args, expected, span, overrides),
            Some("None") => self.check_option_none(args, expected, span),
            Some("Ok") => self.check_result_ok(args, expected, span, overrides),
            Some("Err") => self.check_result_err(args, expected, span, overrides),
            _ => self.check_enum_variant_pattern(path, args, expected, span, overrides),
        }
    }

    fn fresh_option_ty(&mut self, expected: &ITy, span: Span) -> (ITy, ITy) {
        let expected = self.infer.resolve(expected);
        match &expected {
            ITy::Option(elem) => ((**elem).clone(), expected.clone()),
            _ => {
                let elem = self.infer.fresh_var();
                let option_ty = ITy::option(elem.clone());
                self.unify_or_report(&expected, &option_ty, span);
                (elem, option_ty)
            }
        }
    }

    fn fresh_result_ty(&mut self, expected: &ITy, span: Span) -> (ITy, ITy, ITy) {
        let expected = self.infer.resolve(expected);
        match &expected {
            ITy::Result(ok, err) => ((**ok).clone(), (**err).clone(), expected.clone()),
            _ => {
                let ok = self.infer.fresh_var();
                let err = self.infer.fresh_var();
                let result_ty = ITy::result(ok.clone(), err.clone());
                self.unify_or_report(&expected, &result_ty, span);
                (ok, err, result_ty)
            }
        }
    }

    fn check_option_some(
        &mut self,
        args: &[Pattern],
        expected: &ITy,
        span: Span,
        overrides: Option<&HashMap<String, BindingInfo>>,
    ) -> TPat {
        let (elem_ty, option_ty) = self.fresh_option_ty(expected, span);
        let inner = match args.len() {
            1 => self.check_pattern_inner(&args[0], &elem_ty, overrides),
            count => {
                self.error_at(
                    format!("Some pattern expects 1 argument, found {}", count),
                    span,
                );
                TPat::new(elem_ty, TPatKind::Wild, span)
            }
        };
        TPat::new(option_ty, TPatKind::OptionSome(Box::new(inner)), span)
    }

    fn check_option_none(&mut self, args: &[Pattern], expected: &ITy, span: Span) -> TPat {
        let (_, option_ty) = self.fresh_option_ty(expected, span);
        if !args.is_empty() {
            self.error_at(
                format!("None pattern expects no arguments, found {}", args.len()),
                span,
            );
        }
        TPat::new(option_ty, TPatKind::OptionNone, span)
    }

    fn check_result_ok(
        &mut self,
        args: &[Pattern],
        expected: &ITy,
        span: Span,
        overrides: Option<&HashMap<String, BindingInfo>>,
    ) -> TPat {
        let (ok_ty, _, result_ty) = self.fresh_result_ty(expected, span);
        let inner = match args.len() {
            1 => self.check_pattern_inner(&args[0], &ok_ty, overrides),
            count => {
                self.error_at(
                    format!("Ok pattern expects 1 argument, found {}", count),
                    span,
                );
                TPat::new(ok_ty, TPatKind::Wild, span)
            }
        };
        TPat::new(result_ty, TPatKind::ResultOk(Box::new(inner)), span)
    }

    fn check_result_err(
        &mut self,
        args: &[Pattern],
        expected: &ITy,
        span: Span,
        overrides: Option<&HashMap<String, BindingInfo>>,
    ) -> TPat {
        let (_, err_ty, result_ty) = self.fresh_result_ty(expected, span);
        let inner = match args.len() {
            1 => self.check_pattern_inner(&args[0], &err_ty, overrides),
            count => {
                self.error_at(
                    format!("Err pattern expects 1 argument, found {}", count),
                    span,
                );
                TPat::new(err_ty, TPatKind::Wild, span)
            }
        };
        TPat::new(result_ty, TPatKind::ResultErr(Box::new(inner)), span)
    }

    fn check_enum_variant_pattern(
        &mut self,
        path: &ast::PathExpr,
        args: &[Pattern],
        expected: &ITy,
        span: Span,
        overrides: Option<&HashMap<String, BindingInfo>>,
    ) -> TPat {
        if path.segments.len() < 2 {
            self.error_code(
                "expected a pattern, found a path".to_string(),
                path.span,
                "E0532",
            );
            return TPat::new(expected.clone(), TPatKind::Wild, span);
        }
        let type_name = path.segments[path.segments.len() - 2].name.clone();
        let variant_name = path.segments[path.segments.len() - 1].name.clone();
        let enum_id = if type_name == "Self" {
            match self.current_self_key() {
                Some(TypeKey::Enum(id)) => Some(id),
                _ => {
                    self.error_code(
                        "Self is only available inside an impl block".to_string(),
                        span,
                        "E0411",
                    );
                    None
                }
            }
        } else {
            match self.enum_by_name.get(&type_name).copied() {
                Some(id) => Some(id),
                None => {
                    self.error_code(
                        format!("cannot find enum {} in this scope", type_name),
                        span,
                        "E0412",
                    );
                    None
                }
            }
        };
        let enum_id = match enum_id {
            Some(id) => id,
            None => return TPat::new(expected.clone(), TPatKind::Wild, span),
        };
        self.ensure_enum_variants(enum_id);
        let enum_info = match self.enums.get(enum_id) {
            Some(info) => info,
            None => return TPat::new(expected.clone(), TPatKind::Wild, span),
        };
        let variant_index = enum_info
            .variants
            .iter()
            .position(|variant| variant.name == variant_name);
        let variant_index = match variant_index {
            Some(index) => index,
            None => {
                let names: Vec<&str> = enum_info
                    .variants
                    .iter()
                    .map(|variant| variant.name.as_str())
                    .collect();
                let mut diag = Diagnostic::error(
                    format!(
                        "enum {} has no variant named {}",
                        self.enum_name(enum_id),
                        variant_name
                    ),
                    span,
                )
                .with_code("E0599")
                .with_span_label(span);
                if let Some(suggestion) = super::items::closest_name(&variant_name, &names) {
                    diag = diag.with_note(format!("did you mean {}?", suggestion));
                }
                self.diagnostics.push(diag);
                return TPat::new(expected.clone(), TPatKind::Wild, span);
            }
        };
        let enum_ty = ITy::Enum(enum_id);
        self.unify_or_report(expected, &enum_ty, span);
        let field_types = self.variant_fields(enum_id, variant_index);
        if field_types.len() != args.len() {
            if field_types.is_empty() {
                self.error_code(
                    format!(
                        "unit variant {}::{} takes no arguments",
                        self.enum_name(enum_id),
                        variant_name
                    ),
                    span,
                    "E0532",
                );
            } else {
                self.error_code(
                    format!(
                        "variant {}::{} expects {} argument(s), found {}",
                        self.enum_name(enum_id),
                        variant_name,
                        field_types.len(),
                        args.len()
                    ),
                    span,
                    "E0532",
                );
            }
        }
        let mut checked_args = Vec::new();
        for (index, pat) in args.iter().enumerate() {
            let field_ty = field_types
                .get(index)
                .cloned()
                .unwrap_or_else(|| self.infer.fresh_var());
            let checked = self.check_pattern_inner(pat, &field_ty, overrides);
            checked_args.push(checked);
        }
        TPat::new(
            enum_ty,
            TPatKind::Enum {
                enum_id,
                variant: variant_index,
                args: checked_args,
            },
            span,
        )
    }

    fn check_or_pattern(&mut self, alts: &[Pattern], expected: &ITy, span: Span) -> TPat {
        if alts.is_empty() {
            return TPat::new(expected.clone(), TPatKind::Wild, span);
        }
        let first = self.check_pattern_inner(&alts[0], expected, None);
        let mut bindings: HashMap<String, BindingInfo> = HashMap::new();
        collect_bindings(&first, &mut bindings);
        let mut checked = Vec::with_capacity(alts.len());
        checked.push(first);
        for alt in &alts[1..] {
            let checked_alt = self.check_pattern_inner(alt, expected, Some(&bindings));
            let mut alt_bindings: HashMap<String, BindingInfo> = HashMap::new();
            collect_bindings(&checked_alt, &mut alt_bindings);
            let mut missing = Vec::new();
            for name in bindings.keys() {
                if !alt_bindings.contains_key(name) {
                    missing.push(name.clone());
                }
            }
            missing.sort();
            for name in missing {
                self.error_code(
                    format!(
                        "variable {} is not bound in all patterns of this or-pattern",
                        name
                    ),
                    span,
                    "E0408",
                );
            }
            let mut extra = Vec::new();
            for name in alt_bindings.keys() {
                if !bindings.contains_key(name) {
                    extra.push(name.clone());
                }
            }
            extra.sort();
            for name in extra {
                self.error_code(
                    format!(
                        "variable {} is not bound in all patterns of this or-pattern",
                        name
                    ),
                    span,
                    "E0408",
                );
            }
            checked.push(checked_alt);
        }
        TPat::new(expected.clone(), TPatKind::Or(checked), span)
    }

    fn resolve_pattern_struct(&mut self, path: &ast::PathExpr, span: Span) -> Option<usize> {
        let name = match path.simple_name() {
            Some(name) => name.to_string(),
            None => {
                self.error_code(
                    "expected a struct pattern path".to_string(),
                    path.span,
                    "E0532",
                );
                return None;
            }
        };
        if path
            .segments
            .iter()
            .any(|segment| !segment.generics.is_empty())
        {
            self.error_code(
                "patterns do not take generic arguments".to_string(),
                path.span,
                "E0107",
            );
        }
        if let Some(id) = self.struct_by_name.get(&name).copied() {
            return Some(id);
        }
        if name == "Self" {
            if let Some(TypeKey::Struct(id)) = self.current_self_key() {
                return Some(id);
            }
            self.error_code(
                "Self is only available inside an impl block".to_string(),
                span,
                "E0411",
            );
            return None;
        }
        if self.enum_by_name.contains_key(&name) {
            self.error_code(
                format!("expected a struct pattern, found enum {}", name),
                span,
                "E0532",
            );
            return None;
        }
        let mut diag =
            Diagnostic::error(format!("cannot find struct {} in this scope", name), span)
                .with_code("E0412")
                .with_span_label(span);
        if let Some(suggestion) = self.suggest_type_name(&name) {
            diag = diag.with_note(format!("did you mean {}?", suggestion));
        }
        self.diagnostics.push(diag);
        None
    }
}

fn collect_bindings(pat: &TPat, out: &mut HashMap<String, BindingInfo>) {
    match &pat.kind {
        TPatKind::Wild | TPatKind::Lit(_) => {}
        TPatKind::Bind {
            name,
            c_name,
            mutable,
        } => {
            out.entry(name.clone()).or_insert_with(|| BindingInfo {
                c_name: c_name.clone(),
                ty: pat.ty.clone(),
                mutable: *mutable,
            });
        }
        TPatKind::Tuple(pats) => {
            for pat in pats {
                collect_bindings(pat, out);
            }
        }
        TPatKind::Struct { fields, .. } => {
            for (_, pat) in fields {
                collect_bindings(pat, out);
            }
        }
        TPatKind::Enum { args, .. } => {
            for pat in args {
                collect_bindings(pat, out);
            }
        }
        TPatKind::OptionSome(inner) | TPatKind::ResultOk(inner) | TPatKind::ResultErr(inner) => {
            collect_bindings(inner, out)
        }
        TPatKind::OptionNone => {}
        TPatKind::Or(alts) => {
            for pat in alts {
                collect_bindings(pat, out);
            }
        }
    }
}
