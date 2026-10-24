//! Call expressions, method calls, enum constructors and macros.

use crate::ast::{self, ExprKind};
use crate::hir::{HBuiltinAssoc, HBuiltinFn, HMacroKind};
use crate::span::Span;
use crate::typeck::builtins::{
    FormatPiece, builtin_assoc, builtin_fn, builtin_method, parse_alias, parse_format_string,
};
use crate::typeck::check::Checker;
use crate::typeck::items::{TypeKey, closest_name};
use crate::typeck::itype::{ITy, VarKind};
use crate::typeck::tree::{TExpr, TExprKind, TFormatPart, TFormatPartKind, TMacroCall};
use crate::types::{IntTy, SelfKind};

impl<'a> Checker<'a> {
    pub fn check_call(
        &mut self,
        callee: &ast::Expr,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        if let ExprKind::Path(path) = &callee.kind {
            return self.check_call_path(path, args, span, hint);
        }
        let callee_expr = self.check_expr(callee, None);
        self.check_call_value(callee_expr, args, span, hint)
    }

    pub fn check_call_value(
        &mut self,
        callee: TExpr,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let callee_ty = self.infer.resolve(&callee.ty);
        match &callee_ty {
            ITy::Fn(params, ret) => {
                let params = params.clone();
                let ret = (**ret).clone();
                let arg_exprs = self.check_call_args(&params, args, span);
                self.apply_hint(&ret, hint);
                TExpr::new(
                    ret,
                    TExprKind::Call {
                        callee: Box::new(callee),
                        args: arg_exprs,
                    },
                    span,
                )
            }
            ITy::Never => self.never_expr(span),
            _ => {
                let described = self.describe_resolved(&callee_ty);
                self.error_code(
                    format!("call expression requires a function, found {}", described),
                    span,
                    "E0618",
                );
                for arg in args {
                    let _ = self.check_expr(arg, None);
                }
                self.never_expr(span)
            }
        }
    }

    pub fn check_call_path(
        &mut self,
        path: &ast::PathExpr,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let segments = &path.segments;
        if segments.len() == 1 {
            let name = segments[0].name.clone();
            if !segments[0].generics.is_empty() {
                self.error_code(
                    "generic arguments are not allowed here".to_string(),
                    segments[0].span,
                    "E0107",
                );
            }
            if !self.in_const
                && self.find_local(&name).is_some()
                && let Some(callee) = self.resolve_local_name(&name, path.span)
            {
                return self.check_call_value(callee, args, span, hint);
            }
            match name.as_str() {
                "Some" => return self.check_option_some_ctor(args, span, hint),
                "None" => return self.check_option_none_ctor(args, span, hint),
                "Ok" => return self.check_result_ok_ctor(args, span, hint),
                "Err" => return self.check_result_err_ctor(args, span, hint),
                _ => {}
            }
            if self.const_by_name.contains_key(&name) {
                let callee = self.check_path_value(path, None, path.span);
                return self.check_call_value(callee, args, span, hint);
            }
            if let Some(fn_id) = self.fn_by_name.get(&name).copied() {
                return self.check_direct_call(fn_id, None, args, span, hint);
            }
            if builtin_fn(&name).is_some() {
                return self.check_builtin_fn_call(&name, args, span, hint);
            }
            let suggestion = self.suggest_callable(&name);
            self.error_suggest(
                format!("cannot find function {} in this scope", name),
                suggestion,
                span,
                "E0425",
            );
            for arg in args {
                let _ = self.check_expr(arg, None);
            }
            return self.never_expr(span);
        }
        if segments.len() == 2 {
            let type_name = segments[0].name.clone();
            let item = segments[1].name.clone();
            let generics = segments[0].generics.clone();
            if !segments[1].generics.is_empty() {
                self.error_code(
                    "generic arguments are not allowed here".to_string(),
                    segments[1].span,
                    "E0107",
                );
            }
            if let Some(func) = builtin_assoc(&type_name, &item) {
                return self.check_builtin_assoc_call(func, &generics, args, span, hint);
            }
            match (type_name.as_str(), item.as_str()) {
                ("Option", "Some") => return self.check_option_some_ctor(args, span, hint),
                ("Option", "None") => return self.check_option_none_ctor(args, span, hint),
                ("Result", "Ok") => return self.check_result_ok_ctor(args, span, hint),
                ("Result", "Err") => return self.check_result_err_ctor(args, span, hint),
                _ => {}
            }
            if let Some(enum_id) = self.lookup_enum_for_path(&type_name, segments[0].span) {
                self.ensure_enum_variants(enum_id);
                if let Some(variant) = self.find_variant(enum_id, &item) {
                    return self.finish_enum_variant_call(enum_id, variant, args, span, hint);
                }
                if let Some(fn_id) = self
                    .methods
                    .get(&(TypeKey::Enum(enum_id), item.clone()))
                    .copied()
                {
                    if self.fns[fn_id].is_method() {
                        let enum_name = self.enum_name(enum_id);
                        self.error_code(
                            format!(
                                "method {}::{} cannot be called as an associated function",
                                enum_name, item
                            ),
                            span,
                            "E0599",
                        );
                        for arg in args {
                            let _ = self.check_expr(arg, None);
                        }
                        return self.never_expr(span);
                    }
                    return self.check_direct_call(fn_id, None, args, span, hint);
                }
                let enum_name = self.enum_name(enum_id);
                let mut candidates: Vec<String> = Vec::new();
                for index in 0..self.variant_count(enum_id) {
                    candidates.push(self.variant_name(enum_id, index));
                }
                candidates.sort();
                let refs: Vec<&str> = candidates.iter().map(|name| name.as_str()).collect();
                let suggestion = closest_name(&item, &refs);
                self.error_suggest(
                    format!("enum {} has no variant named {}", enum_name, item),
                    suggestion,
                    span,
                    "E0599",
                );
                for arg in args {
                    let _ = self.check_expr(arg, None);
                }
                return self.never_expr(span);
            }
            if let Some(owner) = self.lookup_type_key(&type_name, segments[0].span) {
                if let Some(fn_id) = self.methods.get(&(owner, item.clone())).copied() {
                    if self.fns[fn_id].is_method() {
                        let owner_name = self.type_name(owner);
                        self.error_code(
                            format!(
                                "method {}::{} cannot be called as an associated function",
                                owner_name, item
                            ),
                            span,
                            "E0599",
                        );
                        for arg in args {
                            let _ = self.check_expr(arg, None);
                        }
                        return self.never_expr(span);
                    }
                    return self.check_direct_call(fn_id, None, args, span, hint);
                }
                let owner_name = self.type_name(owner);
                let suggestion = self.suggest_associated(owner, &item);
                self.error_suggest(
                    format!(
                        "no function or associated item named {} found for type {}",
                        item, owner_name
                    ),
                    suggestion,
                    span,
                    "E0599",
                );
                for arg in args {
                    let _ = self.check_expr(arg, None);
                }
                return self.never_expr(span);
            }
            let suggestion = self.suggest_type_name(&type_name);
            self.error_suggest(
                format!("cannot find type {} in this scope", type_name),
                suggestion,
                span,
                "E0412",
            );
            for arg in args {
                let _ = self.check_expr(arg, None);
            }
            return self.never_expr(span);
        }
        self.error_code(
            "paths with more than two segments are not supported".to_string(),
            path.span,
            "E0001",
        );
        for arg in args {
            let _ = self.check_expr(arg, None);
        }
        self.never_expr(span)
    }

    pub fn check_direct_call(
        &mut self,
        fn_id: usize,
        receiver: Option<TExpr>,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let params = self.fns[fn_id].params();
        let ret = self.fns[fn_id].ret();
        let offset = if receiver.is_some() { 1 } else { 0 };
        let expected: Vec<ITy> = params.iter().skip(offset).cloned().collect();
        let arg_exprs = self.check_call_args(&expected, args, span);
        self.apply_hint(&ret, hint);
        TExpr::new(
            ret,
            TExprKind::DirectCall {
                fn_id,
                receiver: receiver.map(Box::new),
                args: arg_exprs,
            },
            span,
        )
    }

    fn finish_enum_variant_call(
        &mut self,
        enum_id: usize,
        variant: usize,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let fields = self.variant_fields(enum_id, variant);
        if fields.is_empty() {
            if !args.is_empty() {
                let variant_name = self.variant_name(enum_id, variant);
                self.error_code(
                    format!(
                        "variant {} takes no arguments but {} were supplied",
                        variant_name,
                        args.len()
                    ),
                    span,
                    "E0061",
                );
                for arg in args {
                    let _ = self.check_expr(arg, None);
                }
            }
            let ty = ITy::Enum(enum_id);
            self.apply_hint(&ty, hint);
            return TExpr::new(
                ty,
                TExprKind::EnumLit {
                    enum_id,
                    variant,
                    args: Vec::new(),
                },
                span,
            );
        }
        let arg_exprs = self.check_call_args(&fields, args, span);
        let ty = ITy::Enum(enum_id);
        self.apply_hint(&ty, hint);
        TExpr::new(
            ty,
            TExprKind::EnumLit {
                enum_id,
                variant,
                args: arg_exprs,
            },
            span,
        )
    }

    fn check_builtin_fn_call(
        &mut self,
        name: &str,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let func = match builtin_fn(name) {
            Some(func) => func,
            None => {
                self.error_code(
                    format!("cannot find function {} in this scope", name),
                    span,
                    "E0425",
                );
                for arg in args {
                    let _ = self.check_expr(arg, None);
                }
                return self.never_expr(span);
            }
        };
        match func {
            HBuiltinFn::Abs => self.check_numeric_builtin(func, args, span, hint, 1),
            HBuiltinFn::Min | HBuiltinFn::Max => {
                self.check_numeric_builtin(func, args, span, hint, 2)
            }
            _ => {
                let (params, ret) = match func {
                    HBuiltinFn::Sqrt
                    | HBuiltinFn::Floor
                    | HBuiltinFn::Ceil
                    | HBuiltinFn::Round
                    | HBuiltinFn::Sin
                    | HBuiltinFn::Cos
                    | HBuiltinFn::Tan
                    | HBuiltinFn::Ln
                    | HBuiltinFn::Log10
                    | HBuiltinFn::Exp => (vec![ITy::f64()], ITy::f64()),
                    HBuiltinFn::Pow => (vec![ITy::f64(), ITy::f64()], ITy::f64()),
                    HBuiltinFn::ReadLine => (Vec::new(), ITy::option(ITy::string())),
                    HBuiltinFn::Args => (Vec::new(), ITy::vec(ITy::string())),
                    HBuiltinFn::Exit => (vec![ITy::Int(IntTy::I32)], ITy::never()),
                    HBuiltinFn::ReadFile => (
                        vec![ITy::string()],
                        ITy::result(ITy::string(), ITy::string()),
                    ),
                    HBuiltinFn::WriteFile => (
                        vec![ITy::string(), ITy::string()],
                        ITy::result(ITy::unit(), ITy::string()),
                    ),
                    _ => (Vec::new(), ITy::unit()),
                };
                let arg_exprs = self.check_call_args(&params, args, span);
                self.apply_hint(&ret, hint);
                TExpr::new(
                    ret,
                    TExprKind::BuiltinCall {
                        func,
                        args: arg_exprs,
                    },
                    span,
                )
            }
        }
    }

    fn check_numeric_builtin(
        &mut self,
        func: HBuiltinFn,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
        arity: usize,
    ) -> TExpr {
        let function_name = func.name();
        if args.len() != arity {
            self.error_code(
                format!(
                    "{} takes {} argument(s) but {} were supplied",
                    function_name,
                    arity,
                    args.len()
                ),
                span,
                "E0061",
            );
            let mut exprs = Vec::new();
            for arg in args {
                exprs.push(self.check_expr(arg, None));
            }
            return TExpr::new(
                hint.cloned().unwrap_or_else(ITy::unit),
                TExprKind::BuiltinCall { func, args: exprs },
                span,
            );
        }
        let hint_ty = hint.map(|hint| self.infer.resolve(hint));
        let numeric_hint = match &hint_ty {
            Some(ITy::Int(_)) | Some(ITy::Float(_)) => hint.map(|hint| self.infer.resolve(hint)),
            _ => None,
        };
        let mut exprs = Vec::new();
        for arg in args {
            let value = self.check_expr(arg, numeric_hint.as_ref());
            exprs.push(value);
        }
        if exprs.len() == 2 {
            let first_ty = exprs[0].ty.clone();
            let second_ty = exprs[1].ty.clone();
            if !self.unify_quiet(&first_ty, &second_ty) {
                let resolved_first = self.infer.resolve(&first_ty);
                let resolved_second = self.infer.resolve(&second_ty);
                self.report_mismatch(&resolved_first, &resolved_second, args[1].span);
            }
        }
        let first_ty = exprs[0].ty.clone();
        let resolved = self.infer.resolve(&first_ty);
        let ret = match &resolved {
            ITy::Int(_) | ITy::Float(_) => {
                self.apply_hint(&resolved, hint);
                resolved.clone()
            }
            ITy::Never => ITy::never(),
            ITy::Var(id) => match self.infer.var_kind(*id) {
                VarKind::IntLiteral => {
                    let ty = ITy::i64();
                    let _ = self.unify_quiet(&resolved, &ty);
                    self.apply_hint(&ty, hint);
                    ty
                }
                VarKind::FloatLiteral => {
                    let ty = ITy::f64();
                    let _ = self.unify_quiet(&resolved, &ty);
                    self.apply_hint(&ty, hint);
                    ty
                }
                VarKind::General => {
                    self.error_code(
                        format!(
                            "type annotations needed for the argument of {}",
                            function_name
                        ),
                        args[0].span,
                        "E0282",
                    );
                    self.infer.fresh_var()
                }
            },
            _ => {
                let described = self.describe_resolved(&resolved);
                self.error_code(
                    format!(
                        "{} requires a numeric argument, found {}",
                        function_name, described
                    ),
                    args[0].span,
                    "E0301",
                );
                ITy::unit()
            }
        };
        TExpr::new(ret, TExprKind::BuiltinCall { func, args: exprs }, span)
    }

    fn check_builtin_assoc_call(
        &mut self,
        func: HBuiltinAssoc,
        generics: &[ast::TypeExpr],
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        match func {
            HBuiltinAssoc::StringNew => {
                if !generics.is_empty() {
                    self.error_code(
                        "generic arguments are not allowed here".to_string(),
                        span,
                        "E0107",
                    );
                }
                let arg_exprs = self.check_call_args(&[], args, span);
                let ret = ITy::string();
                self.apply_hint(&ret, hint);
                TExpr::new(
                    ret,
                    TExprKind::BuiltinAssoc {
                        func,
                        args: arg_exprs,
                        type_args: Vec::new(),
                    },
                    span,
                )
            }
            HBuiltinAssoc::StringFrom => {
                if !generics.is_empty() {
                    self.error_code(
                        "generic arguments are not allowed here".to_string(),
                        span,
                        "E0107",
                    );
                }
                let arg_exprs = self.check_call_args(&[ITy::string()], args, span);
                let ret = ITy::string();
                self.apply_hint(&ret, hint);
                TExpr::new(
                    ret,
                    TExprKind::BuiltinAssoc {
                        func,
                        args: arg_exprs,
                        type_args: Vec::new(),
                    },
                    span,
                )
            }
            HBuiltinAssoc::VecNew | HBuiltinAssoc::VecWithCapacity => {
                let mut type_args: Vec<ITy> = Vec::new();
                for generic in generics {
                    let resolved = self
                        .resolve_ty(generic, self.current_self_key())
                        .unwrap_or_else(|| self.infer.fresh_var());
                    type_args.push(resolved);
                }
                let elem = match type_args.first() {
                    Some(elem) => elem.clone(),
                    None => self.infer.fresh_var(),
                };
                let params = if matches!(func, HBuiltinAssoc::VecWithCapacity) {
                    vec![ITy::i64()]
                } else {
                    Vec::new()
                };
                let arg_exprs = self.check_call_args(&params, args, span);
                let ret = ITy::vec(elem.clone());
                self.apply_hint(&ret, hint);
                TExpr::new(
                    ret,
                    TExprKind::BuiltinAssoc {
                        func,
                        args: arg_exprs,
                        type_args: vec![elem],
                    },
                    span,
                )
            }
        }
    }

    pub fn check_option_some_ctor(
        &mut self,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let elem = match hint {
            Some(hint) => match self.infer.resolve(hint) {
                ITy::Option(elem) => (*elem).clone(),
                _ => self.infer.fresh_var(),
            },
            None => self.infer.fresh_var(),
        };
        if args.len() != 1 {
            self.error_code(
                format!("Some takes 1 argument but {} were supplied", args.len()),
                span,
                "E0061",
            );
            for arg in args {
                let _ = self.check_expr(arg, None);
            }
            return self.never_expr(span);
        }
        let value = self.check_expr(&args[0], Some(&elem));
        if !self.unify_quiet(&elem, &value.ty) {
            let resolved_elem = self.infer.resolve(&elem);
            let resolved_value = self.infer.resolve(&value.ty);
            self.report_mismatch(&resolved_elem, &resolved_value, args[0].span);
        }
        TExpr::new(
            ITy::option(elem),
            TExprKind::OptionSome(Box::new(value)),
            span,
        )
    }

    pub fn check_option_none_ctor(
        &mut self,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        if !args.is_empty() {
            self.error_code(
                format!("None takes no arguments but {} were supplied", args.len()),
                span,
                "E0061",
            );
            for arg in args {
                let _ = self.check_expr(arg, None);
            }
        }
        self.check_option_none_value(hint, span)
    }

    pub fn check_option_none_value(&mut self, hint: Option<&ITy>, span: Span) -> TExpr {
        let elem = match hint {
            Some(hint) => match self.infer.resolve(hint) {
                ITy::Option(elem) => (*elem).clone(),
                _ => self.infer.fresh_var(),
            },
            None => self.infer.fresh_var(),
        };
        let ty = ITy::option(elem);
        self.apply_hint(&ty, hint);
        TExpr::new(ty, TExprKind::OptionNone, span)
    }

    pub fn check_result_ok_ctor(
        &mut self,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let (ok_ty, err_ty) = match hint {
            Some(hint) => match self.infer.resolve(hint) {
                ITy::Result(ok, err) => ((*ok).clone(), (*err).clone()),
                _ => (self.infer.fresh_var(), self.infer.fresh_var()),
            },
            None => (self.infer.fresh_var(), self.infer.fresh_var()),
        };
        if args.len() != 1 {
            self.error_code(
                format!("Ok takes 1 argument but {} were supplied", args.len()),
                span,
                "E0061",
            );
            for arg in args {
                let _ = self.check_expr(arg, None);
            }
            return self.never_expr(span);
        }
        let value = self.check_expr(&args[0], Some(&ok_ty));
        if !self.unify_quiet(&ok_ty, &value.ty) {
            let resolved_ok = self.infer.resolve(&ok_ty);
            let resolved_value = self.infer.resolve(&value.ty);
            self.report_mismatch(&resolved_ok, &resolved_value, args[0].span);
        }
        TExpr::new(
            ITy::result(ok_ty, err_ty),
            TExprKind::ResultOk(Box::new(value)),
            span,
        )
    }

    pub fn check_result_err_ctor(
        &mut self,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let (ok_ty, err_ty) = match hint {
            Some(hint) => match self.infer.resolve(hint) {
                ITy::Result(ok, err) => ((*ok).clone(), (*err).clone()),
                _ => (self.infer.fresh_var(), self.infer.fresh_var()),
            },
            None => (self.infer.fresh_var(), self.infer.fresh_var()),
        };
        if args.len() != 1 {
            self.error_code(
                format!("Err takes 1 argument but {} were supplied", args.len()),
                span,
                "E0061",
            );
            for arg in args {
                let _ = self.check_expr(arg, None);
            }
            return self.never_expr(span);
        }
        let value = self.check_expr(&args[0], Some(&err_ty));
        if !self.unify_quiet(&err_ty, &value.ty) {
            let resolved_err = self.infer.resolve(&err_ty);
            let resolved_value = self.infer.resolve(&value.ty);
            self.report_mismatch(&resolved_err, &resolved_value, args[0].span);
        }
        TExpr::new(
            ITy::result(ok_ty, err_ty),
            TExprKind::ResultErr(Box::new(value)),
            span,
        )
    }

    // The AST shape of a method call needs every one of these inputs; grouping
    // them into a struct would only move the noise to each call site.
    #[allow(clippy::too_many_arguments)]
    pub fn check_method_call(
        &mut self,
        receiver: &ast::Expr,
        method: &str,
        method_span: Span,
        turbofish: &[ast::TypeExpr],
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let receiver_expr = self.check_expr(receiver, None);
        let receiver_ty = self.infer.resolve(&receiver_expr.ty);
        let owner = match &receiver_ty {
            ITy::Struct(id) => Some(TypeKey::Struct(*id)),
            ITy::Enum(id) => Some(TypeKey::Enum(*id)),
            _ => None,
        };
        if let Some(owner) = owner
            && let Some(fn_id) = self.methods.get(&(owner, method.to_string())).copied()
        {
            if matches!(self.fns[fn_id].self_kind, Some(SelfKind::RefMut)) {
                self.require_mutable_place(
                    &receiver_expr,
                    receiver.span,
                    "mutate through this receiver",
                );
            }
            return self.check_direct_call(fn_id, Some(receiver_expr), args, span, hint);
        }
        if !turbofish.is_empty() {
            if method == "parse" && turbofish.len() == 1 && matches!(receiver_ty, ITy::String) {
                let target = self
                    .resolve_ty(&turbofish[0], self.current_self_key())
                    .unwrap_or_else(|| self.infer.fresh_var());
                if let Some((builtin, ret)) = parse_alias(&target) {
                    let arg_exprs = self.check_call_args(&[], args, span);
                    self.apply_hint(&ret, hint);
                    return TExpr::new(
                        ret,
                        TExprKind::BuiltinMethod {
                            receiver: Box::new(receiver_expr),
                            method: builtin,
                            args: arg_exprs,
                        },
                        span,
                    );
                }
                let described = self.describe_resolved(&target);
                self.error_code(
                    format!("unsupported type argument for parse: {}", described),
                    method_span,
                    "E0107",
                );
                for arg in args {
                    let _ = self.check_expr(arg, None);
                }
                return self.never_expr(span);
            }
            self.error_code(
                "generic arguments are only supported for .parse::<T>()".to_string(),
                method_span,
                "E0107",
            );
        }
        if let Some(info) = builtin_method(&receiver_ty, method) {
            if info.mutable_receiver {
                self.require_mutable_place(
                    &receiver_expr,
                    receiver.span,
                    "mutate through this receiver",
                );
            }
            let arg_exprs = self.check_call_args(&info.params, args, span);
            self.apply_hint(&info.ret, hint);
            return TExpr::new(
                info.ret,
                TExprKind::BuiltinMethod {
                    receiver: Box::new(receiver_expr),
                    method: info.method,
                    args: arg_exprs,
                },
                span,
            );
        }
        let described = self.describe_resolved(&receiver_ty);
        let suggestion = self.suggest_method(&receiver_ty, method);
        self.error_suggest(
            format!(
                "no method named {} found for type {} in the current scope",
                method, described
            ),
            suggestion,
            method_span,
            "E0599",
        );
        for arg in args {
            let _ = self.check_expr(arg, None);
        }
        self.never_expr(span)
    }

    pub fn suggest_method(&self, receiver_ty: &ITy, method: &str) -> Option<String> {
        let resolved = self.infer.resolve(receiver_ty);
        let mut candidates: Vec<String> = Vec::new();
        let static_methods: &[&str] = match &resolved {
            ITy::String => &[
                "len",
                "is_empty",
                "push_str",
                "clear",
                "contains",
                "starts_with",
                "ends_with",
                "find",
                "replace",
                "to_uppercase",
                "to_lowercase",
                "trim",
                "split",
                "parse_i64",
                "parse_f64",
                "substring",
                "repeat",
                "parse",
            ],
            ITy::Vec(_) => &[
                "len", "is_empty", "push", "pop", "insert", "remove", "clear", "first", "last",
                "get", "extend", "reverse",
            ],
            ITy::Option(_) => &["is_some", "is_none", "unwrap", "unwrap_or", "expect"],
            ITy::Result(_, _) => &[
                "is_ok",
                "is_err",
                "unwrap",
                "unwrap_or",
                "expect",
                "ok",
                "err",
            ],
            _ => &[],
        };
        for candidate in static_methods {
            candidates.push((*candidate).to_string());
        }
        let owner = match &resolved {
            ITy::Struct(id) => Some(TypeKey::Struct(*id)),
            ITy::Enum(id) => Some(TypeKey::Enum(*id)),
            _ => None,
        };
        if let Some(owner) = owner {
            let mut user_methods: Vec<String> = self
                .methods
                .keys()
                .filter(|entry| entry.0 == owner)
                .map(|entry| entry.1.clone())
                .collect();
            user_methods.sort();
            candidates.extend(user_methods);
        }
        candidates.sort();
        candidates.dedup();
        let refs: Vec<&str> = candidates.iter().map(|name| name.as_str()).collect();
        closest_name(method, &refs)
    }

    fn suggest_associated(&self, owner: TypeKey, item: &str) -> Option<String> {
        let mut candidates: Vec<String> = self
            .methods
            .keys()
            .filter(|entry| entry.0 == owner)
            .map(|entry| entry.1.clone())
            .collect();
        candidates.sort();
        let refs: Vec<&str> = candidates.iter().map(|name| name.as_str()).collect();
        closest_name(item, &refs)
    }

    pub fn check_macro(&mut self, m: &ast::MacroCall, span: Span, hint: Option<&ITy>) -> TExpr {
        let name = m.name.as_str();
        match name {
            "print" | "println" | "eprint" | "eprintln" => {
                let parts = self.check_format_parts(m, span).unwrap_or_default();
                let kind = match name {
                    "print" => HMacroKind::Print,
                    "println" => HMacroKind::Println,
                    "eprint" => HMacroKind::Eprint,
                    _ => HMacroKind::Eprintln,
                };
                TExpr::new(
                    ITy::unit(),
                    TExprKind::Macro(TMacroCall {
                        kind,
                        parts,
                        args: Vec::new(),
                    }),
                    span,
                )
            }
            "format" => {
                let parts = self.check_format_parts(m, span).unwrap_or_default();
                let ty = ITy::string();
                self.apply_hint(&ty, hint);
                TExpr::new(ty, TExprKind::Format { parts }, span)
            }
            "panic" => {
                let parts = if m.args.is_empty() {
                    Vec::new()
                } else if m.args[0].as_string_literal().is_some() {
                    self.check_format_parts(m, span).unwrap_or_default()
                } else if m.args.len() == 1 {
                    let value = self.check_expr(&m.args[0], None);
                    vec![TFormatPart {
                        kind: TFormatPartKind::Arg {
                            expr: Box::new(value),
                            debug: false,
                        },
                    }]
                } else {
                    let literal = self.check_expr(&m.args[0], None);
                    let _ = literal;
                    self.error_code(
                        "panic! expects a format string followed by its arguments".to_string(),
                        span,
                        "E0001",
                    );
                    for arg in &m.args[1..] {
                        let _ = self.check_expr(arg, None);
                    }
                    Vec::new()
                };
                TExpr::new(
                    ITy::unit(),
                    TExprKind::Macro(TMacroCall {
                        kind: HMacroKind::Panic,
                        parts,
                        args: Vec::new(),
                    }),
                    span,
                )
            }
            "assert" => {
                if m.args.len() != 1 {
                    self.error_code(
                        format!(
                            "assert! takes 1 argument but {} were supplied",
                            m.args.len()
                        ),
                        span,
                        "E0061",
                    );
                }
                let mut args = Vec::new();
                for arg in &m.args {
                    let value = self.check_expr(arg, Some(&ITy::bool_()));
                    self.require_bool(&value.ty, arg.span);
                    args.push(value);
                }
                TExpr::new(
                    ITy::unit(),
                    TExprKind::Macro(TMacroCall {
                        kind: HMacroKind::Assert,
                        parts: Vec::new(),
                        args,
                    }),
                    span,
                )
            }
            "assert_eq" => {
                if m.args.len() != 2 {
                    self.error_code(
                        format!(
                            "assert_eq! takes 2 arguments but {} were supplied",
                            m.args.len()
                        ),
                        span,
                        "E0061",
                    );
                }
                let mut args = Vec::new();
                let mut first_ty = self.infer.fresh_var();
                for (index, arg) in m.args.iter().enumerate() {
                    let hint = if index == 0 {
                        None
                    } else {
                        Some(first_ty.clone())
                    };
                    let value = self.check_expr(arg, hint.as_ref());
                    if index == 0 {
                        first_ty = value.ty.clone();
                    } else if !self.unify_quiet(&first_ty, &value.ty) {
                        let resolved_first = self.infer.resolve(&first_ty);
                        let resolved_value = self.infer.resolve(&value.ty);
                        self.report_mismatch(&resolved_first, &resolved_value, arg.span);
                    }
                    args.push(value);
                }
                let resolved_first = self.infer.resolve(&first_ty);
                if !self.equality_supported(&resolved_first) {
                    let described = self.describe_resolved(&resolved_first);
                    self.error_code(
                        format!(
                            "assert_eq! requires a type that supports ==, found {}",
                            described
                        ),
                        span,
                        "E0001",
                    );
                }
                TExpr::new(
                    ITy::unit(),
                    TExprKind::Macro(TMacroCall {
                        kind: HMacroKind::AssertEq,
                        parts: Vec::new(),
                        args,
                    }),
                    span,
                )
            }
            _ => {
                self.error_code(format!("unknown macro {}", name), m.name_span, "E0001");
                for arg in &m.args {
                    let _ = self.check_expr(arg, None);
                }
                self.never_expr(span)
            }
        }
    }

    fn equality_supported(&self, ty: &ITy) -> bool {
        matches!(
            ty,
            ITy::Bool | ITy::Int(_) | ITy::Float(_) | ITy::String | ITy::Never | ITy::Var(_)
        )
    }

    fn check_format_parts(&mut self, m: &ast::MacroCall, span: Span) -> Option<Vec<TFormatPart>> {
        if m.args.is_empty() {
            return Some(Vec::new());
        }
        let literal = match m.args[0].as_string_literal() {
            Some(literal) => literal.to_string(),
            None => {
                self.error_code(
                    "format string must be a string literal".to_string(),
                    m.args[0].span,
                    "E0001",
                );
                for arg in &m.args {
                    let _ = self.check_expr(arg, None);
                }
                return None;
            }
        };
        match parse_format_string(&literal) {
            Ok(pieces) => {
                let holes = pieces
                    .iter()
                    .filter(|piece| matches!(piece, FormatPiece::Hole { .. }))
                    .count();
                let provided = m.args.len() - 1;
                if holes != provided {
                    self.error_code(
                        format!(
                            "{} format argument(s) required, but {} were supplied",
                            holes, provided
                        ),
                        span,
                        "E0001",
                    );
                }
                let mut parts = Vec::new();
                let mut arg_index = 1;
                for piece in pieces {
                    match piece {
                        FormatPiece::Literal(text) => parts.push(TFormatPart {
                            kind: TFormatPartKind::Literal(text),
                        }),
                        FormatPiece::Hole { debug } => {
                            if arg_index < m.args.len() {
                                let expr = self.check_expr(&m.args[arg_index], None);
                                parts.push(TFormatPart {
                                    kind: TFormatPartKind::Arg {
                                        expr: Box::new(expr),
                                        debug,
                                    },
                                });
                            }
                            arg_index += 1;
                        }
                    }
                }
                for extra in m.args.iter().skip(arg_index) {
                    let _ = self.check_expr(extra, None);
                }
                Some(parts)
            }
            Err(message) => {
                self.error_code(message, m.args[0].span, "E0001");
                for arg in &m.args[1..] {
                    let _ = self.check_expr(arg, None);
                }
                None
            }
        }
    }
}
