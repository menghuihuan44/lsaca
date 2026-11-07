//! Trait objects (`dyn Trait`): registration, object safety, coercions and
//! dynamic method dispatch.
//!
//! A `dyn Trait` type is represented during checking as [ITy::Dyn] holding an
//! index into [Checker::trait_objects]. Each registered trait object remembers
//! the trait's instantiated arguments and the trait method signatures (with
//! `Self` already substituted by the arguments) so that method calls can be
//! checked without consulting the trait declaration again.
//!
//! Coercing a concrete value to a trait object (for example `&Rect` to
//! `&dyn Shape`) requires a matching trait implementation; the checker records
//! one vtable per (trait object, concrete implementing type) pair, in
//! first-coercion order.

use std::collections::HashMap;

use crate::ast;
use crate::span::Span;
use crate::types::SelfKind;

use super::check::Checker;
use super::items::sanitize_c_ident;
use super::itype::ITy;
use super::tree::{TExpr, TExprKind};

/// One method of a registered trait object, with `Self` substituted.
#[derive(Clone, Debug)]
pub struct DynMethodInfo {
    pub name: String,
    pub receiver: SelfKind,
    /// Parameters after the receiver.
    pub params: Vec<ITy>,
    pub ret: ITy,
}

/// A registered `dyn Trait<Args>` type.
#[derive(Clone, Debug)]
pub struct TraitObjectInfo {
    pub trait_id: usize,
    /// Bare trait name (without the `dyn` prefix).
    pub name: String,
    pub args: Vec<ITy>,
    pub c_name: String,
    pub methods: Vec<DynMethodInfo>,
    pub span: Span,
}

/// A vtable for one concrete implementing type behind one trait object.
#[derive(Clone, Debug)]
pub struct VtableInfo {
    pub trait_object: usize,
    pub self_ty: ITy,
    /// Instantiated method functions in trait declaration order.
    pub methods: Vec<usize>,
    pub span: Span,
}

impl Checker<'_> {
    /// Register (or reuse) the trait object for `dyn Trait<args>` and return the
    /// index stored in [ITy::Dyn].
    ///
    /// Registration order is first-resolution order, which keeps the emitted
    /// `Program::trait_objects` deterministic for a given source.
    pub fn register_trait_object(&mut self, trait_id: usize, args: Vec<ITy>, span: Span) -> usize {
        let args: Vec<ITy> = args.iter().map(|arg| self.infer.resolve(arg)).collect();
        let key = (trait_id, args.clone());
        if let Some(index) = self.trait_object_index.get(&key) {
            return *index;
        }

        self.check_object_safety(trait_id, span);

        let info = self.traits[trait_id].clone();
        let mut c_name = format!("LsVTable_{}", sanitize_c_ident(&info.name));
        for arg in &args {
            c_name.push('_');
            c_name.push_str(&self.mangle_ty(arg));
        }
        let c_name = self.alloc_c_name(c_name);
        let index = self.trait_objects.len();
        // Reserve the slot before resolving the method signatures: a method may
        // itself mention another trait object (`Box<dyn Other>`), and that inner
        // registration must not recurse into this one.
        self.trait_objects.push(TraitObjectInfo {
            trait_id,
            name: info.name.clone(),
            args: args.clone(),
            c_name,
            methods: Vec::new(),
            span,
        });
        self.trait_object_index
            .insert((trait_id, args.clone()), index);

        let mut env: HashMap<String, ITy> = HashMap::new();
        for (param, arg) in info.generics.iter().zip(args.iter()) {
            env.insert(param.name.clone(), arg.clone());
        }

        let module = info.module;
        let methods = self.with_module(module, |checker| {
            checker.push_generic_env(env);
            let mut methods = Vec::new();
            for method in &info.methods {
                // Associated functions (no receiver) are called statically and
                // never enter a vtable (v2.2 From).
                if method.self_kind.is_none() {
                    continue;
                }
                let mut params = Vec::new();
                let mut failed = false;
                for param in method.params.iter().skip(1) {
                    match checker.resolve_ty(&param.ty, None) {
                        Some(param_ty) => params.push(param_ty),
                        None => {
                            failed = true;
                            break;
                        }
                    }
                }
                let ret = match &method.ret {
                    Some(ret_ty) => match checker.resolve_ty(ret_ty, None) {
                        Some(ret_ty) => ret_ty,
                        None => {
                            failed = true;
                            ITy::Unit
                        }
                    },
                    None => ITy::Unit,
                };
                if failed {
                    continue;
                }
                methods.push(DynMethodInfo {
                    name: method.name.clone(),
                    receiver: method.self_kind.unwrap_or(SelfKind::Value),
                    params,
                    ret,
                });
            }
            checker.pop_generic_env();
            methods
        });
        self.trait_objects[index].methods = methods;
        index
    }

    /// Check the object-safety rules (DESIGN 2.0 section 7) once per trait and
    /// report E0038 with a note explaining the reasons.
    fn check_object_safety(&mut self, trait_id: usize, span: Span) {
        if !self.reported_object_unsafe.insert(trait_id) {
            return;
        }
        let info = self.traits[trait_id].clone();
        let mut problems: Vec<String> = Vec::new();
        for method in &info.methods {
            match method.self_kind {
                Some(SelfKind::Ref) | Some(SelfKind::RefMut) => {}
                Some(SelfKind::Value) => {
                    problems.push(format!("method `{}` takes `self` by value", method.name));
                }
                None => {
                    problems.push(format!("method `{}` does not take a receiver", method.name));
                }
            }
            let mut mentions_self = false;
            for param in method.params.iter().skip(1) {
                mentions_self |= type_mentions_self(&param.ty);
            }
            if let Some(ret) = &method.ret {
                mentions_self |= type_mentions_self(ret);
            }
            if mentions_self {
                problems.push(format!(
                    "method `{}` mentions `Self` outside its receiver",
                    method.name
                ));
            }
        }
        if problems.is_empty() {
            return;
        }
        self.error_code_note(
            format!("trait `{}` cannot be made into an object", info.name),
            span,
            "E0038",
            format!(
                "object-safe traits cannot have non-receiver methods, self by value or Self in signatures: {}",
                problems.join("; ")
            ),
        );
    }

    /// Try to coerce the expression `expr` (whose type is `actual`) to the trait
    /// object type `wanted`. Reports E0277 when the concrete type does not
    /// implement the trait.
    pub fn try_dyn_coerce(
        &mut self,
        expr: TExpr,
        actual: &ITy,
        wanted: &ITy,
        span: Span,
    ) -> Option<TExpr> {
        let wanted = self.infer.resolve(wanted);
        let object = match &wanted {
            ITy::Ref { inner, .. } => match self.infer.resolve(inner) {
                ITy::Dyn(object) => object,
                _ => return None,
            },
            ITy::Box(inner) | ITy::Rc(inner) => match self.infer.resolve(inner) {
                ITy::Dyn(object) => object,
                _ => return None,
            },
            _ => return None,
        };

        let actual = self.infer.resolve(actual);
        let self_ty = match (&wanted, &actual) {
            (
                ITy::Ref {
                    mutable: wanted_mutable,
                    ..
                },
                ITy::Ref {
                    mutable: actual_mutable,
                    inner,
                },
            ) => {
                if *wanted_mutable && !*actual_mutable {
                    return None;
                }
                (**inner).clone()
            }
            (ITy::Box(_), ITy::Box(inner)) | (ITy::Rc(_), ITy::Rc(inner)) => (**inner).clone(),
            _ => return None,
        };
        let self_ty = self.infer.resolve(&self_ty);
        if self_ty.is_unsized() {
            // No upcasting: `Box<dyn A>` to `Box<dyn B>` is a type mismatch.
            return None;
        }
        if self.contains_unresolved(&self_ty) {
            self.error_code(
                "type annotations needed: the concrete type behind this trait-object coercion is not known"
                    .to_string(),
                span,
                "E0282",
            );
            return Some(self.never_expr(span));
        }
        match self.ensure_vtable(object, &self_ty, span) {
            Some(vtable) => Some(TExpr::new(
                wanted,
                TExprKind::DynCoerce {
                    value: Box::new(expr),
                    vtable,
                },
                span,
            )),
            None => Some(self.never_expr(span)),
        }
    }

    /// Return the index of the trait object behind a method-call receiver type.
    fn dyn_receiver_object(&mut self, receiver_ty: &ITy) -> Option<usize> {
        match self.infer.resolve(receiver_ty) {
            ITy::Dyn(object) => Some(object),
            ITy::Box(inner) | ITy::Rc(inner) => match self.infer.resolve(&inner) {
                ITy::Dyn(object) => Some(object),
                _ => None,
            },
            _ => None,
        }
    }

    /// Resolve a method call on a trait object. Returns `None` when the receiver
    /// is not a trait object or the trait declares no such method, so that the
    /// regular method resolution keeps running.
    // One method call needs the receiver expression, both receiver types, the
    // method, its arguments, two spans and the expected type; bundling them would
    // only shuffle the same fields for the single caller.
    #[allow(clippy::too_many_arguments)]
    pub fn check_dyn_method_call(
        &mut self,
        receiver_expr: TExpr,
        raw_ty: &ITy,
        receiver_ty: &ITy,
        method: &str,
        args: &[ast::Expr],
        receiver_span: Span,
        span: Span,
        hint: Option<&ITy>,
    ) -> Option<TExpr> {
        let object_index = self.dyn_receiver_object(receiver_ty)?;
        let object = self.trait_objects.get(object_index)?.clone();
        let (slot, method_info) = object
            .methods
            .iter()
            .enumerate()
            .find(|(_, candidate)| candidate.name == method)
            .map(|(slot, candidate)| (slot, candidate.clone()))?;
        if matches!(method_info.receiver, SelfKind::RefMut) {
            self.require_mutable_receiver(
                &receiver_expr,
                raw_ty,
                receiver_span,
                "mutate through this receiver",
            );
        }
        let arg_exprs = self.check_call_args(&method_info.params, args, span);
        self.apply_hint(&method_info.ret, hint);
        Some(TExpr::new(
            method_info.ret,
            TExprKind::DynCall {
                object: Box::new(receiver_expr),
                trait_object: object_index,
                slot,
                args: arg_exprs,
            },
            span,
        ))
    }

    /// Make sure a vtable exists for (`object`, `self_ty`) and return its index.
    /// Reports E0277 when `self_ty` does not implement the trait.
    fn ensure_vtable(&mut self, object: usize, self_ty: &ITy, span: Span) -> Option<usize> {
        let self_ty = self.infer.resolve(self_ty);
        if let Some(index) = self.vtable_index.get(&(object, self_ty.clone())) {
            return Some(*index);
        }
        let trait_id = self.trait_objects.get(object)?.trait_id;
        let trait_name = self.trait_objects[object].name.clone();
        let Some((impl_id, impl_args)) = self.find_impl_for_trait(&trait_name, trait_id, &self_ty)
        else {
            let found = self.describe_resolved(&self_ty);
            self.error_code_note(
                format!(
                    "the trait `{}` is not implemented for `{}`",
                    trait_name, found
                ),
                span,
                "E0277",
                format!(
                    "to use `{}` as a trait object, the concrete type must implement the trait",
                    trait_name
                ),
            );
            return None;
        };

        self.check_impl_coherence(impl_id, span);
        let info = self.impls[impl_id].clone();
        if !info.generics.is_empty() {
            self.validate_generic_bounds(&info.generics, &impl_args, span, info.module);
        }

        let trait_object = self.trait_objects[object].clone();
        let mut methods = Vec::new();
        let mut complete = true;
        for method in &trait_object.methods {
            let entry = info
                .methods
                .iter()
                .find(|candidate| candidate.name == method.name)
                .cloned();
            let Some(entry) = entry else {
                complete = false;
                continue;
            };
            match self.instantiate_generic_fn(entry.gfn_id, impl_args.clone(), span) {
                Some(fn_id) => {
                    self.verify_trait_method_signature(
                        impl_id,
                        &impl_args,
                        &method.name,
                        span,
                        fn_id,
                    );
                    methods.push(fn_id);
                }
                None => complete = false,
            }
        }
        if !complete {
            return None;
        }

        let index = self.vtables.len();
        self.vtables.push(VtableInfo {
            trait_object: object,
            self_ty: self_ty.clone(),
            methods,
            span,
        });
        self.vtable_index.insert((object, self_ty), index);
        Some(index)
    }

    /// Find the trait implementation whose trait is `trait_id` and whose self
    /// type unifies with `self_ty`.
    fn find_impl_for_trait(
        &mut self,
        trait_name: &str,
        trait_id: usize,
        self_ty: &ITy,
    ) -> Option<(usize, Vec<ITy>)> {
        let impl_ids: Vec<usize> = (0..self.impls.len()).collect();
        for impl_id in impl_ids {
            let info = self.impls[impl_id].clone();
            let Some(trait_ref) = info.trait_ref.clone() else {
                continue;
            };
            if trait_ref.name != trait_name {
                continue;
            }
            let resolved =
                self.with_module(info.module, |checker| checker.lookup_trait(&trait_ref.name));
            if resolved != Some(trait_id) {
                continue;
            }
            if let Some(impl_args) = self.match_impl_self(impl_id, self_ty) {
                return Some((impl_id, impl_args));
            }
        }
        None
    }
}

/// Does the type expression mention `Self` outside the receiver position?
fn type_mentions_self(ty: &ast::TypeExpr) -> bool {
    match ty {
        ast::TypeExpr::Path { name, generics, .. } => {
            name == "Self" || generics.iter().any(type_mentions_self)
        }
        ast::TypeExpr::Tuple { elems, .. } => elems.iter().any(type_mentions_self),
        ast::TypeExpr::Fn { params, ret, .. } => {
            params.iter().any(type_mentions_self) || type_mentions_self(ret)
        }
        ast::TypeExpr::Ref { inner, .. } => type_mentions_self(inner),
        ast::TypeExpr::Dyn(trait_ref) => trait_ref.args.iter().any(type_mentions_self),
        ast::TypeExpr::Slice { elem, .. } => type_mentions_self(elem),
        ast::TypeExpr::Array { elem, .. } => type_mentions_self(elem),
        ast::TypeExpr::Lifetime(_) => false,
    }
}
