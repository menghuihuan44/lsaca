//! Module trees, per-module name scopes and path resolution for version 1.7.
//!
//! Every crate of the crate graph becomes a root module; inline and external
//! mod declarations become child modules. Each module owns its type and value
//! namespaces, its child modules and its use aliases. Path resolution walks
//! those scopes instead of the flat name maps of version 1.6.

use std::collections::HashMap;

use super::check::Checker;
use super::generics::DefOrigin;
use crate::ast::{UseDecl, UseTree, Visibility};
use crate::diag::Diagnostic;
use crate::span::Span;

/// Index into the checker module table.
pub type ModuleId = usize;

/// A definition a module namespace can bind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefRef {
    Struct(usize),
    Enum(usize),
    Alias(usize),
    Trait(usize),
    GenericStruct(usize),
    GenericEnum(usize),
    GenericFn(usize),
    Fn(usize),
    Const(usize),
}

impl DefRef {
    pub fn kind_name(self) -> &'static str {
        match self {
            DefRef::Struct(_) => "struct",
            DefRef::Enum(_) => "enum",
            DefRef::Alias(_) => "type alias",
            DefRef::Trait(_) => "trait",
            DefRef::GenericStruct(_) => "struct",
            DefRef::GenericEnum(_) => "enum",
            DefRef::GenericFn(_) => "function",
            DefRef::Fn(_) => "function",
            DefRef::Const(_) => "constant",
        }
    }

    pub fn is_type_namespace(self) -> bool {
        !matches!(self, DefRef::Fn(_) | DefRef::Const(_))
    }
}

/// A name binding in a module scope.
#[derive(Clone, Debug)]
pub struct Binding {
    pub def: DefRef,
    pub span: Span,
    /// Module that introduced the binding (the importing module for aliases).
    pub scope: ModuleId,
    /// Module that defines the item (field privacy and C names use this one).
    pub module: ModuleId,
    pub vis: Visibility,
    pub imported: bool,
}

/// A child-module binding in a module scope.
#[derive(Clone, Debug)]
pub struct ModuleBinding {
    pub module: ModuleId,
    pub span: Span,
    pub scope: ModuleId,
    pub vis: Visibility,
    pub imported: bool,
}

/// One module of one crate.
#[derive(Clone, Debug)]
pub struct ModuleInfo {
    pub name: String,
    pub parent: Option<ModuleId>,
    pub crate_id: usize,
    pub vis: Visibility,
    /// User program or embedded standard library.
    pub origin: DefOrigin,
    pub span: Span,
    /// Display path: crate name first, then the module names.
    pub path: Vec<String>,
    /// Prefix prepended to C names of items declared directly in this module.
    pub prefix: String,
    pub types: HashMap<String, Binding>,
    pub values: HashMap<String, Binding>,
    pub modules: HashMap<String, ModuleBinding>,
    pub unit_variants: HashMap<String, Vec<(usize, usize)>>,
}

/// One leaf of a flattened use tree, waiting to be resolved.
#[derive(Clone, Debug)]
pub struct ImportLeaf {
    pub module: ModuleId,
    pub vis: Visibility,
    pub prefix: Vec<(String, Span)>,
    pub name: String,
    pub name_span: Span,
    pub alias: Option<String>,
    pub span: Span,
}

enum ImportOutcome {
    Done,
    Unresolved,
}

impl<'a> Checker<'a> {
    // -- crate and module construction --------------------------------------

    /// Start a new crate and create its root module. Returns the crate id.
    pub fn begin_crate(&mut self, name: &str, is_dependency: bool) -> usize {
        let prefix = if is_dependency {
            format!("{}_", sanitize_module_segment(name))
        } else {
            String::new()
        };
        self.push_crate_root(name, is_dependency, prefix, is_dependency, DefOrigin::User)
    }

    /// Start the embedded prelude "crate". Its items keep their version 1.6
    /// C names (no prefix) and it is not reachable through a crate path.
    pub fn begin_prelude_crate(&mut self, name: &str) -> usize {
        let crate_id = self.push_crate_root(name, true, String::new(), false, DefOrigin::Prelude);
        self.prelude_module = Some(self.crate_roots[crate_id]);
        crate_id
    }

    fn push_crate_root(
        &mut self,
        name: &str,
        is_dependency: bool,
        root_prefix: String,
        indexable: bool,
        origin: DefOrigin,
    ) -> usize {
        let crate_id = self.crate_roots.len();
        self.crate_names.push(name.to_string());
        self.crate_is_dependency.push(is_dependency);
        let module = self.add_module(None, name, Span::empty(0), Visibility::Public, crate_id);
        self.modules[module].origin = origin;
        self.modules[module].prefix = root_prefix;
        self.crate_roots.push(module);
        self.current_module = module;
        if indexable {
            self.crate_by_name
                .entry(name.to_string())
                .or_insert(crate_id);
        }
        crate_id
    }

    fn add_module(
        &mut self,
        parent: Option<ModuleId>,
        name: &str,
        name_span: Span,
        vis: Visibility,
        crate_id: usize,
    ) -> ModuleId {
        let (path, prefix) = match parent {
            None => (vec![self.crate_names[crate_id].clone()], String::new()),
            Some(parent) => {
                let parent_prefix = self.modules[parent].prefix.clone();
                let mut path = self.modules[parent].path.clone();
                path.push(name.to_string());
                (
                    path,
                    format!("{}{}_", parent_prefix, sanitize_module_segment(name)),
                )
            }
        };
        let module = self.modules.len();
        self.modules.push(ModuleInfo {
            name: name.to_string(),
            parent,
            crate_id,
            vis,
            origin: match parent {
                Some(parent) => self.modules[parent].origin,
                None => DefOrigin::User,
            },
            span: name_span,
            path,
            prefix,
            types: HashMap::new(),
            values: HashMap::new(),
            modules: HashMap::new(),
            unit_variants: HashMap::new(),
        });
        self.module_items.push(Vec::new());
        module
    }

    /// Create (or reuse) the child module "name" of "parent".
    pub fn child_module(
        &mut self,
        parent: ModuleId,
        name: &str,
        name_span: Span,
        vis: Visibility,
        span: Span,
    ) -> ModuleId {
        if let Some(existing) = self.modules[parent].modules.get(name) {
            let previous = existing.span;
            let module = existing.module;
            self.diagnostics.push(
                Diagnostic::error(
                    format!("the name '{}' is defined multiple times", name),
                    name_span,
                )
                .with_code("E0428")
                .with_label(previous, format!("previous definition of '{}' here", name)),
            );
            return module;
        }
        let crate_id = self.modules[parent].crate_id;
        let module = self.add_module(Some(parent), name, name_span, vis, crate_id);
        self.modules[parent].span = span;
        let binding = ModuleBinding {
            module,
            span: name_span,
            scope: parent,
            vis,
            imported: false,
        };
        self.modules[parent]
            .modules
            .insert(name.to_string(), binding);
        module
    }

    /// Human readable module path used in diagnostics.
    pub fn module_path_string(&self, module: ModuleId) -> String {
        let info = &self.modules[module];
        if info.parent.is_none() {
            if info.crate_id == self.root_crate {
                "crate root".to_string()
            } else {
                info.path[0].clone()
            }
        } else {
            info.path[1..].join("::")
        }
    }

    pub fn current_crate_root(&self) -> ModuleId {
        self.crate_roots[self.modules[self.current_module].crate_id]
    }

    pub fn root_crate_root(&self) -> ModuleId {
        self.crate_roots[self.root_crate]
    }

    // -- visibility ---------------------------------------------------------

    pub fn module_is_descendant(&self, mut module: ModuleId, ancestor: ModuleId) -> bool {
        loop {
            if module == ancestor {
                return true;
            }
            match self.modules[module].parent {
                Some(parent) => module = parent,
                None => return false,
            }
        }
    }

    pub fn visibility_allows(&self, vis: Visibility, defined: ModuleId, from: ModuleId) -> bool {
        match vis {
            Visibility::Public => true,
            Visibility::Crate => self.modules[defined].crate_id == self.modules[from].crate_id,
            Visibility::Private => self.module_is_descendant(from, defined),
        }
    }

    /// Report E0603 when a binding is not visible from the current module.
    pub fn check_binding_visible(&mut self, binding: &Binding, span: Span) -> bool {
        if self.visibility_allows(binding.vis, binding.scope, self.privacy_module()) {
            return true;
        }
        let kind = binding.def.kind_name();
        let name = self
            .binding_name(binding)
            .unwrap_or_else(|| "item".to_string());
        self.error_code(format!("{} '{}' is private", kind, name), span, "E0603");
        false
    }

    /// Report E0603 when a module binding is not visible from the current module.
    pub fn check_module_visible(&mut self, binding: &ModuleBinding, span: Span) -> bool {
        if self.visibility_allows(binding.vis, binding.scope, self.privacy_module()) {
            return true;
        }
        let name = self.modules[binding.module].name.clone();
        self.error_code(format!("module '{}' is private", name), span, "E0603");
        false
    }

    fn binding_name(&self, binding: &Binding) -> Option<String> {
        match binding.def {
            DefRef::Struct(id) => Some(self.structs.get(id)?.name.clone()),
            DefRef::Enum(id) => Some(self.enums.get(id)?.name.clone()),
            DefRef::Alias(id) => Some(self.aliases.get(id)?.name.clone()),
            DefRef::Trait(id) => Some(self.traits.get(id)?.name.clone()),
            DefRef::GenericStruct(id) => Some(self.generic_structs.get(id)?.name.clone()),
            DefRef::GenericEnum(id) => Some(self.generic_enums.get(id)?.name.clone()),
            DefRef::GenericFn(id) => Some(self.generic_fns.get(id)?.name.clone()),
            DefRef::Fn(id) => Some(self.fns.get(id)?.name.clone()),
            DefRef::Const(id) => Some(self.consts.get(id)?.decl.name.clone()),
        }
    }

    // -- scope insertion ----------------------------------------------------

    /// Insert a type-namespace binding. Returns the previous span on conflict.
    pub fn insert_type_binding(
        &mut self,
        module: ModuleId,
        name: &str,
        def: DefRef,
        span: Span,
        vis: Visibility,
    ) -> Option<Span> {
        let binding = Binding {
            def,
            span,
            scope: module,
            module,
            vis,
            imported: false,
        };
        self.modules[module]
            .types
            .insert(name.to_string(), binding)
            .map(|previous| previous.span)
    }

    /// Insert a value-namespace binding. Returns the previous span on conflict.
    pub fn insert_value_binding(
        &mut self,
        module: ModuleId,
        name: &str,
        def: DefRef,
        span: Span,
        vis: Visibility,
    ) -> Option<Span> {
        let binding = Binding {
            def,
            span,
            scope: module,
            module,
            vis,
            imported: false,
        };
        self.modules[module]
            .values
            .insert(name.to_string(), binding)
            .map(|previous| previous.span)
    }

    // -- lookups ------------------------------------------------------------

    pub fn lookup_type_in(&self, module: ModuleId, name: &str) -> Option<Binding> {
        self.modules[module].types.get(name).cloned()
    }

    pub fn lookup_value_in(&self, module: ModuleId, name: &str) -> Option<Binding> {
        self.modules[module].values.get(name).cloned()
    }

    pub fn lookup_module_in(&self, module: ModuleId, name: &str) -> Option<ModuleBinding> {
        self.modules[module].modules.get(name).cloned()
    }

    /// Type binding in the current module, then the prelude.
    pub fn lookup_type_scoped(&self, name: &str) -> Option<Binding> {
        self.lookup_type_in(self.current_module, name)
            .or_else(|| self.lookup_prelude_type(name))
    }

    /// Value binding in the current module, then the prelude.
    pub fn lookup_value_scoped(&self, name: &str) -> Option<Binding> {
        self.lookup_value_in(self.current_module, name)
            .or_else(|| self.lookup_prelude_value(name))
    }

    /// Child module binding in the current module, then the prelude.
    pub fn lookup_module_scoped(&self, name: &str) -> Option<ModuleBinding> {
        self.lookup_module_in(self.current_module, name)
            .or_else(|| self.lookup_prelude_module(name))
    }

    fn lookup_prelude_type(&self, name: &str) -> Option<Binding> {
        let prelude = self.prelude_module?;
        if prelude == self.current_module {
            return None;
        }
        self.lookup_type_in(prelude, name).filter(|binding| {
            self.visibility_allows(binding.vis, binding.scope, self.current_module)
        })
    }

    fn lookup_prelude_value(&self, name: &str) -> Option<Binding> {
        let prelude = self.prelude_module?;
        if prelude == self.current_module {
            return None;
        }
        self.lookup_value_in(prelude, name).filter(|binding| {
            self.visibility_allows(binding.vis, binding.scope, self.current_module)
        })
    }

    fn lookup_prelude_module(&self, name: &str) -> Option<ModuleBinding> {
        let prelude = self.prelude_module?;
        if prelude == self.current_module {
            return None;
        }
        self.lookup_module_in(prelude, name).filter(|binding| {
            self.visibility_allows(binding.vis, binding.scope, self.current_module)
        })
    }

    pub fn lookup_struct(&self, name: &str) -> Option<usize> {
        match self.lookup_type_scoped(name)?.def {
            DefRef::Struct(id) => Some(id),
            _ => None,
        }
    }

    pub fn lookup_enum(&self, name: &str) -> Option<usize> {
        match self.lookup_type_scoped(name)?.def {
            DefRef::Enum(id) => Some(id),
            _ => None,
        }
    }

    pub fn lookup_trait(&self, name: &str) -> Option<usize> {
        match self.lookup_type_scoped(name)?.def {
            DefRef::Trait(id) => Some(id),
            _ => None,
        }
    }

    pub fn lookup_generic_struct(&self, name: &str) -> Option<usize> {
        match self.lookup_type_scoped(name)?.def {
            DefRef::GenericStruct(id) => Some(id),
            _ => None,
        }
    }

    pub fn lookup_generic_enum(&self, name: &str) -> Option<usize> {
        match self.lookup_type_scoped(name)?.def {
            DefRef::GenericEnum(id) => Some(id),
            _ => None,
        }
    }

    pub fn lookup_generic_fn(&self, name: &str) -> Option<usize> {
        match self.lookup_value_scoped(name)?.def {
            DefRef::GenericFn(id) => Some(id),
            _ => None,
        }
    }

    pub fn lookup_fn(&self, name: &str) -> Option<usize> {
        match self.lookup_value_scoped(name)?.def {
            DefRef::Fn(id) => Some(id),
            _ => None,
        }
    }

    pub fn lookup_const(&self, name: &str) -> Option<usize> {
        match self.lookup_value_scoped(name)?.def {
            DefRef::Const(id) => Some(id),
            _ => None,
        }
    }

    /// Unit-variant candidates for a bare variant name in the current scope.
    pub fn unit_variants_scoped(&self, name: &str) -> Vec<(usize, usize)> {
        if let Some(variants) = self.modules[self.current_module].unit_variants.get(name) {
            return variants.clone();
        }
        if let Some(prelude) = self.prelude_module
            && prelude != self.current_module
            && let Some(variants) = self.modules[prelude].unit_variants.get(name)
        {
            return variants.clone();
        }
        Vec::new()
    }

    /// Every unit-variant name visible from the current module.
    pub fn unit_variant_names_scoped(&self) -> Vec<String> {
        let mut names: Vec<String> = self.modules[self.current_module]
            .unit_variants
            .keys()
            .cloned()
            .collect();
        if let Some(prelude) = self.prelude_module
            && prelude != self.current_module
        {
            names.extend(self.modules[prelude].unit_variants.keys().cloned());
        }
        names
    }

    pub fn type_names_scoped(&self) -> Vec<String> {
        let mut names: Vec<String> = self.modules[self.current_module]
            .types
            .keys()
            .cloned()
            .collect();
        if let Some(prelude) = self.prelude_module
            && prelude != self.current_module
        {
            names.extend(self.modules[prelude].types.keys().cloned());
        }
        names
    }

    /// The main function of the root crate root module (entry point only).
    pub fn root_main_fn(&self) -> Option<usize> {
        let root = self.root_crate_root();
        let binding = self.lookup_value_in(root, "main")?;
        match binding.def {
            DefRef::Fn(id) => Some(id),
            _ => None,
        }
    }

    // -- path resolution ----------------------------------------------------

    /// Resolve the module part of a path with more than one segment.
    ///
    /// Returns the base module and how many leading segments were consumed as
    /// module steps. The remaining segments (at most two) are resolved as an
    /// item or a Type::associated item inside the base module.
    pub fn resolve_leading_modules(
        &mut self,
        segments: &[(String, Span)],
        report: bool,
    ) -> Option<(ModuleId, usize)> {
        if segments.is_empty() {
            return Some((self.current_module, 0));
        }
        let mut pos = 0;
        let mut current: ModuleId;
        let mut forced = false;
        let first = &segments[0];
        match first.0.as_str() {
            "crate" => {
                current = self.current_crate_root();
                pos = 1;
                forced = true;
            }
            "self" => {
                current = self.current_module;
                pos = 1;
                forced = true;
            }
            "super" => {
                current = self.current_module;
                while pos < segments.len() && segments[pos].0 == "super" {
                    match self.modules[current].parent {
                        Some(parent) => current = parent,
                        None => {
                            if report {
                                self.error_code(
                                    "'super' cannot be used at the crate root".to_string(),
                                    segments[pos].1,
                                    "E0433",
                                );
                            }
                            return None;
                        }
                    }
                    pos += 1;
                }
                forced = true;
            }
            other => {
                if let Some(binding) = self.lookup_module_scoped(other) {
                    if report && !self.check_module_visible(&binding, first.1) {
                        return None;
                    }
                    current = binding.module;
                    pos = 1;
                } else if let Some(&crate_id) = self.crate_by_name.get(other) {
                    current = self.crate_roots[crate_id];
                    pos = 1;
                    forced = true;
                } else {
                    return None;
                }
            }
        }
        while pos < segments.len() {
            let (name, name_span) = &segments[pos];
            let Some(binding) = self.lookup_module_in(current, name) else {
                break;
            };
            if report && !self.check_module_visible(&binding, *name_span) {
                return None;
            }
            current = binding.module;
            pos += 1;
        }
        if report && forced && pos < segments.len() {
            let (name, name_span) = &segments[pos];
            let is_type = self
                .lookup_type_in(current, name)
                .map(|binding| binding.def.is_type_namespace())
                .unwrap_or(false);
            let remaining = segments.len() - pos;
            if !is_type && remaining > 2 {
                let path = self.module_path_string(current);
                self.error_code(
                    format!("could not find '{}' in '{}'", name, path),
                    *name_span,
                    "E0433",
                );
                return None;
            }
        }
        Some((current, pos))
    }

    /// Whether the struct field at `index` is visible from the current module.
    ///
    /// Field visibility is independent of the visibility of the module that
    /// declares the struct: `pub` fields stay reachable wherever the struct
    /// itself is reachable, private fields only from the defining module and
    /// its descendants, and `pub(crate)` fields from the whole defining crate.
    pub fn field_visible(&self, struct_id: usize, index: usize) -> bool {
        let info = &self.structs[struct_id];
        let declared = info
            .decl_fields
            .get(index)
            .map(|field| field.vis)
            .unwrap_or(Visibility::Public);
        let defined = info.module;
        self.visibility_allows(declared, defined, self.privacy_module())
    }

    /// Resolve the first `prefix_len` segments of `path` as module steps.
    ///
    /// Returns the base module and how many segments were consumed as modules.
    /// A trailing type or item segment is left for the caller: the path
    /// `util::math::double` with `prefix_len = 2` yields the module
    /// `util::math` and `consumed = 2`.
    pub fn resolve_path_prefix(
        &mut self,
        path: &crate::ast::PathExpr,
        prefix_len: usize,
        report: bool,
    ) -> Option<(ModuleId, usize)> {
        let end = prefix_len.min(path.segments.len());
        let segments: Vec<(String, Span)> = path.segments[..end]
            .iter()
            .map(|segment| (segment.name.clone(), segment.span))
            .collect();
        self.resolve_leading_modules(&segments, report)
    }

    /// Whether the type binding `name` in `module` is visible from the
    /// module the path is written in.
    ///
    /// The defining module is the one the path resolved into, so this must be
    /// called before descending into it.
    pub fn check_type_visible_in(&mut self, module: ModuleId, name: &str, span: Span) -> bool {
        match self.lookup_type_in(module, name) {
            Some(binding) => self.check_binding_visible(&binding, span),
            None => true,
        }
    }

    /// Verify the visibility of every module step that a quiet path
    /// resolution consumed.
    ///
    /// Call and value paths first resolve their leading segments without
    /// reporting, so a type path can still fall back to associated-item
    /// syntax. Once the prefix is known to be a module path, each descended
    /// module must be visible from the module the path is written in.
    pub fn check_module_steps_visible(
        &mut self,
        path: &crate::ast::PathExpr,
        consumed: usize,
    ) -> bool {
        let segments = &path.segments;
        if consumed == 0 || segments.is_empty() {
            return true;
        }
        let mut current: ModuleId;
        let mut pos = 0;
        match segments[0].name.as_str() {
            "crate" => {
                current = self.current_crate_root();
                pos = 1;
            }
            "self" => {
                current = self.privacy_module();
                pos = 1;
            }
            "super" => {
                current = self.privacy_module();
                while pos < consumed && segments[pos].name == "super" {
                    let Some(parent) = self.modules[current].parent else {
                        return true;
                    };
                    current = parent;
                    pos += 1;
                }
            }
            first => {
                if let Some(&crate_id) = self.crate_by_name.get(first) {
                    current = self.crate_roots[crate_id];
                    pos = 1;
                } else {
                    let origin = self.privacy_module();
                    let Some(binding) = self.lookup_module_in(origin, first) else {
                        return true;
                    };
                    if !self.check_module_visible(&binding, segments[0].span) {
                        return false;
                    }
                    current = binding.module;
                    pos = 1;
                }
            }
        }
        while pos < consumed {
            let name = segments[pos].name.clone();
            let span = segments[pos].span;
            let Some(binding) = self.lookup_module_in(current, &name) else {
                return true;
            };
            if !self.check_module_visible(&binding, span) {
                return false;
            }
            current = binding.module;
            pos += 1;
        }
        true
    }

    /// Effective visibility of an item declared with `vis` in `module`: the
    /// declared visibility narrowed by every enclosing module.
    pub fn effective_visibility(&self, module: ModuleId, vis: Visibility) -> Visibility {
        let mut effective = vis;
        let mut current = Some(module);
        while let Some(id) = current {
            effective = min_visibility(effective, self.modules[id].vis);
            current = self.modules[id].parent;
        }
        effective
    }

    /// Effective visibility of the name introduced by a `use` declaration.
    fn import_visibility(&self, leaf: &ImportLeaf) -> Visibility {
        self.effective_visibility(leaf.module, leaf.vis)
    }

    /// Report E0364 when a re-export is wider than the item it imports.
    ///
    /// A `use` may only narrow visibility; `pub use` cannot re-export a
    /// private or `pub(crate)` item to the outside world. Returns true when
    /// the import was rejected.
    fn rejects_widening_item_import(&mut self, leaf: &ImportLeaf, binding: &Binding) -> bool {
        // `pub use` of a public item stays legal even when the module that
        // declares it is private (the facade pattern); only the item's own
        // visibility caps the re-export.
        if visibility_rank(self.import_visibility(leaf)) <= visibility_rank(binding.vis) {
            return false;
        }
        let kind = binding.def.kind_name();
        self.error_code(
            format!(
                "{} '{}' is private, and cannot be re-exported",
                kind, leaf.name
            ),
            leaf.name_span,
            "E0364",
        );
        true
    }

    /// Report E0365 when a re-export is wider than the module it imports.
    fn rejects_widening_module_import(
        &mut self,
        leaf: &ImportLeaf,
        binding: &ModuleBinding,
    ) -> bool {
        if visibility_rank(self.import_visibility(leaf)) <= visibility_rank(binding.vis) {
            return false;
        }
        self.error_code(
            format!(
                "module '{}' is private, and cannot be re-exported",
                leaf.name
            ),
            leaf.name_span,
            "E0365",
        );
        true
    }

    // -- imports ------------------------------------------------------------

    /// Flatten one use declaration into leaves stored in pending_imports.
    pub fn collect_use_decl(&mut self, module: ModuleId, decl: &UseDecl) {
        let mut prefix = Vec::new();
        self.flatten_use_tree(module, decl.vis, &mut prefix, &decl.tree);
    }

    fn flatten_use_tree(
        &mut self,
        module: ModuleId,
        vis: Visibility,
        prefix: &mut Vec<(String, Span)>,
        tree: &UseTree,
    ) {
        match tree {
            UseTree::Name { name, name_span } => {
                self.pending_imports.push(ImportLeaf {
                    module,
                    vis,
                    prefix: prefix.clone(),
                    name: name.clone(),
                    name_span: *name_span,
                    alias: None,
                    span: *name_span,
                });
            }
            UseTree::Alias {
                name,
                name_span,
                alias,
                alias_span,
            } => {
                self.pending_imports.push(ImportLeaf {
                    module,
                    vis,
                    prefix: prefix.clone(),
                    name: name.clone(),
                    name_span: *name_span,
                    alias: Some(alias.clone()),
                    span: *alias_span,
                });
            }
            UseTree::Path {
                segment,
                segment_span,
                rest,
                ..
            } => {
                prefix.push((segment.clone(), *segment_span));
                self.flatten_use_tree(module, vis, prefix, rest);
                prefix.pop();
            }
            UseTree::Group { items, .. } => {
                for item in items {
                    self.flatten_use_tree(module, vis, prefix, item);
                }
            }
            UseTree::Glob { span } => {
                self.error_code("glob imports are not supported".to_string(), *span, "E0432");
            }
        }
    }

    /// Resolve every pending import, iterating until no further leaf resolves.
    pub fn process_imports(&mut self) {
        let mut pending = std::mem::take(&mut self.pending_imports);
        loop {
            let mut progress = false;
            let mut unresolved = Vec::new();
            for leaf in pending {
                match self.try_import(&leaf) {
                    ImportOutcome::Done => progress = true,
                    ImportOutcome::Unresolved => unresolved.push(leaf),
                }
            }
            if unresolved.is_empty() {
                break;
            }
            if !progress {
                for leaf in unresolved {
                    let path = import_display_path(&leaf);
                    self.error_code(format!("unresolved import '{}'", path), leaf.span, "E0432");
                }
                break;
            }
            pending = unresolved;
        }
    }

    /// Resolve the module part of a use path.
    ///
    /// Returns `Ok(Some(module))` on success, `Ok(None)` when the path could
    /// not be resolved yet (a later import may make it resolvable), and
    /// `Err(())` when a diagnostic was already reported.
    fn resolve_use_modules(&mut self, leaf: &ImportLeaf) -> Result<Option<ModuleId>, ()> {
        if leaf.prefix.is_empty() {
            return Ok(Some(leaf.module));
        }
        let segments = &leaf.prefix;
        let first = &segments[0];
        let mut pos = 0;
        let mut current: ModuleId;
        match first.0.as_str() {
            "crate" => {
                current = self.crate_roots[self.modules[leaf.module].crate_id];
                pos = 1;
            }
            "self" => {
                current = leaf.module;
                pos = 1;
            }
            "super" => {
                current = leaf.module;
                while pos < segments.len() && segments[pos].0 == "super" {
                    let Some(parent) = self.modules[current].parent else {
                        // `super` escapes the crate root: report E0433 instead of
                        // the generic unresolved-import E0432.
                        self.error_code(
                            "failed to resolve: 'super' at the crate root".to_string(),
                            segments[pos].1,
                            "E0433",
                        );
                        return Err(());
                    };
                    current = parent;
                    pos += 1;
                }
            }
            other => {
                if let Some(binding) = self.lookup_module_in(leaf.module, other) {
                    if !self.visibility_allows(binding.vis, binding.scope, leaf.module) {
                        return Ok(None);
                    }
                    current = binding.module;
                    pos = 1;
                } else if let Some(&crate_id) = self.crate_by_name.get(other) {
                    current = self.crate_roots[crate_id];
                    pos = 1;
                } else {
                    return Ok(None);
                }
            }
        }
        while pos < segments.len() {
            let (name, _) = &segments[pos];
            let Some(binding) = self.lookup_module_in(current, name) else {
                return Ok(None);
            };
            if !self.visibility_allows(binding.vis, binding.scope, leaf.module) {
                return Ok(None);
            }
            current = binding.module;
            pos += 1;
        }
        Ok(Some(current))
    }

    fn try_import(&mut self, leaf: &ImportLeaf) -> ImportOutcome {
        let module = leaf.module;
        if leaf.prefix.is_empty() {
            // A bare import names an item of the current module or the prelude.
            if let Some(binding) = self.lookup_module_in(module, &leaf.name) {
                if self.rejects_widening_module_import(leaf, &binding) {
                    return ImportOutcome::Done;
                }
                if !self.visibility_allows(binding.vis, binding.scope, module) {
                    self.error_code(
                        format!("module '{}' is private", leaf.name),
                        leaf.name_span,
                        "E0603",
                    );
                    return ImportOutcome::Done;
                }
                return self.bind_module_import(leaf, &binding);
            }
            if let Some(binding) = self
                .lookup_type_in(module, &leaf.name)
                .or_else(|| self.lookup_value_in(module, &leaf.name))
            {
                if self.rejects_widening_item_import(leaf, &binding) {
                    return ImportOutcome::Done;
                }
                return self.bind_import_item(leaf, &binding);
            }
            // Fall back to the prelude (std) scope.
            if let Some(prelude) = self.prelude_module
                && prelude != module
            {
                if let Some(binding) = self.lookup_module_in(prelude, &leaf.name) {
                    return self.bind_module_import(leaf, &binding);
                }
                if let Some(binding) = self
                    .lookup_type_in(prelude, &leaf.name)
                    .or_else(|| self.lookup_value_in(prelude, &leaf.name))
                {
                    return self.bind_import_item(leaf, &binding);
                }
            }
            return ImportOutcome::Unresolved;
        }
        let target = match self.resolve_use_modules(leaf) {
            Ok(Some(target)) => target,
            Ok(None) => return ImportOutcome::Unresolved,
            Err(()) => return ImportOutcome::Done,
        };
        if let Some(binding) = self.lookup_module_in(target, &leaf.name) {
            if self.rejects_widening_module_import(leaf, &binding) {
                return ImportOutcome::Done;
            }
            if !self.visibility_allows(binding.vis, binding.scope, leaf.module) {
                self.error_code(
                    format!("module '{}' is private", leaf.name),
                    leaf.name_span,
                    "E0603",
                );
                return ImportOutcome::Done;
            }
            return self.bind_module_import(leaf, &binding);
        }
        let Some(binding) = self
            .lookup_type_in(target, &leaf.name)
            .or_else(|| self.lookup_value_in(target, &leaf.name))
        else {
            return ImportOutcome::Unresolved;
        };
        if self.rejects_widening_item_import(leaf, &binding) {
            return ImportOutcome::Done;
        }
        if !self.visibility_allows(binding.vis, binding.scope, leaf.module) {
            self.error_code(
                format!("{} '{}' is private", binding.def.kind_name(), leaf.name),
                leaf.name_span,
                "E0603",
            );
            return ImportOutcome::Done;
        }
        self.bind_import_item(leaf, &binding)
    }

    fn bind_module_import(&mut self, leaf: &ImportLeaf, binding: &ModuleBinding) -> ImportOutcome {
        let name = leaf.alias.clone().unwrap_or_else(|| leaf.name.clone());
        let effective = min_visibility(leaf.vis, binding.vis);
        let module = leaf.module;
        if let Some(previous) = self.modules[module].modules.get(&name) {
            let was_import = previous.imported;
            let previous_span = previous.span;
            self.report_import_conflict(leaf, &name, previous_span, was_import);
            return ImportOutcome::Done;
        }
        if let Some(previous) = self.modules[module].types.get(&name) {
            let was_import = previous.imported;
            let previous_span = previous.span;
            self.report_import_conflict(leaf, &name, previous_span, was_import);
            return ImportOutcome::Done;
        }
        self.modules[module].modules.insert(
            name,
            ModuleBinding {
                module: binding.module,
                span: leaf.span,
                scope: module,
                vis: effective,
                imported: true,
            },
        );
        ImportOutcome::Done
    }

    fn bind_import_item(&mut self, leaf: &ImportLeaf, binding: &Binding) -> ImportOutcome {
        let name = leaf.alias.clone().unwrap_or_else(|| leaf.name.clone());
        let effective = min_visibility(leaf.vis, binding.vis);
        let module = leaf.module;
        let imported = binding.def.is_type_namespace();
        if imported {
            if let Some(previous) = self.modules[module].types.get(&name) {
                let was_import = previous.imported;
                let previous_span = previous.span;
                self.report_import_conflict(leaf, &name, previous_span, was_import);
                return ImportOutcome::Done;
            }
            if let Some(previous) = self.modules[module].modules.get(&name) {
                let previous_span = previous.span;
                self.report_import_conflict(leaf, &name, previous_span, true);
                return ImportOutcome::Done;
            }
            self.modules[module].types.insert(
                name,
                Binding {
                    def: binding.def,
                    span: leaf.span,
                    scope: module,
                    module: binding.module,
                    vis: effective,
                    imported: true,
                },
            );
        } else {
            if let Some(previous) = self.modules[module].values.get(&name) {
                let was_import = previous.imported;
                let previous_span = previous.span;
                self.report_import_conflict(leaf, &name, previous_span, was_import);
                return ImportOutcome::Done;
            }
            self.modules[module].values.insert(
                name,
                Binding {
                    def: binding.def,
                    span: leaf.span,
                    scope: module,
                    module: binding.module,
                    vis: effective,
                    imported: true,
                },
            );
        }
        ImportOutcome::Done
    }

    fn report_import_conflict(
        &mut self,
        leaf: &ImportLeaf,
        name: &str,
        previous: Span,
        was_import: bool,
    ) {
        if was_import {
            self.diagnostics.push(
                Diagnostic::error(
                    format!("the name '{}' is imported multiple times", name),
                    leaf.span,
                )
                .with_code("E0252")
                .with_label(previous, format!("first import of '{}' here", name)),
            );
        } else {
            self.diagnostics.push(
                Diagnostic::error(
                    format!("the name '{}' is defined multiple times", name),
                    leaf.span,
                )
                .with_code("E0255")
                .with_label(previous, format!("previous definition of '{}' here", name)),
            );
        }
    }
}

/// Numeric rank of a visibility; larger is more visible.
pub(crate) fn visibility_rank(vis: Visibility) -> u8 {
    match vis {
        Visibility::Private => 0,
        Visibility::Crate => 1,
        Visibility::Public => 2,
    }
}

/// Effective visibility of an imported name: the more restrictive of the two.
fn min_visibility(left: Visibility, right: Visibility) -> Visibility {
    if visibility_rank(left) <= visibility_rank(right) {
        left
    } else {
        right
    }
}

fn import_display_path(leaf: &ImportLeaf) -> String {
    let mut parts: Vec<String> = leaf.prefix.iter().map(|(name, _)| name.clone()).collect();
    parts.push(leaf.name.clone());
    parts.join("::")
}

fn sanitize_module_segment(name: &str) -> String {
    super::items::sanitize_c_ident(name)
}
