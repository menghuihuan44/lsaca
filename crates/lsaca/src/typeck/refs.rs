//! Reference typing for v1.3: borrowing, dereferencing, the storage
//! restrictions from the design contract and the implicit reference
//! coercions.
//!
//! This module owns the coercions and the validation rules below; the
//! expression checker calls into it from the construction sites it guards.

use super::check::Checker;
use super::itype::{ITy, VarKind};
use super::tree::{TExpr, TExprKind};
use crate::ast;
use crate::diag::Diagnostic;
use crate::span::Span;

/// Where a reference value ultimately borrows from (v1.3 elision, design R6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provenance {
    /// The reference borrows from this function parameter.
    Param(usize),
    /// The reference borrows from a local value of the current function.
    Local,
    /// The reference comes from somewhere the analysis does not track.
    Unknown,
}

/// Combine the provenance of several alternative branches.
fn common_provenance(mut provenances: Vec<Provenance>) -> Provenance {
    match provenances.pop() {
        None => Provenance::Unknown,
        Some(first) => {
            if provenances.iter().all(|other| *other == first) {
                first
            } else {
                Provenance::Unknown
            }
        }
    }
}

/// Resolve a type and remove every outer reference level.
fn peel_references(checker: &Checker<'_>, ty: &ITy) -> ITy {
    let mut current = checker.infer.resolve(ty);
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 16 {
            return current;
        }
        match current {
            ITy::Ref { inner, .. } => current = checker.infer.resolve(&inner),
            _ => return current,
        }
    }
}

impl<'a> Checker<'a> {
    /// Resolve a type and remove every outer reference level.
    pub fn peel_refs(&self, ty: &ITy) -> ITy {
        peel_references(self, ty)
    }

    /// Peel references for pattern matching together with the match ergonomics
    /// binding mode: None means a by-value binding, Some(false) a shared
    /// reference and Some(true) a mutable reference.
    pub fn peel_pattern_refs(&self, ty: &ITy) -> (ITy, Option<bool>) {
        let mut current = self.infer.resolve(ty);
        let mut mode: Option<bool> = None;
        let mut guard = 0;
        loop {
            guard += 1;
            if guard > 16 {
                return (current, mode);
            }
            match current {
                ITy::Ref { mutable, inner } => {
                    mode = Some(match mode {
                        None => mutable,
                        Some(true) => mutable,
                        Some(false) => false,
                    });
                    current = self.infer.resolve(&inner);
                }
                _ => return (current, mode),
            }
        }
    }

    /// Peel the references and cell types that field, index and method access
    /// implicitly dereference (shared references, Rc and Box payloads).
    pub fn peel_access_refs(&self, ty: &ITy) -> ITy {
        let mut current = self.infer.resolve(ty);
        let mut guard = 0;
        loop {
            guard += 1;
            if guard > 16 {
                return current;
            }
            match current {
                ITy::Ref { inner, .. } => current = self.infer.resolve(&inner),
                ITy::Rc(inner) => current = self.infer.resolve(&inner),
                ITy::Box(inner) => current = self.infer.resolve(&inner),
                _ => return current,
            }
        }
    }

    /// How mutable the data behind a reference chain is. Some(true) means every
    /// level is mutable, Some(false) means at least one level is shared, and
    /// None means the type is not a reference at all.
    pub fn ref_chain_mutability(&self, ty: &ITy) -> Option<bool> {
        let mut current = self.infer.resolve(ty);
        let mut mutable = true;
        let mut seen = false;
        let mut guard = 0;
        loop {
            guard += 1;
            if guard > 16 {
                break;
            }
            match current {
                ITy::Ref {
                    mutable: level,
                    inner,
                } => {
                    seen = true;
                    if !level {
                        mutable = false;
                    }
                    current = self.infer.resolve(&inner);
                }
                _ => break,
            }
        }
        if seen { Some(mutable) } else { None }
    }

    /// Whether a type stores a reference anywhere outside of a function
    /// signature. Used to reject references in aggregates.
    pub fn type_contains_ref(&self, ty: &ITy) -> bool {
        match self.infer.resolve(ty) {
            ITy::Ref { .. } => true,
            ITy::Vec(inner) | ITy::Rc(inner) | ITy::Box(inner) | ITy::Option(inner) => {
                self.type_contains_ref(&inner)
            }
            ITy::Result(ok, err) => self.type_contains_ref(&ok) || self.type_contains_ref(&err),
            ITy::Tuple(elems) => elems.iter().any(|elem| self.type_contains_ref(elem)),
            _ => false,
        }
    }

    /// Wrap an expression in explicit Deref nodes until its type is no longer a
    /// reference. Codegen uses those place expressions for builtin receivers.
    pub fn wrap_derefs(&self, mut expr: TExpr) -> TExpr {
        let mut guard = 0;
        while let ITy::Ref { inner, .. } = self.infer.resolve(&expr.ty) {
            guard += 1;
            if guard > 16 {
                break;
            }
            let ty = self.infer.resolve(&inner);
            let span = expr.span;
            expr = TExpr::new(
                ty,
                TExprKind::Deref {
                    expr: Box::new(expr),
                },
                span,
            );
        }
        expr
    }

    /// Whether a value of this type is a plain value type (no heap ownership,
    /// no references, no function values). Version 1.4 additionally allows
    /// mutable references to owned heap types; see
    /// [Self::is_mutable_reference_target] for the full answer.
    pub fn is_plain_value_type(&mut self, ty: &ITy) -> bool {
        self.is_plain_value_type_at(ty, 0)
    }

    fn is_plain_value_type_at(&mut self, ty: &ITy, depth: usize) -> bool {
        if depth > 64 {
            return false;
        }
        let resolved = self.infer.resolve(ty);
        match resolved {
            ITy::Unit | ITy::Bool | ITy::Int(_) | ITy::Float(_) | ITy::Var(_) => true,
            ITy::Option(inner) => self.is_plain_value_type_at(&inner, depth + 1),
            ITy::Tuple(elems) => elems
                .iter()
                .all(|elem| self.is_plain_value_type_at(elem, depth + 1)),
            ITy::Result(ok, err) => {
                self.is_plain_value_type_at(&ok, depth + 1)
                    && self.is_plain_value_type_at(&err, depth + 1)
            }
            ITy::Struct(struct_id) => {
                self.ensure_struct_fields(struct_id);
                let fields = self.struct_fields(struct_id);
                fields
                    .iter()
                    .all(|(_, _, field_ty)| self.is_plain_value_type_at(field_ty, depth + 1))
            }
            ITy::Enum(enum_id) => {
                self.ensure_enum_variants(enum_id);
                let variant_count = self.variant_count(enum_id);
                let mut plain = true;
                for variant in 0..variant_count {
                    let fields = self.variant_fields(enum_id, variant);
                    for field_ty in &fields {
                        if !self.is_plain_value_type_at(field_ty, depth + 1) {
                            plain = false;
                        }
                    }
                }
                plain
            }
            _ => false,
        }
    }

    /// Reject a type that stores references (struct fields, enum payloads,
    /// collection elements, tuple elements and closure captures).
    pub fn validate_storage_type(&mut self, ty: &ITy, span: Span) -> bool {
        if self.type_contains_ref(ty) {
            let described = self.describe_resolved(ty);
            self.error_code(
                format!(
                    "references cannot be stored in this type in version 1.4 (found {})",
                    described
                ),
                span,
                "E0498",
            );
            return false;
        }
        true
    }

    /// Validate a type that is allowed to be a reference: references may only
    /// appear as function parameter types, return types, local let types and
    /// expression result types. Mutable references must be plain values.
    pub fn validate_value_type(&mut self, ty: &ITy, span: Span) -> bool {
        let resolved = self.infer.resolve(ty);
        match resolved {
            ITy::Ref { mutable, inner } => {
                let mut valid = true;
                if mutable && !self.is_mutable_reference_target(&inner) {
                    let described = self.describe_resolved(&inner);
                    self.error_code(
                        format!(
                            "mutable references to {} are not supported in version 1.4; take &{} or restructure the code",
                            described, described
                        ),
                        span,
                        "E0498",
                    );
                    valid = false;
                }
                if !self.validate_value_type(&inner, span) {
                    valid = false;
                }
                valid
            }
            ITy::Fn(params, ret) => {
                let mut valid = true;
                for param in params.iter() {
                    if !self.validate_value_type(param, span) {
                        valid = false;
                    }
                }
                if !self.validate_value_type(&ret, span) {
                    valid = false;
                }
                valid
            }
            ITy::Vec(inner) | ITy::Rc(inner) | ITy::Box(inner) | ITy::Option(inner) => {
                self.validate_storage_type(&inner, span)
            }
            ITy::Result(ok, err) => {
                self.validate_storage_type(&ok, span) && self.validate_storage_type(&err, span)
            }
            ITy::Tuple(elems) => {
                let mut valid = true;
                for elem in elems.iter() {
                    if !self.validate_storage_type(elem, span) {
                        valid = false;
                    }
                }
                valid
            }
            _ => true,
        }
    }

    /// Coerce an expression to the expected type. Only reference coercions are
    /// implicit: &mut T to &T, and &Rc<T> / &Box<T> to &T.
    pub fn coerce_to(&mut self, expr: TExpr, expected: &ITy, span: Span) -> TExpr {
        let actual = self.infer.resolve(&expr.ty);
        let wanted = self.infer.resolve(expected);
        let mut probe = self.infer.clone();
        if probe.unify(&actual, &wanted).is_ok() {
            let _ = self.unify_quiet(&actual, &wanted);
            return expr;
        }
        if let (
            ITy::Ref {
                inner: actual_inner,
                ..
            },
            ITy::Ref {
                mutable: wanted_mutable,
                inner: wanted_inner,
            },
        ) = (&actual, &wanted)
        {
            let actual_inner = self.infer.resolve(actual_inner);
            let wanted_inner = self.infer.resolve(wanted_inner);
            if !*wanted_mutable {
                let mut probe = self.infer.clone();
                if probe.unify(&actual_inner, &wanted_inner).is_ok() {
                    let _ = self.unify_quiet(&actual_inner, &wanted_inner);
                    return TExpr::new(
                        wanted.clone(),
                        TExprKind::Ref {
                            mutable: false,
                            expr: Box::new(expr),
                        },
                        span,
                    );
                }
                let cell_payload = match &actual_inner {
                    ITy::Rc(inner) | ITy::Box(inner) => Some(self.infer.resolve(inner)),
                    _ => None,
                };
                if let Some(cell_payload) = cell_payload {
                    let mut probe = self.infer.clone();
                    if probe.unify(&cell_payload, &wanted_inner).is_ok() {
                        let _ = self.unify_quiet(&cell_payload, &wanted_inner);
                        return TExpr::new(
                            wanted.clone(),
                            TExprKind::Ref {
                                mutable: false,
                                expr: Box::new(expr),
                            },
                            span,
                        );
                    }
                }
            }
        }
        if !self.unify_quiet(&actual, &wanted) {
            self.report_mismatch(&wanted, &actual, span);
        }
        expr
    }

    /// Check a borrow expression: &place or &mut place.
    pub fn check_ref(
        &mut self,
        mutable: bool,
        inner: &ast::Expr,
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let pointee_hint = hint.and_then(|hint| match self.infer.resolve(hint) {
            ITy::Ref { inner, .. } => Some((*inner).clone()),
            _ => None,
        });
        let inner_expr = self.check_expr(inner, pointee_hint.as_ref());
        let inner_ty = self.infer.resolve(&inner_expr.ty);
        if inner_ty.is_never() {
            return self.never_expr(span);
        }
        if !self.is_place_expr(&inner_expr) {
            self.error_code(
                "cannot borrow a temporary value, bind it to a local first".to_string(),
                span,
                "E0745",
            );
            return self.never_expr(span);
        }
        if mutable {
            let _ = self.require_mutable_place(&inner_expr, span, "borrow this value as mutable");
            if !self.is_mutable_reference_target(&inner_ty) {
                let described = self.describe_resolved(&inner_ty);
                self.error_code(
                    format!(
                        "mutable references to {} are not supported in version 1.4; take &{} or restructure the code",
                        described, described
                    ),
                    span,
                    "E0498",
                );
            }
        }
        let ty = ITy::Ref {
            mutable,
            inner: Box::new(inner_ty),
        };
        self.apply_hint(&ty, hint);
        TExpr::new(
            ty,
            TExprKind::Ref {
                mutable,
                expr: Box::new(inner_expr),
            },
            span,
        )
    }

    /// Check a dereference expression: *reference.
    pub fn check_deref(&mut self, inner: &ast::Expr, span: Span, hint: Option<&ITy>) -> TExpr {
        let inner_expr = self.check_expr(inner, None);
        let inner_ty = self.infer.resolve(&inner_expr.ty);
        match inner_ty {
            ITy::Never => self.never_expr(span),
            ITy::Ref { inner, .. } => {
                let pointee = self.infer.resolve(&inner);
                self.apply_hint(&pointee, hint);
                TExpr::new(
                    pointee,
                    TExprKind::Deref {
                        expr: Box::new(inner_expr),
                    },
                    span,
                )
            }
            ITy::Var(id) => {
                // A literal-only variable cannot be a reference: default the
                // literal and report the concrete type instead of the generic
                // annotations-needed error.
                let literal_default = match self.infer.var_kind(id) {
                    VarKind::IntLiteral => Some(ITy::i64()),
                    VarKind::FloatLiteral => Some(ITy::f64()),
                    VarKind::General => None,
                };
                if let Some(default) = literal_default {
                    let _ = self.unify_quiet(&ITy::Var(id), &default);
                    let described = self.describe_resolved(&default);
                    self.error_code(
                        format!("type {} cannot be dereferenced", described),
                        span,
                        "E0614",
                    );
                    return self.never_expr(span);
                }
                let pointee = match hint {
                    Some(hint) => self.infer.resolve(hint),
                    None => self.infer.fresh_var(),
                };
                let reference = ITy::Ref {
                    mutable: false,
                    inner: Box::new(pointee.clone()),
                };
                if self.unify_quiet(&ITy::Var(id), &reference) {
                    TExpr::new(
                        pointee,
                        TExprKind::Deref {
                            expr: Box::new(inner_expr),
                        },
                        span,
                    )
                } else {
                    self.error_code(
                        "type annotations needed: cannot dereference a value of unknown type"
                            .to_string(),
                        span,
                        "E0282",
                    );
                    self.never_expr(span)
                }
            }
            other => {
                let described = self.describe_resolved(&other);
                self.error_code(
                    format!("type {} cannot be dereferenced", described),
                    span,
                    "E0614",
                );
                self.never_expr(span)
            }
        }
    }

    /// Index of the parameter bound to this local C name, if any.
    pub fn param_index_for(&self, c_name: &str) -> Option<usize> {
        let info = self.fns.get(self.current_fn)?;
        info.param_cnames
            .iter()
            .position(|candidate| candidate == c_name)
    }

    /// Provenance of a place expression: where the place lives.
    pub fn place_provenance(&self, expr: &TExpr) -> Provenance {
        match &expr.kind {
            TExprKind::Local { c_name, .. } => match self.param_index_for(c_name) {
                Some(index) => Provenance::Param(index),
                None => Provenance::Local,
            },
            TExprKind::Capture { .. } => Provenance::Local,
            TExprKind::Field { base, .. } | TExprKind::Index { base, .. } => {
                self.place_provenance(base)
            }
            TExprKind::Deref { expr } => self.reference_source(expr),
            _ => Provenance::Unknown,
        }
    }

    /// Provenance of a reference-valued expression: which input it borrows.
    pub fn reference_source(&self, expr: &TExpr) -> Provenance {
        match &expr.kind {
            TExprKind::Ref { expr, .. } => {
                if self.is_place_expr(expr) {
                    self.place_provenance(expr)
                } else {
                    self.reference_source(expr)
                }
            }
            TExprKind::Local { c_name, .. } => match self.param_index_for(c_name) {
                Some(index) => Provenance::Param(index),
                None => self
                    .ref_local_sources
                    .get(c_name)
                    .copied()
                    .unwrap_or(Provenance::Unknown),
            },
            TExprKind::Capture { .. } => Provenance::Local,
            TExprKind::Deref { expr } => self.reference_source(expr),
            TExprKind::Field { base, .. } | TExprKind::Index { base, .. } => {
                self.reference_source(base)
            }
            TExprKind::Block(block) => match &block.tail {
                Some(tail) => self.reference_source(tail),
                None => Provenance::Unknown,
            },
            TExprKind::If {
                then_block,
                else_expr,
                ..
            } => {
                let mut provenances = Vec::new();
                if let Some(tail) = &then_block.tail {
                    provenances.push(self.reference_source(tail));
                }
                if let Some(else_expr) = else_expr {
                    provenances.push(self.reference_source(else_expr));
                }
                common_provenance(provenances)
            }
            TExprKind::Match { arms, .. } => common_provenance(
                arms.iter()
                    .map(|arm| self.reference_source(&arm.body))
                    .collect(),
            ),
            TExprKind::Cast { expr } | TExprKind::Unary { expr, .. } => self.reference_source(expr),
            _ => Provenance::Unknown,
        }
    }

    /// Check that a returned reference borrows from the elided lifetime source
    /// (design R6). Unknown provenance is left to the borrow checker.
    pub fn check_returned_reference(&mut self, value: &TExpr, source: Option<usize>, span: Span) {
        if !matches!(self.infer.resolve(&value.ty), ITy::Ref { .. }) {
            return;
        }
        match self.reference_source(value) {
            Provenance::Param(actual) => {
                if let Some(expected) = source
                    && actual != expected
                {
                    let param_name = self
                        .fns
                        .get(self.current_fn)
                        .and_then(|info| info.decl.params.get(expected))
                        .map(|param| param.name.clone())
                        .unwrap_or_else(|| format!("p{}", expected));
                    let diag = Diagnostic::error(
                        "returns a reference to a value that is not the elided lifetime source"
                            .to_string(),
                        span,
                    )
                    .with_code("E0515")
                    .with_note(format!(
                        "the return type borrows from parameter '{}', but this reference borrows from another input",
                        param_name
                    ));
                    self.diagnostics.push(diag);
                }
            }
            Provenance::Local => {
                let diag =
                    Diagnostic::error("returns a reference to a local value".to_string(), span)
                        .with_code("E0515")
                        .with_note(
                            "the returned reference must borrow from a function parameter"
                                .to_string(),
                        );
                self.diagnostics.push(diag);
            }
            Provenance::Unknown => {}
        }
    }
}
