//! Generic item registries and monomorphization.
//!
//! HIR stays fully monomorphic: every generic struct, enum and function is
//! registered here in *definition* form and instantiated on demand with fully
//! resolved type arguments. Instances are appended to the concrete registries
//! ([Checker::structs], [Checker::enums], [Checker::fns]) exactly like
//! hand-written items, so lowering and code generation never see generics.

use std::collections::HashMap;

use super::check::Checker;
use super::items::{EnumInfo, FnInfo, FnSig, StructInfo, sanitize_c_ident};
use super::itype::ITy;
use crate::ast::{EnumDecl, FnDecl, GenericParam, StructDecl, TraitRef, TypeExpr};
use crate::diag::Diagnostic;
use crate::span::Span;
use crate::types::SelfKind;

/// Where an item came from: the user program or the embedded standard library.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefOrigin {
    User,
    Prelude,
}

impl DefOrigin {
    pub fn is_prelude(self) -> bool {
        matches!(self, DefOrigin::Prelude)
    }
}

/// Which item owns a generic function definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenericOwner {
    /// A free generic function.
    Free,
    /// A method or associated function of an inherent impl.
    InherentImpl(usize),
    /// A method of a trait impl.
    TraitImpl(usize),
}

/// The definition an instantiated struct came from.
#[derive(Clone, Debug)]
pub struct StructInstance {
    pub def_id: usize,
    pub args: Vec<ITy>,
}

/// The definition an instantiated enum came from.
#[derive(Clone, Debug)]
pub struct EnumInstance {
    pub def_id: usize,
    pub args: Vec<ITy>,
}

/// The definition an instantiated function came from.
#[derive(Clone, Debug)]
pub struct FnInstance {
    pub def_id: usize,
    pub args: Vec<ITy>,
}

/// A generic struct declaration.
#[derive(Clone, Debug)]
pub struct GStructDef {
    pub name: String,
    pub span: Span,
    pub generics: Vec<GenericParam>,
    pub decl: StructDecl,
    pub instances: HashMap<Vec<ITy>, usize>,
}

/// A generic enum declaration.
#[derive(Clone, Debug)]
pub struct GEnumDef {
    pub name: String,
    pub span: Span,
    pub generics: Vec<GenericParam>,
    pub decl: EnumDecl,
    pub instances: HashMap<Vec<ITy>, usize>,
}

/// A generic function declaration: a free function or an impl method whose
/// signature mentions the impl's generic parameters.
#[derive(Clone, Debug)]
pub struct GFnDef {
    pub name: String,
    pub name_span: Span,
    pub span: Span,
    /// The parameters that must be supplied to instantiate this function.
    /// For impl methods these are the impl's generic parameters.
    pub generics: Vec<GenericParam>,
    pub decl: FnDecl,
    pub owner: GenericOwner,
    pub instances: HashMap<Vec<ITy>, usize>,
    pub origin: DefOrigin,
    /// Prefix for the C identifiers of instances.
    pub c_name_base: String,
    /// Display name without type arguments, e.g. "Pair::new" or
    /// "Iterator::next".
    pub display_base: String,
}

/// One method declared in an impl block, with the id of its generic function
/// definition.
#[derive(Clone, Debug)]
pub struct ImplMethod {
    pub name: String,
    pub gfn_id: usize,
}

/// A registered impl block.
#[derive(Clone, Debug)]
pub struct ImplInfo {
    pub id: usize,
    pub generics: Vec<GenericParam>,
    pub self_ty: TypeExpr,
    pub trait_ref: Option<TraitRef>,
    pub methods: Vec<ImplMethod>,
    pub span: Span,
    /// Late coherence checks (E0046/E0407) run only once per impl.
    pub late_checked: bool,
}

impl<'a> Checker<'a> {
    // -----------------------------------------------------------------------
    // Generic environment
    // -----------------------------------------------------------------------

    /// Look a name up in the innermost-to-outermost generic environments.
    pub fn lookup_generic_env(&self, name: &str) -> Option<ITy> {
        for env in self.generic_env.iter().rev() {
            if let Some(ty) = env.get(name) {
                return Some(ty.clone());
            }
        }
        None
    }

    /// Resolve "Self" for the function currently being checked: the instance
    /// type of an instantiated impl method, or the concrete impl target.
    pub fn current_self_ty(&self) -> Option<ITy> {
        if let Some(ty) = self.lookup_generic_env("Self") {
            return Some(ty);
        }
        self.current_self_key().map(|key| match key {
            super::items::TypeKey::Struct(id) => ITy::Struct(id),
            super::items::TypeKey::Enum(id) => ITy::Enum(id),
        })
    }

    pub fn push_generic_env(&mut self, env: HashMap<String, ITy>) {
        self.generic_env.push(env);
    }

    pub fn pop_generic_env(&mut self) {
        self.generic_env.pop();
    }

    /// The instantiation depth of the function currently being checked.
    pub fn current_instantiation_depth(&self) -> usize {
        self.fns
            .get(self.current_fn)
            .map(|info| info.instantiation_depth)
            .unwrap_or(0)
    }

    // -----------------------------------------------------------------------
    // Type arguments
    // -----------------------------------------------------------------------

    /// Resolve an instantiation argument to a fully concrete type. Integer and
    /// float literal variables default to i64 and f64; a remaining general
    /// inference variable cannot name a concrete instance.
    pub fn normalize_instance_arg(&mut self, ty: &ITy) -> Option<ITy> {
        let resolved = self.infer.resolve(ty);
        let resolved = match resolved {
            ITy::Var(id) => match self.infer.var_kind(id) {
                super::itype::VarKind::IntLiteral => {
                    let _ = self.infer.unify(&ITy::Var(id), &ITy::i64());
                    self.infer.resolve(&ITy::Var(id))
                }
                super::itype::VarKind::FloatLiteral => {
                    let _ = self.infer.unify(&ITy::Var(id), &ITy::f64());
                    self.infer.resolve(&ITy::Var(id))
                }
                super::itype::VarKind::General => return None,
            },
            other => other,
        };
        if self.contains_unresolved(&resolved) {
            return None;
        }
        Some(resolved)
    }

    /// Whether a type mentions an unresolved inference variable.
    pub fn contains_unresolved(&self, ty: &ITy) -> bool {
        match self.infer.resolve(ty) {
            ITy::Var(_) => true,
            ITy::Vec(inner) | ITy::Rc(inner) | ITy::Box(inner) | ITy::Option(inner) => {
                self.contains_unresolved(&inner)
            }
            ITy::Result(ok, err) => self.contains_unresolved(&ok) || self.contains_unresolved(&err),
            ITy::HashMap(key, value) => {
                self.contains_unresolved(&key) || self.contains_unresolved(&value)
            }
            ITy::Tuple(elems) => elems.iter().any(|elem| self.contains_unresolved(elem)),
            ITy::Fn(params, ret) => {
                params.iter().any(|param| self.contains_unresolved(param))
                    || self.contains_unresolved(&ret)
            }
            ITy::Ref { inner, .. } => self.contains_unresolved(&inner),
            _ => false,
        }
    }

    pub fn normalize_args(&mut self, args: &[ITy]) -> Option<Vec<ITy>> {
        let mut out = Vec::with_capacity(args.len());
        for arg in args {
            out.push(self.normalize_instance_arg(arg)?);
        }
        Some(out)
    }

    /// Resolve explicit type-argument expressions to concrete types, reporting
    /// E0282 when they are still unknown.
    pub fn resolve_concrete_type_args(
        &mut self,
        exprs: &[TypeExpr],
        _span: Span,
    ) -> Option<Vec<ITy>> {
        let mut args = Vec::with_capacity(exprs.len());
        for expr in exprs {
            let resolved = self.resolve_ty(expr, None)?;
            match self.normalize_instance_arg(&resolved) {
                Some(ty) => args.push(ty),
                None => {
                    self.error_code("type annotations needed".to_string(), expr.span(), "E0282");
                    return None;
                }
            }
        }
        Some(args)
    }

    // -----------------------------------------------------------------------
    // C name and display name mangling
    // -----------------------------------------------------------------------

    /// Mangle one concrete type into the C identifier spelling used for
    /// monomorphized names. This mirrors the code generator's type mangling.
    pub fn mangle_ty(&self, ty: &ITy) -> String {
        let ty = self.infer.resolve(ty);
        match ty {
            ITy::Unit => "unit".to_string(),
            ITy::Bool => "bool".to_string(),
            ITy::Int(int_ty) => int_ty.name().to_string(),
            ITy::Float(float_ty) => float_ty.name().to_string(),
            ITy::String => "String".to_string(),
            ITy::Vec(inner) => format!("Vec_{}", self.mangle_ty(&inner)),
            ITy::Rc(inner) => format!("Rc_{}", self.mangle_ty(&inner)),
            ITy::Box(inner) => format!("Box_{}", self.mangle_ty(&inner)),
            ITy::Option(inner) => format!("Opt_{}", self.mangle_ty(&inner)),
            ITy::Result(ok, err) => format!("Res_{}_{}", self.mangle_ty(&ok), self.mangle_ty(&err)),
            ITy::HashMap(key, value) => {
                format!("Map_{}_{}", self.mangle_ty(&key), self.mangle_ty(&value))
            }
            ITy::Tuple(elems) => {
                let parts: Vec<String> = elems.iter().map(|elem| self.mangle_ty(elem)).collect();
                if parts.is_empty() {
                    "unit".to_string()
                } else {
                    format!("Tup{}_{}", parts.len(), parts.join("_"))
                }
            }
            ITy::Fn(params, ret) => {
                let parts: Vec<String> = params.iter().map(|param| self.mangle_ty(param)).collect();
                format!("Fn{}_to_{}", parts.join("_"), self.mangle_ty(&ret))
            }
            ITy::Ref { mutable, inner } => {
                if mutable {
                    format!("RefMut_{}", self.mangle_ty(&inner))
                } else {
                    format!("Ref_{}", self.mangle_ty(&inner))
                }
            }
            ITy::Struct(id) => self
                .structs
                .get(id)
                .map(|info| info.c_name.clone())
                .unwrap_or_else(|| format!("struct{}", id)),
            ITy::Enum(id) => self
                .enums
                .get(id)
                .map(|info| info.c_name.clone())
                .unwrap_or_else(|| format!("enum{}", id)),
            ITy::Never => "never".to_string(),
            ITy::Var(_) => "var".to_string(),
        }
    }

    fn instance_display(&self, base: &str, args: &[ITy]) -> String {
        let rendered: Vec<String> = args.iter().map(|arg| self.describe_resolved(arg)).collect();
        format!("{}<{}>", base, rendered.join(", "))
    }

    fn instance_c_name(&mut self, base: &str, args: &[ITy]) -> String {
        let mut name = sanitize_c_ident(base);
        for arg in args {
            name.push('_');
            name.push_str(&self.mangle_ty(arg));
        }
        self.alloc_c_name(name)
    }

    // -----------------------------------------------------------------------
    // Struct and enum instantiation
    // -----------------------------------------------------------------------

    pub fn instantiate_struct(
        &mut self,
        def_id: usize,
        args: Vec<ITy>,
        span: Span,
    ) -> Option<usize> {
        let args = match self.normalize_args(&args) {
            Some(args) => args,
            None => {
                self.error_code("type annotations needed".to_string(), span, "E0282");
                return None;
            }
        };
        let def = {
            let def = self.generic_structs.get(def_id)?;
            def.clone()
        };
        let arity = def.generics.len();
        if args.len() != arity {
            self.error_code(
                format!(
                    "struct {} takes {} type argument{} but {} were supplied",
                    def.name,
                    arity,
                    if arity == 1 { "" } else { "s" },
                    args.len()
                ),
                span,
                "E0107",
            );
            return None;
        }
        if let Some(&id) = def.instances.get(&args) {
            return Some(id);
        }
        let name = self.instance_display(&def.name, &args);
        let c_name = self.instance_c_name(&def.name, &args);
        let id = self.structs.len();
        self.structs.push(StructInfo {
            name,
            c_name,
            generic: Some(StructInstance {
                def_id,
                args: args.clone(),
            }),
            span: def.decl.span,
            decl_fields: def.decl.fields.clone(),
            fields: Vec::new(),
            fields_resolved: false,
        });
        self.drop_structs.push(None);
        if let Some(def) = self.generic_structs.get_mut(def_id) {
            def.instances.insert(args.clone(), id);
        }
        let mut env = HashMap::new();
        for (param, arg) in def.generics.iter().zip(args.iter()) {
            env.insert(param.name.clone(), arg.clone());
        }
        self.push_generic_env(env);
        self.ensure_struct_fields(id);
        self.pop_generic_env();
        let instance_ty = ITy::Struct(id);
        self.instantiate_drop_hook(super::items::TypeKey::Struct(id), &instance_ty, span);
        Some(id)
    }

    pub fn instantiate_enum(&mut self, def_id: usize, args: Vec<ITy>, span: Span) -> Option<usize> {
        let args = match self.normalize_args(&args) {
            Some(args) => args,
            None => {
                self.error_code("type annotations needed".to_string(), span, "E0282");
                return None;
            }
        };
        let def = {
            let def = self.generic_enums.get(def_id)?;
            def.clone()
        };
        let arity = def.generics.len();
        if args.len() != arity {
            self.error_code(
                format!(
                    "enum {} takes {} type argument{} but {} were supplied",
                    def.name,
                    arity,
                    if arity == 1 { "" } else { "s" },
                    args.len()
                ),
                span,
                "E0107",
            );
            return None;
        }
        if let Some(&id) = def.instances.get(&args) {
            return Some(id);
        }
        let name = self.instance_display(&def.name, &args);
        let c_name = self.instance_c_name(&def.name, &args);
        let id = self.enums.len();
        self.enums.push(EnumInfo {
            name,
            c_name,
            generic: Some(EnumInstance {
                def_id,
                args: args.clone(),
            }),
            span: def.decl.span,
            decl_variants: def.decl.variants.clone(),
            variants: Vec::new(),
            variants_resolved: false,
        });
        self.drop_enums.push(None);
        if let Some(def) = self.generic_enums.get_mut(def_id) {
            def.instances.insert(args.clone(), id);
        }
        let mut env = HashMap::new();
        for (param, arg) in def.generics.iter().zip(args.iter()) {
            env.insert(param.name.clone(), arg.clone());
        }
        self.push_generic_env(env);
        self.ensure_enum_variants(id);
        self.pop_generic_env();
        let instance_ty = ITy::Enum(id);
        self.instantiate_drop_hook(super::items::TypeKey::Enum(id), &instance_ty, span);
        Some(id)
    }

    /// Match the self type of an impl against a concrete instantiated type,
    /// returning the impl's generic arguments on success.
    ///
    /// Matching is structural and non-reporting: the impl's generic parameters
    /// are bound by walking the self type against the receiver, so generic
    /// instances (for example `__lsaca_VecIter<i64>`) match without resolving
    /// the impl's self type with unresolved inference variables.
    pub fn match_impl_self(&mut self, impl_id: usize, receiver: &ITy) -> Option<Vec<ITy>> {
        let info = self.impls.get(impl_id)?.clone();
        let snapshot = self.infer.clone();
        let mut env = HashMap::new();
        for param in &info.generics {
            let var = self.infer.fresh_var();
            env.insert(param.name.clone(), var);
        }
        let quiet = self.probe_quiet;
        self.probe_quiet = true;
        let matched = self.infer_args_from_ty(&info.self_ty, receiver, &mut env, info.span);
        self.probe_quiet = quiet;
        if !matched {
            self.infer = snapshot;
            return None;
        }
        let mut args = Vec::with_capacity(info.generics.len());
        for param in &info.generics {
            let ty = match env.get(&param.name) {
                Some(ty) => self.infer.resolve(ty),
                None => {
                    self.infer = snapshot;
                    return None;
                }
            };
            args.push(ty);
        }
        let args = match self.normalize_args(&args) {
            Some(args) => args,
            None => {
                self.infer = snapshot;
                return None;
            }
        };
        // Re-unify with the now concrete argument types to keep the bindings
        // and reject candidates whose arguments could not be determined.
        let mut concrete_env = HashMap::new();
        for (param, arg) in info.generics.iter().zip(args.iter()) {
            concrete_env.insert(param.name.clone(), arg.clone());
        }
        self.push_generic_env(concrete_env);
        let concrete_self = self.resolve_ty(&info.self_ty, None);
        self.pop_generic_env();
        let matched = match concrete_self {
            Some(ty) => self.infer.unify(&ty, receiver).is_ok(),
            None => false,
        };
        if !matched {
            self.infer = snapshot;
            return None;
        }
        Some(args)
    }

    /// Resolve the self type of an impl block under its registered generic
    /// arguments.
    pub fn impl_self_ty(&mut self, impl_id: usize, args: &[ITy]) -> Option<ITy> {
        let info = self.impls.get(impl_id)?.clone();
        let mut env = HashMap::new();
        for (param, arg) in info.generics.iter().zip(args.iter()) {
            env.insert(param.name.clone(), arg.clone());
        }
        self.push_generic_env(env);
        let self_ty = self.resolve_ty(&info.self_ty, None);
        self.pop_generic_env();
        self_ty.map(|ty| self.infer.resolve(&ty))
    }

    // -----------------------------------------------------------------------
    // Function instantiation
    // -----------------------------------------------------------------------

    fn generic_signature(
        &mut self,
        def: &GFnDef,
        args: &[ITy],
        self_ty: Option<&ITy>,
    ) -> Option<FnSig> {
        let mut env = HashMap::new();
        for (param, arg) in def.generics.iter().zip(args.iter()) {
            env.insert(param.name.clone(), arg.clone());
        }
        if let Some(self_ty) = self_ty {
            env.insert("Self".to_string(), self_ty.clone());
        }
        self.push_generic_env(env);
        let mut params: Vec<ITy> = Vec::with_capacity(def.decl.params.len());
        let mut seen: HashMap<String, Span> = HashMap::new();
        for param in &def.decl.params {
            if let Some(previous) = seen.get(&param.name) {
                let previous = *previous;
                let diag = Diagnostic::error(
                    format!(
                        "identifier '{}' is bound more than once in this parameter list",
                        param.name
                    ),
                    param.name_span,
                )
                .with_label(
                    previous,
                    format!("previous definition of '{}' here", param.name),
                );
                self.diagnostics.push(diag);
            }
            seen.insert(param.name.clone(), param.name_span);
            let ty = match self.resolve_ty(&param.ty, None) {
                Some(ty) => ty,
                None => self.infer.fresh_var(),
            };
            params.push(ty);
        }
        for (index, param) in def.decl.params.iter().enumerate() {
            if let Some(param_ty) = params.get(index).cloned() {
                self.validate_value_type(&param_ty, param.name_span);
            }
        }
        if let Some(kind) = def.decl.self_kind
            && let Some(receiver_ty) = params.first().cloned()
        {
            let mutable = match kind {
                SelfKind::Ref => Some(false),
                SelfKind::RefMut => Some(true),
                SelfKind::Value => None,
            };
            if let Some(mutable) = mutable {
                params[0] = ITy::Ref {
                    mutable,
                    inner: Box::new(receiver_ty),
                };
            }
        }
        let ret = match &def.decl.ret {
            Some(ret_ty) => match self.resolve_ty(ret_ty, None) {
                Some(ty) => ty,
                None => self.infer.fresh_var(),
            },
            None => ITy::Unit,
        };
        if let Some(ret_ty) = &def.decl.ret {
            let ret_span = ret_ty.span();
            self.validate_value_type(&ret, ret_span);
        }
        let mut ret_ref_source: Option<usize> = None;
        let ret_resolved = self.infer.resolve(&ret);
        if let ITy::Ref { mutable, .. } = ret_resolved {
            let mut sources: Vec<usize> = Vec::new();
            if matches!(
                def.decl.self_kind,
                Some(SelfKind::Ref) | Some(SelfKind::RefMut)
            ) {
                sources.push(0);
            } else {
                for (index, param_ty) in params.iter().enumerate() {
                    if matches!(self.infer.resolve(param_ty), ITy::Ref { .. }) {
                        sources.push(index);
                    }
                }
            }
            let ret_span = def
                .decl
                .ret
                .as_ref()
                .map(|ty| ty.span())
                .unwrap_or(def.name_span);
            if sources.is_empty() {
                let diag = Diagnostic::error("missing lifetime specifier".to_string(), ret_span)
                    .with_code("E0106")
                    .with_note(
                        "this function's return type contains a borrowed value, but there is no reference input to borrow from"
                            .to_string(),
                    );
                self.diagnostics.push(diag);
            } else if sources.len() > 1 {
                let diag = Diagnostic::error("missing lifetime specifier".to_string(), ret_span)
                    .with_code("E0106")
                    .with_note(
                        "this function's return type contains a borrowed value, but the signature does not say which parameter it is borrowed from"
                            .to_string(),
                    );
                self.diagnostics.push(diag);
            } else {
                let source = sources[0];
                ret_ref_source = Some(source);
                if mutable {
                    let from_mutable = if source == 0 && def.decl.self_kind.is_some() {
                        matches!(def.decl.self_kind, Some(SelfKind::RefMut))
                    } else {
                        matches!(
                            self.infer.resolve(&params[source]),
                            ITy::Ref { mutable: true, .. }
                        )
                    };
                    if !from_mutable {
                        let diag = Diagnostic::error(
                            "cannot return a mutable reference from a shared reference source"
                                .to_string(),
                            ret_span,
                        )
                        .with_code("E0596")
                        .with_note(
                            "a &mut return type must borrow from a &mut parameter or from a &mut self receiver"
                                .to_string(),
                        );
                        self.diagnostics.push(diag);
                    }
                }
            }
        }
        self.pop_generic_env();
        Some(FnSig {
            params,
            ret,
            ret_ref_source,
        })
    }

    /// Instantiate a generic function definition with concrete type arguments.
    pub fn instantiate_generic_fn(
        &mut self,
        def_id: usize,
        args: Vec<ITy>,
        span: Span,
    ) -> Option<usize> {
        let args = match self.normalize_args(&args) {
            Some(args) => args,
            None => {
                self.error_code(
                    "type annotations needed for generic arguments".to_string(),
                    span,
                    "E0283",
                );
                return None;
            }
        };
        let def = {
            let def = self.generic_fns.get(def_id)?;
            def.clone()
        };
        let arity = def.generics.len();
        if args.len() != arity {
            self.error_code(
                format!(
                    "function {} takes {} type argument{} but {} were supplied",
                    def.name,
                    arity,
                    if arity == 1 { "" } else { "s" },
                    args.len()
                ),
                span,
                "E0107",
            );
            return None;
        }
        if let Some(&id) = def.instances.get(&args) {
            return Some(id);
        }
        let depth = self.current_instantiation_depth() + 1;
        if depth > 128 {
            self.error_code(
                format!(
                    "recursion limit reached while instantiating generic function ({})",
                    def.name
                ),
                span,
                "E0275",
            );
            return None;
        }
        self.validate_generic_bounds(&def.generics, &args, span);
        let self_ty = match def.owner {
            GenericOwner::InherentImpl(impl_id) | GenericOwner::TraitImpl(impl_id) => {
                self.impl_self_ty(impl_id, &args)
            }
            GenericOwner::Free => None,
        };
        let sig = self.generic_signature(&def, &args, self_ty.as_ref())?;
        let display = match def.owner {
            GenericOwner::TraitImpl(impl_id) => {
                let info = self.impls.get(impl_id)?.clone();
                match self.impl_trait_display(&info, &args) {
                    Some(trait_display) => format!("{}::{}", trait_display, def.name),
                    None => def.display_base.clone(),
                }
            }
            GenericOwner::InherentImpl(_) => match &self_ty {
                Some(self_ty) => {
                    format!("{}::{}", self.describe_resolved(self_ty), def.name)
                }
                None => def.display_base.clone(),
            },
            GenericOwner::Free => {
                if def.generics.is_empty() {
                    def.name.clone()
                } else {
                    self.instance_display(&def.name, &args)
                }
            }
        };
        let c_name = self.instance_c_name(&format!("{}_args", def.c_name_base), &args);
        let param_cnames = (0..sig.params.len())
            .map(|index| format!("p{}", index))
            .collect();
        let id = self.fns.len();
        let owner_key = match &self_ty {
            Some(ITy::Struct(struct_id)) => Some(super::items::TypeKey::Struct(*struct_id)),
            Some(ITy::Enum(enum_id)) => Some(super::items::TypeKey::Enum(*enum_id)),
            _ => None,
        };
        self.fns.push(FnInfo {
            name: def.name.clone(),
            display,
            c_name,
            decl: def.decl.clone(),
            origin: def.origin,
            generic: Some(FnInstance {
                def_id,
                args: args.clone(),
            }),
            self_ty,
            instantiation_depth: depth,
            owner: owner_key,
            self_kind: def.decl.self_kind,
            sig: Some(sig),
            checked: None,
            param_cnames,
            name_span: def.name_span,
            span: def.span,
        });
        if let Some(def) = self.generic_fns.get_mut(def_id) {
            def.instances.insert(args, id);
        }
        self.pending_fns.push(id);
        Some(id)
    }

    /// Display name of a trait impl's trait with its concrete arguments.
    pub fn impl_trait_display(&mut self, info: &ImplInfo, args: &[ITy]) -> Option<String> {
        let trait_ref = info.trait_ref.as_ref()?.clone();
        let trait_id = self.trait_by_name.get(&trait_ref.name).copied()?;
        if trait_ref.args.is_empty() {
            return Some(trait_ref.name.clone());
        }
        let mut env = HashMap::new();
        for (param, arg) in info.generics.iter().zip(args.iter()) {
            env.insert(param.name.clone(), arg.clone());
        }
        self.push_generic_env(env);
        let resolved: Vec<String> = trait_ref
            .args
            .iter()
            .map(|arg| match self.resolve_ty(arg, None) {
                Some(ty) => self.describe_resolved(&ty),
                None => "_".to_string(),
            })
            .collect();
        self.pop_generic_env();
        let _ = trait_id;
        Some(format!("{}<{}>", trait_ref.name, resolved.join(", ")))
    }

    // -----------------------------------------------------------------------
    // Bounds
    // -----------------------------------------------------------------------

    pub fn validate_generic_bounds(&mut self, generics: &[GenericParam], args: &[ITy], span: Span) {
        for (param, arg) in generics.iter().zip(args.iter()) {
            for bound in &param.bounds {
                self.validate_bound(arg, bound, generics, args, span);
            }
        }
    }

    fn validate_bound(
        &mut self,
        arg: &ITy,
        bound: &TypeExpr,
        generics: &[GenericParam],
        args: &[ITy],
        span: Span,
    ) {
        let mut env = HashMap::new();
        for (param, value) in generics.iter().zip(args.iter()) {
            env.insert(param.name.clone(), value.clone());
        }
        match bound {
            TypeExpr::Path {
                name,
                name_span,
                generics: bound_args,
            } => {
                if name == "Drop" {
                    if !self.has_drop_glue(arg) {
                        self.error_code(
                            format!(
                                "the trait bound ({}: Drop) is not satisfied",
                                self.describe_resolved(arg)
                            ),
                            *name_span,
                            "E0277",
                        );
                    }
                    return;
                }
                let trait_id = match self.trait_by_name.get(name).copied() {
                    Some(id) => id,
                    None => {
                        self.error_code(
                            format!("cannot find trait '{}' in this scope", name),
                            *name_span,
                            "E0405",
                        );
                        return;
                    }
                };
                self.push_generic_env(env);
                let mut trait_args = Vec::with_capacity(bound_args.len());
                let mut failed = false;
                for expr in bound_args {
                    match self.resolve_ty(expr, None) {
                        Some(ty) => trait_args.push(ty),
                        None => failed = true,
                    }
                }
                self.pop_generic_env();
                if failed {
                    return;
                }
                let arity = self
                    .traits
                    .get(trait_id)
                    .map(|info| info.generics.len())
                    .unwrap_or(0);
                if trait_args.len() != arity {
                    self.error_code(
                        format!(
                            "trait {} takes {} type argument{} but {} were supplied",
                            name,
                            arity,
                            if arity == 1 { "" } else { "s" },
                            trait_args.len()
                        ),
                        *name_span,
                        "E0107",
                    );
                    return;
                }
                if !self.trait_bound_satisfied(arg, trait_id, &trait_args) {
                    let rendered: Vec<String> = trait_args
                        .iter()
                        .map(|trait_arg| self.describe_resolved(trait_arg))
                        .collect();
                    let bound_text = if rendered.is_empty() {
                        name.clone()
                    } else {
                        format!("{}<{}>", name, rendered.join(", "))
                    };
                    self.error_code(
                        format!(
                            "the trait bound ({}: {}) is not satisfied",
                            self.describe_resolved(arg),
                            bound_text
                        ),
                        span,
                        "E0277",
                    );
                }
            }
            TypeExpr::Fn {
                params,
                ret,
                span: bound_span,
            } => {
                self.push_generic_env(env);
                let mut expected_params = Vec::with_capacity(params.len());
                for param in params {
                    if let Some(ty) = self.resolve_ty(param, None) {
                        expected_params.push(ty);
                    }
                }
                let expected_ret = match self.resolve_ty(ret, None) {
                    Some(ty) => ty,
                    None => ITy::Unit,
                };
                self.pop_generic_env();
                let expected = ITy::function(expected_params, expected_ret);
                if !self.types_unify_probe(arg, &expected) {
                    let rendered = self.describe_resolved(&expected);
                    self.error_code(
                        format!(
                            "the trait bound ({}: {}) is not satisfied",
                            self.describe_resolved(arg),
                            rendered
                        ),
                        *bound_span,
                        "E0277",
                    );
                }
            }
            _ => {
                self.error_code(
                    "trait bounds must be trait names or Fn(..) -> T".to_string(),
                    bound.span(),
                    "E0277",
                );
            }
        }
    }

    /// Non-destructive unification probe: bindings made by the probe are
    /// rolled back.
    pub fn types_unify_probe(&mut self, lhs: &ITy, rhs: &ITy) -> bool {
        let snapshot = self.infer.clone();
        let result = self.infer.unify(lhs, rhs).is_ok();
        self.infer = snapshot;
        result
    }

    // -----------------------------------------------------------------------
    // Call-site argument inference
    // -----------------------------------------------------------------------

    /// Try to resolve the expected type of a parameter for use as an
    /// expression hint. Types that would require instantiating a generic
    /// container with still unknown arguments degrade to a fresh variable.
    pub fn resolve_param_hint(&mut self, ty: &TypeExpr) -> Option<ITy> {
        self.probing = true;
        let resolved = self.resolve_ty(ty, None);
        self.probing = false;
        resolved
    }

    /// Infer generic argument bindings from an already checked argument value
    /// against the declared parameter type. Returns false (and reports E0308)
    /// when the shapes are incompatible.
    pub fn infer_args_from_ty(
        &mut self,
        pattern: &TypeExpr,
        actual: &ITy,
        env: &mut HashMap<String, ITy>,
        report_span: Span,
    ) -> bool {
        let actual_resolved = self.infer.resolve(actual);
        if actual_resolved.is_never() {
            return true;
        }
        match pattern {
            TypeExpr::Path {
                name,
                name_span,
                generics,
            } => {
                if let Some(bound) = env.get(name).cloned() {
                    if self.infer.unify(&bound, &actual_resolved).is_err() {
                        if !self.probe_quiet {
                            let expected = self.infer.resolve(&bound);
                            self.report_mismatch(&expected, &actual_resolved, report_span);
                        }
                        return false;
                    }
                    return true;
                }
                match name.as_str() {
                    "Vec" | "Rc" | "Box" | "Option" => {
                        let actual_inner = match (&actual_resolved, name.as_str()) {
                            (ITy::Vec(inner), "Vec") => (**inner).clone(),
                            (ITy::Rc(inner), "Rc") => (**inner).clone(),
                            (ITy::Box(inner), "Box") => (**inner).clone(),
                            (ITy::Option(inner), "Option") => (**inner).clone(),
                            (ITy::Var(_), _) => {
                                let inner = self.infer.fresh_var();
                                let outer = match name.as_str() {
                                    "Vec" => ITy::vec(inner.clone()),
                                    "Rc" => ITy::rc(inner.clone()),
                                    "Box" => ITy::box_(inner.clone()),
                                    _ => ITy::option(inner.clone()),
                                };
                                let _ = self.infer.unify(&actual_resolved, &outer);
                                inner
                            }
                            _ => {
                                self.report_pattern_mismatch(
                                    pattern,
                                    &actual_resolved,
                                    report_span,
                                );
                                return false;
                            }
                        };
                        if let Some(arg) = generics.first() {
                            return self.infer_args_from_ty(arg, &actual_inner, env, report_span);
                        }
                        return true;
                    }
                    "Result" => {
                        let (ok, err) = match &actual_resolved {
                            ITy::Result(ok, err) => ((**ok).clone(), (**err).clone()),
                            ITy::Var(_) => {
                                let ok = self.infer.fresh_var();
                                let err = self.infer.fresh_var();
                                let rebuilt = ITy::result(ok.clone(), err.clone());
                                let _ = self.infer.unify(&actual_resolved, &rebuilt);
                                (ok, err)
                            }
                            _ => {
                                self.report_pattern_mismatch(
                                    pattern,
                                    &actual_resolved,
                                    report_span,
                                );
                                return false;
                            }
                        };
                        let mut ok_ok = true;
                        if let Some(arg) = generics.first() {
                            ok_ok = self.infer_args_from_ty(arg, &ok, env, report_span) && ok_ok;
                        }
                        if let Some(arg) = generics.get(1) {
                            ok_ok = self.infer_args_from_ty(arg, &err, env, report_span) && ok_ok;
                        }
                        return ok_ok;
                    }
                    "bool" | "String" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32"
                    | "u64" | "f32" | "f64" => {
                        let expected = match self.resolve_ty(pattern, None) {
                            Some(ty) => ty,
                            None => return false,
                        };
                        if self.infer.unify(&expected, &actual_resolved).is_err() {
                            if !self.probe_quiet {
                                self.report_mismatch(&expected, &actual_resolved, report_span);
                            }
                            return false;
                        }
                        return true;
                    }
                    _ => {}
                }
                if let Some(&def_id) = self.generic_struct_by_name.get(name) {
                    return self.infer_args_from_instance(
                        def_id,
                        true,
                        generics,
                        &actual_resolved,
                        env,
                        report_span,
                    );
                }
                if let Some(&def_id) = self.generic_enum_by_name.get(name) {
                    return self.infer_args_from_instance(
                        def_id,
                        false,
                        generics,
                        &actual_resolved,
                        env,
                        report_span,
                    );
                }
                let expected = match self.resolve_ty(pattern, None) {
                    Some(ty) => ty,
                    None => return false,
                };
                if self.infer.unify(&expected, &actual_resolved).is_err() {
                    if !self.probe_quiet {
                        self.report_mismatch(&expected, &actual_resolved, report_span);
                    }
                    return false;
                }
                let _ = name_span;
                true
            }
            TypeExpr::Tuple { elems, .. } => {
                let actual_elems = match &actual_resolved {
                    ITy::Tuple(actual_elems) => actual_elems.clone(),
                    ITy::Var(_) => {
                        let fresh: Vec<ITy> =
                            elems.iter().map(|_| self.infer.fresh_var()).collect();
                        let rebuilt = ITy::tuple(fresh.clone());
                        let _ = self.infer.unify(&actual_resolved, &rebuilt);
                        fresh
                    }
                    _ => {
                        self.report_pattern_mismatch(pattern, &actual_resolved, report_span);
                        return false;
                    }
                };
                if actual_elems.len() != elems.len() {
                    self.report_pattern_mismatch(pattern, &actual_resolved, report_span);
                    return false;
                }
                let mut ok = true;
                for (element, actual_element) in elems.iter().zip(actual_elems.iter()) {
                    ok = self.infer_args_from_ty(element, actual_element, env, report_span) && ok;
                }
                ok
            }
            TypeExpr::Fn { params, ret, span } => {
                let (actual_params, actual_ret) = match &actual_resolved {
                    ITy::Fn(actual_params, actual_ret) => {
                        (actual_params.clone(), (**actual_ret).clone())
                    }
                    ITy::Var(_) => {
                        let fresh: Vec<ITy> =
                            params.iter().map(|_| self.infer.fresh_var()).collect();
                        let fresh_ret = self.infer.fresh_var();
                        let rebuilt = ITy::function(fresh.clone(), fresh_ret.clone());
                        let _ = self.infer.unify(&actual_resolved, &rebuilt);
                        (fresh, fresh_ret)
                    }
                    _ => {
                        self.report_pattern_mismatch(pattern, &actual_resolved, report_span);
                        return false;
                    }
                };
                if actual_params.len() != params.len() {
                    self.error_code("mismatched function types".to_string(), *span, "E0308");
                    return false;
                }
                let mut ok = true;
                for (param_pattern, actual_param) in params.iter().zip(actual_params.iter()) {
                    ok = self.infer_args_from_ty(param_pattern, actual_param, env, report_span)
                        && ok;
                }
                ok = self.infer_args_from_ty(ret, &actual_ret, env, report_span) && ok;
                ok
            }
            TypeExpr::Ref { mutable, inner, .. } => match &actual_resolved {
                ITy::Ref {
                    mutable: actual_mutable,
                    inner: actual_inner,
                } => {
                    if *mutable != *actual_mutable {
                        self.report_pattern_mismatch(pattern, &actual_resolved, report_span);
                        return false;
                    }
                    self.infer_args_from_ty(inner, actual_inner, env, report_span)
                }
                ITy::Var(_) => {
                    let inner_ty = self.infer.fresh_var();
                    let rebuilt = ITy::Ref {
                        mutable: *mutable,
                        inner: Box::new(inner_ty.clone()),
                    };
                    let _ = self.infer.unify(&actual_resolved, &rebuilt);
                    self.infer_args_from_ty(inner, &inner_ty, env, report_span)
                }
                _ => {
                    self.report_pattern_mismatch(pattern, &actual_resolved, report_span);
                    false
                }
            },
        }
    }

    fn infer_args_from_instance(
        &mut self,
        def_id: usize,
        is_struct: bool,
        pattern_args: &[TypeExpr],
        actual: &ITy,
        env: &mut HashMap<String, ITy>,
        report_span: Span,
    ) -> bool {
        let instance = match actual {
            ITy::Struct(id) if is_struct => self
                .structs
                .get(*id)
                .and_then(|info| info.generic.as_ref())
                .filter(|instance| instance.def_id == def_id)
                .map(|instance| instance.args.clone()),
            ITy::Enum(id) if !is_struct => self
                .enums
                .get(*id)
                .and_then(|info| info.generic.as_ref())
                .filter(|instance| instance.def_id == def_id)
                .map(|instance| instance.args.clone()),
            ITy::Var(_) => return true,
            _ => None,
        };
        let instance_args = match instance {
            Some(args) => args,
            None => {
                self.report_pattern_mismatch(
                    &TypeExpr::Path {
                        name: String::new(),
                        name_span: report_span,
                        generics: pattern_args.to_vec(),
                    },
                    actual,
                    report_span,
                );
                return false;
            }
        };
        if pattern_args.len() != instance_args.len() {
            self.report_pattern_mismatch(
                &TypeExpr::Path {
                    name: String::new(),
                    name_span: report_span,
                    generics: pattern_args.to_vec(),
                },
                actual,
                report_span,
            );
            return false;
        }
        let mut ok = true;
        for (pattern_arg, instance_arg) in pattern_args.iter().zip(instance_args.iter()) {
            ok = self.infer_args_from_ty(pattern_arg, instance_arg, env, report_span) && ok;
        }
        ok
    }

    fn report_pattern_mismatch(&mut self, pattern: &TypeExpr, actual: &ITy, span: Span) {
        if self.probe_quiet {
            return;
        }
        let expected_text = pattern.describe();
        let found = self.describe_resolved(actual);
        let diag = Diagnostic::error("mismatched types".to_string(), span)
            .with_code("E0308")
            .with_span_label(span)
            .with_note(format!("expected {}, found {}", expected_text, found));
        self.diagnostics.push(diag);
    }

    // -----------------------------------------------------------------------
    // Drop hooks for generic types
    // -----------------------------------------------------------------------

    /// Instantiate a generic Drop impl for a freshly created concrete instance.
    pub fn instantiate_drop_hook(
        &mut self,
        key: super::items::TypeKey,
        instance_ty: &ITy,
        span: Span,
    ) {
        let existing = match key {
            super::items::TypeKey::Struct(id) => self.drop_structs.get(id).copied().flatten(),
            super::items::TypeKey::Enum(id) => self.drop_enums.get(id).copied().flatten(),
        };
        if existing.is_some() {
            return;
        }
        let candidates: Vec<usize> = self
            .impls
            .iter()
            .filter(|info| {
                info.trait_ref
                    .as_ref()
                    .map(|trait_ref| trait_ref.name == "Drop")
                    .unwrap_or(false)
                    && !info.methods.is_empty()
            })
            .map(|info| info.id)
            .collect();
        for impl_id in candidates {
            let args = match self.match_impl_self(impl_id, instance_ty) {
                Some(args) => args,
                None => continue,
            };
            let info = match self.impls.get(impl_id) {
                Some(info) => info.clone(),
                None => continue,
            };
            let gfn_id = match info.methods.first() {
                Some(method) => method.gfn_id,
                None => continue,
            };
            if let Some(fn_id) = self.instantiate_generic_fn(gfn_id, args, span) {
                match key {
                    super::items::TypeKey::Struct(id) => {
                        if id < self.drop_structs.len() {
                            self.drop_structs[id] = Some(fn_id);
                        }
                    }
                    super::items::TypeKey::Enum(id) => {
                        if id < self.drop_enums.len() {
                            self.drop_enums[id] = Some(fn_id);
                        }
                    }
                }
                return;
            }
        }
    }
}
