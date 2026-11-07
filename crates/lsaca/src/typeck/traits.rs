//! Trait declarations, trait impls and trait dispatch.
//!
//! v1.5 traits are signature-only interfaces. Dispatch is demand driven: an
//! impl is type checked and signature-verified the first time it is used, and
//! its methods are instantiated as ordinary concrete functions (DirectCall in
//! the HIR). There are no trait objects.

use std::collections::HashMap;

use super::check::Checker;
use super::generics::{GFnDef, GenericOwner, ImplInfo, ImplMethod};
use super::items::{only_type_args, only_type_generics, sanitize_c_ident};
use super::itype::ITy;
use super::modules::{DefRef, ModuleId};
use crate::ast::{self, GenericParam, ImplBlock, TraitDecl, TraitMethod, TraitRef};
use crate::diag::Diagnostic;
use crate::span::Span;
use crate::types::SelfKind;

/// A trait declaration.
#[derive(Clone, Debug)]
pub struct TraitInfo {
    pub name: String,
    pub module: ModuleId,
    pub generics: Vec<GenericParam>,
    pub methods: Vec<TraitMethod>,
}

impl<'a> Checker<'a> {
    // -----------------------------------------------------------------------
    // Registration
    // -----------------------------------------------------------------------

    /// Register a trait declaration, validating its shape.
    pub fn register_trait_decl(&mut self, decl: &TraitDecl) {
        let module = self.current_module;
        let vis = self.item_visibility(decl.vis);
        if let Some(previous) = self
            .lookup_type_in(module, &decl.name)
            .map(|binding| binding.span)
        {
            self.report_duplicate_type(&decl.name, decl.name_span, previous);
            return;
        }
        let mut seen: HashMap<String, Span> = HashMap::new();
        for param in &decl.generics {
            if let Some(previous) = seen.get(&param.name) {
                let previous = *previous;
                let diag = Diagnostic::error(
                    format!(
                        "the name '{}' is already used for a generic parameter",
                        param.name
                    ),
                    param.name_span,
                )
                .with_code("E0403")
                .with_label(previous, format!("previous use of '{}' here", param.name));
                self.diagnostics.push(diag);
            }
            seen.insert(param.name.clone(), param.name_span);
        }
        let mut method_names: HashMap<String, Span> = HashMap::new();
        for method in &decl.methods {
            if let Some(previous) = method_names.get(&method.name) {
                let previous = *previous;
                let diag = Diagnostic::error(
                    format!("the name '{}' is defined multiple times", method.name),
                    method.name_span,
                )
                .with_code("E0428")
                .with_label(
                    previous,
                    format!("previous definition of '{}' here", method.name),
                );
                self.diagnostics.push(diag);
                continue;
            }
            method_names.insert(method.name.clone(), method.name_span);
            // v2.2: a trait item without a receiver is an associated function,
            // which the From trait requires ("fn from(value: T) -> Self"). A
            // written receiver must still be the first parameter.
            if method.self_kind.is_some()
                && !method.params.is_empty()
                && method.params[0].name != "self"
            {
                self.error_code(
                    format!("trait method '{}' must take a self receiver", method.name),
                    method.name_span,
                    "E0001",
                );
            }
        }
        let id = self.traits.len();
        self.traits.push(TraitInfo {
            name: decl.name.clone(),
            module,
            generics: only_type_generics(&decl.generics),
            methods: decl.methods.clone(),
        });
        self.insert_type_binding(module, &decl.name, DefRef::Trait(id), decl.name_span, vis);
    }

    /// Register a trait impl block (not a Drop impl, which keeps its v1.4
    /// short-circuit path for concrete types).
    pub fn register_trait_impl_block(&mut self, block: &ImplBlock, trait_ref: &TraitRef) {
        let trait_id = match self.lookup_trait(&trait_ref.name) {
            Some(id) => id,
            None => {
                self.error_code(
                    format!("cannot find trait '{}' in this scope", trait_ref.name),
                    trait_ref.name_span,
                    "E0405",
                );
                return;
            }
        };
        let impl_generics = only_type_generics(&block.generics);
        let trait_args = only_type_args(&trait_ref.args);
        let trait_arity = self.traits[trait_id].generics.len();
        if trait_args.len() != trait_arity {
            self.error_code(
                format!(
                    "trait {} takes {} type argument{} but {} were supplied",
                    trait_ref.name,
                    trait_arity,
                    if trait_arity == 1 { "" } else { "s" },
                    trait_args.len()
                ),
                trait_ref.span,
                "E0107",
            );
            return;
        }
        // A trait can only be implemented for a user-defined struct or enum.
        // Builtin types (String, Vec<T>, tuples, i64, ...) cannot gain trait
        // methods in user code: the runtime has no place to dispatch them.
        // The standard library is exempt so it can implement traits such as
        // Iterator for its builtin types.
        if !self.impl_target_is_user_type(block)
            && self.impl_target_builtin_kind(block).is_none()
            && !self.impl_target_unsized_builtin_kind(block)
        {
            self.report_invalid_impl_target(block);
            return;
        }
        self.validate_impl_generics(&impl_generics);
        self.reject_generic_methods(block);
        let impl_id = self.impls.len();
        let mut methods = Vec::new();
        for func in &block.fns {
            let gfn_id = self.register_generic_method(
                impl_id,
                &impl_generics,
                func,
                GenericOwner::TraitImpl(impl_id),
                format!(
                    "lsaca_fn_{}_{}",
                    sanitize_c_ident(&trait_ref.name),
                    sanitize_c_ident(&func.name)
                ),
                format!("{}::{}", trait_ref.name, func.name),
            );
            methods.push(ImplMethod {
                name: func.name.clone(),
                gfn_id,
            });
        }
        // Trait default bodies: every omitted default is registered as a
        // synthetic impl method so calls on a receiver dispatch to this impl.
        // Written methods were registered first, so an override always wins.
        let trait_info = self.traits[trait_id].clone();
        let written: Vec<String> = block.fns.iter().map(|func| func.name.clone()).collect();
        for method in &trait_info.methods {
            if method.body.is_none() || written.iter().any(|name| name == &method.name) {
                continue;
            }
            let func = self.synthesize_default_method(block, trait_ref, trait_id, method);
            let gfn_id = self.register_generic_method(
                impl_id,
                &impl_generics,
                &func,
                GenericOwner::TraitImpl(impl_id),
                format!(
                    "lsaca_fn_{}_{}",
                    sanitize_c_ident(&trait_ref.name),
                    sanitize_c_ident(&method.name)
                ),
                format!("{}::{}", trait_ref.name, method.name),
            );
            methods.push(ImplMethod {
                name: method.name.clone(),
                gfn_id,
            });
        }
        self.impls.push(ImplInfo {
            id: impl_id,
            module: self.current_module,
            generics: impl_generics.clone(),
            self_ty: block.ty.clone(),
            trait_ref: Some(trait_ref.clone()),
            methods,
            span: block.span,
            late_checked: false,
        });
    }

    pub fn validate_impl_generics(&mut self, generics: &[GenericParam]) {
        let mut seen: HashMap<String, Span> = HashMap::new();
        for param in generics {
            if let Some(previous) = seen.get(&param.name) {
                let previous = *previous;
                let diag = Diagnostic::error(
                    format!(
                        "the name '{}' is already used for a generic parameter",
                        param.name
                    ),
                    param.name_span,
                )
                .with_code("E0403")
                .with_label(previous, format!("previous use of '{}' here", param.name));
                self.diagnostics.push(diag);
            }
            seen.insert(param.name.clone(), param.name_span);
        }
    }

    pub fn reject_generic_methods(&mut self, block: &ImplBlock) {
        for func in &block.fns {
            if !func.generics.is_empty() {
                self.error_code(
                    format!("generic methods are not supported (method '{}')", func.name),
                    func.name_span,
                    "E0001",
                );
            }
        }
    }

    /// Register one impl method as a generic function definition.
    pub fn register_generic_method(
        &mut self,
        _impl_id: usize,
        impl_generics: &[GenericParam],
        func: &ast::FnDecl,
        owner: GenericOwner,
        c_name_base: String,
        display_base: String,
    ) -> usize {
        let def_id = self.generic_fns.len();
        self.generic_fns.push(GFnDef {
            name: func.name.clone(),
            name_span: func.name_span,
            span: func.span,
            generics: impl_generics.to_vec(),
            decl: func.clone(),
            owner,
            instances: HashMap::new(),
            origin: self.collecting_origin,
            module: self.current_module,
            c_name_base,
            display_base,
        });
        def_id
    }

    /// Build the synthetic impl method for an omitted trait default body.
    ///
    /// The trait method signature is written in terms of Self and the trait's
    /// generic parameters; in the impl context Self becomes the impl target type
    /// and the trait parameters become the arguments of the impl's trait
    /// reference. The body is cloned as written and checked in the impl
    /// context, so a call like self.other() resolves to this impl.
    fn synthesize_default_method(
        &self,
        block: &ImplBlock,
        trait_ref: &TraitRef,
        trait_id: usize,
        method: &TraitMethod,
    ) -> ast::FnDecl {
        let mut args: HashMap<String, ast::TypeExpr> = HashMap::new();
        for (index, param) in self.traits[trait_id].generics.iter().enumerate() {
            if let Some(arg) = trait_ref.args.get(index) {
                args.insert(param.name.clone(), arg.clone());
            }
        }
        let mut params = Vec::with_capacity(method.params.len());
        for (index, param) in method.params.iter().enumerate() {
            // The receiver of a trait method is written as Self (or &Self),
            // while impl methods store the plain self type and let self_kind
            // add the reference layer.
            // Only a receiver parameter is replaced by the impl self type; the
            // parameters of an associated function all come from the
            // declaration.
            let ty = if index == 0 && method.self_kind.is_some() {
                block.ty.clone()
            } else {
                substitute_default_ty(&param.ty, &block.ty, &args)
            };
            params.push(ast::Param {
                name: param.name.clone(),
                name_span: param.name_span,
                mutable: param.mutable,
                ty,
            });
        }
        ast::FnDecl {
            name: method.name.clone(),
            name_span: method.name_span,
            vis: ast::Visibility::Private,
            generics: block.generics.clone(),
            params,
            ret: method
                .ret
                .as_ref()
                .map(|ret| substitute_default_ty(ret, &block.ty, &args)),
            body: method.body.clone(),
            is_extern: false,
            self_kind: method.self_kind,
            span: method.span,
        }
    }

    // -----------------------------------------------------------------------
    // Bound checking
    // -----------------------------------------------------------------------

    /// Whether a concrete type satisfies Trait(args) by matching a trait impl.
    pub fn trait_bound_satisfied(&mut self, ty: &ITy, trait_id: usize, trait_args: &[ITy]) -> bool {
        let trait_name = match self.traits.get(trait_id) {
            Some(info) => info.name.clone(),
            None => return false,
        };
        let candidates: Vec<usize> = self
            .impls
            .iter()
            .filter(|info| {
                info.trait_ref
                    .as_ref()
                    .map(|trait_ref| trait_ref.name == trait_name)
                    .unwrap_or(false)
            })
            .map(|info| info.id)
            .collect();
        for impl_id in candidates {
            let snapshot = self.infer.clone();
            let matched = self.match_trait_impl(impl_id, ty, trait_args).is_some();
            self.infer = snapshot;
            if matched {
                return true;
            }
        }
        false
    }

    /// Try to match an impl against a concrete receiver type and trait
    /// arguments. Returns the impl's generic arguments on success. Caller must
    /// snapshot the inference state when probing.
    ///
    /// Matching is structural and non-reporting: the impl's generic parameters
    /// are bound by walking the self type against the receiver, then the trait
    /// arguments are resolved from those bindings and unified with the
    /// requested trait arguments.
    fn match_trait_impl(
        &mut self,
        impl_id: usize,
        receiver: &ITy,
        trait_args: &[ITy],
    ) -> Option<Vec<ITy>> {
        let info = self.impls.get(impl_id)?.clone();
        let trait_ref = info.trait_ref.as_ref()?.clone();
        let mut env = HashMap::new();
        for param in &info.generics {
            let var = self.infer.fresh_var();
            env.insert(param.name.clone(), var);
        }
        let quiet = self.probe_quiet;
        self.probe_quiet = true;
        let matched = self.with_module(info.module, |checker| {
            checker.infer_args_from_ty(&info.self_ty, receiver, &mut env, info.span)
        });
        self.probe_quiet = quiet;
        if !matched {
            return None;
        }
        let resolved_trait_args = self.resolve_trait_args(&trait_ref, &env, info.module)?;
        if resolved_trait_args.len() != trait_args.len() {
            return None;
        }
        for (resolved, expected) in resolved_trait_args.iter().zip(trait_args.iter()) {
            if self.infer.unify(resolved, expected).is_err() {
                return None;
            }
        }
        let mut impl_args = Vec::with_capacity(info.generics.len());
        for param in &info.generics {
            let ty = self.infer.resolve(env.get(&param.name)?);
            impl_args.push(ty);
        }
        self.normalize_args(&impl_args)
    }

    /// Resolve the trait arguments of an impl under the current impl-parameter
    /// bindings. Unlike [Self::trait_impl_args] this tolerates inference
    /// variables, because matching happens before the arguments are concrete.
    fn resolve_trait_args(
        &mut self,
        trait_ref: &ast::TraitRef,
        env: &HashMap<String, ITy>,
        module: ModuleId,
    ) -> Option<Vec<ITy>> {
        self.push_generic_env(env.clone());
        let mut out = Vec::with_capacity(trait_ref.args.len());
        for expr in &trait_ref.args {
            match self.with_module(module, |checker| checker.resolve_ty(expr, None)) {
                Some(ty) => out.push(self.infer.resolve(&ty)),
                None => {
                    self.pop_generic_env();
                    return None;
                }
            }
        }
        self.pop_generic_env();
        Some(out)
    }

    /// Resolve the trait arguments of an impl instantiated with args.
    pub fn trait_impl_args(&mut self, impl_id: usize, args: &[ITy]) -> Option<Vec<ITy>> {
        let info = self.impls.get(impl_id)?.clone();
        let trait_ref = info.trait_ref.as_ref()?.clone();
        let mut env = HashMap::new();
        for (param, arg) in info.generics.iter().zip(args.iter()) {
            env.insert(param.name.clone(), arg.clone());
        }
        self.push_generic_env(env);
        let mut out = Vec::with_capacity(trait_ref.args.len());
        let mut failed = false;
        for expr in &trait_ref.args {
            match self.with_module(info.module, |checker| checker.resolve_ty(expr, None)) {
                Some(ty) => out.push(ty),
                None => failed = true,
            }
        }
        self.pop_generic_env();
        if failed {
            return None;
        }
        // Keep inference variables open: callers (for example the iterator
        // sugar) discover the item type of an adapter while the surrounding
        // method chain is still being checked, so a literal variable must not
        // be pinned to its default type here. Normalization happens at the
        // call site when a concrete instantiation is required.
        Some(out.iter().map(|ty| self.infer.resolve(ty)).collect())
    }

    // -----------------------------------------------------------------------
    // Late coherence checks
    // -----------------------------------------------------------------------

    /// Check E0046 (missing trait item) and E0407 (method not in trait) the
    /// first time an impl is used.
    pub fn check_impl_coherence(&mut self, impl_id: usize, span: Span) {
        let info = match self.impls.get(impl_id) {
            Some(info) => info.clone(),
            None => return,
        };
        if info.late_checked {
            return;
        }
        if let Some(impl_info) = self.impls.get_mut(impl_id) {
            impl_info.late_checked = true;
        }
        let trait_ref = match &info.trait_ref {
            Some(trait_ref) => trait_ref.clone(),
            None => return,
        };
        let trait_id = match self.lookup_trait(&trait_ref.name) {
            Some(id) => id,
            None => return,
        };
        let trait_info = self.traits[trait_id].clone();
        let trait_methods: Vec<String> = trait_info
            .methods
            .iter()
            .map(|method| method.name.clone())
            .collect();
        let impl_methods: Vec<String> = info
            .methods
            .iter()
            .map(|method| method.name.clone())
            .collect();
        // Methods with a default body are not required: an omitted default is
        // registered as a synthetic impl method during impl registration.
        let required: Vec<&String> = trait_info
            .methods
            .iter()
            .filter(|method| method.body.is_none())
            .map(|method| &method.name)
            .collect();
        let mut missing = Vec::new();
        for name in required {
            if !impl_methods.contains(name) {
                missing.push(name.clone());
            }
        }
        if !missing.is_empty() {
            let listed = missing.join(", ");
            self.error_code(
                format!("not all trait items implemented, missing: {}", listed),
                span,
                "E0046",
            );
        }
        for name in &impl_methods {
            if !trait_methods.contains(name) {
                self.error_code(
                    format!(
                        "method '{}' is not a member of trait '{}'",
                        name, trait_info.name
                    ),
                    span,
                    "E0407",
                );
            }
        }
    }

    /// Verify that the impl method name has a signature compatible with the
    /// trait declaration under substitution (E0053).
    pub(crate) fn verify_trait_method_signature(
        &mut self,
        impl_id: usize,
        args: &[ITy],
        name: &str,
        span: Span,
        fn_id: usize,
    ) {
        let info = match self.impls.get(impl_id) {
            Some(info) => info.clone(),
            None => return,
        };
        let trait_ref = match &info.trait_ref {
            Some(trait_ref) => trait_ref.clone(),
            None => return,
        };
        let trait_id = match self.lookup_trait(&trait_ref.name) {
            Some(id) => id,
            None => return,
        };
        let trait_info = self.traits[trait_id].clone();
        let trait_method = match trait_info.methods.iter().find(|method| method.name == name) {
            Some(method) => method.clone(),
            None => return,
        };
        let impl_self = match self.impl_self_ty(impl_id, args) {
            Some(ty) => ty,
            None => return,
        };
        let trait_args = match self.trait_impl_args(impl_id, args) {
            Some(trait_args) => trait_args,
            None => return,
        };
        let mut env = HashMap::new();
        for (param, arg) in trait_info.generics.iter().zip(trait_args.iter()) {
            env.insert(param.name.clone(), arg.clone());
        }
        env.insert("Self".to_string(), impl_self.clone());
        // Trait method signatures are declared by the trait: resolve them in
        // the trait's module even though dispatch happens from user code.
        let previous_module = self.current_module;
        self.current_module = trait_info.module;
        self.push_generic_env(env);
        let mut trait_params: Vec<ITy> = Vec::with_capacity(trait_method.params.len());
        for param in &trait_method.params {
            let ty = match self.resolve_ty(&param.ty, None) {
                Some(ty) => ty,
                None => self.infer.fresh_var(),
            };
            trait_params.push(ty);
        }
        // Trait declarations store the receiver type with its reference already
        // applied (`&mut self` is `Ref{Self}`), while impl method signatures
        // are built from the plain self type and add the receiver reference
        // themselves. Normalize the declaration side to exactly one reference
        // layer for `&self`/`&mut self` so signatures compare correctly.
        if let Some(kind) = trait_method.self_kind
            && let Some(receiver_ty) = trait_params.first().cloned()
        {
            let mut pointee = receiver_ty;
            while let ITy::Ref { inner, .. } = pointee.clone() {
                pointee = *inner;
            }
            trait_params[0] = match kind {
                SelfKind::Ref => ITy::Ref {
                    mutable: false,
                    inner: Box::new(pointee),
                },
                SelfKind::RefMut => ITy::Ref {
                    mutable: true,
                    inner: Box::new(pointee),
                },
                SelfKind::Value => pointee,
            };
        }
        let trait_ret = match &trait_method.ret {
            Some(ret_ty) => match self.resolve_ty(ret_ty, None) {
                Some(ty) => ty,
                None => self.infer.fresh_var(),
            },
            None => ITy::Unit,
        };
        self.pop_generic_env();
        self.current_module = previous_module;
        let fn_sig = match self.fns.get(fn_id).and_then(|info| info.sig.clone()) {
            Some(sig) => sig,
            None => return,
        };
        let mut mismatch: Option<(String, String)> = None;
        if fn_sig.params.len() != trait_params.len() {
            mismatch = Some((
                format!("{} parameter(s) in the trait", trait_params.len()),
                format!("{} parameter(s) here", fn_sig.params.len()),
            ));
        } else {
            for (actual, expected) in fn_sig.params.iter().zip(trait_params.iter()) {
                if !self.types_unify_probe(actual, expected) {
                    mismatch = Some((
                        self.describe_resolved(expected),
                        self.describe_resolved(actual),
                    ));
                    break;
                }
            }
        }
        if mismatch.is_none() && !self.types_unify_probe(&fn_sig.ret, &trait_ret) {
            mismatch = Some((
                self.describe_resolved(&trait_ret),
                self.describe_resolved(&fn_sig.ret),
            ));
        }
        if let Some((expected, found)) = mismatch {
            let diag = Diagnostic::error(
                format!(
                    "method '{}' has an incompatible signature for trait '{}'",
                    name, trait_info.name
                ),
                span,
            )
            .with_code("E0053")
            .with_span_label(span)
            .with_note(format!("expected {}, found {}", expected, found));
            self.diagnostics.push(diag);
        }
    }

    // -----------------------------------------------------------------------
    // Dispatch
    // -----------------------------------------------------------------------

    /// Find all impls that could provide a method for a receiver. Reports
    /// ambiguity diagnostics and returns (impl_id, fn_id) pairs.
    fn candidates_for_method(
        &mut self,
        receiver: &ITy,
        method: &str,
        span: Span,
    ) -> Vec<(usize, usize)> {
        let mut matches: Vec<(usize, usize)> = Vec::new();
        let impl_count = self.impls.len();
        for impl_id in 0..impl_count {
            let info = match self.impls.get(impl_id) {
                Some(info) => info.clone(),
                None => continue,
            };
            if info
                .trait_ref
                .as_ref()
                .map(|trait_ref| trait_ref.name == "Drop")
                .unwrap_or(false)
            {
                continue;
            }
            let method_index = match info.methods.iter().position(|entry| entry.name == method) {
                Some(index) => index,
                None => continue,
            };
            let gfn_id = info.methods[method_index].gfn_id;
            let self_kind = match self.generic_fns.get(gfn_id) {
                Some(def) => def.decl.self_kind,
                None => continue,
            };
            if self_kind.is_none() {
                continue;
            }
            let args = match self.match_impl_self(impl_id, receiver) {
                Some(args) => args,
                None => continue,
            };
            if info.trait_ref.is_some() {
                self.check_impl_coherence(impl_id, span);
            }
            self.validate_generic_bounds(&info.generics, &args, span, info.module);
            if let Some(fn_id) = self.instantiate_generic_fn(gfn_id, args.clone(), span) {
                if info.trait_ref.is_some() {
                    self.verify_trait_method_signature(impl_id, &args, method, span, fn_id);
                }
                matches.push((impl_id, fn_id));
            }
        }
        // Method resolution order: inherent methods take priority over trait
        // methods, so a generic inherent impl shadows a trait method of the
        // same name for a matching receiver.
        if matches
            .iter()
            .any(|(impl_id, _)| self.impls[*impl_id].trait_ref.is_none())
        {
            matches.retain(|(impl_id, _)| self.impls[*impl_id].trait_ref.is_none());
        }
        if matches.len() > 1 {
            let duplicate = match (matches.first(), matches.get(1)) {
                (Some(first), Some(second)) => {
                    let first_info = &self.impls[first.0];
                    let second_info = &self.impls[second.0];
                    match (&first_info.trait_ref, &second_info.trait_ref) {
                        (Some(a), Some(b)) => a.name == b.name,
                        (None, None) => true,
                        _ => false,
                    }
                }
                _ => false,
            };
            if duplicate {
                let trait_name = self.impls[matches[0].0]
                    .trait_ref
                    .as_ref()
                    .map(|trait_ref| trait_ref.name.clone());
                let receiver_name = self.describe_resolved(receiver);
                let message = match trait_name {
                    Some(trait_name) => format!(
                        "conflicting implementations of trait ({}) for type ({})",
                        trait_name, receiver_name
                    ),
                    None => format!(
                        "conflicting implementations of method '{}' for type ({})",
                        method, receiver_name
                    ),
                };
                self.error_code(message, span, "E0119");
            } else {
                self.error_code(
                    "multiple applicable items in scope".to_string(),
                    span,
                    "E0034",
                );
            }
        }
        matches
    }

    /// Dispatch a method call against registered trait and generic inherent
    /// impls. Returns the instantiated function id.
    pub fn dispatch_impl_method(
        &mut self,
        receiver: &ITy,
        method: &str,
        span: Span,
    ) -> Option<usize> {
        let matches = self.candidates_for_method(receiver, method, span);
        matches.first().map(|(_, fn_id)| *fn_id)
    }

    /// Dispatch an associated function call on an instantiated type.
    pub fn dispatch_associated_fn(
        &mut self,
        owner_ty: &ITy,
        name: &str,
        span: Span,
    ) -> Option<usize> {
        let impl_count = self.impls.len();
        let mut matches = Vec::new();
        for impl_id in 0..impl_count {
            let info = match self.impls.get(impl_id) {
                Some(info) => info.clone(),
                None => continue,
            };
            if info
                .trait_ref
                .as_ref()
                .map(|trait_ref| trait_ref.name == "Drop")
                .unwrap_or(false)
            {
                continue;
            }
            let method_index = match info.methods.iter().position(|entry| entry.name == name) {
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
            let args = match self.match_impl_self(impl_id, owner_ty) {
                Some(args) => args,
                None => continue,
            };
            if info.trait_ref.is_some() {
                self.check_impl_coherence(impl_id, span);
            }
            self.validate_generic_bounds(&info.generics, &args, span, info.module);
            let Some(fn_id) = self.instantiate_generic_fn(gfn_id, args.clone(), span) else {
                continue;
            };
            // Associated functions of a trait impl are signature-checked like
            // methods: From::from must agree with its declaration (E0053).
            if info.trait_ref.is_some() {
                self.verify_trait_method_signature(impl_id, &args, name, span, fn_id);
            }
            matches.push(fn_id);
        }
        if matches.len() > 1 {
            self.error_code(
                "multiple applicable items in scope".to_string(),
                span,
                "E0034",
            );
        }
        matches.first().copied()
    }

    /// Find the impl of a named trait for a concrete receiver type, returning
    /// the impl id and its generic arguments.
    pub fn find_trait_impl(
        &mut self,
        receiver: &ITy,
        trait_name: &str,
    ) -> Option<(usize, Vec<ITy>)> {
        let trait_id = self.lookup_trait(trait_name)?;
        let trait_arity = self.traits[trait_id].generics.len();
        let impl_count = self.impls.len();
        // Deferred signature inference can mint several nominal instances of
        // one generic definition before their arguments resolve; once they do,
        // those instances denote the same type and matching must agree on one.
        let receiver = self.canonical_instance_ty(receiver);
        for impl_id in 0..impl_count {
            let info = match self.impls.get(impl_id) {
                Some(info) => info.clone(),
                None => continue,
            };
            let name_match =
                matches!(&info.trait_ref, Some(trait_ref) if trait_ref.name == trait_name);
            if !name_match {
                continue;
            }
            let args = match self.match_impl_self(impl_id, &receiver) {
                Some(args) => args,
                None => continue,
            };
            // The trait arguments only need to agree with the trait arity here;
            // callers work with the impl's own generic arguments.
            match self.trait_impl_args(impl_id, &args) {
                Some(trait_args) if trait_args.len() == trait_arity => {}
                _ => continue,
            }
            return Some((impl_id, args));
        }
        None
    }

    /// Find the `from` function of an `impl From<Found> for Expected` impl, if
    /// one exists. The `?` operator uses this to convert a propagated error to
    /// the enclosing function's error type (v2.2).
    pub fn find_error_conversion(
        &mut self,
        expected: &ITy,
        found: &ITy,
        span: Span,
    ) -> Option<usize> {
        let trait_id = self.lookup_trait("From")?;
        if self.traits[trait_id].generics.len() != 1 {
            return None;
        }
        let expected = self.canonical_instance_ty(expected);
        let found = self.infer.resolve(found);
        let impl_count = self.impls.len();
        for impl_id in 0..impl_count {
            let info = match self.impls.get(impl_id) {
                Some(info) => info.clone(),
                None => continue,
            };
            let name_match = matches!(
                &info.trait_ref,
                Some(trait_ref) if trait_ref.name == "From"
            );
            if !name_match {
                continue;
            }
            let args = match self.match_impl_self(impl_id, &expected) {
                Some(args) => args,
                None => continue,
            };
            let trait_args = match self.trait_impl_args(impl_id, &args) {
                Some(trait_args) if trait_args.len() == 1 => trait_args,
                _ => continue,
            };
            if !self.types_unify_probe(&trait_args[0], &found) {
                continue;
            }
            let gfn_id = info
                .methods
                .iter()
                .find(|entry| entry.name == "from")
                .map(|entry| entry.gfn_id)?;
            let fn_id = self.instantiate_generic_fn(gfn_id, args.clone(), span)?;
            self.verify_trait_method_signature(impl_id, &args, "from", span, fn_id);
            return Some(fn_id);
        }
        None
    }
}

/// Rewrite a trait method type expression for the context of an impl of that
/// trait: Self becomes the impl target type and the trait's generic parameters
/// become the arguments of the impl's trait reference.
fn substitute_default_ty(
    ty: &ast::TypeExpr,
    self_ty: &ast::TypeExpr,
    args: &HashMap<String, ast::TypeExpr>,
) -> ast::TypeExpr {
    match ty {
        ast::TypeExpr::Path {
            name,
            name_span,
            generics,
        } => {
            if name == "Self" && generics.is_empty() {
                return self_ty.clone();
            }
            if generics.is_empty()
                && let Some(replacement) = args.get(name)
            {
                return replacement.clone();
            }
            ast::TypeExpr::Path {
                name: name.clone(),
                name_span: *name_span,
                generics: generics
                    .iter()
                    .map(|arg| substitute_default_ty(arg, self_ty, args))
                    .collect(),
            }
        }
        ast::TypeExpr::Tuple { elems, span } => ast::TypeExpr::Tuple {
            elems: elems
                .iter()
                .map(|elem| substitute_default_ty(elem, self_ty, args))
                .collect(),
            span: *span,
        },
        ast::TypeExpr::Fn { params, ret, span } => ast::TypeExpr::Fn {
            params: params
                .iter()
                .map(|param| substitute_default_ty(param, self_ty, args))
                .collect(),
            ret: Box::new(substitute_default_ty(ret, self_ty, args)),
            span: *span,
        },
        ast::TypeExpr::Ref {
            mutable,
            lifetime,
            inner,
            span,
        } => ast::TypeExpr::Ref {
            mutable: *mutable,
            lifetime: lifetime.clone(),
            inner: Box::new(substitute_default_ty(inner, self_ty, args)),
            span: *span,
        },
        ast::TypeExpr::Dyn(trait_ref) => ast::TypeExpr::Dyn(ast::TraitRef {
            name: trait_ref.name.clone(),
            name_span: trait_ref.name_span,
            args: trait_ref
                .args
                .iter()
                .map(|arg| substitute_default_ty(arg, self_ty, args))
                .collect(),
            span: trait_ref.span,
        }),
        ast::TypeExpr::Slice { elem, span } => ast::TypeExpr::Slice {
            elem: Box::new(substitute_default_ty(elem, self_ty, args)),
            span: *span,
        },
        ast::TypeExpr::Array { elem, len, span } => ast::TypeExpr::Array {
            elem: Box::new(substitute_default_ty(elem, self_ty, args)),
            len: len.clone(),
            span: *span,
        },
        ast::TypeExpr::Lifetime(lifetime) => ast::TypeExpr::Lifetime(lifetime.clone()),
    }
}
