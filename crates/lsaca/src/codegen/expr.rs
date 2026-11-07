//! Expressions: places, operators, casts, calls and aggregate literals.

use crate::hir::{HBinaryOp, HExpr, HExprKind, HFieldRef, HLit, HUnaryOp};
use crate::span::Span;
use crate::types::{FloatTy, IntTy, SelfKind, Ty};

use super::emit::Gen;
use super::ops;
use super::types;
use super::util;

impl<'a> Gen<'a> {
    // ----- places ----------------------------------------------------------

    pub(crate) fn field_lvalue(&mut self, base: &HExpr, field: HFieldRef) -> String {
        let base_lv = self.auto_deref_base(base);
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

    /// Lvalue of an element, with a runtime bounds check.
    ///
    /// Vectors, slices, strings and arrays all store their elements behind the
    /// same data member. A fixed array carries its length in the type, so the
    /// check compares against that constant instead of a stored length.
    pub(crate) fn index_lvalue(&mut self, base: &HExpr, index: &HExpr, span: Span) -> String {
        let (base_lv, base_ty) = self.auto_deref_place(base);
        let index_value = self.gen_owned(index);
        let loc = self.loc(span);
        let length = match &base_ty {
            Ty::Array(_, len) => format!("{}", len),
            _ => format!("({}).len", base_lv),
        };
        self.w.line(format!(
            "if ((uint64_t)({}) >= (uint64_t)({})) {{",
            index_value, length
        ));
        self.w.indent();
        self.w.line(format!(
            "ls_panic_bounds({}, (int64_t)({}), (int64_t)({}));",
            loc, length, index_value
        ));
        self.w.dedent();
        self.w.line("}");
        format!("({}).data[(size_t)({})]", base_lv, index_value)
    }

    /// Lvalue of a place base with auto-deref applied: references are
    /// dereferenced and `Rc`/`Box` handles are unwrapped to their payload
    /// cell. Any other value is returned unchanged.
    pub(crate) fn auto_deref_base(&mut self, base: &HExpr) -> String {
        self.auto_deref_place(base).0
    }

    /// Like auto_deref_base, but also returns the place type after auto-deref,
    /// for callers that need to know whether they end up at a vector, a slice
    /// or a fixed array.
    pub(crate) fn auto_deref_place(&mut self, base: &HExpr) -> (String, Ty) {
        let mut lvalue = if base.is_place() {
            self.gen_place(base)
        } else {
            self.gen_owned(base)
        };
        let mut ty = base.ty.clone();
        for _ in 0..8 {
            match &ty {
                Ty::Ref { inner, .. } => {
                    if inner.is_unsized() {
                        // A reference to an unsized value is the fat value
                        // itself: the data and length live in the descriptor.
                        break;
                    }
                    lvalue = format!("(*({}))", lvalue);
                    ty = (**inner).clone();
                }
                Ty::Rc(inner) | Ty::Box(inner) => {
                    lvalue = format!("(({})->payload)", lvalue);
                    ty = (**inner).clone();
                }
                _ => break,
            }
        }
        (lvalue, ty)
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
                    let int_ty = *int_ty;
                    if self.debug_checks {
                        let call = format!(
                            "ls_neg_{}_checked({}, {})",
                            util::runtime_int_suffix(int_ty),
                            util::runtime_int_argument(int_ty, &value),
                            self.loc(span)
                        );
                        util::runtime_int_result(int_ty, call)
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
        // Operands of every binary operator are borrowed: comparisons read
        // through references and the string operators never take ownership.
        let left = self.gen_borrowed(lhs);
        let operand_ty = lhs.ty.clone();
        // `String + &str` appends borrowed text: the right hand side is a
        // fat `LsStr` value, while the runtime concatenation helper takes a
        // `const LsString*`. A read only stack descriptor bridges the two
        // shapes without copying or allocating. An owned `String` operand is
        // already the right shape and is borrowed directly.
        let right = if op == HBinaryOp::Add && operand_ty.is_string() && !rhs.ty.is_string() {
            self.string_text_argument(rhs, "String addition")
        } else {
            self.gen_borrowed(rhs)
        };
        let result_ty = ty.clone();
        // Release builds keep division and remainder checked because the runtime
        // reports division by zero, but a remainder by a positive constant can
        // neither divide by zero nor overflow, so it lowers to the plain C
        // operator. This keeps hot filters such as `x % 2` free of a runtime
        // call while debug builds still report overflow.
        let expression = match (op, &rhs.kind) {
            (HBinaryOp::Rem, HExprKind::Lit(HLit::Int { value, .. }))
                if !self.debug_checks && *value > 0 =>
            {
                let ctype = self.ctype(&result_ty);
                format!("({})(({}) % ({}))", ctype, left, right)
            }
            _ => self.binop_value(op, &operand_ty, &result_ty, &left, &right, span),
        };
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
                let suffix = util::runtime_int_suffix(int_ty);
                let left = util::runtime_int_argument(int_ty, lhs);
                let right = util::runtime_int_argument(int_ty, rhs);
                match op {
                    HBinaryOp::Div | HBinaryOp::Rem => {
                        let op_name = if op == HBinaryOp::Div { "div" } else { "rem" };
                        let call = format!(
                            "ls_{}_{}_checked({}, {}, {})",
                            op_name, suffix, left, right, loc
                        );
                        util::runtime_int_result(int_ty, call)
                    }
                    _ => {
                        let op_name = arithmetic_name(op);
                        if self.debug_checks {
                            let call = format!(
                                "ls_{}_{}_checked({}, {}, {})",
                                op_name, suffix, left, right, loc
                            );
                            util::runtime_int_result(int_ty, call)
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
            let call = format!(
                "ls_{}_{}_checked({}, (int64_t)({}), {})",
                op_name,
                util::runtime_int_suffix(int_ty),
                util::runtime_int_argument(int_ty, lhs),
                rhs,
                loc
            );
            return util::runtime_int_result(int_ty, call);
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
                // The right-hand side of an assignment is a consuming context:
                // transfer it into a temporary first so the old place can be
                // released without observing a partially written value.
                let stored = if self.is_trivial_copy(&place_ty) {
                    self.gen_owned(value)
                } else {
                    let stored = self.declare_temp(&place_ty);
                    self.gen_move_into(value, &stored);
                    stored
                };
                if let HExprKind::Local { c_name, .. } = &place.kind {
                    self.emit_release_guarded(
                        &place_ty,
                        &lvalue,
                        self.live_flag(c_name).as_deref(),
                    );
                } else {
                    self.emit_release(&place_ty, &lvalue);
                }
                self.w.line(format!("{} = {};", lvalue, stored));
                self.mark_moved(&stored);
                if let HExprKind::Local { c_name, .. } = &place.kind {
                    self.set_live(c_name, true);
                }
            }
            Some(binop) => {
                let old = self.declare_temp(&place_ty);
                self.emit_copy_into(&old, &place_ty, &lvalue);
                // `text += &str` concatenates borrowed text: bridge a fat
                // `LsStr` right hand side to the `const LsString*` the
                // runtime helper expects (see `gen_binary`).
                let right =
                    if binop == HBinaryOp::Add && place_ty.is_string() && !value.ty.is_string() {
                        self.string_text_argument(value, "String addition assignment")
                    } else {
                        self.gen_owned(value)
                    };
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
            (Ty::Int(_), Ty::Char) => format!("(uint32_t)({})", value),
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
        // Calling a function value does not consume it. When the callee is a
        // place (a local binding, a field of a struct, an indexed slot, ...)
        // the two fields can be read straight from its C lvalue: emitting an
        // owned temporary would copy (retain) the closure environment before
        // every call and release it again afterwards for no gain. A non-place
        // callee still needs the temporary so that the expression is evaluated
        // exactly once.
        let function = if callee.is_place() {
            self.gen_place(callee)
        } else {
            self.gen_owned(callee)
        };
        let param_types: Vec<Ty> = match &callee.ty {
            Ty::Fn(params, _) => params.clone(),
            _ => Vec::new(),
        };
        let mut argument_values = Vec::with_capacity(args.len());
        for (index, argument) in args.iter().enumerate() {
            let param = param_types
                .get(index)
                .cloned()
                .unwrap_or_else(|| argument.ty.clone());
            if param.is_ref() {
                argument_values.push(self.gen_borrowed(argument));
            } else if self.is_trivial_copy(&param) {
                argument_values.push(self.gen_owned(argument));
            } else {
                // By-value argument of a managed type: ownership moves into
                // the callee, so it is transferred into a fresh temporary.
                let value = self.declare_temp(&param);
                self.gen_move_into(argument, &value);
                argument_values.push(value);
            }
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
        span: Span,
    ) {
        let item = match self.program.fns.get(fn_id) {
            Some(item) => item.clone(),
            None => {
                let zero = self.zero_expr(ty);
                self.w.line(format!("{} = {};", target, zero));
                return;
            }
        };
        // Intrinsic functions have no emitted definition: the call is expanded
        // inline by the intrinsic table instead.
        if item.intrinsic {
            self.gen_intrinsic_call(&item, receiver, args, ty, target, span);
            return;
        }
        // A standard-library forwarding wrapper is expanded at the call site:
        // its body only forwards its own parameters to an intrinsic, so the
        // intrinsic emitter produces the specialised code directly and no dead
        // wrapper function is emitted.
        if let Some(intrinsic_id) = self.inline_plan.inline_target(fn_id) {
            let intrinsic = self.program.fns[intrinsic_id].clone();
            let self_param = self.program.fns[fn_id]
                .params
                .first()
                .map(|parameter| parameter.ty.clone());
            // The borrow conversion intrinsics read the declared reference
            // type (they return a view of the receiver), so their call sites
            // need an explicit borrow of a by-value receiver. The other
            // forwarding wrappers hand the receiver to the legacy builtin
            // emitters, which accept the value form directly and avoid an
            // extra pointer temporary.
            let full_name = intrinsic.name.as_str();
            let bare_name = full_name.split('<').next().unwrap_or(full_name).trim();
            let needs_reference = matches!(
                bare_name,
                "__string_as_str" | "__string_as_mut_str" | "__vec_as_slice" | "__vec_as_mut_slice"
            );
            let mut call_args: Vec<HExpr> = Vec::with_capacity(args.len() + 1);
            if let Some(receiver) = receiver {
                let mut argument = (*receiver).clone();
                if needs_reference
                    && let Some(param_ty) = self_param.clone()
                    && let Ty::Ref { inner, mutable } = &param_ty
                    && argument.ty == **inner
                {
                    let mutable = *mutable;
                    argument = HExpr::new(
                        param_ty,
                        HExprKind::Ref {
                            mutable,
                            expr: Box::new(argument),
                        },
                        span,
                    );
                }
                call_args.push(argument);
            }
            call_args.extend(args.iter().cloned());
            self.gen_intrinsic_call(&intrinsic, None, &call_args, ty, target, span);
            return;
        }
        let mut arguments: Vec<String> = Vec::new();
        // Values whose ownership is transferred to the callee: the caller must
        // not release them. These are the by-value receiver and the arguments.
        let mut moved_values: Vec<String> = Vec::new();
        if let Some(receiver_expr) = receiver {
            let receiver_ty = receiver_expr.ty.clone();
            let self_ty = item
                .params
                .first()
                .map(|parameter| {
                    let ty = parameter.ty.clone();
                    // The receiver parameter is stored as a reference to the
                    // impl type; pointer setup works on the pointee type.
                    match item.self_kind {
                        Some(SelfKind::Ref) | Some(SelfKind::RefMut) => {
                            ty.ref_inner().cloned().unwrap_or(ty)
                        }
                        _ => ty,
                    }
                })
                .unwrap_or(Ty::Unit);
            match item.self_kind {
                Some(SelfKind::Ref) | Some(SelfKind::RefMut) => {
                    if self_ty.is_unsized() {
                        // Unsized receivers are fat descriptors passed by
                        // value: the receiver expression already produces one.
                        let value = self.gen_borrowed(receiver_expr);
                        arguments.push(value);
                    } else {
                        // A shared receiver whose pointee is a pointer keeps the
                        // reference type's const qualification (see
                        // fn_param_declarations).
                        let pointee_ctype = self.ctype(&self_ty);
                        let pointer_ctype = if matches!(item.self_kind, Some(SelfKind::Ref))
                            && pointee_ctype.ends_with('*')
                        {
                            item.params
                                .first()
                                .map(|parameter| self.ctype(&parameter.ty))
                                .unwrap_or_else(|| format!("{}*", pointee_ctype))
                        } else {
                            format!("{}*", pointee_ctype)
                        };
                        let pointer =
                            self.receiver_pointer(receiver_expr, &self_ty, &pointer_ctype);
                        arguments.push(pointer);
                    }
                }
                _ => {
                    if receiver_ty.is_ref() {
                        // A by-value method reached through a reference: the
                        // callee consumes a copy of the pointee.
                        let pointer = self.gen_owned(receiver_expr);
                        let copy = self.declare_temp(&self_ty);
                        let pointee = format!("(*({}))", pointer);
                        let pointee = if self.is_pointer_type(&self_ty) {
                            format!("(({})({}))", self.ctype(&self_ty), pointee)
                        } else {
                            pointee
                        };
                        self.emit_copy_into(&copy, &self_ty, &pointee);
                        arguments.push(copy.clone());
                        moved_values.push(copy);
                    } else {
                        // A by-value receiver is consumed: transfer ownership
                        // into a temporary that the callee takes over.
                        let value = self.declare_temp(&self_ty);
                        self.gen_move_into(receiver_expr, &value);
                        arguments.push(value.clone());
                        moved_values.push(value);
                    }
                }
            }
        }
        let receiver_params = usize::from(item.self_kind.is_some());
        for (index, argument) in args.iter().enumerate() {
            let param = item
                .params
                .get(index + receiver_params)
                .map(|parameter| parameter.ty.clone())
                .unwrap_or_else(|| argument.ty.clone());
            if param.is_ref() {
                arguments.push(self.gen_borrowed(argument));
            } else if self.is_trivial_copy(&param) {
                let value = self.gen_owned(argument);
                arguments.push(value);
            } else {
                let value = self.declare_temp(&param);
                self.gen_move_into(argument, &value);
                arguments.push(value.clone());
                moved_values.push(value);
            }
        }
        let call = format!("{}({})", item.c_name, arguments.join(", "));
        if ty.is_unit() {
            self.w.line(format!("{};", call));
            let zero = self.zero_expr(ty);
            self.w.line(format!("{} = {};", target, zero));
        } else {
            self.w.line(format!("{} = {};", target, call));
        }
        for value in &moved_values {
            self.mark_moved(value);
        }
    }

    /// A pointer of type `pointer_ctype` that points at the self value of a
    /// method receiver, for the `&self` / `&mut self` calling convention.
    ///
    /// Handles receivers that are already references (the pointer value is
    /// forwarded), `Rc`/`Box` handles (the pointer targets the payload cell)
    /// and plain place or value expressions (the address of the value).
    fn receiver_pointer(&mut self, expr: &HExpr, self_ty: &Ty, pointer_ctype: &str) -> String {
        let mut ty = expr.ty.clone();
        let mut pointer_value: Option<String> = None;
        let mut lvalue;
        if ty.is_ref() {
            let pointer = self.gen_owned(expr);
            ty = ty.ref_inner().cloned().unwrap_or(Ty::Unit);
            lvalue = format!("(*({}))", pointer);
            pointer_value = Some(format!("({}){}", pointer_ctype, pointer));
        } else if expr.is_place() {
            lvalue = self.gen_place(expr);
        } else {
            lvalue = self.gen_owned(expr);
        }
        let mut crossed_cell = false;
        for _ in 0..8 {
            // Methods implemented on Rc<T>/Box<T> itself take the cell handle,
            // not its payload: the impl self type is the cell type, so stop as
            // soon as the receiver already has it. Only method calls that
            // auto-dereference through the cell (self type is the pointee)
            // need to cross it.
            if ty == *self_ty {
                break;
            }
            match &ty {
                Ty::Rc(inner) | Ty::Box(inner) => {
                    lvalue = format!("(({})->payload)", lvalue);
                    ty = (**inner).clone();
                    crossed_cell = true;
                }
                _ => break,
            }
        }
        if !crossed_cell
            && let Some(pointer) = pointer_value
            && ty == *self_ty
        {
            return pointer;
        }
        format!("({})&({})", pointer_ctype, lvalue)
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
            let field = format!("{}.{}", target, types::field_cname(&name));
            self.gen_move_into(expression, &field);
        }
    }

    pub(crate) fn gen_tuple(&mut self, elems: &[HExpr], _ty: &Ty, target: &str) {
        for (index, element) in elems.iter().enumerate() {
            let field = format!("{}.{}", target, types::positional_field(index));
            self.gen_move_into(element, &field);
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
            let slot = format!("{}.data[{}]", target, index);
            self.gen_move_into(element, &slot);
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

    /// A fixed array literal.
    ///
    /// The elements are stored inline in the generated struct, so each one is
    /// moved into its own slot: no allocation and no extra copy.
    pub(crate) fn gen_array(&mut self, elems: &[HExpr], ty: &Ty, target: &str) {
        let len = match ty {
            Ty::Array(_, len) => *len,
            _ => elems.len(),
        };
        if elems.len() != len {
            util::internal_error("an array literal does not match the length of its type");
        }
        for (index, element) in elems.iter().enumerate() {
            let slot = format!("{}.data[{}]", target, index);
            self.gen_move_into(element, &slot);
        }
    }

    /// A repeated array literal: the single value is evaluated once and copied
    /// into every element slot.
    pub(crate) fn gen_array_repeat(&mut self, value: &HExpr, len: usize, ty: &Ty, target: &str) {
        if len == 0 {
            return;
        }
        let element_ty = match ty {
            Ty::Array(elem, _) => (**elem).clone(),
            _ => value.ty.clone(),
        };
        let item = self.gen_owned(value);
        let copied = ops::copy_expr(self.names, &element_ty, &item);
        if len <= 8 {
            for index in 0..len {
                self.w
                    .line(format!("{}.data[{}] = {};", target, index, copied));
            }
        } else {
            // Large repeats stay compact: copy element by element in a loop.
            let index = self.fresh("i");
            self.w.line(format!(
                "for (size_t {} = 0; {} < (size_t){}; {} += 1) {{",
                index, index, len, index
            ));
            self.w.indent();
            self.w
                .line(format!("{}.data[{}] = {};", target, index, copied));
            self.w.dedent();
            self.w.line("}");
        }
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
            let slot = format!(
                "{}.{}.{}.{}",
                target,
                types::UNION_FIELD,
                types::variant_member(&variant.name),
                types::positional_field(index)
            );
            self.gen_move_into(argument, &slot);
        }
    }

    pub(crate) fn gen_option_some(&mut self, inner: &HExpr, ty: &Ty, target: &str) {
        if ops::is_frozen_shared(ty) {
            let value = self.gen_owned(inner);
            self.w
                .line(format!("{} = ls_opt_string_some({});", target, value));
            self.mark_moved(&value);
        } else {
            self.w
                .line(format!("{}.{} = (int32_t)1;", target, types::TAG_FIELD));
            let slot = format!(
                "{}.{}.{}",
                target,
                types::UNION_FIELD,
                types::OPTION_SOME_MEMBER
            );
            self.gen_move_into(inner, &slot);
        }
    }

    pub(crate) fn gen_result_variant(&mut self, is_ok: bool, inner: &HExpr, ty: &Ty, target: &str) {
        if ops::is_frozen_shared(ty) {
            let value = self.gen_owned(inner);
            let constructor = if is_ok {
                "ls_res_string_ok"
            } else {
                "ls_res_string_err"
            };
            self.w
                .line(format!("{} = {}({});", target, constructor, value));
            self.mark_moved(&value);
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
            let slot = format!("{}.{}.{}", target, types::UNION_FIELD, member);
            self.gen_move_into(inner, &slot);
        }
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
