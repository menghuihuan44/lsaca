//! Expressions: places, operators, casts, calls and aggregate literals.

use crate::hir::{HBinaryOp, HExpr, HFieldRef, HUnaryOp};
use crate::span::Span;
use crate::types::{FloatTy, IntTy, SelfKind, Ty};

use super::emit::Gen;
use super::ops;
use super::types;
use super::util;

impl<'a> Gen<'a> {
    // ----- places ----------------------------------------------------------

    pub(crate) fn field_lvalue(&mut self, base: &HExpr, field: HFieldRef) -> String {
        let base_lv = if base.is_place() {
            self.gen_place(base)
        } else {
            self.gen_owned(base)
        };
        match field {
            HFieldRef::Tuple(index) => {
                format!("({}).{}", base_lv, types::positional_field(index))
            }
            HFieldRef::Struct { struct_id, index } => {
                let name = self
                    .program
                    .structs
                    .get(struct_id)
                    .and_then(|item| item.fields.get(index))
                    .map(|field| field.name.clone())
                    .unwrap_or_else(|| format!("field{}", index));
                format!("({}).{}", base_lv, types::field_cname(&name))
            }
        }
    }

    /// Lvalue of a vector element, with a runtime bounds check.
    pub(crate) fn index_lvalue(&mut self, base: &HExpr, index: &HExpr, span: Span) -> String {
        let base_lv = if base.is_place() {
            self.gen_place(base)
        } else {
            self.gen_owned(base)
        };
        let index_value = self.gen_owned(index);
        let loc = self.loc(span);
        self.w.line(format!(
            "if ((uint64_t)({}) >= (uint64_t)({}).len) {{",
            index_value, base_lv
        ));
        self.w.indent();
        self.w.line(format!(
            "ls_panic_bounds({}, (int64_t)({}).len, (int64_t)({}));",
            loc, base_lv, index_value
        ));
        self.w.dedent();
        self.w.line("}");
        format!("({}).data[(size_t)({})]", base_lv, index_value)
    }

    // ----- unary and binary operators --------------------------------------

    pub(crate) fn gen_unary(
        &mut self,
        op: HUnaryOp,
        inner: &HExpr,
        ty: &Ty,
        target: &str,
        span: Span,
    ) {
        let value = self.gen_owned(inner);
        let _ = ty;
        let expression = match op {
            HUnaryOp::Not => {
                if inner.ty.is_integer() {
                    let ctype = self.ctype(&inner.ty);
                    format!("({})(~({}))", ctype, value)
                } else {
                    format!("!({})", value)
                }
            }
            HUnaryOp::Neg => match &inner.ty {
                Ty::Int(int_ty) if int_ty.is_signed() => {
                    if self.debug_checks {
                        format!(
                            "ls_neg_{}_checked({}, {})",
                            int_ty.name(),
                            value,
                            self.loc(span)
                        )
                    } else {
                        unchecked_neg(&inner.ty, &value)
                    }
                }
                Ty::Int(_) => {
                    let ctype = self.ctype(&inner.ty);
                    format!("({})(0u - ({}))", ctype, value)
                }
                _ => format!("-({})", value),
            },
        };
        self.w.line(format!("{} = {};", target, expression));
    }

    pub(crate) fn gen_binary(
        &mut self,
        op: HBinaryOp,
        lhs: &HExpr,
        rhs: &HExpr,
        ty: &Ty,
        target: &str,
        span: Span,
    ) {
        let left = self.gen_owned(lhs);
        let right = self.gen_owned(rhs);
        let operand_ty = lhs.ty.clone();
        let result_ty = ty.clone();
        let expression = self.binop_value(op, &operand_ty, &result_ty, &left, &right, span);
        self.w.line(format!("{} = {};", target, expression));
    }

    fn binop_value(
        &mut self,
        op: HBinaryOp,
        operand_ty: &Ty,
        result_ty: &Ty,
        lhs: &str,
        rhs: &str,
        span: Span,
    ) -> String {
        let loc = self.loc(span);
        match op {
            HBinaryOp::Eq => ops::eq_expr(self.names, operand_ty, lhs, rhs),
            HBinaryOp::Ne => {
                let equal = ops::eq_expr(self.names, operand_ty, lhs, rhs);
                format!("!({})", equal)
            }
            HBinaryOp::Lt | HBinaryOp::Le | HBinaryOp::Gt | HBinaryOp::Ge => {
                let symbol = comparison_symbol(op);
                if operand_ty.is_string() {
                    format!("(ls_str_cmp({}, {}) {} 0)", lhs, rhs, symbol)
                } else {
                    format!("(({}) {} ({}))", lhs, symbol, rhs)
                }
            }
            HBinaryOp::Add | HBinaryOp::Sub | HBinaryOp::Mul | HBinaryOp::Div | HBinaryOp::Rem => {
                self.arithmetic_value(op, operand_ty, result_ty, lhs, rhs, &loc)
            }
            HBinaryOp::BitAnd | HBinaryOp::BitOr | HBinaryOp::BitXor => {
                let symbol = match op {
                    HBinaryOp::BitAnd => "&",
                    HBinaryOp::BitOr => "|",
                    _ => "^",
                };
                if operand_ty.is_integer() {
                    let ctype = self.ctype(operand_ty);
                    format!("({})(({}) {} ({}))", ctype, lhs, symbol, rhs)
                } else {
                    format!("(({}) {} ({}))", lhs, symbol, rhs)
                }
            }
            HBinaryOp::Shl | HBinaryOp::Shr => self.shift_value(op, operand_ty, lhs, rhs, &loc),
        }
    }

    fn arithmetic_value(
        &mut self,
        op: HBinaryOp,
        operand_ty: &Ty,
        _result_ty: &Ty,
        lhs: &str,
        rhs: &str,
        loc: &str,
    ) -> String {
        match operand_ty {
            Ty::String => match op {
                HBinaryOp::Add => format!("ls_str_concat({}, {})", lhs, rhs),
                _ => "ls_str_new()".to_string(),
            },
            Ty::Int(int_ty) => {
                let int_ty = *int_ty;
                match op {
                    HBinaryOp::Div => format!(
                        "ls_div_{}_checked({}, {}, {})",
                        int_ty.name(),
                        lhs,
                        rhs,
                        loc
                    ),
                    HBinaryOp::Rem => format!(
                        "ls_rem_{}_checked({}, {}, {})",
                        int_ty.name(),
                        lhs,
                        rhs,
                        loc
                    ),
                    _ => {
                        let op_name = arithmetic_name(op);
                        if self.debug_checks {
                            format!(
                                "ls_{}_{}_checked({}, {}, {})",
                                op_name,
                                int_ty.name(),
                                lhs,
                                rhs,
                                loc
                            )
                        } else {
                            unchecked_arithmetic(op, int_ty, lhs, rhs)
                        }
                    }
                }
            }
            Ty::Float(float_ty) => match op {
                HBinaryOp::Rem => {
                    let cast = if *float_ty == FloatTy::F64 {
                        "fmod"
                    } else {
                        "fmodf"
                    };
                    format!("({})({}(({}), ({})))", float_ty.c_type(), cast, lhs, rhs)
                }
                _ => {
                    let symbol = arithmetic_symbol(op);
                    format!("({})(({}) {} ({}))", float_ty.c_type(), lhs, symbol, rhs)
                }
            },
            Ty::Bool => {
                let symbol = arithmetic_symbol(op);
                format!("(({}) {} ({}))", lhs, symbol, rhs)
            }
            _ => self.zero_expr(operand_ty),
        }
    }

    fn shift_value(
        &mut self,
        op: HBinaryOp,
        operand_ty: &Ty,
        lhs: &str,
        rhs: &str,
        loc: &str,
    ) -> String {
        let int_ty = match operand_ty {
            Ty::Int(int_ty) => *int_ty,
            _ => IntTy::I64,
        };
        let is_left = op == HBinaryOp::Shl;
        if self.debug_checks {
            let op_name = if is_left { "shl" } else { "shr" };
            return format!(
                "ls_{}_{}_checked({}, (int64_t)({}), {})",
                op_name,
                int_ty.name(),
                lhs,
                rhs,
                loc
            );
        }
        let ctype = int_ty.c_type();
        let mask = int_ty.bits().saturating_sub(1);
        let shift = format!("((uint64_t)({}) & (uint64_t){})", rhs, mask);
        if is_left {
            let unsigned = int_ty.unsigned_counterpart().c_type();
            format!("({})(({})({}) << {})", ctype, unsigned, lhs, shift)
        } else {
            format!("({})(({}) >> {})", ctype, lhs, shift)
        }
    }

    // ----- assignment ------------------------------------------------------

    pub(crate) fn gen_assign(
        &mut self,
        place: &HExpr,
        op: Option<HBinaryOp>,
        value: &HExpr,
        ty: &Ty,
        target: &str,
        span: Span,
    ) {
        let lvalue = self.gen_place(place);
        let place_ty = place.ty.clone();
        match op {
            None => {
                let stored = self.gen_owned(value);
                self.emit_release(&place_ty, &lvalue);
                self.w.line(format!("{} = {};", lvalue, stored));
                self.mark_moved(&stored);
            }
            Some(binop) => {
                let old = self.declare_temp(&place_ty);
                self.emit_copy_into(&old, &place_ty, &lvalue);
                let right = self.gen_owned(value);
                let expression = self.binop_value(binop, &place_ty, &place_ty, &old, &right, span);
                let stored = self.declare_temp(&place_ty);
                self.w.line(format!("{} = {};", stored, expression));
                self.emit_release(&place_ty, &lvalue);
                self.w.line(format!("{} = {};", lvalue, stored));
                self.mark_moved(&stored);
            }
        }
        let unit = self.zero_expr(ty);
        self.w.line(format!("{} = {};", target, unit));
    }

    // ----- casts -----------------------------------------------------------

    pub(crate) fn gen_cast(&mut self, inner: &HExpr, ty: &Ty, target: &str, _span: Span) {
        let value = self.gen_owned(inner);
        let expression = match (&inner.ty, ty) {
            (Ty::Int(_), Ty::Int(int_ty)) => format!("({})({})", int_ty.c_type(), value),
            (Ty::Int(_), Ty::Float(float_ty)) => format!("({})({})", float_ty.c_type(), value),
            (Ty::Float(_), Ty::Int(int_ty)) => {
                format!("lsaca_cast_f2i_{}((double)({}))", int_ty.name(), value)
            }
            (Ty::Float(_), Ty::Float(float_ty)) => format!("({})({})", float_ty.c_type(), value),
            (Ty::Bool, Ty::Int(int_ty)) => format!("({})(({}) ? 1 : 0)", int_ty.c_type(), value),
            (Ty::Bool, Ty::Float(float_ty)) => {
                format!("({})(({}) ? 1 : 0)", float_ty.c_type(), value)
            }
            (Ty::Int(_), Ty::Bool) => format!("(({}) != 0)", value),
            (Ty::Float(_), Ty::Bool) => format!("(({}) != 0)", value),
            (_, Ty::Int(int_ty)) => format!("({})({})", int_ty.c_type(), value),
            (_, Ty::Float(float_ty)) => format!("({})({})", float_ty.c_type(), value),
            _ => format!("({})", value),
        };
        self.w.line(format!("{} = {};", target, expression));
    }

    // ----- calls -----------------------------------------------------------

    pub(crate) fn gen_call(&mut self, callee: &HExpr, args: &[HExpr], ty: &Ty, target: &str) {
        let function = self.gen_owned(callee);
        let mut argument_values = Vec::with_capacity(args.len());
        for argument in args {
            argument_values.push(self.gen_owned(argument));
        }
        let mut call = format!("({}).fn_ptr(({}).env", function, function);
        for argument in &argument_values {
            call.push_str(", ");
            call.push_str(argument);
        }
        call.push(')');
        if ty.is_unit() {
            self.w.line(format!("{};", call));
            let zero = self.zero_expr(ty);
            self.w.line(format!("{} = {};", target, zero));
        } else {
            self.w.line(format!("{} = {};", target, call));
        }
        for argument in &argument_values {
            self.mark_moved(argument);
        }
    }

    pub(crate) fn gen_direct_call(
        &mut self,
        fn_id: usize,
        receiver: Option<&HExpr>,
        args: &[HExpr],
        ty: &Ty,
        target: &str,
        _span: Span,
    ) {
        let item = match self.program.fns.get(fn_id) {
            Some(item) => item.clone(),
            None => {
                let zero = self.zero_expr(ty);
                self.w.line(format!("{} = {};", target, zero));
                return;
            }
        };
        let mut arguments: Vec<String> = Vec::new();
        let mut receiver_release: Option<(Ty, String)> = None;
        if let Some(receiver_expr) = receiver {
            match item.self_kind {
                Some(SelfKind::Ref) | Some(SelfKind::RefMut) => {
                    if receiver_expr.is_place() {
                        let place = self.gen_place(receiver_expr);
                        arguments.push(format!("&({})", place));
                    } else {
                        let temporary = self.gen_owned(receiver_expr);
                        arguments.push(format!("&{}", temporary));
                        receiver_release = Some((receiver_expr.ty.clone(), temporary));
                    }
                }
                _ => {
                    let value = self.gen_owned(receiver_expr);
                    arguments.push(value);
                }
            }
        }
        let mut argument_values: Vec<String> = Vec::new();
        for argument in args {
            let value = self.gen_owned(argument);
            arguments.push(value.clone());
            argument_values.push(value);
        }
        let call = format!("{}({})", item.c_name, arguments.join(", "));
        if ty.is_unit() {
            self.w.line(format!("{};", call));
            let zero = self.zero_expr(ty);
            self.w.line(format!("{} = {};", target, zero));
        } else {
            self.w.line(format!("{} = {};", target, call));
        }
        for value in &argument_values {
            self.mark_moved(value);
        }
        if let Some((receiver_ty, temporary)) = receiver_release {
            self.emit_release(&receiver_ty, &temporary);
            self.mark_moved(&temporary);
        }
    }

    // ----- aggregate literals ----------------------------------------------

    pub(crate) fn gen_struct_lit(
        &mut self,
        struct_id: usize,
        fields: &[(usize, HExpr)],
        _ty: &Ty,
        target: &str,
    ) {
        for (index, expression) in fields {
            let name = self
                .program
                .structs
                .get(struct_id)
                .and_then(|item| item.fields.get(*index))
                .map(|field| field.name.clone())
                .unwrap_or_else(|| format!("field{}", index));
            let value = self.gen_owned(expression);
            self.w.line(format!(
                "{}.{} = {};",
                target,
                types::field_cname(&name),
                value
            ));
            self.mark_moved(&value);
        }
    }

    pub(crate) fn gen_tuple(&mut self, elems: &[HExpr], _ty: &Ty, target: &str) {
        for (index, element) in elems.iter().enumerate() {
            let value = self.gen_owned(element);
            self.w.line(format!(
                "{}.{} = {};",
                target,
                types::positional_field(index),
                value
            ));
            self.mark_moved(&value);
        }
    }

    pub(crate) fn gen_vec_lit(&mut self, elems: &[HExpr], ty: &Ty, target: &str, _span: Span) {
        let element_ty = element_type(ty);
        let element_ctype = self.ctype(&element_ty);
        let count = elems.len();
        if count > 0 {
            self.w.line(format!(
                "{}.data = ({}*)ls_vec_alloc((size_t){}, sizeof({}));",
                target, element_ctype, count, element_ctype
            ));
            self.w.line(format!("{}.cap = (size_t){};", target, count));
        }
        for (index, element) in elems.iter().enumerate() {
            let value = self.gen_owned(element);
            self.w
                .line(format!("{}.data[{}] = {};", target, index, value));
            self.mark_moved(&value);
        }
        self.w.line(format!("{}.len = (size_t){};", target, count));
    }

    pub(crate) fn gen_vec_repeat(
        &mut self,
        value: &HExpr,
        count: &HExpr,
        ty: &Ty,
        target: &str,
        span: Span,
    ) {
        let element_ty = element_type(ty);
        let element_ctype = self.ctype(&element_ty);
        let count_value = self.gen_owned(count);
        let loc = self.loc(span);
        self.w
            .line(format!("if ((int64_t)({}) < 0) {{", count_value));
        self.w.indent();
        self.w.line(format!(
            "ls_panic_cstr({}, {});",
            loc,
            util::c_string_literal("capacity overflow")
        ));
        self.w.dedent();
        self.w.line("}");
        self.w.line(format!(
            "{}.data = ({}*)ls_vec_alloc((size_t)({}), sizeof({}));",
            target, element_ctype, count_value, element_ctype
        ));
        self.w
            .line(format!("{}.len = (size_t)({});", target, count_value));
        self.w
            .line(format!("{}.cap = (size_t)({});", target, count_value));
        let item = self.gen_owned(value);
        let index = self.fresh("i");
        self.w.line(format!(
            "for (size_t {} = 0; {} < (size_t)({}); {} += 1) {{",
            index, index, count_value, index
        ));
        self.w.indent();
        let copied = ops::copy_expr(self.names, &element_ty, &item);
        self.w
            .line(format!("{}.data[{}] = {};", target, index, copied));
        self.w.dedent();
        self.w.line("}");
    }

    pub(crate) fn gen_enum_lit(
        &mut self,
        enum_id: usize,
        variant_index: usize,
        args: &[HExpr],
        _ty: &Ty,
        target: &str,
    ) {
        let variant = match self
            .program
            .enums
            .get(enum_id)
            .and_then(|item| item.variants.get(variant_index))
        {
            Some(variant) => variant.clone(),
            None => return,
        };
        self.w.line(format!(
            "{}.{} = (int32_t){};",
            target,
            types::TAG_FIELD,
            variant.tag
        ));
        for (index, argument) in args.iter().enumerate() {
            let value = self.gen_owned(argument);
            self.w.line(format!(
                "{}.{}.{}.{} = {};",
                target,
                types::UNION_FIELD,
                types::variant_member(&variant.name),
                types::positional_field(index),
                value
            ));
            self.mark_moved(&value);
        }
    }

    pub(crate) fn gen_option_some(&mut self, inner: &HExpr, ty: &Ty, target: &str) {
        let value = self.gen_owned(inner);
        if ops::is_frozen_shared(ty) {
            self.w
                .line(format!("{} = ls_opt_string_some({});", target, value));
        } else {
            self.w
                .line(format!("{}.{} = (int32_t)1;", target, types::TAG_FIELD));
            self.w.line(format!(
                "{}.{}.{} = {};",
                target,
                types::UNION_FIELD,
                types::OPTION_SOME_MEMBER,
                value
            ));
        }
        self.mark_moved(&value);
    }

    pub(crate) fn gen_result_variant(&mut self, is_ok: bool, inner: &HExpr, ty: &Ty, target: &str) {
        let value = self.gen_owned(inner);
        if ops::is_frozen_shared(ty) {
            let constructor = if is_ok {
                "ls_res_string_ok"
            } else {
                "ls_res_string_err"
            };
            self.w
                .line(format!("{} = {}({});", target, constructor, value));
        } else {
            let tag = if is_ok { 0 } else { 1 };
            let member = if is_ok {
                types::RESULT_OK_MEMBER
            } else {
                types::RESULT_ERR_MEMBER
            };
            self.w.line(format!(
                "{}.{} = (int32_t){};",
                target,
                types::TAG_FIELD,
                tag
            ));
            self.w.line(format!(
                "{}.{}.{} = {};",
                target,
                types::UNION_FIELD,
                member,
                value
            ));
        }
        self.mark_moved(&value);
    }
}

fn comparison_symbol(op: HBinaryOp) -> &'static str {
    match op {
        HBinaryOp::Lt => "<",
        HBinaryOp::Le => "<=",
        HBinaryOp::Gt => ">",
        HBinaryOp::Ge => ">=",
        _ => "==",
    }
}

fn arithmetic_symbol(op: HBinaryOp) -> &'static str {
    match op {
        HBinaryOp::Add => "+",
        HBinaryOp::Sub => "-",
        HBinaryOp::Mul => "*",
        HBinaryOp::Div => "/",
        HBinaryOp::Rem => "%",
        _ => "+",
    }
}

fn arithmetic_name(op: HBinaryOp) -> &'static str {
    match op {
        HBinaryOp::Add => "add",
        HBinaryOp::Sub => "sub",
        HBinaryOp::Mul => "mul",
        HBinaryOp::Div => "div",
        HBinaryOp::Rem => "rem",
        _ => "add",
    }
}

fn unchecked_neg(ty: &Ty, value: &str) -> String {
    match ty {
        Ty::Int(int_ty) => {
            let ctype = int_ty.c_type();
            if int_ty.is_signed() {
                if int_ty.bits() < 64 {
                    format!("({})((int64_t)0 - (int64_t)({}))", ctype, value)
                } else {
                    format!("({})(0u - (uint64_t)({}))", ctype, value)
                }
            } else {
                format!("({})(0u - ({}))", ctype, value)
            }
        }
        _ => format!("-({})", value),
    }
}

fn unchecked_arithmetic(op: HBinaryOp, int_ty: IntTy, lhs: &str, rhs: &str) -> String {
    let ctype = int_ty.c_type();
    let symbol = arithmetic_symbol(op);
    if int_ty.is_signed() {
        let unsigned = int_ty.unsigned_counterpart().c_type();
        format!(
            "({})(({})({}) {} ({})({}))",
            ctype, unsigned, lhs, symbol, unsigned, rhs
        )
    } else {
        format!("({})(({}) {} ({}))", ctype, lhs, symbol, rhs)
    }
}

fn element_type(ty: &Ty) -> Ty {
    match ty {
        Ty::Vec(element) => (**element).clone(),
        _ => Ty::Unit,
    }
}
