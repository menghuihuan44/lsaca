//! Operator, assignment and cast checking.

use crate::ast::{self, BinaryOp as AstBinaryOp, UnaryOp as AstUnaryOp};
use crate::hir::{HBinaryOp, HUnaryOp};
use crate::span::Span;
use crate::typeck::check::Checker;
use crate::typeck::itype::{ITy, VarKind};
use crate::typeck::tree::{TBlock, TExpr, TExprKind, TLit};

impl<'a> Checker<'a> {
    pub fn check_unary(
        &mut self,
        op: AstUnaryOp,
        inner: &ast::Expr,
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let value = self.check_expr(inner, hint);
        let ty = self.infer.resolve(&value.ty);
        let unary = |expr: TExpr| TExprKind::Unary {
            op: match op {
                AstUnaryOp::Neg => HUnaryOp::Neg,
                AstUnaryOp::Not => HUnaryOp::Not,
            },
            expr: Box::new(expr),
        };
        match op {
            AstUnaryOp::Neg => match &ty {
                ITy::Int(int_ty) => {
                    if !int_ty.is_signed() {
                        self.error_code(
                            format!("cannot apply unary - to unsigned type {}", int_ty.name()),
                            span,
                            "E0600",
                        );
                        return self.never_expr(span);
                    }
                    TExpr::new(ty.clone(), unary(value), span)
                }
                ITy::Float(_) => TExpr::new(ty.clone(), unary(value), span),
                ITy::Never => TExpr::new(ITy::never(), unary(value), span),
                ITy::Var(id) => match self.infer.var_kind(*id) {
                    VarKind::General => {
                        self.error_code(
                            "type annotations needed for the operand of unary -".to_string(),
                            span,
                            "E0282",
                        );
                        self.never_expr(span)
                    }
                    _ => TExpr::new(ty.clone(), unary(value), span),
                },
                _ => {
                    self.error_code(
                        format!(
                            "cannot apply unary - to type {}",
                            self.describe_resolved(&ty)
                        ),
                        span,
                        "E0600",
                    );
                    self.never_expr(span)
                }
            },
            AstUnaryOp::Not => match &ty {
                ITy::Bool => TExpr::new(ty.clone(), unary(value), span),
                ITy::Int(_) => TExpr::new(ty.clone(), unary(value), span),
                ITy::Never => TExpr::new(ITy::never(), unary(value), span),
                ITy::Var(id) => match self.infer.var_kind(*id) {
                    VarKind::General => {
                        self.error_code(
                            "type annotations needed for the operand of unary !".to_string(),
                            span,
                            "E0282",
                        );
                        self.never_expr(span)
                    }
                    _ => TExpr::new(ty.clone(), unary(value), span),
                },
                _ => {
                    self.error_code(
                        format!(
                            "cannot apply unary ! to type {}",
                            self.describe_resolved(&ty)
                        ),
                        span,
                        "E0600",
                    );
                    self.never_expr(span)
                }
            },
        }
    }

    pub fn check_binary(
        &mut self,
        op: AstBinaryOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        if matches!(op, AstBinaryOp::And | AstBinaryOp::Or) {
            let left = self.check_expr(lhs, Some(&ITy::bool_()));
            self.require_bool(&left.ty, lhs.span);
            let right = self.check_expr(rhs, Some(&ITy::bool_()));
            self.require_bool(&right.ty, rhs.span);
            let then_block = TBlock {
                stmts: Vec::new(),
                tail: Some(Box::new(right)),
                span: rhs.span,
            };
            let literal = !matches!(op, AstBinaryOp::And);
            let else_expr = TExpr::new(ITy::bool_(), TExprKind::Lit(TLit::Bool(literal)), span);
            let ty = ITy::bool_();
            self.apply_hint(&ty, hint);
            return TExpr::new(
                ty,
                TExprKind::If {
                    cond: Box::new(left),
                    then_block,
                    else_expr: Some(Box::new(else_expr)),
                },
                span,
            );
        }
        let left = self.check_expr(lhs, hint);
        let right = self.check_expr(rhs, Some(&left.ty));
        let (hop, result) = self.check_binary_op_semantics(op, &left.ty, &right.ty, span);
        self.apply_hint(&result, hint);
        TExpr::new(
            result,
            TExprKind::Binary {
                op: hop,
                lhs: Box::new(left),
                rhs: Box::new(right),
            },
            span,
        )
    }

    fn hop_of(op: AstBinaryOp) -> HBinaryOp {
        match op {
            AstBinaryOp::Add => HBinaryOp::Add,
            AstBinaryOp::Sub => HBinaryOp::Sub,
            AstBinaryOp::Mul => HBinaryOp::Mul,
            AstBinaryOp::Div => HBinaryOp::Div,
            AstBinaryOp::Rem => HBinaryOp::Rem,
            AstBinaryOp::Eq => HBinaryOp::Eq,
            AstBinaryOp::Ne => HBinaryOp::Ne,
            AstBinaryOp::Lt => HBinaryOp::Lt,
            AstBinaryOp::Le => HBinaryOp::Le,
            AstBinaryOp::Gt => HBinaryOp::Gt,
            AstBinaryOp::Ge => HBinaryOp::Ge,
            AstBinaryOp::BitAnd => HBinaryOp::BitAnd,
            AstBinaryOp::BitOr => HBinaryOp::BitOr,
            AstBinaryOp::BitXor => HBinaryOp::BitXor,
            AstBinaryOp::Shl => HBinaryOp::Shl,
            AstBinaryOp::Shr => HBinaryOp::Shr,
            AstBinaryOp::And => HBinaryOp::BitAnd,
            AstBinaryOp::Or => HBinaryOp::BitOr,
        }
    }

    fn is_int_literal_var(&self, ty: &ITy) -> bool {
        match ty {
            ITy::Var(id) => matches!(self.infer.var_kind(*id), VarKind::IntLiteral),
            _ => false,
        }
    }

    fn is_float_literal_var(&self, ty: &ITy) -> bool {
        match ty {
            ITy::Var(id) => matches!(self.infer.var_kind(*id), VarKind::FloatLiteral),
            _ => false,
        }
    }

    fn is_general_var(&self, ty: &ITy) -> bool {
        match ty {
            ITy::Var(id) => matches!(self.infer.var_kind(*id), VarKind::General),
            _ => false,
        }
    }

    fn e0282_operands(&mut self, op: AstBinaryOp, span: Span) {
        self.error_code(
            format!(
                "type annotations needed for the operands of {}",
                op.symbol()
            ),
            span,
            "E0282",
        );
    }

    pub fn check_binary_op_semantics(
        &mut self,
        op: AstBinaryOp,
        lhs_ty: &ITy,
        rhs_ty: &ITy,
        span: Span,
    ) -> (HBinaryOp, ITy) {
        let left = self.infer.resolve(lhs_ty);
        let right = self.infer.resolve(rhs_ty);
        let hop = Self::hop_of(op);
        let same = self.unify_quiet(&left, &right);
        let unified = self.infer.resolve(&left);
        if !same && !left.is_never() && !right.is_never() {
            self.report_mismatch(&left, &right, span);
            let fallback = match op {
                AstBinaryOp::Eq | AstBinaryOp::Ne => ITy::bool_(),
                AstBinaryOp::Lt | AstBinaryOp::Le | AstBinaryOp::Gt | AstBinaryOp::Ge => {
                    ITy::bool_()
                }
                _ => ITy::unit(),
            };
            return (hop, fallback);
        }
        match op {
            AstBinaryOp::Add => match &unified {
                ITy::String => (hop, ITy::string()),
                ITy::Int(_) | ITy::Float(_) => (hop, unified.clone()),
                ITy::Never => (hop, ITy::never()),
                _ if self.is_int_literal_var(&unified) => (hop, unified.clone()),
                _ if self.is_float_literal_var(&unified) => (hop, unified.clone()),
                _ => {
                    if self.is_general_var(&unified) {
                        self.e0282_operands(op, span);
                    } else {
                        self.report_operator_error(op, &left, &right, span);
                    }
                    (hop, ITy::unit())
                }
            },
            AstBinaryOp::Sub | AstBinaryOp::Mul | AstBinaryOp::Div | AstBinaryOp::Rem => {
                match &unified {
                    ITy::Int(_) | ITy::Float(_) => (hop, unified.clone()),
                    ITy::Never => (hop, ITy::never()),
                    _ if self.is_int_literal_var(&unified) => (hop, unified.clone()),
                    _ if self.is_float_literal_var(&unified) => (hop, unified.clone()),
                    _ => {
                        if self.is_general_var(&unified) {
                            self.e0282_operands(op, span);
                        } else {
                            self.report_operator_error(op, &left, &right, span);
                        }
                        (hop, ITy::unit())
                    }
                }
            }
            AstBinaryOp::Eq | AstBinaryOp::Ne => match &unified {
                // Rc<T> handles compare by identity: the generated code compares
                // the cell pointers, so comparing the handles of a cycle is safe.
                ITy::Bool | ITy::Int(_) | ITy::Float(_) | ITy::String | ITy::Rc(_) | ITy::Never => {
                    (hop, ITy::bool_())
                }
                _ if self.is_int_literal_var(&unified) => (hop, ITy::bool_()),
                _ if self.is_float_literal_var(&unified) => (hop, ITy::bool_()),
                _ => {
                    if self.is_general_var(&unified) {
                        self.e0282_operands(op, span);
                    } else {
                        self.report_operator_error(op, &left, &right, span);
                    }
                    (hop, ITy::bool_())
                }
            },
            AstBinaryOp::Lt | AstBinaryOp::Le | AstBinaryOp::Gt | AstBinaryOp::Ge => match &unified
            {
                ITy::Int(_) | ITy::Float(_) | ITy::String | ITy::Never => (hop, ITy::bool_()),
                _ if self.is_int_literal_var(&unified) => (hop, ITy::bool_()),
                _ if self.is_float_literal_var(&unified) => (hop, ITy::bool_()),
                _ => {
                    if self.is_general_var(&unified) {
                        self.e0282_operands(op, span);
                    } else {
                        self.report_operator_error(op, &left, &right, span);
                    }
                    (hop, ITy::bool_())
                }
            },
            AstBinaryOp::BitAnd
            | AstBinaryOp::BitOr
            | AstBinaryOp::BitXor
            | AstBinaryOp::Shl
            | AstBinaryOp::Shr => match &unified {
                ITy::Int(_) => (hop, unified.clone()),
                ITy::Never => (hop, ITy::never()),
                _ if self.is_int_literal_var(&unified) => (hop, unified.clone()),
                _ => {
                    if self.is_general_var(&unified) {
                        self.e0282_operands(op, span);
                    } else {
                        self.report_operator_error(op, &left, &right, span);
                    }
                    (hop, ITy::unit())
                }
            },
            AstBinaryOp::And | AstBinaryOp::Or => (hop, ITy::bool_()),
        }
    }

    pub fn report_operator_error(&mut self, op: AstBinaryOp, left: &ITy, right: &ITy, span: Span) {
        let message = if left == right {
            format!(
                "binary operation {} cannot be applied to type {}",
                op.symbol(),
                self.describe_resolved(left)
            )
        } else {
            format!(
                "binary operation {} cannot be applied to types {} and {}",
                op.symbol(),
                self.describe_resolved(left),
                self.describe_resolved(right)
            )
        };
        self.error_code(message, span, "E0369");
    }

    pub fn check_assign(
        &mut self,
        op: Option<AstBinaryOp>,
        target: &ast::Expr,
        value: &ast::Expr,
        span: Span,
    ) -> TExpr {
        let target_expr = self.check_expr(target, None);
        if !self.is_place_expr(&target_expr) {
            self.error_code(
                "cannot assign to this expression: the left-hand side is not a place expression"
                    .to_string(),
                target.span,
                "E0596",
            );
        } else if !self.is_mutable_place(&target_expr) {
            self.error_code(
                "cannot assign to immutable value: the binding has not been declared as mutable"
                    .to_string(),
                target.span,
                "E0596",
            );
        }
        let value_expr = self.check_expr(value, Some(&target_expr.ty));
        if !self.unify_quiet(&target_expr.ty, &value_expr.ty) {
            let expected = self.infer.resolve(&target_expr.ty);
            let found = self.infer.resolve(&value_expr.ty);
            self.report_mismatch(&expected, &found, value.span);
        }
        let hop = match op {
            Some(ast_op) => {
                if matches!(ast_op, AstBinaryOp::And | AstBinaryOp::Or) {
                    self.error_code(
                        format!(
                            "binary operation {} cannot be used as an assignment operator",
                            ast_op.symbol()
                        ),
                        span,
                        "E0369",
                    );
                    None
                } else {
                    let (hop, _) = self.check_binary_op_semantics(
                        ast_op,
                        &target_expr.ty,
                        &value_expr.ty,
                        span,
                    );
                    Some(hop)
                }
            }
            None => None,
        };
        TExpr::new(
            ITy::unit(),
            TExprKind::Assign {
                target: Box::new(target_expr),
                op: hop,
                value: Box::new(value_expr),
            },
            span,
        )
    }

    pub fn check_cast(
        &mut self,
        inner: &ast::Expr,
        ty: &ast::TypeExpr,
        span: Span,
        _hint: Option<&ITy>,
    ) -> TExpr {
        let target = match self.resolve_ty(ty, self.current_self_key()) {
            Some(target) => target,
            None => return self.never_expr(span),
        };
        let value = self.check_expr(inner, None);
        let from = self.infer.resolve(&value.ty);
        if !self.cast_allowed(&from, &target) {
            self.error_code(
                format!(
                    "non-primitive cast: {} as {}",
                    self.describe_resolved(&from),
                    self.describe(&target)
                ),
                span,
                "E0605",
            );
        }
        TExpr::new(
            target,
            TExprKind::Cast {
                expr: Box::new(value),
            },
            span,
        )
    }

    fn cast_allowed(&self, from: &ITy, target: &ITy) -> bool {
        if from.is_never() || target.is_never() {
            return true;
        }
        if self.is_general_var(from) || self.is_general_var(target) {
            return true;
        }
        if self.is_int_literal_var(from) || self.is_float_literal_var(from) {
            return true;
        }
        if self.is_int_literal_var(target) || self.is_float_literal_var(target) {
            return true;
        }
        match (from, target) {
            (ITy::Int(_), ITy::Int(_)) => true,
            (ITy::Int(_), ITy::Float(_)) => true,
            (ITy::Float(_), ITy::Int(_)) => true,
            (ITy::Float(_), ITy::Float(_)) => true,
            (ITy::Bool, ITy::Int(_)) => true,
            (ITy::Bool, ITy::Bool) => true,
            (left, right) => left == right,
        }
    }
}
