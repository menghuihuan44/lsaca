//! Checker core: state, scopes, diagnostics helpers and function checking.

use std::collections::{HashMap, HashSet};

use super::dyn_traits::{TraitObjectInfo, VtableInfo};
use super::generics::{DefOrigin, GEnumDef, GFnDef, GStructDef, ImplInfo};
use super::items::{
    AliasInfo, ConstItem, ConstState, ConstVal, EnumInfo, FnInfo, StructInfo, TypeKey,
};
use super::itype::{ITy, Infer};
use super::modules::{ImportLeaf, ModuleId, ModuleInfo};
use super::refs::Provenance;
use super::traits::TraitInfo;
use super::tree::{TBlock, TClosure, TExpr, TStmt};
use crate::ast::ImplBlock;
use crate::diag::{Diagnostic, Diagnostics};
use crate::span::{SourceLookup, Span};
use crate::types::{SelfKind, Ty};

/// A local binding in scope.
#[derive(Clone, Debug)]
pub struct LocalInfo {
    pub name: String,
    pub c_name: String,
    pub ty: ITy,
    pub mutable: bool,
}

/// A value captured by a closure.
#[derive(Clone, Debug)]
pub struct CaptureInfo {
    pub name: String,
    pub ty: ITy,
    pub mutable: bool,
    /// Expression reading the captured value in the enclosing frame.
    pub parent_expr: TExpr,
}

/// One function or closure body being checked.
pub struct Frame {
    pub ret: ITy,
    pub scopes: Vec<HashMap<String, LocalInfo>>,
    pub captures: Vec<CaptureInfo>,
    /// Capture entries indexed by the original local C name.
    pub capture_map: HashMap<String, usize>,
    pub is_closure: bool,
    pub loop_depth: usize,
}

impl Frame {
    pub fn new(ret: ITy) -> Frame {
        Frame {
            ret,
            scopes: vec![HashMap::new()],
            captures: Vec::new(),
            capture_map: HashMap::new(),
            is_closure: false,
            loop_depth: 0,
        }
    }

    pub fn find_local(&self, name: &str) -> Option<&LocalInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info);
            }
        }
        None
    }

    pub fn find_local_by_c_name(&self, c_name: &str) -> Option<&LocalInfo> {
        for scope in self.scopes.iter().rev() {
            for info in scope.values() {
                if info.c_name == c_name {
                    return Some(info);
                }
            }
        }
        None
    }
}

pub struct Checker<'a> {
    /// The source files of the program being checked. Diagnostics produced by
    /// the checker carry global spans, so rendering them needs the same map.
    #[allow(dead_code)]
    pub sources: &'a dyn SourceLookup,
    pub structs: Vec<StructInfo>,
    pub enums: Vec<EnumInfo>,
    pub aliases: Vec<AliasInfo>,
    pub consts: Vec<ConstItem>,
    pub fns: Vec<FnInfo>,
    /// Methods and associated functions keyed by (owner type, name).
    pub methods: HashMap<(TypeKey, String), usize>,
    // -- version 1.7 module tree --------------------------------------------
    /// One module per crate root, declared module and the embedded prelude.
    pub modules: Vec<ModuleInfo>,
    /// Items collected for every module, parallel to `modules`.
    pub module_items: Vec<Vec<crate::ast::Item>>,
    /// Module whose scope is used to resolve names right now.
    pub current_module: ModuleId,
    /// Module that privacy is evaluated from. Normally this is
    /// [Self::current_module], but descending a path into another module keeps
    /// the module the path is written in, so private items of the target module
    /// stay private exactly as in Rust.
    pub access_module: Option<ModuleId>,
    /// Root module of every crate in the graph.
    pub crate_roots: Vec<ModuleId>,
    pub crate_names: Vec<String>,
    pub crate_is_dependency: Vec<bool>,
    /// Dependency crates reachable through the extern prelude.
    pub crate_by_name: HashMap<String, usize>,
    pub root_crate: usize,
    /// Root module of the embedded prelude, if any.
    pub prelude_module: Option<ModuleId>,
    /// Use-tree leaves collected but not resolved yet.
    pub pending_imports: Vec<ImportLeaf>,
    pub used_c_names: HashSet<String>,
    pub c_name_counter: usize,
    pub infer: Infer,
    pub frames: Vec<Frame>,
    pub diagnostics: Diagnostics,
    /// Closure bodies by id. Slots are reserved before the body is checked so
    /// ids are deterministic even for nested closures.
    pub closures: Vec<Option<TClosure>>,
    pub local_counter: usize,
    pub closure_counter: usize,
    pub current_fn: usize,
    /// Names bound by the pattern currently being checked.
    pub pattern_names: Vec<HashMap<String, Span>>,
    /// True while checking a constant initializer (locals are not visible).
    pub in_const: bool,
    /// Match ergonomics binding mode of the pattern currently being checked:
    /// None binds by value, Some(false) binds through a shared reference and
    /// Some(true) binds through a mutable reference.
    pub pattern_binding_mode: Option<bool>,
    /// Provenance of reference-valued locals of the function being checked.
    pub ref_local_sources: HashMap<String, Provenance>,
    /// Drop hook (a method id in `fns`) for every struct, filled in by
    /// `impl Drop for T` registration. `None` means the type has no user drop
    /// glue of its own (its fields may still have some).
    pub drop_structs: Vec<Option<usize>>,
    /// Drop hook (a method id in `fns`) for every enum, see [Self::drop_structs].
    pub drop_enums: Vec<Option<usize>>,
    /// Types with a user `impl Drop` hook that enclose the pattern currently
    /// being checked. Destructuring them can never move non-Copy parts out
    /// (E0509), so by-value bindings below such a layer are rejected.
    pub pattern_drop_hook_stack: Vec<ITy>,
    /// Generic struct declarations, instantiated on demand.
    pub generic_structs: Vec<GStructDef>,
    /// Generic enum declarations, instantiated on demand.
    pub generic_enums: Vec<GEnumDef>,
    /// Generic function definitions (free functions and generic impl methods).
    pub generic_fns: Vec<GFnDef>,
    /// Registered impl blocks (inherent generic and trait impls).
    pub impls: Vec<ImplInfo>,
    /// Impl blocks waiting to be registered once every program (user and
    /// prelude) has registered its type and trait names. Deferring them lets
    /// user impls name traits that come from the prelude (for example
    /// `Iterator`) regardless of collection order.
    pub pending_impls: Vec<(ImplBlock, DefOrigin, ModuleId)>,
    /// Registered trait declarations.
    pub traits: Vec<TraitInfo>,
    /// Generic parameter environments, innermost last. Also holds "Self" for
    /// instantiated impl methods.
    pub generic_env: Vec<HashMap<String, ITy>>,
    /// Instantiated generic function ids whose bodies still need checking.
    pub pending_fns: Vec<usize>,
    /// Origin assigned to items registered right now.
    pub collecting_origin: DefOrigin,
    /// True while registering the embedded standard library: its items are
    /// recorded as `pub` so user code can see them.
    pub force_public_items: bool,
    /// True while resolving a type only to build an expression hint; generic
    /// instantiations with unknown arguments degrade to a fresh variable
    /// instead of reporting E0282.
    pub probing: bool,
    /// True while probing candidate impls: shape mismatches are not reported.
    pub probe_quiet: bool,
    /// True while resolving the signature of a deferred generic function
    /// instantiation. User generic structs mentioned in that signature are
    /// instantiated with their still-open inference variables preserved, so a
    /// later constraint (for example a `collect` target type) can determine
    /// them; the instance is named once those variables are resolved.
    pub defer_open_instances: bool,
    /// Key types of invalid HashMap instantiations that already produced a
    /// diagnostic. A HashMap type is validated both where it is annotated and
    /// where a value is created, but only one diagnostic is reported.
    pub reported_hash_map_key_errors: HashSet<String>,
    /// Trait objects (`dyn Trait<...>`) in first-resolution order. `ITy::Dyn`
    /// stores an index into this table.
    pub trait_objects: Vec<TraitObjectInfo>,
    /// Vtables for (trait object, concrete self type) pairs, in first-coercion
    /// order. Shared by every pointer form of the same concrete type.
    pub vtables: Vec<VtableInfo>,
    /// Index of a trait object by (trait id, resolved arguments).
    pub trait_object_index: HashMap<(usize, Vec<ITy>), usize>,
    /// Index of a vtable by (trait object, concrete self type).
    pub vtable_index: HashMap<(usize, ITy), usize>,
    /// Traits that already produced an object-safety diagnostic.
    pub reported_object_unsafe: HashSet<usize>,
}

/// Internal standard-library items are prefixed so user code cannot collide
/// with them (the prefix is reserved by the embedded prelude). It never belongs
/// in a user-facing diagnostic, so it is stripped when a name is rendered.
fn diagnostic_item_name(name: &str) -> String {
    name.strip_prefix("__lsaca_").unwrap_or(name).to_string()
}

impl<'a> Checker<'a> {
    pub fn new(sources: &'a dyn SourceLookup) -> Checker<'a> {
        Checker {
            sources,
            structs: Vec::new(),
            enums: Vec::new(),
            aliases: Vec::new(),
            consts: Vec::new(),
            fns: Vec::new(),
            methods: HashMap::new(),
            modules: Vec::new(),
            module_items: Vec::new(),
            current_module: 0,
            access_module: None,
            crate_roots: Vec::new(),
            crate_names: Vec::new(),
            crate_is_dependency: Vec::new(),
            crate_by_name: HashMap::new(),
            root_crate: 0,
            prelude_module: None,
            pending_imports: Vec::new(),
            used_c_names: HashSet::new(),
            c_name_counter: 0,
            infer: Infer::new(),
            frames: Vec::new(),
            diagnostics: Diagnostics::new(),
            closures: Vec::new(),
            local_counter: 0,
            closure_counter: 0,
            current_fn: 0,
            pattern_names: Vec::new(),
            in_const: false,
            pattern_binding_mode: None,
            ref_local_sources: HashMap::new(),
            drop_structs: Vec::new(),
            drop_enums: Vec::new(),
            pattern_drop_hook_stack: Vec::new(),
            generic_structs: Vec::new(),
            generic_enums: Vec::new(),
            generic_fns: Vec::new(),
            impls: Vec::new(),
            pending_impls: Vec::new(),
            traits: Vec::new(),
            generic_env: Vec::new(),
            pending_fns: Vec::new(),
            collecting_origin: DefOrigin::User,
            force_public_items: false,
            probing: false,
            probe_quiet: false,
            defer_open_instances: false,
            reported_hash_map_key_errors: HashSet::new(),
            trait_objects: Vec::new(),
            vtables: Vec::new(),
            trait_object_index: HashMap::new(),
            vtable_index: HashMap::new(),
            reported_object_unsafe: HashSet::new(),
        }
    }

    // -- module context -----------------------------------------------------

    /// Run `f` with `module` as the module whose scope owns the item being
    /// resolved, then restore the previous module.
    ///
    /// Type expressions are owned by the item that declares them (an impl's
    /// self type, a generic function's signature, a trait's method signature).
    /// Resolving such a type while checking an instantiation triggered from a
    /// different module must temporarily switch the scope to the declaring
    /// module; otherwise a prelude adapter instantiated for a user type could
    /// not name that user type.
    pub fn with_module<R>(&mut self, module: ModuleId, f: impl FnOnce(&mut Self) -> R) -> R {
        let previous = self.current_module;
        self.current_module = module;
        let result = f(self);
        self.current_module = previous;
        result
    }
    /// The module privacy checks are evaluated from.
    pub fn privacy_module(&self) -> ModuleId {
        self.access_module.unwrap_or(self.current_module)
    }

    /// Run `f` with `module` as the resolution scope while keeping privacy
    /// evaluated from the module that named the path.
    ///
    /// Descending `outer::inner` must not make private items of `outer` or
    /// `inner` visible: a path is still written in the module it appears in.
    pub fn with_resolved_module<R>(
        &mut self,
        module: ModuleId,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous_module = self.current_module;
        let previous_access = self.access_module;
        self.access_module = Some(previous_module);
        self.current_module = module;
        let result = f(self);
        self.current_module = previous_module;
        self.access_module = previous_access;
        result
    }

    // -- diagnostics --------------------------------------------------------

    pub fn error_at(&mut self, message: String, span: Span) {
        self.diagnostics.push(Diagnostic::error(message, span));
    }

    pub fn error_code(&mut self, message: String, span: Span, code: &'static str) {
        self.diagnostics
            .push(Diagnostic::error(message, span).with_code(code));
    }

    /// Report a coded error with one explanatory note.
    pub fn error_code_note(
        &mut self,
        message: String,
        span: Span,
        code: &'static str,
        note: impl Into<String>,
    ) {
        self.diagnostics.push(
            Diagnostic::error(message, span)
                .with_code(code)
                .with_note(note),
        );
    }

    pub fn warning_code(&mut self, message: String, span: Span, code: &'static str) {
        self.diagnostics
            .push(Diagnostic::warning(message, span).with_code(code));
    }

    /// Report a mismatched-types error with a span label.
    pub fn report_mismatch(&mut self, expected: &ITy, found: &ITy, span: Span) {
        let expected = self.describe(expected);
        let found = self.describe(found);
        let diag = Diagnostic::error("mismatched types".to_string(), span)
            .with_code("E0308")
            .with_span_label(span)
            .with_note(format!("expected {}, found {}", expected, found));
        self.diagnostics.push(diag);
    }

    /// Unify two types, reporting a mismatch when that fails.
    pub fn unify_or_report(&mut self, expected: &ITy, found: &ITy, span: Span) -> bool {
        let expected_resolved = self.infer.resolve(expected);
        let found_resolved = self.infer.resolve(found);
        match self.infer.unify(&expected_resolved, &found_resolved) {
            Ok(()) => true,
            Err(()) => {
                self.report_mismatch(&expected_resolved, &found_resolved, span);
                false
            }
        }
    }

    // -- naming -------------------------------------------------------------

    /// Allocate a fresh local variable C name.
    pub fn fresh_local_c_name(&mut self) -> String {
        self.local_counter += 1;
        format!("v{}", self.local_counter)
    }

    pub fn fresh_closure_c_name(&mut self) -> String {
        let id = self.closure_counter;
        self.closure_counter += 1;
        format!("lsaca_closure_{}", id)
    }

    pub fn reserve_closure(&mut self) -> usize {
        self.closures.push(None);
        self.closures.len() - 1
    }

    pub fn set_closure(&mut self, id: usize, closure: TClosure) {
        if id < self.closures.len() {
            self.closures[id] = Some(closure);
        }
    }

    // -- type helpers -------------------------------------------------------

    pub fn type_name(&self, key: TypeKey) -> String {
        match key {
            TypeKey::Struct(id) => self.struct_name(id),
            TypeKey::Enum(id) => self.enum_name(id),
        }
    }

    /// Display name of a struct for diagnostics. The embedded standard library
    /// is an implementation detail, so its internal `__lsaca_` prefix is not
    /// part of any user-visible message (adapters show as `VecIter<i64>`).
    pub fn struct_name(&self, id: usize) -> String {
        self.structs
            .get(id)
            .map(|info| diagnostic_item_name(&info.name))
            .unwrap_or_else(|| "?".to_string())
    }

    /// Display name of an enum for diagnostics; see [Checker::struct_name].
    pub fn enum_name(&self, id: usize) -> String {
        self.enums
            .get(id)
            .map(|info| diagnostic_item_name(&info.name))
            .unwrap_or_else(|| "?".to_string())
    }

    pub fn variant_name(&self, enum_id: usize, variant: usize) -> String {
        self.enums
            .get(enum_id)
            .and_then(|info| info.variants.get(variant))
            .map(|variant| variant.name.clone())
            .unwrap_or_else(|| "?".to_string())
    }

    pub fn variant_fields(&self, enum_id: usize, variant: usize) -> Vec<ITy> {
        self.enums
            .get(enum_id)
            .and_then(|info| info.variants.get(variant))
            .map(|variant| variant.fields.clone())
            .unwrap_or_default()
    }

    pub fn variant_count(&self, enum_id: usize) -> usize {
        self.enums
            .get(enum_id)
            .map(|info| info.variants.len())
            .unwrap_or(0)
    }

    pub fn struct_fields(&self, id: usize) -> Vec<(String, Span, ITy)> {
        self.structs
            .get(id)
            .map(|info| {
                info.fields
                    .iter()
                    .map(|field| (field.name.clone(), field.name_span, field.ty.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Render a type for diagnostics.
    pub fn describe(&self, ty: &ITy) -> String {
        let ty = self.infer.resolve(ty);
        self.describe_resolved(&ty)
    }

    pub fn describe_resolved(&self, ty: &ITy) -> String {
        match ty {
            ITy::Unit => "()".to_string(),
            ITy::Bool => "bool".to_string(),
            ITy::Char => "char".to_string(),
            ITy::Int(int_ty) => int_ty.name().to_string(),
            ITy::Float(float_ty) => float_ty.name().to_string(),
            ITy::String => "String".to_string(),
            ITy::Vec(elem) => format!("Vec<{}>", self.describe_resolved(elem)),
            ITy::Rc(elem) => format!("Rc<{}>", self.describe_resolved(elem)),
            ITy::Box(elem) => format!("Box<{}>", self.describe_resolved(elem)),
            ITy::Ref { mutable, inner } => {
                let inner = self.describe_resolved(inner);
                if *mutable {
                    format!("&mut {}", inner)
                } else {
                    format!("&{}", inner)
                }
            }
            ITy::Option(elem) => format!("Option<{}>", self.describe_resolved(elem)),
            ITy::Result(ok, err) => format!(
                "Result<{}, {}>",
                self.describe_resolved(ok),
                self.describe_resolved(err)
            ),
            ITy::HashMap(key, value) => format!(
                "HashMap<{}, {}>",
                self.describe_resolved(key),
                self.describe_resolved(value)
            ),
            ITy::Tuple(elems) => {
                let parts: Vec<String> = elems
                    .iter()
                    .map(|elem| self.describe_resolved(elem))
                    .collect();
                format!("({})", parts.join(", "))
            }
            ITy::Fn(params, ret) => {
                let parts: Vec<String> = params
                    .iter()
                    .map(|param| self.describe_resolved(param))
                    .collect();
                format!(
                    "fn({}) -> {}",
                    parts.join(", "),
                    self.describe_resolved(ret)
                )
            }
            ITy::Struct(id) => self.struct_name(*id),
            ITy::Enum(id) => self.enum_name(*id),
            ITy::Slice(elem) => format!("[{}]", self.describe_resolved(elem)),
            ITy::Str => "str".to_string(),
            ITy::Dyn(id) => match self.trait_objects.get(*id) {
                Some(object) if object.args.is_empty() => format!("dyn {}", object.name),
                Some(object) => {
                    let args: Vec<String> = object
                        .args
                        .iter()
                        .map(|arg| self.describe_resolved(arg))
                        .collect();
                    format!("dyn {}<{}>", object.name, args.join(", "))
                }
                None => format!("dyn trait{}", id),
            },
            ITy::Never => "!".to_string(),
            ITy::Var(id) => match self.infer.var_kind(*id) {
                super::itype::VarKind::IntLiteral => "{integer}".to_string(),
                super::itype::VarKind::FloatLiteral => "{float}".to_string(),
                super::itype::VarKind::General => "_".to_string(),
            },
        }
    }

    pub fn const_value(&self, id: usize) -> Option<(Ty, ConstVal)> {
        match &self.consts.get(id)?.state {
            ConstState::Done { ty, value } => Some((ty.clone(), value.clone())),
            _ => None,
        }
    }

    // -- scopes -------------------------------------------------------------

    pub fn push_scope(&mut self) {
        if let Some(frame) = self.frames.last_mut() {
            frame.scopes.push(HashMap::new());
        }
    }

    pub fn pop_scope(&mut self) {
        if let Some(frame) = self.frames.last_mut()
            && frame.scopes.len() > 1
        {
            frame.scopes.pop();
        }
    }

    /// Declare a binding in the innermost scope of the current frame.
    pub fn declare_local(&mut self, name: &str, ty: ITy, mutable: bool, span: Span) -> String {
        let c_name = self.fresh_local_c_name();
        self.declare_local_with_c_name(name, ty, mutable, c_name.clone(), span);
        c_name
    }

    pub fn declare_local_with_c_name(
        &mut self,
        name: &str,
        ty: ITy,
        mutable: bool,
        c_name: String,
        _span: Span,
    ) {
        let info = LocalInfo {
            name: name.to_string(),
            c_name,
            ty,
            mutable,
        };
        if let Some(frame) = self.frames.last_mut()
            && let Some(scope) = frame.scopes.last_mut()
        {
            scope.insert(name.to_string(), info);
        }
    }

    /// Find a local binding by name, innermost frame and scope first.
    pub fn find_local(&self, name: &str) -> Option<(usize, LocalInfo)> {
        for (index, frame) in self.frames.iter().enumerate().rev() {
            if let Some(info) = frame.find_local(name) {
                return Some((index, info.clone()));
            }
        }
        None
    }

    /// Resolve a bare name to a local or captured value expression, inserting
    /// capture entries into every intervening closure frame as needed.
    pub fn resolve_local_name(&mut self, name: &str, span: Span) -> Option<TExpr> {
        let (level, info) = self.find_local(name)?;
        let last = self.frames.len().checked_sub(1)?;
        if level == last {
            return Some(TExpr::new(
                info.ty.clone(),
                super::tree::TExprKind::Local {
                    name: info.name.clone(),
                    c_name: info.c_name.clone(),
                    mutable: info.mutable,
                },
                span,
            ));
        }
        let mut expr = TExpr::new(
            info.ty.clone(),
            super::tree::TExprKind::Local {
                name: info.name.clone(),
                c_name: info.c_name.clone(),
                mutable: info.mutable,
            },
            span,
        );
        for level_index in (level + 1)..=last {
            expr = self.ensure_capture(
                level_index,
                &info.name,
                &info.c_name,
                &info.ty,
                info.mutable,
                span,
            );
        }
        Some(expr)
    }

    fn ensure_capture(
        &mut self,
        level: usize,
        name: &str,
        c_name: &str,
        ty: &ITy,
        mutable: bool,
        span: Span,
    ) -> TExpr {
        if let Some(index) = self.frames[level].capture_map.get(c_name) {
            let index = *index;
            return TExpr::new(
                ty.clone(),
                super::tree::TExprKind::Capture {
                    index,
                    name: name.to_string(),
                },
                span,
            );
        }
        if self.type_contains_ref(ty) {
            self.error_code(
                "references cannot be captured by closures".to_string(),
                span,
                "E0498",
            );
        }
        let parent_expr = self.capture_parent_expr(level, name, c_name, ty, mutable, span);
        let info = CaptureInfo {
            name: name.to_string(),
            ty: ty.clone(),
            mutable,
            parent_expr,
        };
        let index = self.frames[level].captures.len();
        self.frames[level].captures.push(info);
        self.frames[level]
            .capture_map
            .insert(c_name.to_string(), index);
        TExpr::new(
            ty.clone(),
            super::tree::TExprKind::Capture {
                index,
                name: name.to_string(),
            },
            span,
        )
    }

    fn capture_parent_expr(
        &mut self,
        level: usize,
        name: &str,
        c_name: &str,
        ty: &ITy,
        mutable: bool,
        span: Span,
    ) -> TExpr {
        if level == 0 {
            return TExpr::new(ty.clone(), super::tree::TExprKind::Break, span);
        }
        if let Some(info) = self.frames[level - 1].find_local_by_c_name(c_name) {
            return TExpr::new(
                info.ty.clone(),
                super::tree::TExprKind::Local {
                    name: info.name.clone(),
                    c_name: info.c_name.clone(),
                    mutable: info.mutable,
                },
                span,
            );
        }
        if self.frames[level - 1].capture_map.contains_key(c_name) {
            return self.ensure_capture(level - 1, name, c_name, ty, mutable, span);
        }
        TExpr::new(ty.clone(), super::tree::TExprKind::Break, span)
    }

    /// The impl target of the function currently being checked.
    pub fn current_self_key(&self) -> Option<TypeKey> {
        self.fns.get(self.current_fn).and_then(|info| info.owner)
    }

    pub fn in_loop(&self) -> bool {
        self.frames
            .last()
            .map(|frame| frame.loop_depth > 0)
            .unwrap_or(false)
    }

    // -- function bodies ----------------------------------------------------

    /// Check every function body: first all concrete user functions, then every
    /// function instantiated from a generic body, in instantiation order.
    pub fn check_all_fns(&mut self) {
        let mut id = 0;
        while id < self.fns.len() {
            if self.fns[id].generic.is_none() {
                self.check_fn(id);
            }
            id += 1;
        }
        let mut cursor = 0;
        while cursor < self.pending_fns.len() {
            let fn_id = self.pending_fns[cursor];
            cursor += 1;
            self.check_fn(fn_id);
        }
    }

    pub fn check_fn(&mut self, id: usize) {
        if self.fns[id].checked.is_some() {
            return;
        }
        let sig = match self.fns[id].sig.clone() {
            Some(sig) => sig,
            None => return,
        };
        let decl = self.fns[id].decl.clone();
        // An extern function has no body: there is nothing to type-check.
        // HIR lowering turns it into an intrinsic declaration with an empty
        // block, so this function stays unchecked rather than a checked unit.
        let Some(body_src) = decl.body.clone() else {
            return;
        };
        let param_cnames = self.fns[id].param_cnames.clone();
        let self_kind = self.fns[id].self_kind;
        let origin = self.fns[id].origin;
        let generic = self.fns[id].generic.clone();
        let self_ty = self.fns[id].self_ty.clone();
        // A deferred instance is still generic at this point: the call site did
        // not determine its type arguments. Its standard-library body calls
        // generic functions that need them, so checking it would report an E0282
        // from inside the prelude (an internal error). The inference failure
        // belongs to the caller: report it as an ordinary user diagnostic at the
        // recorded call site instead.
        if let Some(span) = self.fns[id].deferred_span {
            // Variables pinned to an integer or float literal normalize to i32
            // or f64, so only genuinely open arguments make the body checkable.
            let args = self.fns[id]
                .generic
                .as_ref()
                .map(|instance| instance.args.clone());
            match args.map(|args| self.normalize_args(&args)) {
                Some(None) => {
                    self.error_code_note(
                        "type annotations needed".to_string(),
                        span,
                        "E0282",
                        "cannot infer the type arguments for this call; annotate the binding or the expected type"
                            .to_string(),
                    );
                    return;
                }
                Some(Some(concrete)) => {
                    // Deferred instantiation postponed the trait-bound check until
                    // the type arguments of this instance are known. The caller
                    // receives the E0277 at its own call site, like every concrete
                    // instantiation; the body itself is legal under the declared
                    // bounds and is not monomorphized for an invalid instance.
                    if let Some(instance) = generic.as_ref()
                        && let Some(def) = self.generic_fns.get(instance.def_id).cloned()
                    {
                        let errors_before = self.diagnostics.error_count();
                        self.validate_generic_bounds(&def.generics, &concrete, span, def.module);
                        if self.diagnostics.error_count() > errors_before {
                            return;
                        }
                    }
                }
                None => {}
            }
        }
        let diagnostics_before = self.diagnostics.len();
        let previous_module = self.current_module;
        self.current_module = self.fns[id].module;
        self.current_fn = id;
        let mut pushed_env = false;
        if let Some(instance) = generic
            && let Some(def) = self.generic_fns.get(instance.def_id).cloned()
        {
            let mut env = HashMap::new();
            for (param, arg) in def.generics.iter().zip(instance.args.iter()) {
                env.insert(param.name.clone(), arg.clone());
            }
            if let Some(self_ty) = self_ty {
                env.insert("Self".to_string(), self_ty);
            }
            self.push_generic_env(env);
            pushed_env = true;
        }
        self.ref_local_sources.clear();
        let mut frame = Frame::new(sig.ret.clone());
        for (index, param) in decl.params.iter().enumerate() {
            let ty = sig.params.get(index).cloned().unwrap_or(ITy::Unit);
            let mut mutable = param.mutable;
            if param.name == "self" && self_kind == Some(SelfKind::RefMut) {
                mutable = true;
            }
            let c_name = param_cnames
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("p{}", index));
            frame.scopes[0].insert(
                param.name.clone(),
                LocalInfo {
                    name: param.name.clone(),
                    c_name,
                    ty,
                    mutable,
                },
            );
        }
        self.frames.push(frame);
        let mut body = self.check_block(&body_src, Some(&sig.ret));
        self.frames.pop();
        if pushed_env {
            self.pop_generic_env();
        }
        // Coercions apply to return expressions, and the implicit return is the
        // tail expression of the body block.
        if let Some(tail) = body.tail.take() {
            let tail_span = tail.span;
            let coerced = self.coerce_to(*tail, &sig.ret, tail_span);
            body.tail = Some(Box::new(coerced));
        }
        let body_ty = block_type(&body);
        if !self.unify_or_report(&sig.ret, &body_ty, body_src.span) {
            // error already reported
        }
        if let Some(tail) = &body.tail {
            let source = sig.ret_ref_source;
            let tail = (**tail).clone();
            self.check_returned_reference(&tail, source, tail.span);
        }
        self.fns[id].checked = Some(body);
        if origin.is_prelude() {
            // Diagnostics produced while checking a std-library body are
            // compiler bugs from the user's point of view: report them as
            // internal standard-library errors without prelude spans.
            let collected = std::mem::replace(&mut self.diagnostics, Diagnostics::new());
            let mut items = collected.into_vec();
            let tail = items.split_off(diagnostics_before.min(items.len()));
            for diagnostic in items {
                self.diagnostics.push(diagnostic);
            }
            for diagnostic in tail {
                self.diagnostics
                    .push(crate::prelude::internal_error(diagnostic));
            }
        }
        self.current_module = previous_module;
    }

    /// Check a block, opening a fresh scope for its statements.
    pub fn check_block(&mut self, block: &crate::ast::Block, hint: Option<&ITy>) -> TBlock {
        self.push_scope();
        let mut stmts = Vec::new();
        for stmt in &block.stmts {
            if let Some(stmt) = self.check_stmt(stmt) {
                stmts.push(stmt);
            }
        }
        let tail = block
            .tail
            .as_ref()
            .map(|expr| Box::new(self.check_expr(expr, hint)));
        self.pop_scope();
        TBlock {
            stmts,
            tail,
            span: block.span,
        }
    }

    fn check_stmt(&mut self, stmt: &crate::ast::Stmt) -> Option<TStmt> {
        use crate::ast::Stmt;
        match stmt {
            Stmt::Let(let_stmt) => self.check_let_stmt(let_stmt),
            Stmt::Expr(expr) => Some(TStmt::Expr(self.check_expr(expr, None))),
            Stmt::Item(_) => None,
            Stmt::Empty(_) => None,
        }
    }

    fn check_let_stmt(&mut self, stmt: &crate::ast::LetStmt) -> Option<TStmt> {
        let self_key = self.current_self_key();
        let ty_span = match &stmt.ty {
            Some(ty) => ty.span(),
            None => stmt.span,
        };
        let annotated = match &stmt.ty {
            Some(ty) => self.resolve_ty(ty, self_key),
            None => None,
        };
        match &stmt.init {
            Some(init) => {
                let mut init_expr = self.check_expr(init, annotated.as_ref());
                let expected = match annotated {
                    Some(annotated) => {
                        self.validate_value_type(&annotated, ty_span);
                        init_expr = self.coerce_to(init_expr, &annotated, init.span);
                        annotated
                    }
                    None => init_expr.ty.clone(),
                };
                let pat = self.check_irrefutable_pattern(&stmt.pat, &expected, stmt.span);
                if matches!(self.infer.resolve(&pat.ty), ITy::Ref { .. })
                    && let super::tree::TPatKind::Bind { c_name, .. } = &pat.kind
                {
                    let source = self.reference_source(&init_expr);
                    self.ref_local_sources.insert(c_name.clone(), source);
                }
                Some(TStmt::Let {
                    pat,
                    init: Some(init_expr),
                    span: stmt.span,
                })
            }
            None => {
                self.error_code(
                    "cannot declare a binding without an initializer".to_string(),
                    stmt.span,
                    "E0001",
                );
                let expected = annotated.unwrap_or(ITy::Unit);
                let _ = self.check_irrefutable_pattern(&stmt.pat, &expected, stmt.span);
                None
            }
        }
    }
}

/// The type of a block: its tail expression type, or a diverging last
/// statement, or unit.
pub fn block_type(block: &TBlock) -> ITy {
    if let Some(tail) = &block.tail {
        return tail.ty.clone();
    }
    if let Some(TStmt::Expr(expr)) = block.stmts.last()
        && self_is_never(&expr.ty)
    {
        return ITy::Never;
    }
    ITy::Unit
}

fn self_is_never(ty: &ITy) -> bool {
    ty.is_never()
}
