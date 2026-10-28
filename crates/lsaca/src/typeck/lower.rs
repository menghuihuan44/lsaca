//! Lowering from the typed tree to the final HIR.
//!
//! Finalization happens here: inference variables are defaulted and resolved,
//! deferred literal range checks are validated, and every type in the output is
//! concrete.

use crate::hir::*;
use crate::span::Span;
use crate::types::{FloatTy, IntTy, Ty};

use super::check::Checker;
use super::items::ConstState;
use super::itype::ITy;
use super::tree::*;

impl<'a> Checker<'a> {
    pub fn build_hir(&mut self, name: String) -> Program {
        self.infer.default_literals();
        self.validate_literal_checks();
        let structs = self.lower_structs();
        let enums = self.lower_enums();
        let fns = self.lower_fns();
        let consts = self.lower_consts();
        let closures = self.lower_closures();
        let main_fn = self.fn_by_name.get("main").copied().unwrap_or(0);
        // "impl Drop for T" registration recorded one hook per user type. The
        // checker function ids and the lowered HIR function ids coincide
        // because lower_fns lowers every function in dense id order.
        let drop_structs = self.drop_structs.clone();
        let drop_enums = self.drop_enums.clone();
        let mut program = Program {
            name,
            structs,
            enums,
            fns,
            consts,
            closures,
            main_fn,
            copy_structs: Vec::new(),
            copy_enums: Vec::new(),
            drop_structs: Vec::new(),
            drop_enums: Vec::new(),
        };
        program.classify_types(drop_structs, drop_enums);
        program
    }

    fn validate_literal_checks(&mut self) {
        let int_checks = self.infer.int_checks().to_vec();
        for check in int_checks {
            let resolved = self.infer.resolve(&check.ty);
            if let ITy::Int(int_ty) = resolved
                && !int_ty.fits(check.value)
            {
                self.error_code(
                    format!("literal out of range for {}: {}", int_ty.name(), check.raw),
                    check.span,
                    "E0080",
                );
            }
        }
        let float_checks = self.infer.float_checks().to_vec();
        for check in float_checks {
            let resolved = self.infer.resolve(&check.ty);
            match resolved {
                ITy::Float(FloatTy::F32)
                    if (check.value as f32).is_infinite() && check.value.is_finite() =>
                {
                    self.error_code(
                        format!("literal out of range for f32: {}", check.raw),
                        check.span,
                        "E0080",
                    );
                }
                ITy::Float(FloatTy::F64) if !check.value.is_finite() => {
                    self.error_code(
                        format!("literal out of range for f64: {}", check.raw),
                        check.span,
                        "E0080",
                    );
                }
                _ => {}
            }
        }
    }

    fn finalize_ty(&mut self, ty: &ITy, span: Span) -> Ty {
        match self.infer.finalize(ty) {
            Ok(ty) => ty,
            Err(()) => {
                self.error_code(
                    "type annotations needed: the type of this expression is not known".to_string(),
                    span,
                    "E0282",
                );
                Ty::Unit
            }
        }
    }

    fn finalize_opt(&mut self, ty: Option<&ITy>, span: Span) -> Ty {
        match ty {
            Some(ty) => self.finalize_ty(ty, span),
            None => Ty::Unit,
        }
    }

    fn lower_structs(&mut self) -> Vec<HStruct> {
        let mut out = Vec::new();
        for id in 0..self.structs.len() {
            self.ensure_struct_fields(id);
            let info = self.structs[id].clone();
            let mut fields = Vec::new();
            for field in &info.fields {
                fields.push(HField {
                    name: field.name.clone(),
                    ty: self.finalize_ty(&field.ty, field.name_span),
                });
            }
            out.push(HStruct {
                name: info.name.clone(),
                fields,
                span: info.span,
            });
        }
        out
    }

    fn lower_enums(&mut self) -> Vec<HEnum> {
        let mut out = Vec::new();
        for id in 0..self.enums.len() {
            self.ensure_enum_variants(id);
            let info = self.enums[id].clone();
            let mut variants = Vec::new();
            for (tag, variant) in info.variants.iter().enumerate() {
                let mut fields = Vec::new();
                for field in &variant.fields {
                    fields.push(self.finalize_ty(field, variant.span));
                }
                variants.push(HVariant {
                    name: variant.name.clone(),
                    tag,
                    fields,
                });
            }
            out.push(HEnum {
                name: info.name.clone(),
                variants,
                span: info.span,
            });
        }
        out
    }

    fn lower_fns(&mut self) -> Vec<HFn> {
        let mut out = Vec::new();
        for id in 0..self.fns.len() {
            let info = self.fns[id].clone();
            let mut params = Vec::new();
            for (index, param) in info.decl.params.iter().enumerate() {
                let c_name = info
                    .param_cnames
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| format!("p{}", index));
                let ty = match &info.sig {
                    Some(sig) => self.finalize_opt(sig.params.get(index), param.name_span),
                    None => Ty::Unit,
                };
                params.push(HParam {
                    name: param.name.clone(),
                    c_name,
                    ty,
                });
            }
            let ret = match &info.sig {
                Some(sig) => self.finalize_ty(&sig.ret, info.name_span),
                None => Ty::Unit,
            };
            let body = match &info.checked {
                Some(block) => self.lower_block(block),
                None => HBlock {
                    stmts: Vec::new(),
                    tail: None,
                    span: info.span,
                },
            };
            out.push(HFn {
                name: info.display.clone(),
                c_name: info.c_name.clone(),
                params,
                ret,
                self_kind: info.self_kind,
                body,
                span: info.span,
            });
        }
        out
    }

    fn lower_consts(&mut self) -> Vec<HConst> {
        let mut out = Vec::new();
        for id in 0..self.consts.len() {
            self.ensure_const(id);
            let item = self.consts[id].clone();
            let (ty, value) = match &item.state {
                ConstState::Done { ty, value } => (ty.clone(), Self::lower_const_value(value)),
                _ => (Ty::Unit, ConstValue::Unit),
            };
            out.push(HConst {
                name: item.decl.name.clone(),
                c_name: item.c_name.clone(),
                ty,
                value,
                span: item.decl.span,
            });
        }
        out
    }

    fn lower_const_value(value: &super::items::ConstVal) -> ConstValue {
        use super::items::ConstVal;
        match value {
            ConstVal::Int(raw) => ConstValue::Int(*raw),
            ConstVal::Float(raw) => ConstValue::Float(*raw),
            ConstVal::Bool(raw) => ConstValue::Bool(*raw),
            ConstVal::Str(raw) => ConstValue::Str(raw.clone()),
            ConstVal::Unit => ConstValue::Unit,
        }
    }

    fn lower_closures(&mut self) -> Vec<HClosure> {
        let closures = self.closures.clone();
        let mut out = Vec::new();
        for entry in closures {
            let closure = match entry {
                Some(closure) => closure,
                None => continue,
            };
            let mut params = Vec::new();
            for param in &closure.params {
                params.push(HParam {
                    name: param.name.clone(),
                    c_name: param.c_name.clone(),
                    ty: self.finalize_ty(&param.ty, closure.span),
                });
            }
            let ret = self.finalize_ty(&closure.ret, closure.span);
            let mut captures = Vec::new();
            for capture in &closure.captures {
                captures.push(HCapture {
                    name: capture.name.clone(),
                    ty: self.finalize_ty(&capture.ty, closure.span),
                });
            }
            let body = self.lower_block(&closure.body);
            out.push(HClosure {
                id: closure.id,
                name: format!("closure#{}", closure.id),
                c_name: closure.c_name,
                params,
                ret,
                captures,
                body,
                span: closure.span,
            });
        }
        out
    }

    pub fn lower_block(&mut self, block: &TBlock) -> HBlock {
        let mut stmts = Vec::new();
        for stmt in &block.stmts {
            match stmt {
                TStmt::Let { pat, init, span } => {
                    let pat = self.lower_pat(pat);
                    let init = init.as_ref().map(|init| self.lower_expr(init));
                    stmts.push(HStmt::Let {
                        pat,
                        init,
                        span: *span,
                    });
                }
                TStmt::Expr(expr) => stmts.push(HStmt::Expr(self.lower_expr(expr))),
            }
        }
        let tail = block
            .tail
            .as_ref()
            .map(|tail| Box::new(self.lower_expr(tail)));
        HBlock {
            stmts,
            tail,
            span: block.span,
        }
    }

    pub fn lower_expr(&mut self, expr: &TExpr) -> HExpr {
        let ty = self.finalize_ty(&expr.ty, expr.span);
        let span = expr.span;
        let kind = match &expr.kind {
            TExprKind::Lit(lit) => HExprKind::Lit(Self::lower_lit(lit, &ty)),
            TExprKind::Local { name, c_name, .. } => HExprKind::Local {
                name: name.clone(),
                c_name: c_name.clone(),
            },
            TExprKind::Capture { index, name } => HExprKind::Capture {
                index: *index,
                name: name.clone(),
            },
            TExprKind::ConstRef { const_id } => HExprKind::ConstRef {
                c_name: self.const_c_name(*const_id),
            },
            TExprKind::FnRef { fn_id } => HExprKind::FnRef { fn_id: *fn_id },
            TExprKind::Closure { closure_id } => {
                let parent_exprs: Vec<TExpr> = match self.closures.get(*closure_id) {
                    Some(Some(closure)) => closure
                        .captures
                        .iter()
                        .map(|capture| capture.parent_expr.clone())
                        .collect(),
                    _ => Vec::new(),
                };
                let captures = parent_exprs
                    .iter()
                    .map(|parent| self.lower_expr(parent))
                    .collect();
                HExprKind::Closure {
                    closure_id: *closure_id,
                    captures,
                }
            }
            TExprKind::Call { callee, args } => HExprKind::Call {
                callee: Box::new(self.lower_expr(callee)),
                args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
            },
            TExprKind::DirectCall {
                fn_id,
                receiver,
                args,
            } => HExprKind::DirectCall {
                fn_id: *fn_id,
                receiver: receiver
                    .as_ref()
                    .map(|receiver| Box::new(self.lower_expr(receiver))),
                args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
            },
            TExprKind::BuiltinMethod {
                receiver,
                method,
                args,
            } => HExprKind::BuiltinMethod {
                receiver: Box::new(self.lower_expr(receiver)),
                method: *method,
                args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
            },
            TExprKind::BuiltinAssoc {
                func,
                args,
                type_args,
            } => HExprKind::BuiltinAssoc {
                func: *func,
                args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                type_args: type_args
                    .iter()
                    .map(|type_arg| self.finalize_ty(type_arg, span))
                    .collect(),
            },
            TExprKind::BuiltinCall { func, args } => HExprKind::BuiltinCall {
                func: *func,
                args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
            },
            TExprKind::Field { base, field } => HExprKind::Field {
                base: Box::new(self.lower_expr(base)),
                field: *field,
            },
            TExprKind::Index { base, index } => HExprKind::Index {
                base: Box::new(self.lower_expr(base)),
                index: Box::new(self.lower_expr(index)),
            },
            TExprKind::Unary { op, expr } => HExprKind::Unary {
                op: *op,
                expr: Box::new(self.lower_expr(expr)),
            },
            TExprKind::Binary { op, lhs, rhs } => HExprKind::Binary {
                op: *op,
                lhs: Box::new(self.lower_expr(lhs)),
                rhs: Box::new(self.lower_expr(rhs)),
            },
            TExprKind::Assign { target, op, value } => HExprKind::Assign {
                target: Box::new(self.lower_expr(target)),
                op: *op,
                value: Box::new(self.lower_expr(value)),
            },
            TExprKind::Cast { expr } => HExprKind::Cast {
                expr: Box::new(self.lower_expr(expr)),
            },
            TExprKind::Block(block) => HExprKind::Block(self.lower_block(block)),
            TExprKind::If {
                cond,
                then_block,
                else_expr,
            } => HExprKind::If {
                cond: Box::new(self.lower_expr(cond)),
                then_block: self.lower_block(then_block),
                else_expr: else_expr
                    .as_ref()
                    .map(|else_expr| Box::new(self.lower_expr(else_expr))),
            },
            TExprKind::Match { scrutinee, arms } => HExprKind::Match {
                scrutinee: Box::new(self.lower_expr(scrutinee)),
                arms: arms
                    .iter()
                    .map(|arm| HArm {
                        pat: self.lower_pat(&arm.pat),
                        guard: arm.guard.as_ref().map(|guard| self.lower_expr(guard)),
                        body: self.lower_expr(&arm.body),
                    })
                    .collect(),
            },
            TExprKind::Loop { body } => HExprKind::Loop {
                body: self.lower_block(body),
            },
            TExprKind::While { cond, body } => HExprKind::While {
                cond: Box::new(self.lower_expr(cond)),
                body: self.lower_block(body),
            },
            TExprKind::ForRange {
                pat,
                start,
                end,
                inclusive,
                body,
            } => HExprKind::ForRange {
                pat: self.lower_pat(pat),
                start: Box::new(self.lower_expr(start)),
                end: Box::new(self.lower_expr(end)),
                inclusive: *inclusive,
                body: self.lower_block(body),
            },
            TExprKind::ForVec { pat, vec, body } => HExprKind::ForVec {
                pat: self.lower_pat(pat),
                vec: Box::new(self.lower_expr(vec)),
                body: self.lower_block(body),
            },
            TExprKind::StructLit { struct_id, fields } => HExprKind::StructLit {
                struct_id: *struct_id,
                fields: fields
                    .iter()
                    .map(|(index, value)| (*index, self.lower_expr(value)))
                    .collect(),
            },
            TExprKind::Tuple(elems) => {
                HExprKind::Tuple(elems.iter().map(|elem| self.lower_expr(elem)).collect())
            }
            TExprKind::VecLit { elems } => HExprKind::VecLit {
                elems: elems.iter().map(|elem| self.lower_expr(elem)).collect(),
            },
            TExprKind::VecRepeat { value, count } => HExprKind::VecRepeat {
                value: Box::new(self.lower_expr(value)),
                count: Box::new(self.lower_expr(count)),
            },
            TExprKind::EnumLit {
                enum_id,
                variant,
                args,
            } => HExprKind::EnumLit {
                enum_id: *enum_id,
                variant: *variant,
                args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
            },
            TExprKind::OptionSome(inner) => HExprKind::OptionSome(Box::new(self.lower_expr(inner))),
            TExprKind::OptionNone => HExprKind::OptionNone,
            TExprKind::ResultOk(inner) => HExprKind::ResultOk(Box::new(self.lower_expr(inner))),
            TExprKind::ResultErr(inner) => HExprKind::ResultErr(Box::new(self.lower_expr(inner))),
            TExprKind::Format { parts } => HExprKind::Format {
                parts: self.lower_format_parts(parts),
            },
            TExprKind::Macro(call) => HExprKind::Macro(HMacroCall {
                kind: call.kind,
                parts: self.lower_format_parts(&call.parts),
                args: call.args.iter().map(|arg| self.lower_expr(arg)).collect(),
            }),
            TExprKind::Ref { mutable, expr } => HExprKind::Ref {
                mutable: *mutable,
                expr: Box::new(self.lower_expr(expr)),
            },
            TExprKind::Deref { expr } => HExprKind::Deref {
                expr: Box::new(self.lower_expr(expr)),
            },
            TExprKind::Try {
                expr,
                ret,
                value_c_name,
                error_c_name,
            } => {
                let operand = self.lower_expr(expr);
                self.lower_try(operand, ret, value_c_name, error_c_name, span)
            }
            TExprKind::Break => HExprKind::Break,
            TExprKind::Continue => HExprKind::Continue,
            TExprKind::Return(value) => {
                HExprKind::Return(value.as_ref().map(|value| Box::new(self.lower_expr(value))))
            }
        };
        HExpr::new(ty, kind, span)
    }

    fn lower_format_parts(&mut self, parts: &[TFormatPart]) -> Vec<HFormatPart> {
        parts
            .iter()
            .map(|part| match &part.kind {
                TFormatPartKind::Literal(text) => HFormatPart {
                    kind: HFormatPartKind::Literal(text.clone()),
                },
                TFormatPartKind::Arg { expr, spec } => HFormatPart {
                    kind: HFormatPartKind::Arg {
                        expr: self.lower_expr(expr),
                        spec: Box::new(spec.clone()),
                    },
                },
            })
            .collect()
    }

    fn lower_lit(lit: &TLit, ty: &Ty) -> HLit {
        match lit {
            TLit::Int(value) => {
                let int_ty = match ty {
                    Ty::Int(int_ty) => *int_ty,
                    _ => IntTy::I64,
                };
                HLit::Int {
                    value: *value,
                    int_ty,
                }
            }
            TLit::Float(value) => {
                let float_ty = match ty {
                    Ty::Float(float_ty) => *float_ty,
                    _ => FloatTy::F64,
                };
                HLit::Float {
                    value: *value,
                    float_ty,
                }
            }
            TLit::Bool(value) => HLit::Bool(*value),
            TLit::Str(value) => HLit::Str(value.clone()),
            TLit::Unit => HLit::Unit,
        }
    }

    fn lower_pat_lit(&mut self, lit: &TLit, ty: &Ty) -> HLit {
        Self::lower_lit(lit, ty)
    }

    pub fn lower_pat(&mut self, pat: &TPat) -> HPat {
        let ty = self.finalize_ty(&pat.ty, pat.span);
        let kind = match &pat.kind {
            TPatKind::Wild => HPatKind::Wild,
            TPatKind::Bind {
                name,
                c_name,
                mutable: _,
            } => HPatKind::Bind {
                c_name: c_name.clone(),
                name: name.clone(),
            },
            TPatKind::Lit(lit) => HPatKind::Lit(self.lower_pat_lit(lit, &ty)),
            TPatKind::Tuple(elems) => {
                HPatKind::Tuple(elems.iter().map(|elem| self.lower_pat(elem)).collect())
            }
            TPatKind::Struct {
                struct_id,
                fields,
                has_rest,
            } => HPatKind::Struct {
                struct_id: *struct_id,
                fields: fields
                    .iter()
                    .map(|(index, pat)| (*index, self.lower_pat(pat)))
                    .collect(),
                has_rest: *has_rest,
            },
            TPatKind::Enum {
                enum_id,
                variant,
                args,
            } => HPatKind::Enum {
                enum_id: *enum_id,
                variant: *variant,
                args: args.iter().map(|arg| self.lower_pat(arg)).collect(),
            },
            TPatKind::OptionSome(inner) => HPatKind::OptionSome(Box::new(self.lower_pat(inner))),
            TPatKind::OptionNone => HPatKind::OptionNone,
            TPatKind::ResultOk(inner) => HPatKind::ResultOk(Box::new(self.lower_pat(inner))),
            TPatKind::ResultErr(inner) => HPatKind::ResultErr(Box::new(self.lower_pat(inner))),
            TPatKind::Or(alternatives) => HPatKind::Or(
                alternatives
                    .iter()
                    .map(|alternative| self.lower_pat(alternative))
                    .collect(),
            ),
        };
        HPat {
            ty,
            kind,
            span: pat.span,
        }
    }

    /// Lower a try expression into a match that returns the payload in the
    /// success arm and propagates None or Err with an early return in the
    /// failure arm. The operand expression is evaluated exactly once.
    fn lower_try(
        &mut self,
        operand: HExpr,
        ret: &ITy,
        value_c_name: &str,
        error_c_name: &str,
        span: Span,
    ) -> HExprKind {
        let ret_ty = self.finalize_ty(ret, span);
        let operand_ty = operand.ty.clone();
        match &operand_ty {
            Ty::Option(payload) => {
                let payload = (**payload).clone();
                let some_pat = HPat {
                    ty: operand_ty.clone(),
                    kind: HPatKind::OptionSome(Box::new(HPat {
                        ty: payload.clone(),
                        kind: HPatKind::Bind {
                            c_name: value_c_name.to_string(),
                            name: "value".to_string(),
                        },
                        span,
                    })),
                    span,
                };
                let none_pat = HPat {
                    ty: operand_ty.clone(),
                    kind: HPatKind::OptionNone,
                    span,
                };
                let value_expr = HExpr::new(
                    payload.clone(),
                    HExprKind::Local {
                        name: "value".to_string(),
                        c_name: value_c_name.to_string(),
                    },
                    span,
                );
                let none_return = HExpr::new(
                    Ty::Unit,
                    HExprKind::Return(Some(Box::new(HExpr::new(
                        ret_ty.clone(),
                        HExprKind::OptionNone,
                        span,
                    )))),
                    span,
                );
                HExprKind::Match {
                    scrutinee: Box::new(operand),
                    arms: vec![
                        HArm {
                            pat: some_pat,
                            guard: None,
                            body: value_expr,
                        },
                        HArm {
                            pat: none_pat,
                            guard: None,
                            body: none_return,
                        },
                    ],
                }
            }
            Ty::Result(ok_ty, err_ty) => {
                let ok_ty = (**ok_ty).clone();
                let err_ty = (**err_ty).clone();
                let ok_pat = HPat {
                    ty: operand_ty.clone(),
                    kind: HPatKind::ResultOk(Box::new(HPat {
                        ty: ok_ty.clone(),
                        kind: HPatKind::Bind {
                            c_name: value_c_name.to_string(),
                            name: "value".to_string(),
                        },
                        span,
                    })),
                    span,
                };
                let err_pat = HPat {
                    ty: operand_ty.clone(),
                    kind: HPatKind::ResultErr(Box::new(HPat {
                        ty: err_ty.clone(),
                        kind: HPatKind::Bind {
                            c_name: error_c_name.to_string(),
                            name: "error".to_string(),
                        },
                        span,
                    })),
                    span,
                };
                let ok_expr = HExpr::new(
                    ok_ty.clone(),
                    HExprKind::Local {
                        name: "value".to_string(),
                        c_name: value_c_name.to_string(),
                    },
                    span,
                );
                let err_local = HExpr::new(
                    err_ty.clone(),
                    HExprKind::Local {
                        name: "error".to_string(),
                        c_name: error_c_name.to_string(),
                    },
                    span,
                );
                let err_return = HExpr::new(
                    Ty::Unit,
                    HExprKind::Return(Some(Box::new(HExpr::new(
                        ret_ty.clone(),
                        HExprKind::ResultErr(Box::new(err_local)),
                        span,
                    )))),
                    span,
                );
                HExprKind::Match {
                    scrutinee: Box::new(operand),
                    arms: vec![
                        HArm {
                            pat: ok_pat,
                            guard: None,
                            body: ok_expr,
                        },
                        HArm {
                            pat: err_pat,
                            guard: None,
                            body: err_return,
                        },
                    ],
                }
            }
            _ => {
                // The checker rejects every other operand type, so this arm is
                // unreachable for accepted programs. Keep lowering total.
                HExprKind::Block(HBlock {
                    stmts: Vec::new(),
                    tail: Some(Box::new(operand)),
                    span,
                })
            }
        }
    }

    fn const_c_name(&self, const_id: usize) -> String {
        self.consts
            .get(const_id)
            .map(|item| item.c_name.clone())
            .unwrap_or_else(|| format!("lsaca_const_{}", const_id))
    }
}
