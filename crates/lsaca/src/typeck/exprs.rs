//! Expression checking: dispatch, control flow, literals and closure bodies.
//!
//! Operators live in ops.rs, calls and macros in calls.rs, path resolution in
//! paths.rs.

mod calls;
mod ops;
mod paths;

use super::check::{Checker, Frame, block_type};
use super::items::TypeKey;
use super::itype::ITy;
use super::tree::{TBlock, TCapture, TClosure, TExpr, TExprKind, TLit, TParam};
use crate::ast::{self, ExprKind};
use crate::diag::Diagnostic;
use crate::hir::HFieldRef;
use crate::span::Span;

impl<'a> Checker<'a> {
    /// Check one expression, using the hint as the expected type where that
    /// helps local inference.
    pub fn check_expr(&mut self, expr: &ast::Expr, hint: Option<&ITy>) -> TExpr {
        let span = expr.span;
        match &expr.kind {
            ExprKind::Lit(lit) => self.check_literal(lit, hint, span),
            ExprKind::Path(path) => self.check_path_value(path, hint, span),
            ExprKind::Unary { op, expr: inner } => self.check_unary(*op, inner, span, hint),
            ExprKind::Binary { op, lhs, rhs } => self.check_binary(*op, lhs, rhs, span, hint),
            ExprKind::Assign { op, target, value } => self.check_assign(*op, target, value, span),
            ExprKind::Cast { expr: inner, ty } => self.check_cast(inner, ty, span, hint),
            ExprKind::Call { callee, args } => self.check_call(callee, args, span, hint),
            ExprKind::MethodCall {
                receiver,
                method,
                method_span,
                turbofish,
                args,
            } => {
                self.check_method_call(receiver, method, *method_span, turbofish, args, span, hint)
            }
            ExprKind::Field {
                base,
                name,
                name_span,
            } => self.check_field(base, name, *name_span, span, hint),
            ExprKind::TupleField {
                base,
                index,
                index_span,
            } => self.check_tuple_field(base, *index, *index_span, span, hint),
            ExprKind::Index { base, index } => self.check_index(base, index, span, hint),
            ExprKind::If(if_expr) => self.check_if(if_expr, hint),
            ExprKind::Match(match_expr) => self.check_match(match_expr, hint),
            ExprKind::Block(block) => self.check_block_expr(block, hint, span),
            ExprKind::Loop { body } => self.check_loop(body, span),
            ExprKind::While { cond, body } => self.check_while(cond, body, span),
            ExprKind::For { pat, iter, body } => self.check_for(pat, iter, body, span),
            ExprKind::Range { .. } => {
                self.error_code(
                    "range expressions are only allowed as the iterable of a for loop".to_string(),
                    span,
                    "E0001",
                );
                self.never_expr(span)
            }
            ExprKind::Closure(closure) => self.check_closure(closure, hint, span),
            ExprKind::StructLit { path, fields } => self.check_struct_lit(path, fields, span, hint),
            ExprKind::Tuple(elems) => self.check_tuple(elems, span, hint),
            ExprKind::Vec(vec_lit) => self.check_vec_lit(vec_lit, span, hint),
            ExprKind::Macro(mac) => self.check_macro(mac, span, hint),
            ExprKind::Break(value) => self.check_break(value.as_deref(), span),
            ExprKind::Continue => self.check_continue(span),
            ExprKind::Return(value) => self.check_return(value.as_deref(), span),
        }
    }

    fn check_literal(&mut self, lit: &ast::Lit, hint: Option<&ITy>, span: Span) -> TExpr {
        match lit {
            ast::Lit::Int { value, raw } => {
                let ty = self.infer.int_literal(*value, raw.clone(), span);
                self.apply_hint(&ty, hint);
                TExpr::new(ty, TExprKind::Lit(TLit::Int(*value)), span)
            }
            ast::Lit::Float { value, raw } => {
                let ty = self.infer.float_literal(*value, raw.clone(), span);
                self.apply_hint(&ty, hint);
                TExpr::new(ty, TExprKind::Lit(TLit::Float(*value)), span)
            }
            ast::Lit::Bool(value) => {
                let ty = ITy::bool_();
                self.apply_hint(&ty, hint);
                TExpr::new(ty, TExprKind::Lit(TLit::Bool(*value)), span)
            }
            ast::Lit::Str(value) => {
                let ty = ITy::string();
                self.apply_hint(&ty, hint);
                TExpr::new(ty, TExprKind::Lit(TLit::Str(value.clone())), span)
            }
        }
    }

    fn check_block_expr(&mut self, block: &ast::Block, hint: Option<&ITy>, span: Span) -> TExpr {
        let inner = self.check_block(block, hint);
        let ty = block_type(&inner);
        if let Some(hint) = hint {
            let _ = self.unify_quiet(hint, &ty);
        }
        TExpr::new(ty, TExprKind::Block(inner), span)
    }

    fn check_if(&mut self, if_expr: &ast::IfExpr, hint: Option<&ITy>) -> TExpr {
        let span = if_expr.span;
        let cond = self.check_expr(&if_expr.cond, Some(&ITy::bool_()));
        self.require_bool(&cond.ty, if_expr.cond.span);
        let then_block = self.check_block(&if_expr.then_block, hint);
        let then_ty = block_type(&then_block);
        let (else_expr, result_ty) = match &if_expr.else_expr {
            Some(else_ast) => {
                let expected = match hint {
                    Some(hint) => hint.clone(),
                    None => then_ty.clone(),
                };
                let else_expr = self.check_expr(else_ast, Some(&expected));
                let else_ty = else_expr.ty.clone();
                if !self.unify_quiet(&then_ty, &else_ty) {
                    let resolved_then = self.infer.resolve(&then_ty);
                    let resolved_else = self.infer.resolve(&else_ty);
                    self.report_mismatch(&resolved_then, &resolved_else, else_ast.span);
                }
                let result = if then_ty.is_never() {
                    else_ty.clone()
                } else {
                    then_ty.clone()
                };
                (Some(Box::new(else_expr)), result)
            }
            None => {
                if !self.unify_quiet(&ITy::unit(), &then_ty) {
                    let resolved = self.infer.resolve(&then_ty);
                    self.report_mismatch(&ITy::unit(), &resolved, if_expr.then_block.span);
                }
                (None, ITy::unit())
            }
        };
        TExpr::new(
            result_ty,
            TExprKind::If {
                cond: Box::new(cond),
                then_block,
                else_expr,
            },
            span,
        )
    }

    fn check_match(&mut self, match_expr: &ast::MatchExpr, hint: Option<&ITy>) -> TExpr {
        let scrutinee = self.check_expr(&match_expr.scrutinee, None);
        let scrutinee_ty = self.infer.resolve(&scrutinee.ty);
        let mut arms = Vec::new();
        let mut result_ty: Option<ITy> = None;
        for arm in &match_expr.arms {
            self.push_scope();
            let pat = self.check_pattern(&arm.pat, &scrutinee_ty);
            let guard = match &arm.guard {
                Some(guard_ast) => {
                    let guard = self.check_expr(guard_ast, Some(&ITy::bool_()));
                    self.require_bool(&guard.ty, guard_ast.span);
                    Some(guard)
                }
                None => None,
            };
            let body_hint = match hint {
                Some(hint) => Some(hint.clone()),
                None => result_ty.clone(),
            };
            let body = self.check_expr(&arm.body, body_hint.as_ref());
            self.pop_scope();
            let body_ty = body.ty.clone();
            match &result_ty {
                Some(previous) => {
                    let previous = previous.clone();
                    if !self.unify_quiet(&previous, &body_ty) {
                        let resolved_previous = self.infer.resolve(&previous);
                        let resolved_body = self.infer.resolve(&body_ty);
                        self.report_mismatch(&resolved_previous, &resolved_body, arm.body.span);
                    }
                }
                None => {
                    if !body_ty.is_never() {
                        result_ty = Some(body_ty.clone());
                    }
                }
            }
            arms.push(super::tree::TArm { pat, guard, body });
        }
        self.check_match_exhaustive(&scrutinee_ty, &arms, match_expr.span);
        let result = result_ty.unwrap_or_else(ITy::never);
        if let Some(hint) = hint {
            let _ = self.unify_quiet(hint, &result);
        }
        TExpr::new(
            result,
            TExprKind::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            match_expr.span,
        )
    }

    fn check_loop(&mut self, body: &ast::Block, span: Span) -> TExpr {
        self.bump_loop_depth(1);
        let body = self.check_block(body, None);
        self.bump_loop_depth(-1);
        TExpr::new(ITy::never(), TExprKind::Loop { body }, span)
    }

    fn check_while(&mut self, cond: &ast::Expr, body: &ast::Block, span: Span) -> TExpr {
        let cond = self.check_expr(cond, Some(&ITy::bool_()));
        self.require_bool(&cond.ty, cond.span);
        self.bump_loop_depth(1);
        let body = self.check_block(body, None);
        self.bump_loop_depth(-1);
        TExpr::new(
            ITy::unit(),
            TExprKind::While {
                cond: Box::new(cond),
                body,
            },
            span,
        )
    }

    fn check_for(
        &mut self,
        pat: &ast::Pattern,
        iter: &ast::Expr,
        body: &ast::Block,
        span: Span,
    ) -> TExpr {
        if let ExprKind::Range {
            start,
            end,
            inclusive,
        } = &iter.kind
        {
            let start_expr = self.check_expr(start, Some(&ITy::i64()));
            self.require_i64(&start_expr.ty, start.span);
            let end_expr = self.check_expr(end, Some(&ITy::i64()));
            self.require_i64(&end_expr.ty, end.span);
            self.push_scope();
            let pat = self.check_irrefutable_pattern(pat, &ITy::i64(), pat.span);
            self.bump_loop_depth(1);
            let body = self.check_block(body, None);
            self.bump_loop_depth(-1);
            self.pop_scope();
            return TExpr::new(
                ITy::unit(),
                TExprKind::ForRange {
                    pat,
                    start: Box::new(start_expr),
                    end: Box::new(end_expr),
                    inclusive: *inclusive,
                    body,
                },
                span,
            );
        }
        let iter_expr = self.check_expr(iter, None);
        let iter_ty = self.infer.resolve(&iter_expr.ty);
        let elem_ty = match &iter_ty {
            ITy::Vec(elem) => (**elem).clone(),
            ITy::Never => ITy::Unit,
            _ => {
                let described = self.describe_resolved(&iter_ty);
                self.error_code(
                    format!(
                        "cannot iterate over a value of type {}; for loops require a range or Vec<T>",
                        described
                    ),
                    iter.span,
                    "E0277",
                );
                ITy::Unit
            }
        };
        self.push_scope();
        let pat = self.check_irrefutable_pattern(pat, &elem_ty, pat.span);
        self.bump_loop_depth(1);
        let body = self.check_block(body, None);
        self.bump_loop_depth(-1);
        self.pop_scope();
        TExpr::new(
            ITy::unit(),
            TExprKind::ForVec {
                pat,
                vec: Box::new(iter_expr),
                body,
            },
            span,
        )
    }

    fn check_break(&mut self, value: Option<&ast::Expr>, span: Span) -> TExpr {
        if !self.in_loop() {
            let mut diagnostic = Diagnostic::error("break outside of a loop".to_string(), span)
                .with_code("E0268")
                .with_span_label(span);
            if self
                .frames
                .last()
                .map(|frame| frame.is_closure)
                .unwrap_or(false)
            {
                diagnostic =
                    diagnostic.with_note("break cannot cross a closure boundary".to_string());
            }
            self.diagnostics.push(diagnostic);
        }
        if let Some(value) = value {
            let _ = self.check_expr(value, None);
            self.error_code(
                "break with a value is not supported; loops have type ()".to_string(),
                value.span,
                "E0308",
            );
        }
        TExpr::new(ITy::never(), TExprKind::Break, span)
    }

    fn check_continue(&mut self, span: Span) -> TExpr {
        if !self.in_loop() {
            let mut diagnostic = Diagnostic::error("continue outside of a loop".to_string(), span)
                .with_code("E0268")
                .with_span_label(span);
            if self
                .frames
                .last()
                .map(|frame| frame.is_closure)
                .unwrap_or(false)
            {
                diagnostic =
                    diagnostic.with_note("continue cannot cross a closure boundary".to_string());
            }
            self.diagnostics.push(diagnostic);
        }
        TExpr::new(ITy::never(), TExprKind::Continue, span)
    }

    fn check_return(&mut self, value: Option<&ast::Expr>, span: Span) -> TExpr {
        if self.in_const {
            self.error_code(
                "return is not allowed in a constant initializer".to_string(),
                span,
                "E0001",
            );
            if let Some(value) = value {
                let _ = self.check_expr(value, None);
            }
            return self.never_expr(span);
        }
        let expected = self
            .frames
            .last()
            .map(|frame| frame.ret.clone())
            .unwrap_or_else(ITy::unit);
        let value = match value {
            Some(value_ast) => {
                let value = self.check_expr(value_ast, Some(&expected));
                if !self.unify_quiet(&expected, &value.ty) {
                    let resolved_expected = self.infer.resolve(&expected);
                    let resolved_value = self.infer.resolve(&value.ty);
                    self.report_mismatch(&resolved_expected, &resolved_value, value_ast.span);
                }
                Some(Box::new(value))
            }
            None => {
                if !self.unify_quiet(&expected, &ITy::unit()) {
                    let resolved = self.infer.resolve(&expected);
                    self.report_mismatch(&resolved, &ITy::unit(), span);
                }
                None
            }
        };
        TExpr::new(ITy::never(), TExprKind::Return(value), span)
    }

    fn check_closure(
        &mut self,
        closure: &ast::ClosureExpr,
        hint: Option<&ITy>,
        span: Span,
    ) -> TExpr {
        let hint_signature = match hint {
            Some(hint) => match self.infer.resolve(hint) {
                ITy::Fn(params, ret) if params.len() == closure.params.len() => {
                    Some((params, (*ret).clone()))
                }
                _ => None,
            },
            None => None,
        };
        let closure_id = self.reserve_closure();
        let c_name = self.fresh_closure_c_name();
        let mut param_tys: Vec<ITy> = Vec::new();
        for (index, param) in closure.params.iter().enumerate() {
            let annotated = match &param.ty {
                Some(ty) => self.resolve_ty(ty, self.current_self_key()),
                None => None,
            };
            let from_hint = hint_signature
                .as_ref()
                .and_then(|(params, _)| params.get(index).cloned());
            let ty = match annotated {
                Some(ty) => {
                    if let Some(expected) = &from_hint {
                        let _ = self.unify_quiet(expected, &ty);
                    }
                    ty
                }
                None => match from_hint {
                    Some(expected) => expected,
                    None => self.infer.fresh_var(),
                },
            };
            param_tys.push(ty);
        }
        let from_hint_ret = hint_signature.as_ref().map(|(_, ret)| ret.clone());
        let ret = match &closure.ret {
            Some(ty) => {
                let resolved = self
                    .resolve_ty(ty, self.current_self_key())
                    .unwrap_or_else(|| self.infer.fresh_var());
                if let Some(expected) = &from_hint_ret {
                    let _ = self.unify_quiet(expected, &resolved);
                }
                resolved
            }
            None => match from_hint_ret {
                Some(expected) => expected,
                None => self.infer.fresh_var(),
            },
        };
        let mut frame = Frame::new(ret.clone());
        frame.is_closure = true;
        self.frames.push(frame);
        let mut params = Vec::new();
        for (index, param) in closure.params.iter().enumerate() {
            let ty = param_tys[index].clone();
            match &param.pat.kind {
                ast::PatternKind::Binding { name, mutable } => {
                    let c_name = self.declare_local(name, ty.clone(), *mutable, param.pat.span);
                    params.push(TParam {
                        name: name.clone(),
                        c_name,
                        ty,
                    });
                }
                ast::PatternKind::Wild => {
                    let c_name = self.fresh_local_c_name();
                    self.declare_local_with_c_name(
                        "_",
                        ty.clone(),
                        false,
                        c_name.clone(),
                        param.pat.span,
                    );
                    params.push(TParam {
                        name: "_".to_string(),
                        c_name,
                        ty,
                    });
                }
                _ => {
                    self.error_code(
                        "closure parameters must be bindings or wildcards".to_string(),
                        param.pat.span,
                        "E0001",
                    );
                    let c_name = self.fresh_local_c_name();
                    params.push(TParam {
                        name: "_".to_string(),
                        c_name,
                        ty,
                    });
                }
            }
        }
        let body_expr = self.check_expr(&closure.body, Some(&ret));
        if !self.unify_quiet(&ret, &body_expr.ty) {
            let resolved_ret = self.infer.resolve(&ret);
            let resolved_body = self.infer.resolve(&body_expr.ty);
            self.report_mismatch(&resolved_ret, &resolved_body, closure.body.span);
        }
        let captures = self
            .frames
            .pop()
            .map(|frame| frame.captures)
            .unwrap_or_default();
        let captures = captures
            .into_iter()
            .map(|capture| TCapture {
                name: capture.name,
                ty: capture.ty,
                parent_expr: capture.parent_expr,
            })
            .collect();
        let body = TBlock {
            stmts: Vec::new(),
            tail: Some(Box::new(body_expr)),
            span: closure.body.span,
        };
        self.set_closure(
            closure_id,
            TClosure {
                id: closure_id,
                c_name,
                params,
                ret: ret.clone(),
                captures,
                body,
                span,
            },
        );
        let ty = ITy::function(param_tys, ret);
        self.apply_hint(&ty, hint);
        TExpr::new(ty, TExprKind::Closure { closure_id }, span)
    }

    fn check_tuple(&mut self, elems: &[ast::Expr], span: Span, hint: Option<&ITy>) -> TExpr {
        let expected_elems = match hint {
            Some(hint) => match self.infer.resolve(hint) {
                ITy::Tuple(expected) if expected.len() == elems.len() => Some(expected),
                ITy::Unit if elems.is_empty() => Some(Vec::new()),
                _ => None,
            },
            None => None,
        };
        let mut exprs = Vec::new();
        let mut actual = Vec::new();
        for (index, elem) in elems.iter().enumerate() {
            let expected = expected_elems
                .as_ref()
                .and_then(|expected| expected.get(index).cloned());
            let value = self.check_expr(elem, expected.as_ref());
            if let Some(expected) = &expected
                && !self.unify_quiet(expected, &value.ty)
            {
                let resolved_expected = self.infer.resolve(expected);
                let resolved_value = self.infer.resolve(&value.ty);
                self.report_mismatch(&resolved_expected, &resolved_value, elem.span);
            }
            actual.push(value.ty.clone());
            exprs.push(value);
        }
        let ty = match expected_elems {
            Some(expected) => ITy::tuple(expected),
            None => ITy::tuple(actual),
        };
        TExpr::new(ty, TExprKind::Tuple(exprs), span)
    }

    fn check_vec_lit(&mut self, lit: &ast::VecLit, span: Span, hint: Option<&ITy>) -> TExpr {
        let expected_elem = match hint {
            Some(hint) => match self.infer.resolve(hint) {
                ITy::Vec(elem) => Some((*elem).clone()),
                _ => None,
            },
            None => None,
        };
        match lit {
            ast::VecLit::List(elems) => {
                let elem_ty = expected_elem.unwrap_or_else(|| self.infer.fresh_var());
                let mut exprs = Vec::new();
                for elem in elems {
                    let value = self.check_expr(elem, Some(&elem_ty));
                    if !self.unify_quiet(&elem_ty, &value.ty) {
                        let resolved_elem = self.infer.resolve(&elem_ty);
                        let resolved_value = self.infer.resolve(&value.ty);
                        self.report_mismatch(&resolved_elem, &resolved_value, elem.span);
                    }
                    exprs.push(value);
                }
                TExpr::new(ITy::vec(elem_ty), TExprKind::VecLit { elems: exprs }, span)
            }
            ast::VecLit::Repeat { value, count } => {
                let elem_ty = expected_elem.unwrap_or_else(|| self.infer.fresh_var());
                let value = self.check_expr(value, Some(&elem_ty));
                if !self.unify_quiet(&elem_ty, &value.ty) {
                    let resolved_elem = self.infer.resolve(&elem_ty);
                    let resolved_value = self.infer.resolve(&value.ty);
                    self.report_mismatch(&resolved_elem, &resolved_value, span);
                }
                let count = self.check_expr(count, Some(&ITy::i64()));
                if !self.unify_quiet(&ITy::i64(), &count.ty) {
                    let resolved = self.infer.resolve(&count.ty);
                    self.report_mismatch(&ITy::i64(), &resolved, count.span);
                }
                if let Some(known) = self.const_i64(&count)
                    && known < 0
                {
                    self.error_code(
                        "the repeat count of vec![value; count] must be non-negative".to_string(),
                        count.span,
                        "E0001",
                    );
                }
                TExpr::new(
                    ITy::vec(elem_ty),
                    TExprKind::VecRepeat {
                        value: Box::new(value),
                        count: Box::new(count),
                    },
                    span,
                )
            }
        }
    }

    fn const_i64(&self, expr: &TExpr) -> Option<i128> {
        match &expr.kind {
            TExprKind::Lit(TLit::Int(value)) => Some(*value),
            TExprKind::Unary {
                op: crate::hir::HUnaryOp::Neg,
                expr,
            } => match &expr.kind {
                TExprKind::Lit(TLit::Int(value)) => Some(-*value),
                _ => None,
            },
            _ => None,
        }
    }

    fn check_struct_lit(
        &mut self,
        path: &ast::PathExpr,
        fields: &[ast::FieldInit],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let struct_id = match self.resolve_struct_path(path) {
            Some(struct_id) => struct_id,
            None => {
                for field in fields {
                    let _ = self.check_expr(&field.value, None);
                }
                return self.never_expr(span);
            }
        };
        self.ensure_struct_fields(struct_id);
        let struct_fields = self.struct_fields(struct_id);
        let struct_name = self.struct_name(struct_id);
        let mut seen: Vec<String> = Vec::new();
        let mut values: Vec<(usize, TExpr)> = Vec::new();
        for field in fields {
            if seen.contains(&field.name) {
                self.error_code(
                    format!("field {} specified more than once", field.name),
                    field.name_span,
                    "E0062",
                );
                continue;
            }
            seen.push(field.name.clone());
            let index = struct_fields
                .iter()
                .position(|(name, _, _)| *name == field.name);
            match index {
                Some(index) => {
                    let expected = struct_fields[index].2.clone();
                    let value = self.check_expr(&field.value, Some(&expected));
                    if !self.unify_quiet(&expected, &value.ty) {
                        let resolved_expected = self.infer.resolve(&expected);
                        let resolved_value = self.infer.resolve(&value.ty);
                        self.report_mismatch(&resolved_expected, &resolved_value, field.value.span);
                    }
                    values.push((index, value));
                }
                None => {
                    let candidates: Vec<&str> = struct_fields
                        .iter()
                        .map(|(name, _, _)| name.as_str())
                        .collect();
                    let suggestion = super::items::closest_name(&field.name, &candidates);
                    self.error_suggest(
                        format!("struct {} has no field named {}", struct_name, field.name),
                        suggestion,
                        field.name_span,
                        "E0560",
                    );
                    let _ = self.check_expr(&field.value, None);
                }
            }
        }
        for (name, _, _) in &struct_fields {
            if !seen.contains(name) {
                self.error_code(
                    format!("missing field {} in initializer of {}", name, struct_name),
                    span,
                    "E0063",
                );
            }
        }
        let ty = ITy::Struct(struct_id);
        self.apply_hint(&ty, hint);
        TExpr::new(
            ty,
            TExprKind::StructLit {
                struct_id,
                fields: values,
            },
            span,
        )
    }

    /// Resolve the path of a struct literal to a struct id.
    pub fn resolve_struct_path(&mut self, path: &ast::PathExpr) -> Option<usize> {
        let span = path.span;
        let name = match path.simple_name() {
            Some(name) => name.to_string(),
            None => {
                self.error_code(
                    "struct literals must use a single struct name".to_string(),
                    span,
                    "E0422",
                );
                return None;
            }
        };
        if name == "Self" {
            return match self.current_self_key() {
                Some(TypeKey::Struct(id)) => Some(id),
                Some(TypeKey::Enum(_)) => {
                    self.error_code(
                        "expected a struct literal, found an enum".to_string(),
                        span,
                        "E0532",
                    );
                    None
                }
                None => {
                    self.error_code(
                        "Self is only available inside an impl block".to_string(),
                        span,
                        "E0411",
                    );
                    None
                }
            };
        }
        if let Some(id) = self.struct_by_name.get(&name).copied() {
            return Some(id);
        }
        if self.enum_by_name.contains_key(&name) {
            self.error_code(
                format!("expected a struct literal, found enum {}", name),
                span,
                "E0532",
            );
            return None;
        }
        let suggestion = self.suggest_type_name(&name);
        self.error_suggest(
            format!("cannot find struct {} in this scope", name),
            suggestion,
            span,
            "E0412",
        );
        None
    }

    fn check_field(
        &mut self,
        base: &ast::Expr,
        name: &str,
        _name_span: Span,
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let base_expr = self.check_expr(base, None);
        let base_ty = self.infer.resolve(&base_expr.ty);
        match &base_ty {
            ITy::Struct(id) => {
                let id = *id;
                self.ensure_struct_fields(id);
                let fields = self.struct_fields(id);
                if let Some(index) = fields.iter().position(|(field, _, _)| field == name) {
                    let field_ty = fields[index].2.clone();
                    self.apply_hint(&field_ty, hint);
                    return TExpr::new(
                        field_ty,
                        TExprKind::Field {
                            base: Box::new(base_expr),
                            field: HFieldRef::Struct {
                                struct_id: id,
                                index,
                            },
                        },
                        span,
                    );
                }
                let candidates: Vec<&str> =
                    fields.iter().map(|(field, _, _)| field.as_str()).collect();
                let suggestion = super::items::closest_name(name, &candidates);
                let struct_name = self.struct_name(id);
                self.error_suggest(
                    format!("no field {} on type {}", name, struct_name),
                    suggestion,
                    span,
                    "E0609",
                );
                self.never_expr(span)
            }
            ITy::Var(_) => {
                self.error_code(
                    "type annotations needed: cannot access a field on a value of unknown type"
                        .to_string(),
                    span,
                    "E0282",
                );
                self.never_expr(span)
            }
            ITy::Never => self.never_expr(span),
            _ => {
                let described = self.describe_resolved(&base_ty);
                self.error_code(
                    format!("no field {} on type {}", name, described),
                    span,
                    "E0609",
                );
                self.never_expr(span)
            }
        }
    }

    fn check_tuple_field(
        &mut self,
        base: &ast::Expr,
        index: usize,
        _index_span: Span,
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let base_expr = self.check_expr(base, None);
        let base_ty = self.infer.resolve(&base_expr.ty);
        match &base_ty {
            ITy::Tuple(elems) => {
                if index < elems.len() {
                    let field_ty = elems[index].clone();
                    self.apply_hint(&field_ty, hint);
                    TExpr::new(
                        field_ty,
                        TExprKind::Field {
                            base: Box::new(base_expr),
                            field: HFieldRef::Tuple(index),
                        },
                        span,
                    )
                } else {
                    self.error_code(
                        format!(
                            "tuple index {} is out of bounds for a tuple of {} element(s)",
                            index,
                            elems.len()
                        ),
                        span,
                        "E0609",
                    );
                    self.never_expr(span)
                }
            }
            ITy::Var(_) => {
                self.error_code(
                    "type annotations needed: cannot access a tuple field on a value of unknown type"
                        .to_string(),
                    span,
                    "E0282",
                );
                self.never_expr(span)
            }
            ITy::Never => self.never_expr(span),
            _ => {
                let described = self.describe_resolved(&base_ty);
                self.error_code(
                    format!("no tuple field {} on type {}", index, described),
                    span,
                    "E0609",
                );
                self.never_expr(span)
            }
        }
    }

    fn check_index(
        &mut self,
        base: &ast::Expr,
        index: &ast::Expr,
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let base_expr = self.check_expr(base, None);
        let index_expr = self.check_expr(index, Some(&ITy::i64()));
        if !self.unify_quiet(&ITy::i64(), &index_expr.ty) {
            let resolved = self.infer.resolve(&index_expr.ty);
            self.report_mismatch(&ITy::i64(), &resolved, index.span);
        }
        let base_ty = self.infer.resolve(&base_expr.ty);
        match &base_ty {
            ITy::Vec(elem) => {
                let elem = (**elem).clone();
                self.apply_hint(&elem, hint);
                TExpr::new(
                    elem,
                    TExprKind::Index {
                        base: Box::new(base_expr),
                        index: Box::new(index_expr),
                    },
                    span,
                )
            }
            ITy::Never => self.never_expr(span),
            _ => {
                let described = self.describe_resolved(&base_ty);
                self.error_code(
                    format!("cannot index into a value of type {}", described),
                    span,
                    "E0608",
                );
                self.never_expr(span)
            }
        }
    }

    // -- helpers ------------------------------------------------------------

    pub fn apply_hint(&mut self, ty: &ITy, hint: Option<&ITy>) {
        if let Some(hint) = hint {
            let _ = self.unify_quiet(hint, ty);
        }
    }

    pub fn unify_quiet(&mut self, lhs: &ITy, rhs: &ITy) -> bool {
        let lhs = self.infer.resolve(lhs);
        let rhs = self.infer.resolve(rhs);
        self.infer.unify(&lhs, &rhs).is_ok()
    }

    pub fn never_expr(&self, span: Span) -> TExpr {
        TExpr::new(ITy::never(), TExprKind::Break, span)
    }

    pub fn require_bool(&mut self, ty: &ITy, span: Span) {
        let resolved = self.infer.resolve(ty);
        match &resolved {
            ITy::Bool | ITy::Never => {}
            ITy::Var(_) => {
                let _ = self.unify_quiet(&resolved, &ITy::bool_());
            }
            _ => self.report_mismatch(&ITy::bool_(), &resolved, span),
        }
    }

    pub fn require_i64(&mut self, ty: &ITy, span: Span) {
        let resolved = self.infer.resolve(ty);
        match &resolved {
            ITy::Int(crate::types::IntTy::I64) | ITy::Never => {}
            ITy::Var(_) => {
                let _ = self.unify_quiet(&resolved, &ITy::i64());
            }
            _ => self.report_mismatch(&ITy::i64(), &resolved, span),
        }
    }

    pub fn bump_loop_depth(&mut self, delta: i32) {
        if let Some(frame) = self.frames.last_mut() {
            if delta > 0 {
                frame.loop_depth += 1;
            } else {
                frame.loop_depth = frame.loop_depth.saturating_sub(1);
            }
        }
    }

    pub fn check_call_args(
        &mut self,
        expected: &[ITy],
        args: &[ast::Expr],
        span: Span,
    ) -> Vec<TExpr> {
        if expected.len() != args.len() {
            self.error_code(
                format!(
                    "this function takes {} argument(s) but {} were supplied",
                    expected.len(),
                    args.len()
                ),
                span,
                "E0061",
            );
        }
        let mut exprs = Vec::new();
        for (index, arg) in args.iter().enumerate() {
            let expected_ty = expected.get(index).cloned();
            let value = self.check_expr(arg, expected_ty.as_ref());
            if let Some(expected_ty) = &expected_ty
                && !self.unify_quiet(expected_ty, &value.ty)
            {
                let resolved_expected = self.infer.resolve(expected_ty);
                let resolved_value = self.infer.resolve(&value.ty);
                self.report_mismatch(&resolved_expected, &resolved_value, arg.span);
            }
            exprs.push(value);
        }
        exprs
    }

    pub fn is_place_expr(&self, expr: &TExpr) -> bool {
        matches!(
            expr.kind,
            TExprKind::Local { .. }
                | TExprKind::Capture { .. }
                | TExprKind::Field { .. }
                | TExprKind::Index { .. }
        )
    }

    pub fn is_mutable_place(&self, expr: &TExpr) -> bool {
        match &expr.kind {
            TExprKind::Local { mutable, .. } => *mutable,
            TExprKind::Capture { index, .. } => self
                .frames
                .last()
                .and_then(|frame| frame.captures.get(*index))
                .map(|capture| capture.mutable)
                .unwrap_or(false),
            TExprKind::Field { base, .. } => self.is_mutable_place(base),
            TExprKind::Index { base, .. } => self.is_mutable_place(base),
            _ => false,
        }
    }

    pub fn require_mutable_place(&mut self, expr: &TExpr, span: Span, action: &str) -> bool {
        if !self.is_place_expr(expr) {
            self.error_code(
                format!("cannot {}: the receiver is not a place expression", action),
                span,
                "E0596",
            );
            return false;
        }
        if !self.is_mutable_place(expr) {
            self.error_code(
                format!("cannot {}: the binding is not declared as mutable", action),
                span,
                "E0596",
            );
            return false;
        }
        true
    }

    pub fn error_suggest(
        &mut self,
        message: String,
        suggestion: Option<String>,
        span: Span,
        code: &'static str,
    ) {
        match suggestion {
            Some(suggestion) => self.error_code(
                format!("{}; did you mean {}?", message, suggestion),
                span,
                code,
            ),
            None => self.error_code(message, span, code),
        }
    }
}
