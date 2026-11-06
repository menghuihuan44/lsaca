//! Slice and `str` expressions: `base[start..end]` behind a reference, plus the
//! implicit `&Vec<T> -> &[T]`, `&mut Vec<T> -> &mut [T]`, `&String -> &str`
//! and `&mut String -> &mut str` coercions (DESIGN 2.0 section 8).
//!
//! Every slice value lowers to a reference to a slice node; a bare unsized
//! slice expression is an unsized-placement error (E0001).

use crate::ast;
use crate::span::Span;

use super::check::Checker;
use super::itype::ITy;
use super::tree::{TExpr, TExprKind};

impl Checker<'_> {
    /// Check a slice bound expression. Bounds are `usize` indices.
    pub fn check_slice_bound(&mut self, bound: &ast::Expr) -> TExpr {
        let expected = ITy::usize();
        let expr = self.check_expr(bound, Some(&expected));
        let ty = self.infer.resolve(&expr.ty);
        if !self.unify_quiet(&expected, &ty) {
            self.report_mismatch(&expected, &ty, bound.span);
        }
        expr
    }

    /// The unsized element type produced by slicing the value `base`.
    fn slice_element_ty(&mut self, base: &TExpr) -> Option<ITy> {
        let ty = self.infer.resolve(&base.ty);
        match &ty {
            ITy::Vec(elem) | ITy::Slice(elem) => Some(ITy::slice((**elem).clone())),
            ITy::String | ITy::Str => Some(ITy::Str),
            _ => None,
        }
    }

    /// Turn an already-checked slice base into the place expression that is
    /// sliced (references are dereferenced).
    fn slice_place(&mut self, expr: TExpr, span: Span) -> Option<TExpr> {
        let expr = self.wrap_derefs(expr);
        if !self.is_place_expr(&expr) {
            self.error_code(
                "cannot borrow a temporary value, bind it to a local first".to_string(),
                span,
                "E0745",
            );
            return None;
        }
        Some(expr)
    }

    /// Check `&base[start..end]` / `&mut base[start..end]`.
    // Slice syntax carries four independent AST parts plus the reference
    // context; bundling them would only shuffle the same arguments for the
    // single caller.
    #[allow(clippy::too_many_arguments)]
    pub fn check_slice_ref(
        &mut self,
        mutable: bool,
        base: &ast::Expr,
        start: Option<&ast::Expr>,
        end: Option<&ast::Expr>,
        inclusive: bool,
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let base_expr = self.check_expr(base, None);
        if matches!(self.infer.resolve(&base_expr.ty), ITy::Never) {
            return self.never_expr(span);
        }
        let start_expr = start.map(|start| self.check_slice_bound(start));
        let end_expr = end.map(|end| self.check_slice_bound(end));
        let Some(base_expr) = self.slice_place(base_expr, base.span) else {
            return self.never_expr(span);
        };
        let Some(inner_ty) = self.slice_element_ty(&base_expr) else {
            let base_ty = self.infer.resolve(&base_expr.ty);
            let described = self.describe_resolved(&base_ty);
            self.error_code(
                format!("cannot slice a value of type {}", described),
                base.span,
                "E0608",
            );
            return self.never_expr(span);
        };
        if mutable {
            self.require_mutable_place(&base_expr, span, "borrow this value as mutable");
        }
        let ty = ITy::Ref {
            mutable,
            inner: Box::new(inner_ty),
        };
        if let Some(hint) = hint {
            let _ = self.unify_quiet(&ty, hint);
        }
        TExpr::new(
            ty,
            TExprKind::Slice {
                base: Box::new(base_expr),
                start: start_expr.map(Box::new),
                end: end_expr.map(Box::new),
                inclusive,
            },
            span,
        )
    }

    /// Check a slice expression that is not behind a reference. Slicing is only
    /// meaningful behind `&` or `&mut`, so this reports the unsized-placement
    /// error (E0001) and yields the never type.
    pub fn check_bare_slice(
        &mut self,
        base: &ast::Expr,
        start: Option<&ast::Expr>,
        end: Option<&ast::Expr>,
        _inclusive: bool,
        span: Span,
    ) -> TExpr {
        let base_expr = self.check_expr(base, None);
        if matches!(self.infer.resolve(&base_expr.ty), ITy::Never) {
            return self.never_expr(span);
        }
        if let Some(start) = start {
            self.check_slice_bound(start);
        }
        if let Some(end) = end {
            self.check_slice_bound(end);
        }
        let base_expr = self.wrap_derefs(base_expr);
        match self.slice_element_ty(&base_expr) {
            Some(inner_ty) => {
                let message = Self::unsized_type_message(&inner_ty);
                self.error_code(message, span, "E0001");
            }
            None => {
                let base_ty = self.infer.resolve(&base_expr.ty);
                let described = self.describe_resolved(&base_ty);
                self.error_code(
                    format!("cannot slice a value of type {}", described),
                    base.span,
                    "E0608",
                );
            }
        }
        self.never_expr(span)
    }

    /// Coerce `&Vec<T>`/`&mut Vec<T>` to `&[T]`/`&mut [T]` and `&String`/
    /// `&mut String` to `&str`/`&mut str`.
    pub fn try_slice_coerce(
        &mut self,
        expr: TExpr,
        actual: &ITy,
        wanted: &ITy,
        span: Span,
    ) -> Option<TExpr> {
        let wanted = self.infer.resolve(wanted);
        let (wanted_mutable, wanted_inner) = match &wanted {
            ITy::Ref { mutable, inner } => (*mutable, (**inner).clone()),
            _ => return None,
        };
        let wanted_inner = self.infer.resolve(&wanted_inner);
        if !matches!(wanted_inner, ITy::Slice(_) | ITy::Str) {
            return None;
        }
        let actual = self.infer.resolve(actual);
        let (actual_mutable, actual_inner) = match &actual {
            ITy::Ref { mutable, inner } => (*mutable, (**inner).clone()),
            _ => return None,
        };
        if wanted_mutable && !actual_mutable {
            return None;
        }
        let actual_inner = self.infer.resolve(&actual_inner);
        let inner_ty = match (&wanted_inner, &actual_inner) {
            (ITy::Slice(wanted_elem), ITy::Vec(actual_elem))
            | (ITy::Slice(wanted_elem), ITy::Slice(actual_elem)) => {
                if !self.unify_quiet(wanted_elem, actual_elem) {
                    return None;
                }
                ITy::slice((**wanted_elem).clone())
            }
            (ITy::Str, ITy::String) | (ITy::Str, ITy::Str) => ITy::Str,
            _ => return None,
        };
        let base = self.wrap_derefs(expr);
        let result_ty = ITy::Ref {
            mutable: wanted_mutable,
            inner: Box::new(inner_ty),
        };
        let _ = self.unify_quiet(&result_ty, &wanted);
        Some(TExpr::new(
            result_ty,
            TExprKind::Slice {
                base: Box::new(base),
                start: None,
                end: None,
                inclusive: false,
            },
            span,
        ))
    }
}
