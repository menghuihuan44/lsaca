//! Constant declaration evaluation and constant folding.

use super::check::{Checker, Frame};
use super::items::{ConstState, ConstVal};
use super::itype::ITy;
use super::tree::{TBlock, TExpr, TExprKind, TFormatPartKind, TLit, TStmt};
use crate::hir::{HBinaryOp, HUnaryOp};
use crate::types::{FloatTy, IntTy, Ty};

/// Result of trying to fold an expression into a constant.
enum FoldOutcome {
    Value(ConstVal),
    NotConst,
}

/// Supported constant types: scalars and String.
fn const_type_supported(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Unit | Ty::Bool | Ty::Char | Ty::Int(_) | Ty::Float(_) | Ty::String
    )
}

fn folded(value: ConstVal) -> FoldOutcome {
    FoldOutcome::Value(value)
}

impl<'a> Checker<'a> {
    /// Evaluate a constant declaration on demand.
    pub fn ensure_const(&mut self, id: usize) {
        if id >= self.consts.len() {
            return;
        }
        match self.consts[id].state {
            ConstState::Unresolved => {}
            ConstState::InProgress => {
                let decl = self.consts[id].decl.clone();
                self.error_code(
                    format!("cycle detected when evaluating constant {}", decl.name),
                    decl.name_span,
                    "E0391",
                );
                self.consts[id].state = ConstState::Done {
                    ty: Ty::Unit,
                    value: ConstVal::Unit,
                };
                return;
            }
            ConstState::Done { .. } => return,
        }
        self.consts[id].state = ConstState::InProgress;
        let decl = self.consts[id].decl.clone();
        if decl.ty.span().is_empty() {
            // still check the initializer below with unit
        }
        let raw_ty = self.resolve_ty(&decl.ty, None);
        let check_ty = raw_ty.unwrap_or(ITy::Unit);
        let previous_in_const = self.in_const;
        self.in_const = true;
        let frame = Frame::new(check_ty.clone());
        self.frames.push(frame);
        let value_expr = self.check_expr(&decl.value, Some(&check_ty));
        self.frames.pop();
        self.in_const = previous_in_const;
        self.unify_or_report(&check_ty, &value_expr.ty, decl.value.span);
        let finalized = self.infer.finalize(&check_ty).unwrap_or(Ty::Unit);
        if !const_type_supported(&finalized) {
            let described = self.describe_resolved(&check_ty);
            self.error_code(
                format!(
                    "constants of type {} are not supported; constant types must be scalars or String",
                    described
                ),
                decl.ty.span(),
                "E0001",
            );
            self.consts[id].state = ConstState::Done {
                ty: Ty::Unit,
                value: ConstVal::Unit,
            };
            return;
        }
        let diagnostics_before = self.diagnostics.len();
        let outcome = self.fold_expr(&value_expr);
        let value = match outcome {
            FoldOutcome::Value(value) => {
                if let (Ty::Int(int_ty), ConstVal::Int(raw)) = (&finalized, &value) {
                    if !int_ty.fits(*raw) {
                        self.error_code(
                            format!("literal out of range for {}", int_ty.name()),
                            value_expr.span,
                            "E0080",
                        );
                        ConstVal::Unit
                    } else {
                        value
                    }
                } else {
                    value
                }
            }
            FoldOutcome::NotConst => {
                if self.diagnostics.len() == diagnostics_before {
                    self.error_code(
                        "constant initializer must be a constant expression".to_string(),
                        decl.value.span,
                        "E0015",
                    );
                }
                ConstVal::Unit
            }
        };
        self.consts[id].state = ConstState::Done {
            ty: finalized,
            value,
        };
    }

    fn fold_expr(&mut self, expr: &TExpr) -> FoldOutcome {
        match &expr.kind {
            TExprKind::Lit(lit) => match lit {
                TLit::Int(value) => folded(ConstVal::Int(*value)),
                TLit::Float(value) => folded(ConstVal::Float(*value)),
                TLit::Bool(value) => folded(ConstVal::Bool(*value)),
                TLit::Char(value) => folded(ConstVal::Char(*value)),
                TLit::Str(value) => folded(ConstVal::Str(value.clone())),
                TLit::Unit => folded(ConstVal::Unit),
            },
            TExprKind::ConstRef { const_id } => {
                self.ensure_const(*const_id);
                match self.const_value(*const_id) {
                    Some((_, value)) => folded(value),
                    None => FoldOutcome::NotConst,
                }
            }
            TExprKind::Unary { op, expr: inner } => self.fold_unary(*op, inner, expr),
            TExprKind::Binary { op, lhs, rhs } => self.fold_binary(*op, lhs, rhs, expr),
            TExprKind::Cast { expr: inner } => self.fold_cast(inner, expr),
            TExprKind::If {
                cond,
                then_block,
                else_expr,
            } => {
                let condition = match self.fold_expr(cond) {
                    FoldOutcome::Value(value) => value,
                    FoldOutcome::NotConst => return FoldOutcome::NotConst,
                };
                match condition {
                    ConstVal::Bool(true) => self.fold_block(then_block),
                    ConstVal::Bool(false) => match else_expr {
                        Some(else_expr) => self.fold_expr(else_expr),
                        None => folded(ConstVal::Unit),
                    },
                    _ => FoldOutcome::NotConst,
                }
            }
            TExprKind::Block(block) => self.fold_block(block),
            TExprKind::Format { parts } => {
                let mut text = String::new();
                for part in parts {
                    match &part.kind {
                        TFormatPartKind::Literal(literal) => text.push_str(literal),
                        TFormatPartKind::Arg { expr, spec } => {
                            // A width, precision, sign or alignment needs the
                            // runtime formatter, so such format strings are not
                            // folded to a constant.
                            if !spec.is_plain() {
                                return FoldOutcome::NotConst;
                            }
                            let value = match self.fold_expr(expr) {
                                FoldOutcome::Value(value) => value,
                                FoldOutcome::NotConst => return FoldOutcome::NotConst,
                            };
                            match format_const(&value, spec.debug) {
                                Some(rendered) => text.push_str(&rendered),
                                None => return FoldOutcome::NotConst,
                            }
                        }
                    }
                }
                folded(ConstVal::Str(text))
            }
            _ => FoldOutcome::NotConst,
        }
    }

    fn fold_block(&mut self, block: &TBlock) -> FoldOutcome {
        for stmt in &block.stmts {
            match stmt {
                TStmt::Expr(expr) => match self.fold_expr(expr) {
                    FoldOutcome::Value(_) => {}
                    FoldOutcome::NotConst => return FoldOutcome::NotConst,
                },
                TStmt::Let { .. } => return FoldOutcome::NotConst,
            }
        }
        match &block.tail {
            Some(tail) => self.fold_expr(tail),
            None => folded(ConstVal::Unit),
        }
    }

    fn fold_unary(&mut self, op: HUnaryOp, inner: &TExpr, expr: &TExpr) -> FoldOutcome {
        let value = match self.fold_expr(inner) {
            FoldOutcome::Value(value) => value,
            FoldOutcome::NotConst => return FoldOutcome::NotConst,
        };
        match op {
            HUnaryOp::Neg => match value {
                ConstVal::Int(raw) => {
                    let target = self.infer.resolve(&expr.ty);
                    match target {
                        ITy::Int(int_ty) => match raw.checked_neg() {
                            Some(negated) => self.finish_int(int_ty, negated, expr.span),
                            None => {
                                self.overflow_error(expr.span);
                                FoldOutcome::NotConst
                            }
                        },
                        _ => FoldOutcome::NotConst,
                    }
                }
                ConstVal::Float(raw) => folded(ConstVal::Float(-raw)),
                _ => FoldOutcome::NotConst,
            },
            HUnaryOp::Not => match value {
                ConstVal::Bool(raw) => folded(ConstVal::Bool(!raw)),
                ConstVal::Int(raw) => {
                    let target = self.infer.resolve(&expr.ty);
                    match target {
                        ITy::Int(int_ty) => {
                            let negated = wrap_int(!raw, int_ty);
                            self.finish_int(int_ty, negated, expr.span)
                        }
                        _ => FoldOutcome::NotConst,
                    }
                }
                _ => FoldOutcome::NotConst,
            },
        }
    }

    fn fold_binary(
        &mut self,
        op: HBinaryOp,
        lhs: &TExpr,
        rhs: &TExpr,
        expr: &TExpr,
    ) -> FoldOutcome {
        let left = match self.fold_expr(lhs) {
            FoldOutcome::Value(value) => value,
            FoldOutcome::NotConst => return FoldOutcome::NotConst,
        };
        let right = match self.fold_expr(rhs) {
            FoldOutcome::Value(value) => value,
            FoldOutcome::NotConst => return FoldOutcome::NotConst,
        };
        use HBinaryOp::*;
        match (&left, &right) {
            (ConstVal::Int(lhs_value), ConstVal::Int(rhs_value)) => {
                let target = self.infer.resolve(&lhs.ty);
                let int_ty = match target {
                    ITy::Int(int_ty) => int_ty,
                    _ => return FoldOutcome::NotConst,
                };
                match op {
                    Add | Sub | Mul => {
                        let result = match op {
                            Add => lhs_value.checked_add(*rhs_value),
                            Sub => lhs_value.checked_sub(*rhs_value),
                            _ => lhs_value.checked_mul(*rhs_value),
                        };
                        match result {
                            Some(result) => self.finish_int(int_ty, result, expr.span),
                            None => {
                                self.overflow_error(expr.span);
                                FoldOutcome::NotConst
                            }
                        }
                    }
                    Div | Rem => {
                        if *rhs_value == 0 {
                            self.division_error(expr.span);
                            return FoldOutcome::NotConst;
                        }
                        let result = if op == Div {
                            lhs_value.checked_div(*rhs_value)
                        } else {
                            lhs_value.checked_rem(*rhs_value)
                        };
                        match result {
                            Some(result) => self.finish_int(int_ty, result, expr.span),
                            None => {
                                self.overflow_error(expr.span);
                                FoldOutcome::NotConst
                            }
                        }
                    }
                    BitAnd => {
                        self.finish_int(int_ty, wrap_int(lhs_value & rhs_value, int_ty), expr.span)
                    }
                    BitOr => {
                        self.finish_int(int_ty, wrap_int(lhs_value | rhs_value, int_ty), expr.span)
                    }
                    BitXor => {
                        self.finish_int(int_ty, wrap_int(lhs_value ^ rhs_value, int_ty), expr.span)
                    }
                    Shl | Shr => {
                        if *rhs_value < 0 || *rhs_value >= int_ty.bits() as i128 {
                            self.error_code(
                                "constant evaluation failed: shift amount is out of range"
                                    .to_string(),
                                expr.span,
                                "E0080",
                            );
                            return FoldOutcome::NotConst;
                        }
                        let shift = *rhs_value as u32;
                        if op == Shl {
                            let raw = ((*lhs_value as u128) << shift) as i128;
                            self.finish_int(int_ty, wrap_int(raw, int_ty), expr.span)
                        } else if int_ty.is_signed() {
                            self.finish_int(int_ty, wrap_int(lhs_value >> shift, int_ty), expr.span)
                        } else {
                            let raw = ((*lhs_value as u128) >> shift) as i128;
                            self.finish_int(int_ty, wrap_int(raw, int_ty), expr.span)
                        }
                    }
                    Eq => folded(ConstVal::Bool(lhs_value == rhs_value)),
                    Ne => folded(ConstVal::Bool(lhs_value != rhs_value)),
                    Lt => folded(ConstVal::Bool(lhs_value < rhs_value)),
                    Le => folded(ConstVal::Bool(lhs_value <= rhs_value)),
                    Gt => folded(ConstVal::Bool(lhs_value > rhs_value)),
                    Ge => folded(ConstVal::Bool(lhs_value >= rhs_value)),
                }
            }
            (ConstVal::Float(lhs_value), ConstVal::Float(rhs_value)) => {
                let float_ty = match self.infer.resolve(&lhs.ty) {
                    ITy::Float(float_ty) => float_ty,
                    _ => return FoldOutcome::NotConst,
                };
                match op {
                    Add => folded(ConstVal::Float(round_float(
                        float_ty,
                        lhs_value + rhs_value,
                    ))),
                    Sub => folded(ConstVal::Float(round_float(
                        float_ty,
                        lhs_value - rhs_value,
                    ))),
                    Mul => folded(ConstVal::Float(round_float(
                        float_ty,
                        lhs_value * rhs_value,
                    ))),
                    Div => folded(ConstVal::Float(round_float(
                        float_ty,
                        lhs_value / rhs_value,
                    ))),
                    Rem => folded(ConstVal::Float(round_float(
                        float_ty,
                        lhs_value % rhs_value,
                    ))),
                    Eq => folded(ConstVal::Bool(lhs_value == rhs_value)),
                    Ne => folded(ConstVal::Bool(lhs_value != rhs_value)),
                    Lt => folded(ConstVal::Bool(lhs_value < rhs_value)),
                    Le => folded(ConstVal::Bool(lhs_value <= rhs_value)),
                    Gt => folded(ConstVal::Bool(lhs_value > rhs_value)),
                    Ge => folded(ConstVal::Bool(lhs_value >= rhs_value)),
                    _ => FoldOutcome::NotConst,
                }
            }
            (ConstVal::Bool(lhs_value), ConstVal::Bool(rhs_value)) => match op {
                Eq => folded(ConstVal::Bool(lhs_value == rhs_value)),
                Ne => folded(ConstVal::Bool(lhs_value != rhs_value)),
                _ => FoldOutcome::NotConst,
            },
            (ConstVal::Char(lhs_value), ConstVal::Char(rhs_value)) => match op {
                Eq => folded(ConstVal::Bool(lhs_value == rhs_value)),
                Ne => folded(ConstVal::Bool(lhs_value != rhs_value)),
                Lt => folded(ConstVal::Bool(lhs_value < rhs_value)),
                Le => folded(ConstVal::Bool(lhs_value <= rhs_value)),
                Gt => folded(ConstVal::Bool(lhs_value > rhs_value)),
                Ge => folded(ConstVal::Bool(lhs_value >= rhs_value)),
                _ => FoldOutcome::NotConst,
            },
            (ConstVal::Str(lhs_value), ConstVal::Str(rhs_value)) => match op {
                Add => folded(ConstVal::Str(format!("{}{}", lhs_value, rhs_value))),
                Eq => folded(ConstVal::Bool(lhs_value == rhs_value)),
                Ne => folded(ConstVal::Bool(lhs_value != rhs_value)),
                Lt => folded(ConstVal::Bool(lhs_value < rhs_value)),
                Le => folded(ConstVal::Bool(lhs_value <= rhs_value)),
                Gt => folded(ConstVal::Bool(lhs_value > rhs_value)),
                Ge => folded(ConstVal::Bool(lhs_value >= rhs_value)),
                _ => FoldOutcome::NotConst,
            },
            _ => FoldOutcome::NotConst,
        }
    }

    fn fold_cast(&mut self, inner: &TExpr, expr: &TExpr) -> FoldOutcome {
        let value = match self.fold_expr(inner) {
            FoldOutcome::Value(value) => value,
            FoldOutcome::NotConst => return FoldOutcome::NotConst,
        };
        let target = self.infer.resolve(&expr.ty);
        match (&value, &target) {
            (ConstVal::Int(raw), ITy::Int(int_ty)) => {
                self.finish_int(*int_ty, wrap_int(*raw, *int_ty), expr.span)
            }
            (ConstVal::Int(raw), ITy::Float(float_ty)) => {
                folded(ConstVal::Float(round_float(*float_ty, *raw as f64)))
            }
            (ConstVal::Float(raw), ITy::Float(float_ty)) => {
                folded(ConstVal::Float(round_float(*float_ty, *raw)))
            }
            (ConstVal::Float(raw), ITy::Int(int_ty)) => {
                let converted = saturating_float_to_int(*raw, *int_ty);
                self.finish_int(*int_ty, converted, expr.span)
            }
            (ConstVal::Bool(raw), ITy::Int(int_ty)) => {
                let raw = if *raw { 1 } else { 0 };
                self.finish_int(*int_ty, raw, expr.span)
            }
            (ConstVal::Char(raw), ITy::Int(int_ty)) => {
                self.finish_int(*int_ty, wrap_int(*raw as u32 as i128, *int_ty), expr.span)
            }
            (ConstVal::Char(raw), ITy::Float(float_ty)) => {
                folded(ConstVal::Float(round_float(*float_ty, *raw as u32 as f64)))
            }
            // Only u8 -> char casts pass the type checker. Constant folding
            // still runs over cast expressions that the checker already
            // rejected, so this arm can additionally report an invalid scalar
            // (e.g. "1114112 as char") as a secondary E0080 diagnostic; valid
            // u8 values always fit in 0..=255.
            (ConstVal::Int(raw), ITy::Char) => match char::from_u32(*raw as u32) {
                Some(character) if *raw >= 0 && *raw <= 0x10FFFF => {
                    folded(ConstVal::Char(character))
                }
                _ => {
                    self.error_code(
                        "constant evaluation failed: value is not a valid character".to_string(),
                        expr.span,
                        "E0080",
                    );
                    FoldOutcome::NotConst
                }
            },
            _ => FoldOutcome::NotConst,
        }
    }

    fn finish_int(&mut self, int_ty: IntTy, value: i128, span: crate::span::Span) -> FoldOutcome {
        if int_ty.fits(value) {
            folded(ConstVal::Int(value))
        } else {
            self.error_code(
                format!(
                    "constant evaluation failed: value is out of range for {}",
                    int_ty.name()
                ),
                span,
                "E0080",
            );
            FoldOutcome::NotConst
        }
    }

    fn overflow_error(&mut self, span: crate::span::Span) {
        self.error_code(
            "constant evaluation failed: arithmetic overflow".to_string(),
            span,
            "E0080",
        );
    }

    fn division_error(&mut self, span: crate::span::Span) {
        self.error_code(
            "constant evaluation failed: division by zero".to_string(),
            span,
            "E0080",
        );
    }
}

fn round_float(float_ty: FloatTy, value: f64) -> f64 {
    match float_ty {
        FloatTy::F32 => (value as f32) as f64,
        FloatTy::F64 => value,
    }
}

fn wrap_int(value: i128, int_ty: IntTy) -> i128 {
    let bits = int_ty.bits();
    if bits >= 128 {
        return value;
    }
    let mask: u128 = (1u128 << bits) - 1;
    let raw = (value as u128) & mask;
    if int_ty.is_signed() {
        let sign = 1u128 << (bits - 1);
        if raw & sign != 0 {
            (raw as i128) - (1i128 << bits)
        } else {
            raw as i128
        }
    } else {
        raw as i128
    }
}

fn saturating_float_to_int(value: f64, int_ty: IntTy) -> i128 {
    if value.is_nan() {
        return 0;
    }
    let (minimum, maximum) = int_ty.range();
    if value <= minimum as f64 {
        return minimum;
    }
    if value >= maximum as f64 {
        return maximum;
    }
    value.trunc() as i128
}

fn format_const(value: &ConstVal, debug: bool) -> Option<String> {
    match value {
        ConstVal::Int(raw) => Some(raw.to_string()),
        ConstVal::Float(raw) => Some(format!("{}", raw)),
        ConstVal::Bool(raw) => Some(if *raw { "true" } else { "false" }.to_string()),
        ConstVal::Char(raw) => {
            if debug {
                Some(format!("{:?}", raw))
            } else {
                Some(raw.to_string())
            }
        }
        ConstVal::Str(raw) => {
            if debug {
                Some(format!("{:?}", raw))
            } else {
                Some(raw.clone())
            }
        }
        ConstVal::Unit => Some("()".to_string()),
    }
}
