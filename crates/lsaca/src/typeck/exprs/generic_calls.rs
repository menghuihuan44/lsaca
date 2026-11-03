//! Call resolution for generic functions and associated functions.
//!
//! Generic calls cannot be resolved by the ordinary call path because the
//! callee does not exist as a concrete function yet: type arguments are
//! inferred from the checked argument values, the concrete instance is
//! instantiated, and the call is emitted as a DirectCall.

use std::collections::HashMap;

use crate::ast;
use crate::span::Span;

use crate::typeck::check::Checker;
use crate::typeck::generics::ImplInfo;
use crate::typeck::items::TypeKey;
use crate::typeck::itype::ITy;
use crate::typeck::modules::{BuiltinTypeKind, DefRef, ModuleId};
use crate::typeck::tree::{TExpr, TExprKind};
use crate::types::SelfKind;

impl<'a> Checker<'a> {
    /// How many generic parameters a definition has, for arity messages.
    fn generic_arity(generics: &[ast::GenericParam]) -> usize {
        generics.len()
    }

    /// Expression hint taken from a callable bound on a bare type parameter.
    ///
    /// For `fn apply<F: Fn(i64) -> i64>(f: F)` the parameter type is just `F`,
    /// so without this hint an unannotated closure argument would be checked
    /// with no expected signature and its parameter/return types would stay
    /// fresh variables that E0283 cannot resolve later. The bound is resolved in
    /// the same generic environment as the rest of the call, so bounds that
    /// mention other type parameters (`F: Fn(T) -> T`) share their inference
    /// variables.
    fn callable_bound_hint(
        &mut self,
        generics: &[ast::GenericParam],
        param_ty: &ast::TypeExpr,
        env: &HashMap<String, ITy>,
    ) -> Option<ITy> {
        let ast::TypeExpr::Path {
            name,
            generics: args,
            ..
        } = param_ty
        else {
            return None;
        };
        if !args.is_empty() {
            return None;
        }
        let param = generics.iter().find(|generic| &generic.name == name)?;
        for bound in &param.bounds {
            let ast::TypeExpr::Fn { params, ret, .. } = bound else {
                continue;
            };
            self.push_generic_env(env.clone());
            let mut resolved_params = Vec::with_capacity(params.len());
            let mut resolved_ret = None;
            let mut resolved = true;
            for bound_param in params {
                match self.resolve_ty(bound_param, None) {
                    Some(ty) => resolved_params.push(ty),
                    None => {
                        resolved = false;
                        break;
                    }
                }
            }
            if resolved {
                resolved_ret = self.resolve_ty(ret, None);
                if resolved_ret.is_none() {
                    resolved = false;
                }
            }
            self.pop_generic_env();
            if resolved && let Some(bound_ret) = resolved_ret {
                return Some(ITy::function(resolved_params, bound_ret));
            }
        }
        None
    }

    /// Check the argument expressions of a generic call, inferring the type
    /// arguments of the enclosing generic item from the value shapes.
    fn infer_callable_args(
        &mut self,
        generics: &[ast::GenericParam],
        params: &[ast::Param],
        args: &[ast::Expr],
        span: Span,
        module: ModuleId,
    ) -> (HashMap<String, ITy>, Vec<TExpr>, bool) {
        self.infer_callable_args_inner(generics, params, args, span, module)
    }

    /// [Self::infer_callable_args] with the callee's module in scope only while
    /// the declared parameter types are resolved to expression hints.
    ///
    /// The argument expressions belong to the call site, so they are checked in
    /// the caller's module. Resolving them in the callee's module made names
    /// from the caller invisible (for example a struct literal or a local
    /// function passed to `Box::new`), even though the callee lives in the
    /// standard library.
    fn infer_callable_args_inner(
        &mut self,
        generics: &[ast::GenericParam],
        params: &[ast::Param],
        args: &[ast::Expr],
        span: Span,
        module: ModuleId,
    ) -> (HashMap<String, ITy>, Vec<TExpr>, bool) {
        let caller_module = self.current_module;
        let mut env = HashMap::new();
        for param in generics {
            env.insert(param.name.clone(), self.infer.fresh_var());
        }
        let mut checked = Vec::with_capacity(args.len());
        if params.len() != args.len() {
            self.error_code(
                format!(
                    "this function takes {} argument(s) but {} were supplied",
                    params.len(),
                    args.len()
                ),
                span,
                "E0061",
            );
            for arg in args {
                checked.push(self.check_expr(arg, None));
            }
            return (env, checked, false);
        }
        // The generic parameters are in scope while the parameter types are
        // resolved to hints: a parameter type such as `T` must resolve to the
        // inference variable bound in `env` instead of leaking an E0412 at the
        // declaration site. All hints are resolved first, in the callee's
        // module, before any argument expression is checked.
        let mut hints = Vec::with_capacity(args.len());
        for param in params {
            self.current_module = module;
            self.push_generic_env(env.clone());
            let bound_hint = self.callable_bound_hint(generics, &param.ty, &env);
            let hint = match bound_hint {
                Some(hint) => Some(hint),
                None => self.resolve_param_hint(&param.ty),
            };
            self.pop_generic_env();
            self.current_module = caller_module;
            hints.push(hint);
        }
        let mut ok = true;
        for ((arg, param), hint) in args.iter().zip(params.iter()).zip(hints.iter()) {
            // Check the argument at the call site.
            let arg_expr = self.check_expr(arg, hint.as_ref());
            // Infer the generic arguments from the value shape against the
            // parameter type, which is written in the callee's module.
            self.current_module = module;
            self.push_generic_env(env.clone());
            if !self.infer_args_from_ty(&param.ty, &arg_expr.ty, &mut env, arg.span) {
                ok = false;
            }
            self.pop_generic_env();
            self.current_module = caller_module;
            checked.push(arg_expr);
        }
        (env, checked, ok)
    }

    /// Resolve the inferred generic argument bindings to concrete types,
    /// reporting E0283 when one is still unconstrained.
    fn resolve_env_args(
        &mut self,
        generics: &[ast::GenericParam],
        env: &HashMap<String, ITy>,
        span: Span,
    ) -> Option<Vec<ITy>> {
        let mut out = Vec::with_capacity(generics.len());
        for param in generics {
            let ty = match env.get(&param.name) {
                Some(ty) => self.infer.resolve(ty),
                None => {
                    self.error_code(
                        "type annotations needed for generic arguments".to_string(),
                        span,
                        "E0283",
                    );
                    return None;
                }
            };
            match self.normalize_instance_arg(&ty) {
                Some(ty) => out.push(ty),
                None => {
                    self.error_code(
                        "type annotations needed for generic arguments".to_string(),
                        span,
                        "E0283",
                    );
                    return None;
                }
            }
        }
        Some(out)
    }

    /// Build the DirectCall expression for an instantiated generic function,
    /// coercing the already checked arguments to the concrete parameter types.
    pub fn finish_generic_call(
        &mut self,
        fn_id: usize,
        receiver: Option<TExpr>,
        checked: Vec<TExpr>,
        shapes_ok: bool,
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let params = self.fns[fn_id].params();
        let ret = self.fns[fn_id].ret();
        let offset = if receiver.is_some() { 1 } else { 0 };
        let mut args = Vec::with_capacity(checked.len());
        for (index, arg) in checked.into_iter().enumerate() {
            let expected = match params.get(index + offset) {
                Some(expected) => expected.clone(),
                None => {
                    args.push(arg);
                    continue;
                }
            };
            if shapes_ok {
                let arg_span = arg.span;
                args.push(self.coerce_to(arg, &expected, arg_span));
            } else {
                args.push(arg);
            }
        }
        self.apply_hint(&ret, hint);
        TExpr::new(
            ret,
            TExprKind::DirectCall {
                fn_id,
                receiver: receiver.map(Box::new),
                args,
            },
            span,
        )
    }

    fn check_plain_args(&mut self, args: &[ast::Expr], span: Span) -> TExpr {
        for arg in args {
            let _ = self.check_expr(arg, None);
        }
        self.never_expr(span)
    }

    /// Builtin marker kind for a bare type name used as a static receiver.
    ///
    /// Standard-library marker declarations (`pub struct Vec<T> { internal: () }`)
    /// register as [DefRef::BuiltinType], so `Vec::new()` does not reach the
    /// ordinary associated-function dispatch. The legacy builtin constructor
    /// table is only a fallback for names no standard-library impl provides.
    pub fn builtin_marker_kind(&self, type_name: &str) -> Option<BuiltinTypeKind> {
        match self
            .lookup_type_scoped(type_name)
            .map(|binding| binding.def)
        {
            Some(DefRef::BuiltinType(kind)) => Some(kind),
            Some(_) => None,
            None => BuiltinTypeKind::from_name(type_name),
        }
    }

    /// Whether a resolved type has the shape of a builtin marker instantiation.
    fn marker_shape_matches(kind: BuiltinTypeKind, ty: &ITy) -> bool {
        matches!(
            (kind, ty),
            (BuiltinTypeKind::Vec, ITy::Vec(_))
                | (BuiltinTypeKind::String, ITy::String)
                | (BuiltinTypeKind::Option, ITy::Option(_))
                | (BuiltinTypeKind::Result, ITy::Result(_, _))
                | (BuiltinTypeKind::Rc, ITy::Rc(_))
                | (BuiltinTypeKind::Box, ITy::Box(_))
                | (BuiltinTypeKind::HashMap, ITy::HashMap(_, _))
                | (BuiltinTypeKind::Char, ITy::Char)
        )
    }

    /// Whether the generic argument of a builtin marker shape is an unsized
    /// type (`Box<dyn Trait>`, `Rc<str>`). Such a hint must not drive generic
    /// argument inference: the value is produced by a slice/trait-object
    /// coercion, not by the generic constructor itself.
    fn marker_argument_is_unsized(kind: BuiltinTypeKind, ty: &ITy) -> bool {
        match (kind, ty) {
            (_, ITy::Vec(inner))
            | (_, ITy::Rc(inner))
            | (_, ITy::Box(inner))
            | (_, ITy::Option(inner)) => inner.is_unsized(),
            (_, ITy::Result(ok, err)) | (_, ITy::HashMap(ok, err)) => {
                ok.is_unsized() || err.is_unsized()
            }
            _ => false,
        }
    }

    /// The concrete builtin marker instance a static receiver names, when it can
    /// be determined without checking the arguments (turbofish or expected type).
    fn marker_owner_instance(
        &mut self,
        kind: BuiltinTypeKind,
        explicit: &[ast::TypeExpr],
        hint: Option<&ITy>,
        span: Span,
    ) -> Option<ITy> {
        if !explicit.is_empty() {
            let args = self.resolve_concrete_type_args(explicit, span)?;
            return kind.resolve(&args);
        }
        if let Some(hint) = hint {
            let hint_ty = self.infer.resolve(hint);
            if !self.contains_unresolved(&hint_ty)
                && Self::marker_shape_matches(kind, &hint_ty)
                && !Self::marker_argument_is_unsized(kind, &hint_ty)
            {
                return Some(hint_ty);
            }
        }
        if kind.arity() == 0 {
            return kind.resolve(&[]);
        }
        None
    }

    /// Resolve `Vec::new()`, `String::from`, `HashMap::new()` and friends
    /// through the standard-library impl tables before the legacy builtin
    /// constructor table.
    ///
    /// Returns `None` when no impl provides the item (the caller then keeps the
    /// legacy builtin behaviour). Once an impl exists, its errors are reported
    /// instead of silently falling back.
    pub fn check_builtin_assoc_impl_call(
        &mut self,
        type_name: &str,
        explicit: &[ast::TypeExpr],
        item: &str,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> Option<TExpr> {
        let kind = self.builtin_marker_kind(type_name)?;
        let candidates = self.assoc_candidates(type_name, item);
        if candidates.is_empty() {
            return None;
        }
        let errors_before = self.diagnostics.error_count();
        if let Some(owner) = self.marker_owner_instance(kind, explicit, hint, span) {
            if let Some(fn_id) = self.dispatch_associated_fn(&owner, item, span) {
                return Some(self.check_direct_call(fn_id, None, args, span, hint));
            }
            // A found-but-invalid associated function must surface its errors
            // instead of falling back to the legacy constructor.
            if self.diagnostics.error_count() > errors_before {
                return Some(self.check_plain_args(args, span));
            }
            return None;
        }
        if self.diagnostics.error_count() > errors_before {
            return Some(self.check_plain_args(args, span));
        }
        if !explicit.is_empty() {
            // Wrong-arity turbofish keeps the legacy diagnostics.
            return None;
        }
        if candidates.len() > 1 {
            self.error_code(
                "multiple applicable items in scope".to_string(),
                span,
                "E0034",
            );
            return Some(self.check_plain_args(args, span));
        }
        let (impl_id, gfn_id) = candidates[0];
        let info = self.impls.get(impl_id)?.clone();
        let gfn = self.generic_fns.get(gfn_id)?.clone();
        let (env, checked, ok) =
            self.infer_callable_args(&info.generics, &gfn.decl.params, args, span, info.module);
        let mut resolved_args = Vec::with_capacity(info.generics.len());
        for param in &info.generics {
            match env.get(&param.name) {
                Some(ty) => resolved_args.push(self.infer.resolve(ty)),
                None => {
                    let var = self.infer.fresh_var();
                    resolved_args.push(var);
                }
            }
        }
        let mut concrete = Vec::with_capacity(resolved_args.len());
        let mut all_concrete = true;
        for ty in &resolved_args {
            match self.normalize_instance_arg(ty) {
                Some(normalized) => concrete.push(normalized),
                None => {
                    all_concrete = false;
                    break;
                }
            }
        }
        let fn_id = if all_concrete {
            self.validate_generic_bounds(&info.generics, &concrete, span, info.module);
            self.instantiate_generic_fn(gfn_id, concrete, span)
        } else {
            self.instantiate_generic_fn_deferred(gfn_id, resolved_args, span)
        };
        let Some(fn_id) = fn_id else {
            return Some(self.never_expr(span));
        };
        Some(self.finish_generic_call(fn_id, None, checked, ok, span, hint))
    }

    /// Resolve a call to a free generic function, with or without turbofish.
    pub fn check_generic_free_call(
        &mut self,
        def_id: usize,
        explicit: &[ast::TypeExpr],
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let def = match self.generic_fns.get(def_id) {
            Some(def) => def.clone(),
            None => return self.check_plain_args(args, span),
        };
        let arity = Self::generic_arity(&def.generics);
        if !explicit.is_empty() {
            if explicit.len() != arity {
                self.error_code(
                    format!(
                        "function {} takes {} type argument{} but {} were supplied",
                        def.name,
                        arity,
                        if arity == 1 { "" } else { "s" },
                        explicit.len()
                    ),
                    span,
                    "E0107",
                );
                return self.check_plain_args(args, span);
            }
            let type_args = match self.resolve_concrete_type_args(explicit, span) {
                Some(type_args) => type_args,
                None => return self.check_plain_args(args, span),
            };
            match self.instantiate_generic_fn(def_id, type_args, span) {
                Some(fn_id) => return self.check_direct_call(fn_id, None, args, span, hint),
                None => return self.check_plain_args(args, span),
            }
        }
        if def.decl.params.len() != args.len() {
            self.error_code(
                format!(
                    "this function takes {} argument(s) but {} were supplied",
                    def.decl.params.len(),
                    args.len()
                ),
                span,
                "E0061",
            );
            return self.check_plain_args(args, span);
        }
        let (env, checked, ok) =
            self.infer_callable_args(&def.generics, &def.decl.params, args, span, def.module);
        let type_args = match self.resolve_env_args(&def.generics, &env, span) {
            Some(type_args) => type_args,
            None => {
                return TExpr::new(ITy::Never, TExprKind::Break, span);
            }
        };
        match self.instantiate_generic_fn(def_id, type_args, span) {
            Some(fn_id) => self.finish_generic_call(fn_id, None, checked, ok, span, hint),
            None => TExpr::new(ITy::Never, TExprKind::Break, span),
        }
    }

    /// Whether an impl block's self type is rooted at the given type name.
    fn impl_targets_name(&self, info: &ImplInfo, type_name: &str) -> bool {
        match &info.self_ty {
            ast::TypeExpr::Path { name, .. } => name == type_name,
            _ => false,
        }
    }

    /// Find associated-function candidates (methods without a receiver) for a
    /// named type.
    fn assoc_candidates(&self, type_name: &str, item: &str) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for info in &self.impls {
            if info
                .trait_ref
                .as_ref()
                .map(|trait_ref| trait_ref.name == "Drop")
                .unwrap_or(false)
            {
                continue;
            }
            if !self.impl_targets_name(info, type_name) {
                continue;
            }
            let method_index = match info.methods.iter().position(|method| method.name == item) {
                Some(index) => index,
                None => continue,
            };
            let gfn_id = info.methods[method_index].gfn_id;
            let self_kind = match self.generic_fns.get(gfn_id) {
                Some(def) => def.decl.self_kind,
                None => continue,
            };
            if self_kind.is_some() {
                continue;
            }
            out.push((info.id, gfn_id));
        }
        out
    }

    /// Resolve an associated function call of a generic type, inferring the
    /// impl type arguments from the arguments when no turbofish is written.
    pub fn check_generic_assoc_call(
        &mut self,
        type_name: &str,
        explicit: &[ast::TypeExpr],
        item: &str,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        if !explicit.is_empty() {
            let owner = match self.resolve_generic_instance(type_name, explicit, span) {
                Some(owner) => owner,
                None => return self.check_plain_args(args, span),
            };
            return self.check_associated_on_instance(&owner, item, args, span, hint);
        }
        if let Some(owner) = self.hinted_instance_of(type_name, hint) {
            return self.check_associated_on_instance(&owner, item, args, span, hint);
        }
        let candidates = self.assoc_candidates(type_name, item);
        if candidates.len() > 1 {
            self.error_code(
                "multiple applicable items in scope".to_string(),
                span,
                "E0034",
            );
            return self.check_plain_args(args, span);
        }
        let (impl_id, gfn_id) = match candidates.first() {
            Some(candidate) => *candidate,
            None => {
                self.error_code(
                    format!(
                        "no function or associated item named {} found for type {}",
                        item, type_name
                    ),
                    span,
                    "E0599",
                );
                return self.check_plain_args(args, span);
            }
        };
        let info = match self.impls.get(impl_id) {
            Some(info) => info.clone(),
            None => return self.check_plain_args(args, span),
        };
        let gfn = match self.generic_fns.get(gfn_id) {
            Some(def) => def.clone(),
            None => return self.check_plain_args(args, span),
        };
        let (env, checked, ok) =
            self.infer_callable_args(&info.generics, &gfn.decl.params, args, span, info.module);
        let impl_args = match self.resolve_env_args(&info.generics, &env, span) {
            Some(impl_args) => impl_args,
            None => return TExpr::new(ITy::Never, TExprKind::Break, span),
        };
        self.validate_generic_bounds(&info.generics, &impl_args, span, info.module);
        let fn_id = match self.instantiate_generic_fn(gfn_id, impl_args, span) {
            Some(fn_id) => fn_id,
            None => return TExpr::new(ITy::Never, TExprKind::Break, span),
        };
        self.finish_generic_call(fn_id, None, checked, ok, span, hint)
    }

    /// Instantiate a generic type from explicit turbofish arguments.
    pub fn resolve_generic_instance(
        &mut self,
        type_name: &str,
        explicit: &[ast::TypeExpr],
        span: Span,
    ) -> Option<ITy> {
        if let Some(def_id) = self.lookup_generic_struct(type_name) {
            let args = self.resolve_concrete_type_args(explicit, span)?;
            return self.instantiate_struct(def_id, args, span).map(ITy::Struct);
        }
        if let Some(def_id) = self.lookup_generic_enum(type_name) {
            let args = self.resolve_concrete_type_args(explicit, span)?;
            return self.instantiate_enum(def_id, args, span).map(ITy::Enum);
        }
        None
    }

    /// The struct or enum instance named by the call when the expected type
    /// already determines the generic arguments.
    pub fn hinted_instance_of(&mut self, type_name: &str, hint: Option<&ITy>) -> Option<ITy> {
        let hint = self.infer.resolve(hint?);
        match &hint {
            ITy::Struct(id) => {
                let info = self.structs.get(*id)?;
                if let Some(instance) = &info.generic
                    && self.generic_structs.get(instance.def_id)?.name == type_name
                {
                    return Some(ITy::Struct(*id));
                }
                None
            }
            ITy::Enum(id) => {
                let info = self.enums.get(*id)?;
                if let Some(instance) = &info.generic
                    && self.generic_enums.get(instance.def_id)?.name == type_name
                {
                    return Some(ITy::Enum(*id));
                }
                None
            }
            _ => None,
        }
    }

    /// Check Self::item(...) and Type::item(...) for a known type instance.
    pub fn check_associated_on_instance(
        &mut self,
        owner_ty: &ITy,
        item: &str,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let owner_key = match owner_ty {
            ITy::Struct(id) => Some(TypeKey::Struct(*id)),
            ITy::Enum(id) => Some(TypeKey::Enum(*id)),
            _ => None,
        };
        if let ITy::Enum(enum_id) = owner_ty {
            self.ensure_enum_variants(*enum_id);
            if let Some(variant) = self.find_variant(*enum_id, item) {
                return self.finish_enum_variant_call(*enum_id, variant, args, span, hint);
            }
        }
        if let Some(owner_key) = owner_key
            && let Some(fn_id) = self.methods.get(&(owner_key, item.to_string())).copied()
        {
            if !self.method_visible(fn_id) {
                let owner_name = self.type_name(owner_key);
                self.error_code(
                    format!("associated function {}::{} is private", owner_name, item),
                    span,
                    "E0624",
                );
                return self.check_plain_args(args, span);
            }
            if self.fns[fn_id].self_kind.is_none() {
                return self.check_direct_call(fn_id, None, args, span, hint);
            }
            let owner_name = self.type_name(owner_key);
            self.error_code(
                format!(
                    "method {}::{} cannot be called as an associated function",
                    owner_name, item
                ),
                span,
                "E0599",
            );
            return self.check_plain_args(args, span);
        }
        if let Some(fn_id) = self.dispatch_associated_fn(owner_ty, item, span) {
            return self.check_direct_call(fn_id, None, args, span, hint);
        }
        let described = self.describe_resolved(owner_ty);
        self.error_code(
            format!(
                "no function or associated item named {} found for type {}",
                item, described
            ),
            span,
            "E0599",
        );
        self.check_plain_args(args, span)
    }

    /// Resolve Self::item(...) inside a method body.
    pub fn check_self_assoc_call(
        &mut self,
        item: &str,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> Option<TExpr> {
        let self_ty = self.current_self_ty()?;
        Some(self.check_associated_on_instance(&self_ty, item, args, span, hint))
    }

    /// Check a method call against trait and generic inherent impls.
    ///
    /// The argument list mirrors the call-site data that the dispatcher needs;
    /// bundling it into a struct would obscure this hot path.
    #[allow(clippy::too_many_arguments)]
    pub fn check_impl_method_call(
        &mut self,
        receiver_expr: &TExpr,
        receiver_ty: &ITy,
        raw_ty: &ITy,
        method: &str,
        method_span: Span,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> Option<TExpr> {
        let errors_before = self.diagnostics.error_count();
        let fn_id = match self.dispatch_impl_method(receiver_ty, method, method_span) {
            Some(fn_id) => fn_id,
            None => {
                if self.diagnostics.error_count() > errors_before {
                    for arg in args {
                        let _ = self.check_expr(arg, None);
                    }
                    return Some(self.never_expr(span));
                }
                return None;
            }
        };
        if matches!(self.fns[fn_id].self_kind, Some(SelfKind::RefMut)) {
            self.require_mutable_receiver(
                receiver_expr,
                raw_ty,
                receiver_expr.span,
                "mutate through this receiver",
            );
        }
        let receiver_expr = self.wrap_derefs(receiver_expr.clone());
        Some(self.check_direct_call(fn_id, Some(receiver_expr), args, span, hint))
    }

    /// Whether a type name is an associated function of a generic definition.
    pub fn generic_assoc_candidates_exist(&self, type_name: &str, item: &str) -> bool {
        !self.assoc_candidates(type_name, item).is_empty()
    }

    /// Resolve `E::Variant(...)` for a generic enum. The concrete enum instance
    /// comes from the turbofish arguments or from the expected type: a variant
    /// constructor has no self type from which the parameters could be
    /// inferred, so an unconstrained call is E0282.
    pub fn check_generic_enum_call(
        &mut self,
        type_name: &str,
        explicit: &[ast::TypeExpr],
        item: &str,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        if !explicit.is_empty() {
            let owner = match self.resolve_generic_instance(type_name, explicit, span) {
                Some(owner) => owner,
                None => return self.check_plain_args(args, span),
            };
            return self.check_associated_on_instance(&owner, item, args, span, hint);
        }
        if let Some(owner) = self.hinted_instance_of(type_name, hint) {
            return self.check_associated_on_instance(&owner, item, args, span, hint);
        }
        if self.generic_assoc_candidates_exist(type_name, item) {
            return self.check_generic_assoc_call(type_name, explicit, item, args, span, hint);
        }
        let def_id = match self.lookup_generic_enum(type_name) {
            Some(def_id) => def_id,
            None => return self.check_plain_args(args, span),
        };
        let variant_exists = self
            .generic_enums
            .get(def_id)
            .map(|def| def.decl.variants.iter().any(|variant| variant.name == item))
            .unwrap_or(false);
        if variant_exists {
            self.error_code("type annotations needed".to_string(), span, "E0282");
        } else {
            self.error_code(
                format!("enum {} has no variant named {}", type_name, item),
                span,
                "E0599",
            );
        }
        self.check_plain_args(args, span)
    }
}
