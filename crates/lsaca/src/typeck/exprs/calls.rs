//! Call expressions, method calls, enum constructors and macros.

use std::collections::HashMap;

use super::combinators::is_combinator_method;
use crate::ast::{self, ExprKind};
use crate::hir::{HBuiltinAssoc, HBuiltinFn, HBuiltinMethod, HMacroKind};
use crate::span::Span;
use crate::typeck::builtins::{
    FormatPiece, FormatValueKind, builtin_assoc, builtin_fn, builtin_method, format_spec_error,
    parse_alias, parse_format_string,
};
use crate::typeck::check::Checker;
use crate::typeck::items::{TypeKey, closest_name};
use crate::typeck::iterators::is_iterator_sugar_name;
use crate::typeck::itype::{ITy, VarKind};
use crate::typeck::modules::{DefRef, ModuleId};
use crate::typeck::tree::{TExpr, TExprKind, TFormatPart, TFormatPartKind, TMacroCall};
use crate::types::{FloatTy, IntTy, SelfKind};

/// Maximum number of path segments resolved by one call path. Module paths may
/// be arbitrarily deep, so this is only an anti-DoS guard.
const MAX_CALL_PATH_SEGMENTS: usize = 256;

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

    // -----------------------------------------------------------------------
    // Tuple and unit struct construction
    // -----------------------------------------------------------------------

    /// Check a call whose callee names a struct. Tuple struct constructors are
    /// the only structs callable like functions; calling a named or unit struct
    /// is an error. Returns None when the name does not resolve to a struct.
    fn check_struct_ctor_call(
        &mut self,
        path: &ast::PathExpr,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> Option<TExpr> {
        let segment = path.segments.last()?;
        let name = segment.name.clone();
        let explicit = segment.generics.clone();
        if let Some(def_id) = self.lookup_generic_struct(&name) {
            return Some(self.check_generic_tuple_struct_call(def_id, &explicit, args, span, hint));
        }
        let id = self.lookup_struct(&name)?;
        let (tuple, unit) = (self.structs[id].tuple, self.structs[id].unit);
        if !tuple {
            let kind = if unit { "unit struct" } else { "struct" };
            self.error_code(
                format!(
                    "expected tuple struct or tuple variant, found {} {}",
                    kind, name
                ),
                span,
                "E0532",
            );
            return Some(self.check_ctor_plain_args(args, span));
        }
        self.ensure_struct_fields(id);
        let fields = self.struct_fields(id);
        if fields.len() != args.len() {
            self.error_code(
                format!(
                    "this struct takes {} argument{} but {} {} supplied",
                    fields.len(),
                    if fields.len() == 1 { "" } else { "s" },
                    args.len(),
                    if args.len() == 1 { "was" } else { "were" }
                ),
                span,
                "E0061",
            );
            return Some(self.check_ctor_plain_args(args, span));
        }
        let values = self.check_struct_ctor_fields(&fields, args);
        Some(self.finish_struct_ctor(id, values, span, hint))
    }

    fn check_ctor_plain_args(&mut self, args: &[ast::Expr], span: Span) -> TExpr {
        for arg in args {
            let _ = self.check_expr(arg, None);
        }
        self.never_expr(span)
    }

    fn check_generic_tuple_struct_call(
        &mut self,
        def_id: usize,
        explicit: &[ast::TypeExpr],
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let def = match self.generic_structs.get(def_id) {
            Some(def) => def.clone(),
            None => return self.never_expr(span),
        };
        if !matches!(def.decl.kind, ast::StructKind::Tuple(_)) {
            self.error_code(
                format!(
                    "expected tuple struct or tuple variant, found struct {}",
                    def.name
                ),
                span,
                "E0532",
            );
            return self.check_ctor_plain_args(args, span);
        }
        let field_defs = def.decl.kind.fields();
        if field_defs.len() != args.len() {
            self.error_code(
                format!(
                    "this struct takes {} argument{} but {} {} supplied",
                    field_defs.len(),
                    if field_defs.len() == 1 { "" } else { "s" },
                    args.len(),
                    if args.len() == 1 { "was" } else { "were" }
                ),
                span,
                "E0061",
            );
            return self.check_ctor_plain_args(args, span);
        }
        let owner = if !explicit.is_empty() {
            match self.resolve_concrete_type_args(explicit, span) {
                Some(type_args) => self.instantiate_struct(def_id, type_args, span),
                None => return self.never_expr(span),
            }
        } else if let Some(ITy::Struct(id)) = self.hinted_instance_of(&def.name, hint) {
            Some(id)
        } else {
            // Infer the type arguments from the argument types: check the
            // arguments without a hint, then match the declared field types
            // against the types those arguments produced.
            let checked: Vec<TExpr> = args.iter().map(|arg| self.check_expr(arg, None)).collect();
            let type_args = match self.infer_struct_type_args(def_id, &checked, span) {
                Some(type_args) => type_args,
                None => return self.never_expr(span),
            };
            let id = match self.instantiate_struct(def_id, type_args, span) {
                Some(id) => id,
                None => return self.never_expr(span),
            };
            self.ensure_struct_fields(id);
            let fields = self.struct_fields(id);
            let mut values = Vec::with_capacity(checked.len());
            for (index, (value, field)) in checked.into_iter().zip(fields.iter()).enumerate() {
                let expected = field.2.clone();
                let value = self.coerce_to(value, &expected, args[index].span);
                let resolved = self.infer.resolve(&expected);
                self.validate_storage_type(&resolved, args[index].span);
                values.push((index, value));
            }
            return self.finish_struct_ctor(id, values, span, hint);
        };
        let id = match owner {
            Some(id) => id,
            None => return self.never_expr(span),
        };
        self.ensure_struct_fields(id);
        let fields = self.struct_fields(id);
        let values = self.check_struct_ctor_fields(&fields, args);
        self.finish_struct_ctor(id, values, span, hint)
    }

    fn check_struct_ctor_fields(
        &mut self,
        fields: &[(String, Span, ITy)],
        args: &[ast::Expr],
    ) -> Vec<(usize, TExpr)> {
        let mut values = Vec::with_capacity(args.len());
        for (index, (arg, field)) in args.iter().zip(fields.iter()).enumerate() {
            let expected = field.2.clone();
            let value = self.check_expr(arg, Some(&expected));
            let value = self.coerce_to(value, &expected, arg.span);
            let resolved = self.infer.resolve(&expected);
            self.validate_storage_type(&resolved, arg.span);
            values.push((index, value));
        }
        values
    }

    fn finish_struct_ctor(
        &mut self,
        id: usize,
        values: Vec<(usize, TExpr)>,
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let ty = ITy::Struct(id);
        self.apply_hint(&ty, hint);
        TExpr::new(
            ty,
            TExprKind::StructLit {
                struct_id: id,
                fields: values,
            },
            span,
        )
    }

    /// Infer the generic arguments of a tuple struct from the argument types,
    /// matching the declared field type expressions structurally.
    fn infer_struct_type_args(
        &mut self,
        def_id: usize,
        checked: &[TExpr],
        span: Span,
    ) -> Option<Vec<ITy>> {
        let def = self.generic_structs.get(def_id)?.clone();
        let field_defs = def.decl.kind.fields();
        let mut env: HashMap<String, ITy> = HashMap::new();
        for (field, value) in field_defs.iter().zip(checked.iter()) {
            let actual = self.infer.resolve(&value.ty);
            let actual = match self.concretize_inference_ty(&actual) {
                Some(actual) => actual,
                None => {
                    self.error_code("type annotations needed".to_string(), span, "E0282");
                    return None;
                }
            };
            self.match_inference_ty(&field.ty, &actual, &def.generics, &mut env);
        }
        let mut out = Vec::with_capacity(def.generics.len());
        for param in &def.generics {
            match env.get(&param.name) {
                Some(ty) => out.push(ty.clone()),
                None => {
                    self.error_code(
                        format!(
                            "type annotations needed: cannot infer type argument {}",
                            param.name
                        ),
                        span,
                        "E0282",
                    );
                    return None;
                }
            }
        }
        Some(out)
    }

    /// Replace still-unconstrained literal variables with their default type so
    /// they can be matched against declared generic field types.
    fn concretize_inference_ty(&self, ty: &ITy) -> Option<ITy> {
        match ty {
            ITy::Var(id) => match self.infer.var_kind(*id) {
                VarKind::IntLiteral => Some(ITy::i32()),
                VarKind::FloatLiteral => Some(ITy::f64()),
                VarKind::General => None,
            },
            ITy::Ref { mutable, inner } => Some(ITy::Ref {
                mutable: *mutable,
                inner: Box::new(self.concretize_inference_ty(inner)?),
            }),
            ITy::Slice(elem) => Some(ITy::slice(self.concretize_inference_ty(elem)?)),
            ITy::Vec(elem) => Some(ITy::vec(self.concretize_inference_ty(elem)?)),
            ITy::Rc(elem) => Some(ITy::rc(self.concretize_inference_ty(elem)?)),
            ITy::Box(elem) => Some(ITy::box_(self.concretize_inference_ty(elem)?)),
            ITy::Option(elem) => Some(ITy::option(self.concretize_inference_ty(elem)?)),
            ITy::Result(ok, err) => Some(ITy::result(
                self.concretize_inference_ty(ok)?,
                self.concretize_inference_ty(err)?,
            )),
            ITy::HashMap(key, value) => Some(ITy::hash_map(
                self.concretize_inference_ty(key)?,
                self.concretize_inference_ty(value)?,
            )),
            ITy::Tuple(elems) => {
                let elems: Option<Vec<ITy>> = elems
                    .iter()
                    .map(|elem| self.concretize_inference_ty(elem))
                    .collect();
                Some(ITy::Tuple(elems?))
            }
            ITy::Fn(params, ret) => {
                let params: Option<Vec<ITy>> = params
                    .iter()
                    .map(|param| self.concretize_inference_ty(param))
                    .collect();
                Some(ITy::function(params?, self.concretize_inference_ty(ret)?))
            }
            other => Some(other.clone()),
        }
    }

    /// Structurally match a declared field type expression against an argument
    /// type, recording type parameters seen for the first time.
    fn match_inference_ty(
        &mut self,
        type_expr: &ast::TypeExpr,
        actual: &ITy,
        params: &[ast::GenericParam],
        env: &mut HashMap<String, ITy>,
    ) {
        match type_expr {
            ast::TypeExpr::Path { name, generics, .. } => {
                if generics.is_empty() && params.iter().any(|param| param.name == *name) {
                    match env.get(name) {
                        Some(existing) => {
                            let existing = existing.clone();
                            let _ = self.unify_quiet(&existing, actual);
                        }
                        None => {
                            env.insert(name.clone(), actual.clone());
                        }
                    }
                    return;
                }
                let resolved = self.infer.resolve(actual);
                match (generics.as_slice(), &resolved) {
                    ([elem], ITy::Vec(inner))
                    | ([elem], ITy::Rc(inner))
                    | ([elem], ITy::Box(inner))
                    | ([elem], ITy::Option(inner))
                    | ([elem], ITy::Slice(inner)) => {
                        let inner = (**inner).clone();
                        self.match_inference_ty(elem, &inner, params, env);
                    }
                    ([ok, err], ITy::Result(ok_ty, err_ty)) => {
                        let ok_ty = (**ok_ty).clone();
                        let err_ty = (**err_ty).clone();
                        self.match_inference_ty(ok, &ok_ty, params, env);
                        self.match_inference_ty(err, &err_ty, params, env);
                    }
                    ([key, value], ITy::HashMap(key_ty, value_ty)) => {
                        let key_ty = (**key_ty).clone();
                        let value_ty = (**value_ty).clone();
                        self.match_inference_ty(key, &key_ty, params, env);
                        self.match_inference_ty(value, &value_ty, params, env);
                    }
                    _ => {}
                }
            }
            ast::TypeExpr::Ref { inner, .. } => {
                if let ITy::Ref {
                    inner: actual_inner,
                    ..
                } = self.infer.resolve(actual)
                {
                    let actual_inner = (*actual_inner).clone();
                    self.match_inference_ty(inner, &actual_inner, params, env);
                }
            }
            ast::TypeExpr::Slice { elem, .. } => {
                if let ITy::Slice(actual_elem) = self.infer.resolve(actual) {
                    let actual_elem = (*actual_elem).clone();
                    self.match_inference_ty(elem, &actual_elem, params, env);
                }
            }
            ast::TypeExpr::Tuple { elems, .. } => {
                if let ITy::Tuple(actual_elems) = self.infer.resolve(actual)
                    && actual_elems.len() == elems.len()
                {
                    for (elem, actual_elem) in elems.iter().zip(actual_elems.iter()) {
                        let actual_elem = actual_elem.clone();
                        self.match_inference_ty(elem, &actual_elem, params, env);
                    }
                }
            }
            ast::TypeExpr::Fn {
                params: param_exprs,
                ret,
                ..
            } => {
                if let ITy::Fn(actual_params, actual_ret) = self.infer.resolve(actual)
                    && actual_params.len() == param_exprs.len()
                {
                    for (param_expr, actual_param) in param_exprs.iter().zip(actual_params.iter()) {
                        let actual_param = actual_param.clone();
                        self.match_inference_ty(param_expr, &actual_param, params, env);
                    }
                    let actual_ret = (*actual_ret).clone();
                    self.match_inference_ty(ret, &actual_ret, params, env);
                }
            }
            ast::TypeExpr::Dyn(_) => {}
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
                if let Some(def_id) = self.lookup_generic_fn(&name) {
                    let explicit = segments[0].generics.clone();
                    return self.check_generic_free_call(def_id, &explicit, args, span, hint);
                }
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
            if self.lookup_const(&name).is_some() {
                let callee = self.check_path_value(path, None, path.span);
                return self.check_call_value(callee, args, span, hint);
            }
            if let Some(fn_id) = self.lookup_fn(&name) {
                return self.check_direct_call(fn_id, None, args, span, hint);
            }
            if let Some(def_id) = self.lookup_generic_fn(&name) {
                return self.check_generic_free_call(def_id, &[], args, span, hint);
            }
            if builtin_fn(&name).is_some() {
                return self.check_builtin_fn_call(&name, args, span, hint);
            }
            if let Some(expr) = self.check_struct_ctor_call(path, args, span, hint) {
                return expr;
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
        if segments.len() > MAX_CALL_PATH_SEGMENTS {
            self.error_code(
                format!("path has more than {} segments", MAX_CALL_PATH_SEGMENTS),
                path.span,
                "E0001",
            );
            for arg in args {
                let _ = self.check_expr(arg, None);
            }
            return self.never_expr(span);
        }
        // Uniform path resolution: the leading segments may name a module (or
        // start with crate/self/super or a dependency crate), in which case the
        // call is resolved inside that module.
        let prefix_len = segments.len() - 1;
        if let Some((module, consumed)) = self.resolve_path_prefix(path, prefix_len, false)
            && consumed > 0
        {
            // The prefix resolved quietly so that associated-item syntax
            // keeps working, but every module it walked must be visible.
            if !self.check_module_steps_visible(path, consumed) {
                for arg in args {
                    let _ = self.check_expr(arg, None);
                }
                return self.never_expr(span);
            }
            if consumed == prefix_len {
                let segment = path.segments[prefix_len].clone();
                return self.check_module_item_call(module, &segment, args, span, hint);
            }
            if consumed + 2 == segments.len() {
                let rest = ast::PathExpr {
                    segments: path.segments[consumed..].to_vec(),
                    span: path.span,
                };
                return self.with_resolved_module(module, |checker| {
                    checker.check_call_path(&rest, args, span, hint)
                });
            }
            let module_path = self.module_path_string(module);
            self.error_code(
                format!(
                    "could not find '{}' in '{}'",
                    segments[consumed].name, module_path
                ),
                segments[0].span,
                "E0433",
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
            if type_name == "Self"
                && let Some(expr) = self.check_self_assoc_call(&item, args, span, hint)
            {
                return expr;
            }
            if self.lookup_generic_enum(&type_name).is_some() {
                return self
                    .check_generic_enum_call(&type_name, &generics, &item, args, span, hint);
            }
            if self.lookup_generic_struct(&type_name).is_some() {
                return self
                    .check_generic_assoc_call(&type_name, &generics, &item, args, span, hint);
            }
            if !segments[1].generics.is_empty() {
                self.error_code(
                    "generic arguments are not allowed here".to_string(),
                    segments[1].span,
                    "E0107",
                );
            }
            if let Some(expr) =
                self.check_builtin_assoc_impl_call(&type_name, &generics, &item, args, span, hint)
            {
                return expr;
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
                    if !self.method_visible(fn_id) {
                        let owner_name = self.type_name(owner);
                        self.error_code(
                            format!("associated function {}::{} is private", owner_name, item),
                            span,
                            "E0624",
                        );
                        for arg in args {
                            let _ = self.check_expr(arg, None);
                        }
                        return self.never_expr(span);
                    }
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
        let first = &segments[0];
        self.error_code(
            format!("cannot find module '{}' in this scope", first.name),
            first.span,
            "E0433",
        );
        for arg in args {
            let _ = self.check_expr(arg, None);
        }
        self.never_expr(span)
    }

    /// Resolve a module-qualified call such as <mod>util::math::double</mod>.
    pub fn check_module_item_call(
        &mut self,
        module: ModuleId,
        segment: &ast::PathSegment,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let name = segment.name.clone();
        let generics = segment.generics.clone();
        if let Some(binding) = self.lookup_value_in(module, &name) {
            if !self.check_binding_visible(&binding, segment.span) {
                for arg in args {
                    let _ = self.check_expr(arg, None);
                }
                return self.never_expr(span);
            }
            match binding.def {
                DefRef::Fn(fn_id) => {
                    if !generics.is_empty() {
                        self.error_code(
                            "generic arguments are not allowed here".to_string(),
                            segment.span,
                            "E0107",
                        );
                    }
                    return self.check_direct_call(fn_id, None, args, span, hint);
                }
                DefRef::GenericFn(def_id) => {
                    return self.check_generic_free_call(def_id, &generics, args, span, hint);
                }
                DefRef::Const(id) => {
                    if !generics.is_empty() {
                        self.error_code(
                            "generic arguments are not allowed here".to_string(),
                            segment.span,
                            "E0107",
                        );
                    }
                    self.ensure_const(id);
                    if let Some((ty, _)) = self.const_value(id) {
                        let ity = ITy::from_ty(&ty);
                        let callee =
                            TExpr::new(ity, TExprKind::ConstRef { const_id: id }, segment.span);
                        return self.check_call_value(callee, args, span, hint);
                    }
                    for arg in args {
                        let _ = self.check_expr(arg, None);
                    }
                    return self.never_expr(span);
                }
                other => {
                    self.error_code(
                        format!("expected a value, found {}", other.kind_name()),
                        span,
                        "E0423",
                    );
                    for arg in args {
                        let _ = self.check_expr(arg, None);
                    }
                    return self.never_expr(span);
                }
            }
        }
        let mut candidates: Vec<String> = self.modules[module].values.keys().cloned().collect();
        candidates.sort();
        candidates.retain(|candidate| !candidate.starts_with("__lsaca_"));
        let refs: Vec<&str> = candidates
            .iter()
            .map(|candidate| candidate.as_str())
            .collect();
        let suggestion = closest_name(&name, &refs);
        self.error_suggest(
            format!("cannot find function {} in this scope", name),
            suggestion,
            span,
            "E0425",
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

    pub fn finish_enum_variant_call(
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
                    let ty = ITy::i32();
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
            HBuiltinAssoc::StringFromChar => {
                if !generics.is_empty() {
                    self.error_code(
                        "generic arguments are not allowed here".to_string(),
                        span,
                        "E0107",
                    );
                }
                let arg_exprs = self.check_call_args(&[ITy::char_()], args, span);
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
            HBuiltinAssoc::RcNew => {
                let mut explicit: Vec<ITy> = Vec::new();
                for generic in generics {
                    let resolved = self
                        .resolve_ty(generic, self.current_self_key())
                        .unwrap_or_else(|| self.infer.fresh_var());
                    explicit.push(resolved);
                }
                let inner = match explicit.first() {
                    Some(inner) => inner.clone(),
                    None => match hint {
                        Some(hint) => match self.infer.resolve(hint) {
                            ITy::Rc(inner) => (*inner).clone(),
                            _ => self.infer.fresh_var(),
                        },
                        None => self.infer.fresh_var(),
                    },
                };
                let arg_exprs = self.check_call_args(std::slice::from_ref(&inner), args, span);
                // The payload may only be inferred from the argument (e.g.
                // `Rc::new(&text)`), so validate the resolved payload type.
                let resolved_inner = self.infer.resolve(&inner);
                self.validate_storage_type(&resolved_inner, span);
                let ret = ITy::rc(inner.clone());
                self.apply_hint(&ret, hint);
                TExpr::new(
                    ret,
                    TExprKind::BuiltinAssoc {
                        func,
                        args: arg_exprs,
                        type_args: vec![inner],
                    },
                    span,
                )
            }
            HBuiltinAssoc::BoxNew => {
                let mut explicit: Vec<ITy> = Vec::new();
                for generic in generics {
                    let resolved = self
                        .resolve_ty(generic, self.current_self_key())
                        .unwrap_or_else(|| self.infer.fresh_var());
                    explicit.push(resolved);
                }
                let inner = match explicit.first() {
                    Some(inner) => inner.clone(),
                    None => match hint {
                        Some(hint) => match self.infer.resolve(hint) {
                            ITy::Box(inner) => (*inner).clone(),
                            _ => self.infer.fresh_var(),
                        },
                        None => self.infer.fresh_var(),
                    },
                };
                let arg_exprs = self.check_call_args(std::slice::from_ref(&inner), args, span);
                // See HBuiltinAssoc::RcNew: validate the inferred payload.
                let resolved_inner = self.infer.resolve(&inner);
                self.validate_storage_type(&resolved_inner, span);
                let ret = ITy::box_(inner.clone());
                self.apply_hint(&ret, hint);
                TExpr::new(
                    ret,
                    TExprKind::BuiltinAssoc {
                        func,
                        args: arg_exprs,
                        type_args: vec![inner],
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
                    // `with_capacity` takes a length, i.e. a usize.
                    vec![ITy::usize()]
                } else {
                    Vec::new()
                };
                let arg_exprs = self.check_call_args(&params, args, span);
                // The element type may be inferred later (e.g. a `Vec::new()`
                // that is pushed a reference into); validating the resolved
                // element here catches explicit turbofish payloads too.
                let resolved_elem = self.infer.resolve(&elem);
                self.validate_storage_type(&resolved_elem, span);
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
            HBuiltinAssoc::HashMapNew => {
                let mut explicit: Vec<ITy> = Vec::new();
                for generic in generics {
                    let resolved = self
                        .resolve_ty(generic, self.current_self_key())
                        .unwrap_or_else(|| self.infer.fresh_var());
                    explicit.push(resolved);
                }
                let hint_ty = hint.map(|hint| self.infer.resolve(hint));
                let (key, value) = match explicit.as_slice() {
                    [key, value] => (key.clone(), value.clone()),
                    [] => match hint_ty {
                        Some(ITy::HashMap(key, value)) => ((*key).clone(), (*value).clone()),
                        _ => {
                            // Without a turbofish or an expected type the key and
                            // value types cannot be inferred.
                            self.error_code(
                                "type annotations needed for HashMap key and value types"
                                    .to_string(),
                                span,
                                "E0282",
                            );
                            (self.infer.fresh_var(), self.infer.fresh_var())
                        }
                    },
                    other => {
                        self.error_code(
                            format!(
                                "HashMap takes 2 type arguments but {} were supplied",
                                other.len()
                            ),
                            span,
                            "E0107",
                        );
                        (self.infer.fresh_var(), self.infer.fresh_var())
                    }
                };
                let arg_exprs = self.check_call_args(&[], args, span);
                self.validate_hash_map_type(&key, &value, span);
                let ret = ITy::hash_map(key.clone(), value.clone());
                self.apply_hint(&ret, hint);
                TExpr::new(
                    ret,
                    TExprKind::BuiltinAssoc {
                        func,
                        args: arg_exprs,
                        type_args: vec![key, value],
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
        let value = self.coerce_to(value, &elem, args[0].span);
        // The payload type may only become concrete through unification
        // (e.g. `Some(&text)` with no annotation), so the storage check has to
        // run after inference; a pre-check on the unresolved variable would
        // accept a reference payload and crash when the owner moves.
        let resolved_elem = self.infer.resolve(&elem);
        self.validate_storage_type(&resolved_elem, args[0].span);
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
        self.validate_storage_type(&elem, span);
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
        let value = self.coerce_to(value, &ok_ty, args[0].span);
        // Validate after inference: `Ok(&x)` infers the payload as a
        // reference only while checking the argument.
        let resolved_ok = self.infer.resolve(&ok_ty);
        self.validate_storage_type(&resolved_ok, args[0].span);
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
        let value = self.coerce_to(value, &err_ty, args[0].span);
        // See check_result_ok_ctor: the error payload may be inferred.
        let resolved_err = self.infer.resolve(&err_ty);
        self.validate_storage_type(&resolved_err, args[0].span);
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
        let raw_ty = self.infer.resolve(&receiver_expr.ty);
        // Auto-dereference: method lookup and builtin dispatch happen on the
        // pointee type; the lowered receiver is dereferenced to match it.
        let receiver_ty = self.peel_refs(&raw_ty);
        if let ITy::Never = raw_ty {
            for arg in args {
                let _ = self.check_expr(arg, None);
            }
            return self.never_expr(span);
        }
        // Generic methods do not exist in this language version: a turbofish on
        // any method is rejected before method lookup, otherwise a non-generic
        // inherent method would silently ignore the type arguments. The
        // exceptions are .parse::<T>() and the iterator sugar methods, which
        // take the type arguments of their prelude function.
        if !turbofish.is_empty() && method != "parse" && !is_iterator_sugar_name(method) {
            let count = turbofish.len();
            self.error_code(
                format!(
                    "method {} takes 0 generic arguments but {} {} supplied",
                    method,
                    count,
                    if count == 1 { "was" } else { "were" }
                ),
                method_span,
                "E0107",
            );
            for arg in args {
                let _ = self.check_expr(arg, None);
            }
            return self.never_expr(span);
        }
        if method == "parse" && turbofish.len() > 1 {
            let count = turbofish.len();
            self.error_code(
                format!(
                    "method parse takes 1 generic argument but {} were supplied",
                    count
                ),
                method_span,
                "E0107",
            );
            for arg in args {
                let _ = self.check_expr(arg, None);
            }
            return self.never_expr(span);
        }
        let owner = match &receiver_ty {
            ITy::Struct(id) => Some(TypeKey::Struct(*id)),
            ITy::Enum(id) => Some(TypeKey::Enum(*id)),
            _ => None,
        };
        if let Some(owner) = owner
            && let Some(fn_id) = self.methods.get(&(owner, method.to_string())).copied()
        {
            if !self.method_visible(fn_id) {
                let type_name = self.type_name(owner);
                self.error_code(
                    format!("method {}::{} is private", type_name, method),
                    method_span,
                    "E0624",
                );
                for arg in args {
                    let _ = self.check_expr(arg, None);
                }
                return self.never_expr(span);
            }
            if self.fns[fn_id].self_kind.is_none() {
                let type_name = self.type_name(owner);
                self.error_code(
                    format!(
                        "associated function {}::{} cannot be called with method syntax",
                        type_name, method
                    ),
                    method_span,
                    "E0599",
                );
                for arg in args {
                    let _ = self.check_expr(arg, None);
                }
                return self.never_expr(span);
            }
            if matches!(self.fns[fn_id].self_kind, Some(SelfKind::RefMut)) {
                self.require_mutable_receiver(
                    &receiver_expr,
                    &raw_ty,
                    receiver.span,
                    "mutate through this receiver",
                );
            }
            let receiver_expr = self.wrap_derefs(receiver_expr);
            return self.check_direct_call(fn_id, Some(receiver_expr), args, span, hint);
        }
        // Only .parse::<T>() consumes a method turbofish: every other turbofish
        // form was rejected above (generic methods do not exist and the iterator
        // sugar owns its own type arguments).
        if !turbofish.is_empty()
            && method == "parse"
            && turbofish.len() == 1
            && matches!(receiver_ty, ITy::String | ITy::Str)
        {
            let target = self
                .resolve_ty(&turbofish[0], self.current_self_key())
                .unwrap_or_else(|| self.infer.fresh_var());
            let target = self.infer.resolve(&target);
            // v2.1: parse::<T>() is sugar for the standard-library method with
            // the same target type, which returns the typed ParseIntError or
            // ParseFloatError of that method. Routing through the impl keeps the
            // generic spelling and the explicit parse_i64/parse_f64 call in
            // agreement instead of returning the legacy String error.
            let std_name = match &target {
                ITy::Int(IntTy::I64) => Some("parse_i64"),
                ITy::Float(FloatTy::F64) => Some("parse_f64"),
                _ => None,
            };
            if let Some(std_name) = std_name {
                if let Some(result) = self.check_impl_method_call(
                    &receiver_expr,
                    &receiver_ty,
                    &raw_ty,
                    std_name,
                    method_span,
                    args,
                    span,
                    hint,
                ) {
                    return result;
                }
                // Programs checked without the standard library (the typeck
                // unit tests) have no ParseIntError/ParseFloatError type, so
                // the legacy builtin remains as a fallback with its String error.
                if let Some((builtin, ret)) = parse_alias(&target) {
                    let arg_exprs = self.check_call_args(&[], args, span);
                    self.apply_hint(&ret, hint);
                    let receiver_expr = self.wrap_derefs(receiver_expr);
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
        // Trait-object receivers dispatch through the vtable recorded for the
        // trait object; other receivers never match here, so the generic and
        // trait impl dispatch below keeps working for concrete types.
        if let Some(result) = self.check_dyn_method_call(
            receiver_expr.clone(),
            &raw_ty,
            &receiver_ty,
            method,
            args,
            receiver.span,
            span,
            hint,
        ) {
            return result;
        }
        // Trait and generic inherent methods were deferred during collection.
        // They are resolved before the derived .clone() and before builtin
        // methods so that a user type implementing Iterator<T> with its own
        // next()/count() keeps them; Vec/String have no trait impls, so their
        // builtin methods still resolve below.
        if let Some(result) = self.check_impl_method_call(
            &receiver_expr,
            &receiver_ty,
            &raw_ty,
            method,
            method_span,
            args,
            span,
            hint,
        ) {
            return result;
        }
        // .clone() is derived automatically for every type. It takes no
        // arguments and clones the referent through any number of references.
        // Types with drop glue cannot be cloned because the copy would run a
        // user destructor a second time.
        // String::to_string() is a convenient alias of the derived clone.
        if method == "clone" || (method == "to_string" && matches!(receiver_ty, ITy::String)) {
            let clone_ty = receiver_ty.clone();
            let arg_exprs = self.check_call_args(&[], args, span);
            self.apply_hint(&clone_ty, hint);
            if self.has_drop_glue(&clone_ty) {
                let described = self.describe_resolved(&clone_ty);
                self.error_code(
                    format!(
                        "type {} implements Drop and cannot be cloned; use references instead",
                        described
                    ),
                    method_span,
                    "E0277",
                );
            }
            let receiver_expr = self.wrap_derefs(receiver_expr);
            return TExpr::new(
                clone_ty,
                TExprKind::BuiltinMethod {
                    receiver: Box::new(receiver_expr),
                    method: HBuiltinMethod::Clone,
                    args: arg_exprs,
                },
                span,
            );
        }
        if is_combinator_method(&receiver_ty, method) {
            let receiver_expr = self.wrap_derefs(receiver_expr);
            return self.check_combinator_method(receiver_expr, method, args, span, hint);
        }
        // Iterator method sugar lowers to calls of the instantiated prelude
        // helpers. It runs after inherent/trait dispatch (so user methods win)
        // and before the builtin method table.
        if let Some(result) = self.check_iterator_sugar(
            &receiver_expr,
            &receiver_ty,
            method,
            method_span,
            turbofish,
            args,
            span,
            hint,
        ) {
            return result;
        }
        // HashMap methods are resolved here because their signatures depend on
        // the concrete key and value types (and on whether the value type can
        // be cloned).
        if let ITy::HashMap(key, value) = &receiver_ty
            && let Some(info) = self.hash_map_method(key, value, method, method_span)
        {
            if info.mutable_receiver {
                self.require_mutable_receiver(
                    &receiver_expr,
                    &raw_ty,
                    receiver.span,
                    "mutate through this receiver",
                );
            }
            let arg_exprs = self.check_call_args(&info.params, args, span);
            // The key and value types come from the map, but `insert` payloads
            // can still be inferred when the map type itself is not annotated.
            // Validate the resolved parameter types so references are rejected
            // inside containers, while top-level borrowed text stays allowed.
            for param in &info.params {
                let resolved = self.infer.resolve(param);
                self.validate_builtin_param(&resolved, span);
            }
            // MapGetWith returns the closure result by moving it out of the
            // map. References would point into the map and could dangle after
            // the map is mutated or dropped, so only storable owned values are
            // accepted.
            if matches!(info.method, HBuiltinMethod::MapGetWith) {
                let resolved_ret = self.infer.resolve(&info.ret);
                if let ITy::Option(result) = &resolved_ret {
                    self.validate_storage_type(result, method_span);
                }
            }
            self.apply_hint(&info.ret, hint);
            let receiver_expr = self.wrap_derefs(receiver_expr);
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
        if let Some(info) = builtin_method(&receiver_ty, method) {
            if info.mutable_receiver {
                self.require_mutable_receiver(
                    &receiver_expr,
                    &raw_ty,
                    receiver.span,
                    "mutate through this receiver",
                );
            }
            let arg_exprs = self.check_call_args(&info.params, args, span);
            // Storing methods (Vec::push/insert/extend, Rc::set, Box::set) take
            // owned payloads. The element type may be a variable before the
            // call, so validate the resolved parameter types after inference.
            for param in &info.params {
                let resolved = self.infer.resolve(param);
                self.validate_builtin_param(&resolved, span);
            }
            self.apply_hint(&info.ret, hint);
            let receiver_expr = self.wrap_derefs(receiver_expr);
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

    /// Require that the receiver of a mutating method can be mutated: either it
    /// is an assignable place, or it goes through a mutable reference.
    pub fn require_mutable_receiver(
        &mut self,
        receiver: &TExpr,
        receiver_ty: &ITy,
        span: Span,
        action: &str,
    ) -> bool {
        match self.ref_chain_mutability(receiver_ty) {
            Some(true) => true,
            Some(false) => {
                self.error_code(
                    format!(
                        "cannot {}: cannot mutate through a shared reference",
                        action
                    ),
                    span,
                    "E0596",
                );
                false
            }
            None => self.require_mutable_place(receiver, span, action),
        }
    }

    /// Validate a resolved parameter type of a built-in method.
    ///
    /// Parameters of the legacy fallback table are mostly owned payloads that
    /// the method stores, so references nested inside them are rejected. The
    /// one documented exception is a shared `&str` text input (for example
    /// `String::push_str` or `String::contains`): it is a borrowed view that
    /// the method only reads, and version 2.1 moved every standard-library text
    /// parameter to `&str`.
    fn validate_builtin_param(&mut self, ty: &ITy, span: Span) -> bool {
        if let ITy::Ref {
            mutable: false,
            inner,
        } = ty
            && matches!(self.infer.resolve(inner), ITy::Str)
        {
            return true;
        }
        self.validate_storage_type(ty, span)
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
            ITy::Rc(_) => &["get", "set", "strong_count"],
            ITy::Box(_) => &["get", "set"],
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
                            spec: crate::hir::FormatSpec::plain(),
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
            "assert_ne" => {
                // Rust's assert_ne!: two operands, optionally followed by a
                // format string and its arguments for the custom failure
                // message. A missing operand is an arity error; extra trailing
                // expressions are checked as format arguments.
                if m.args.len() < 2 {
                    self.error_code(
                        format!(
                            "assert_ne! takes 2 arguments but {} were supplied",
                            m.args.len()
                        ),
                        span,
                        "E0061",
                    );
                }
                let mut args = Vec::new();
                let operand_count = m.args.len().min(2);
                let mut first_ty = self.infer.fresh_var();
                for (index, arg) in m.args.iter().take(operand_count).enumerate() {
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
                if operand_count == 2 {
                    let resolved_first = self.infer.resolve(&first_ty);
                    if !self.equality_supported(&resolved_first) {
                        let described = self.describe_resolved(&resolved_first);
                        self.error_code(
                            format!(
                                "assert_ne! requires a type that supports !=, found {}",
                                described
                            ),
                            span,
                            "E0001",
                        );
                    }
                }
                let parts = if m.args.len() > 2 {
                    let format_call = ast::MacroCall {
                        name: m.name.clone(),
                        name_span: m.name_span,
                        args: m.args[2..].to_vec(),
                        span: m.args[2].span,
                    };
                    self.check_format_parts(&format_call, span)
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                TExpr::new(
                    ITy::unit(),
                    TExprKind::Macro(TMacroCall {
                        kind: HMacroKind::AssertNe,
                        parts,
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

    /// Whether `==`/!= can be applied to a value of this type. Scalars,
    /// String, references (compare the referent), Vec, Option, Result, tuples,
    /// user structs/enums, HashMap and Box (compare the payload) support
    /// equality; Rc compares by identity; function values do not support it.
    pub fn equality_supported(&self, ty: &ITy) -> bool {
        let mut structs = vec![false; self.structs.len()];
        let mut enums = vec![false; self.enums.len()];
        self.equality_supported_inner(ty, &mut structs, &mut enums)
    }

    fn equality_supported_inner(&self, ty: &ITy, structs: &mut [bool], enums: &mut [bool]) -> bool {
        match self.infer.resolve(ty) {
            ITy::Bool
            | ITy::Char
            | ITy::Int(_)
            | ITy::Float(_)
            | ITy::String
            | ITy::Never
            | ITy::Var(_) => true,
            // Rc<T> handles compare by identity (the generated code compares
            // the cell pointers), so the pointee never matters.
            ITy::Rc(_) => true,
            ITy::Box(inner) | ITy::Ref { inner, .. } | ITy::Vec(inner) | ITy::Option(inner) => {
                self.equality_supported_inner(&inner, structs, enums)
            }
            ITy::Result(ok, err) => {
                self.equality_supported_inner(&ok, structs, enums)
                    && self.equality_supported_inner(&err, structs, enums)
            }
            ITy::Tuple(elems) => elems
                .iter()
                .all(|elem| self.equality_supported_inner(elem, structs, enums)),
            // Two maps compare equal when their key/value pairs support ==.
            ITy::HashMap(key, value) => {
                self.equality_supported_inner(&key, structs, enums)
                    && self.equality_supported_inner(&value, structs, enums)
            }
            // Recursive types are compared through their Box/pointer layers;
            // cycle protection keeps the classification terminating.
            ITy::Struct(id) => {
                if structs.get(id).copied().unwrap_or(true) {
                    return true;
                }
                structs[id] = true;

                self.structs[id]
                    .fields
                    .iter()
                    .all(|field| self.equality_supported_inner(&field.ty, structs, enums))
            }
            ITy::Enum(id) => {
                if enums.get(id).copied().unwrap_or(true) {
                    return true;
                }
                enums[id] = true;
                self.enums[id].variants.iter().all(|variant| {
                    variant
                        .fields
                        .iter()
                        .all(|field| self.equality_supported_inner(field, structs, enums))
                })
            }
            _ => false,
        }
    }

    /// Classify an already type checked format argument so that format
    /// specifiers can be validated before literal variables are defaulted.
    fn format_value_kind(&self, ty: &ITy) -> FormatValueKind {
        match ty {
            ITy::String => FormatValueKind::String,
            ITy::Bool => FormatValueKind::Bool,
            ITy::Char => FormatValueKind::Char,
            ITy::Int(int_ty) => {
                if int_ty.is_signed() {
                    FormatValueKind::SignedInt
                } else {
                    FormatValueKind::UnsignedInt
                }
            }
            ITy::Float(_) => FormatValueKind::Float,
            ITy::Var(id) => match self.infer.var_kind(*id) {
                VarKind::IntLiteral => FormatValueKind::SignedInt,
                VarKind::FloatLiteral => FormatValueKind::Float,
                VarKind::General => FormatValueKind::Other,
            },
            _ => FormatValueKind::Other,
        }
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
                        FormatPiece::Hole { spec } => {
                            if arg_index < m.args.len() {
                                let expr = self.check_expr(&m.args[arg_index], None);
                                if let Some(message) =
                                    format_spec_error(&spec, self.format_value_kind(&expr.ty))
                                {
                                    self.error_code(message, m.args[arg_index].span, "E0001");
                                }
                                parts.push(TFormatPart {
                                    kind: TFormatPartKind::Arg {
                                        expr: Box::new(expr),
                                        spec,
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
