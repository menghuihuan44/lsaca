//! Item collection, hoisting, type name resolution and layout checks.
//!
//! All items are collected before any body is checked (items are hoisted and
//! visible throughout the program). Items nested in blocks are hoisted too.

use std::collections::{HashMap, HashSet};

use super::check::Checker;
use super::generics::{
    DefOrigin, EnumInstance, FnInstance, GEnumDef, GFnDef, GStructDef, GenericOwner, ImplInfo,
    ImplMethod, StructInstance,
};
use super::itype::ITy;
use super::tree::TBlock;
use crate::ast::{
    self, Block, EnumDecl, Expr, ExprKind, FieldDef, FnDecl, ImplBlock, Item, LetStmt, Program,
    Stmt, StructDecl, TraitRef, TypeAliasDecl, TypeExpr, VariantDef, VecLit,
};
use crate::diag::Diagnostic;
use crate::span::Span;
use crate::types::{SelfKind, Ty};

/// Identifies a struct or enum by its dense index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TypeKey {
    Struct(usize),
    Enum(usize),
}

#[derive(Clone, Debug)]
pub struct StructField {
    pub name: String,
    pub name_span: Span,
    pub ty: ITy,
}

#[derive(Clone, Debug)]
pub struct StructInfo {
    pub name: String,
    /// Unique C identifier for this type (generic instances get a mangled name).
    pub c_name: String,
    /// Set for monomorphized generic instances.
    pub generic: Option<StructInstance>,
    pub span: Span,
    pub decl_fields: Vec<FieldDef>,
    pub fields: Vec<StructField>,
    pub fields_resolved: bool,
}

#[derive(Clone, Debug)]
pub struct VariantInfo {
    pub name: String,
    pub span: Span,
    pub fields: Vec<ITy>,
}

#[derive(Clone, Debug)]
pub struct EnumInfo {
    pub name: String,
    /// Unique C identifier for this type (generic instances get a mangled name).
    pub c_name: String,
    /// Set for monomorphized generic instances.
    pub generic: Option<EnumInstance>,
    pub span: Span,
    pub decl_variants: Vec<VariantDef>,
    pub variants: Vec<VariantInfo>,
    pub variants_resolved: bool,
}

#[derive(Clone, Debug)]
pub struct AliasInfo {
    pub name: String,
    pub span: Span,
    pub target: TypeExpr,
    pub resolved: Option<ITy>,
    pub resolving: bool,
}

/// A constant value produced by const evaluation.
#[derive(Clone, Debug)]
pub enum ConstVal {
    Int(i128),
    Float(f64),
    Bool(bool),
    Char(char),
    Str(String),
    Unit,
}

impl ConstVal {
    pub fn to_lit(&self) -> super::tree::TLit {
        use super::tree::TLit;
        match self {
            ConstVal::Int(value) => TLit::Int(*value),
            ConstVal::Float(value) => TLit::Float(*value),
            ConstVal::Bool(value) => TLit::Bool(*value),
            ConstVal::Char(value) => TLit::Char(*value),
            ConstVal::Str(value) => TLit::Str(value.clone()),
            ConstVal::Unit => TLit::Unit,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ConstState {
    Unresolved,
    InProgress,
    Done { ty: Ty, value: ConstVal },
}

#[derive(Clone, Debug)]
pub struct ConstItem {
    pub decl: ast::ConstDecl,
    pub c_name: String,
    pub state: ConstState,
}

#[derive(Clone, Debug)]
pub struct FnSig {
    pub params: Vec<ITy>,
    pub ret: ITy,
    /// Parameter index the elided return lifetime is borrowed from, if the
    /// return type is a reference (v1.3 elision, design R6).
    pub ret_ref_source: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct FnInfo {
    pub name: String,
    pub display: String,
    pub c_name: String,
    pub decl: FnDecl,
    /// Which program declared this function (user code or the std prelude).
    pub origin: DefOrigin,
    /// Set for functions instantiated from a generic definition.
    pub generic: Option<FnInstance>,
    /// Instantiated impl self type of a method, if any.
    pub self_ty: Option<ITy>,
    /// Monomorphization depth, used for the E0275 recursion limit.
    pub instantiation_depth: usize,
    pub owner: Option<TypeKey>,
    pub self_kind: Option<SelfKind>,
    pub sig: Option<FnSig>,
    pub checked: Option<TBlock>,
    pub param_cnames: Vec<String>,
    pub name_span: Span,
    pub span: Span,
}

impl FnInfo {
    pub fn is_method(&self) -> bool {
        self.owner.is_some() && self.self_kind.is_some()
    }

    pub fn ret(&self) -> ITy {
        match &self.sig {
            Some(sig) => sig.ret.clone(),
            None => ITy::Unit,
        }
    }

    pub fn params(&self) -> Vec<ITy> {
        match &self.sig {
            Some(sig) => sig.params.clone(),
            None => Vec::new(),
        }
    }
}

pub fn sanitize_c_ident(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    if out
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        out.insert(0, '_');
    }
    out
}

// ---------------------------------------------------------------------------
// AST walking: gather every item, including ones nested inside blocks.
// ---------------------------------------------------------------------------

pub fn gather_items(program: &Program) -> Vec<&Item> {
    let mut out = Vec::new();
    for item in &program.items {
        gather_item(item, &mut out);
    }
    out
}

fn gather_item<'a>(item: &'a Item, out: &mut Vec<&'a Item>) {
    out.push(item);
    match item {
        Item::Fn(decl) => scan_block(&decl.body, out),
        Item::Impl(block) => {
            for func in &block.fns {
                scan_block(&func.body, out);
            }
        }
        Item::Const(decl) => scan_expr(&decl.value, out),
        _ => {}
    }
}

fn scan_block<'a>(block: &'a Block, out: &mut Vec<&'a Item>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Item(item) => gather_item(item, out),
            Stmt::Let(LetStmt { init, .. }) => {
                if let Some(init) = init {
                    scan_expr(init, out);
                }
            }
            Stmt::Expr(expr) => scan_expr(expr, out),
            Stmt::Empty(_) => {}
        }
    }
    if let Some(tail) = &block.tail {
        scan_expr(tail, out);
    }
}

fn scan_expr<'a>(expr: &'a Expr, out: &mut Vec<&'a Item>) {
    match &expr.kind {
        ExprKind::Lit(_) | ExprKind::Path(_) | ExprKind::Continue => {}
        ExprKind::Unary { expr, .. }
        | ExprKind::Cast { expr, .. }
        | ExprKind::TupleField { base: expr, .. }
        | ExprKind::Try { expr }
        | ExprKind::Ref { expr, .. }
        | ExprKind::Deref { expr }
        | ExprKind::Field { base: expr, .. } => scan_expr(expr, out),
        ExprKind::Binary { lhs, rhs, .. } => {
            scan_expr(lhs, out);
            scan_expr(rhs, out);
        }
        ExprKind::Assign { target, value, .. } => {
            scan_expr(target, out);
            scan_expr(value, out);
        }
        ExprKind::Call { callee, args } => {
            scan_expr(callee, out);
            for arg in args {
                scan_expr(arg, out);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            scan_expr(receiver, out);
            for arg in args {
                scan_expr(arg, out);
            }
        }
        ExprKind::Index { base, index } => {
            scan_expr(base, out);
            scan_expr(index, out);
        }
        ExprKind::If(if_expr) => {
            scan_expr(&if_expr.cond, out);
            scan_block(&if_expr.then_block, out);
            if let Some(else_expr) = &if_expr.else_expr {
                scan_expr(else_expr, out);
            }
        }
        ExprKind::Match(match_expr) => {
            scan_expr(&match_expr.scrutinee, out);
            for arm in &match_expr.arms {
                if let Some(guard) = &arm.guard {
                    scan_expr(guard, out);
                }
                scan_expr(&arm.body, out);
            }
        }
        ExprKind::Block(block) => scan_block(block, out),
        ExprKind::Loop { body } => scan_block(body, out),
        ExprKind::While { cond, body } => {
            scan_expr(cond, out);
            scan_block(body, out);
        }
        ExprKind::For { iter, body, .. } => {
            scan_expr(iter, out);
            scan_block(body, out);
        }
        ExprKind::Range { start, end, .. } => {
            scan_expr(start, out);
            scan_expr(end, out);
        }
        ExprKind::Closure(closure) => scan_expr(&closure.body, out),
        ExprKind::StructLit { fields, .. } => {
            for field in fields {
                scan_expr(&field.value, out);
            }
        }
        ExprKind::Tuple(exprs) => {
            for expr in exprs {
                scan_expr(expr, out);
            }
        }
        ExprKind::Vec(vec_lit) => match vec_lit {
            VecLit::List(exprs) => {
                for expr in exprs {
                    scan_expr(expr, out);
                }
            }
            VecLit::Repeat { value, count } => {
                scan_expr(value, out);
                scan_expr(count, out);
            }
        },
        ExprKind::Macro(macro_call) => {
            for arg in &macro_call.args {
                scan_expr(arg, out);
            }
        }
        ExprKind::Break(value) => {
            if let Some(value) = value {
                scan_expr(value, out);
            }
        }
        ExprKind::Return(value) => {
            if let Some(value) = value {
                scan_expr(value, out);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Collection phase
// ---------------------------------------------------------------------------

impl<'a> Checker<'a> {
    /// Register every item of one program. Call [Checker::resolve_all_types]
    /// once after all programs (user and prelude) have been collected.
    pub fn collect_items_only(&mut self, program: &Program, origin: DefOrigin) {
        self.collecting_origin = origin;
        let items = gather_items(program);
        for item in &items {
            match item {
                Item::Struct(decl) => self.register_struct(decl),
                Item::Enum(decl) => self.register_enum(decl),
                Item::TypeAlias(decl) => self.register_alias(decl),
                Item::Trait(decl) => self.register_trait_decl(decl),
                _ => {}
            }
        }
        for item in &items {
            match item {
                Item::Fn(decl) => self.register_free_fn(decl),
                Item::Impl(block) => self.pending_impls.push((block.clone(), origin)),
                Item::Const(decl) => self.register_const(decl),
                _ => {}
            }
        }
        self.collecting_origin = DefOrigin::User;
    }

    /// Register every deferred impl block. Called after all programs (user and
    /// prelude) have been collected, so impls can name traits declared by any
    /// of them.
    pub fn register_pending_impls(&mut self) {
        let pending = std::mem::take(&mut self.pending_impls);
        for (block, origin) in pending {
            self.collecting_origin = origin;
            self.register_impl(&block);
        }
        self.collecting_origin = DefOrigin::User;
    }

    pub fn type_name_conflict(&self, name: &str) -> Option<Span> {
        if let Some(id) = self.struct_by_name.get(name) {
            return self.structs.get(*id).map(|info| info.span);
        }
        if let Some(id) = self.enum_by_name.get(name) {
            return self.enums.get(*id).map(|info| info.span);
        }
        if let Some(id) = self.alias_by_name.get(name) {
            return self.aliases.get(*id).map(|info| info.span);
        }
        if let Some(id) = self.generic_struct_by_name.get(name) {
            return self.generic_structs.get(*id).map(|info| info.span);
        }
        if let Some(id) = self.generic_enum_by_name.get(name) {
            return self.generic_enums.get(*id).map(|info| info.span);
        }
        if let Some(id) = self.trait_by_name.get(name) {
            return self.traits.get(*id).map(|info| info.span);
        }
        None
    }

    fn report_duplicate_type(&mut self, name: &str, span: Span) {
        let mut diag = Diagnostic::error(
            format!("the name '{}' is defined multiple times", name),
            span,
        )
        .with_code("E0428");
        if let Some(previous) = self.type_name_conflict(name) {
            diag = diag.with_label(previous, format!("previous definition of '{}' here", name));
        }
        self.diagnostics.push(diag);
    }

    fn register_struct(&mut self, decl: &StructDecl) {
        if self.type_name_conflict(&decl.name).is_some() {
            self.report_duplicate_type(&decl.name, decl.name_span);
            return;
        }
        if !decl.generics.is_empty() {
            self.validate_generic_params(&decl.generics);
            let def_id = self.generic_structs.len();
            self.generic_structs.push(GStructDef {
                name: decl.name.clone(),
                span: decl.span,
                generics: decl.generics.clone(),
                decl: decl.clone(),
                instances: HashMap::new(),
            });
            self.generic_struct_by_name
                .insert(decl.name.clone(), def_id);
            return;
        }
        let id = self.structs.len();
        let c_name = self.alloc_c_name(sanitize_c_ident(&decl.name));
        self.structs.push(StructInfo {
            name: decl.name.clone(),
            c_name,
            generic: None,
            span: decl.span,
            decl_fields: decl.fields.clone(),
            fields: Vec::new(),
            fields_resolved: false,
        });
        // One drop-hook slot per struct, filled in by register_drop_impl.
        self.drop_structs.push(None);
        self.struct_by_name.insert(decl.name.clone(), id);
    }

    fn register_enum(&mut self, decl: &EnumDecl) {
        if self.type_name_conflict(&decl.name).is_some() {
            self.report_duplicate_type(&decl.name, decl.name_span);
            return;
        }
        if !decl.generics.is_empty() {
            self.validate_generic_params(&decl.generics);
            let def_id = self.generic_enums.len();
            self.generic_enums.push(GEnumDef {
                name: decl.name.clone(),
                span: decl.span,
                generics: decl.generics.clone(),
                decl: decl.clone(),
                instances: HashMap::new(),
            });
            self.generic_enum_by_name.insert(decl.name.clone(), def_id);
            return;
        }
        let id = self.enums.len();
        let c_name = self.alloc_c_name(sanitize_c_ident(&decl.name));
        self.enums.push(EnumInfo {
            name: decl.name.clone(),
            c_name,
            generic: None,
            span: decl.span,
            decl_variants: decl.variants.clone(),
            variants: Vec::new(),
            variants_resolved: false,
        });
        // One drop-hook slot per enum, filled in by register_drop_impl.
        self.drop_enums.push(None);
        self.enum_by_name.insert(decl.name.clone(), id);
    }

    /// Validate the shape of a generic parameter list (duplicate names E0403).
    pub fn validate_generic_params(&mut self, generics: &[ast::GenericParam]) {
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

    /// Reject the reserved method names drop and clone in inherent impls.
    fn check_method_name_reserved(&mut self, _type_name: &str, func: &FnDecl) {
        if func.name == "drop" {
            self.error_at(
                "the method name 'drop' is reserved for the Drop trait; rename the method"
                    .to_string(),
                func.name_span,
            );
        } else if func.name == "clone" {
            self.error_at(
                "the method name 'clone' is reserved: clone is derived automatically for every type"
                    .to_string(),
                func.name_span,
            );
        }
    }

    fn register_alias(&mut self, decl: &TypeAliasDecl) {
        if self.type_name_conflict(&decl.name).is_some() {
            self.report_duplicate_type(&decl.name, decl.name_span);
            return;
        }
        if !decl.generics.is_empty() {
            self.error_code(
                "type aliases cannot be generic".to_string(),
                decl.name_span,
                "E0001",
            );
        }
        let id = self.aliases.len();
        self.aliases.push(AliasInfo {
            name: decl.name.clone(),
            span: decl.span,
            target: decl.ty.clone(),
            resolved: None,
            resolving: false,
        });
        self.alias_by_name.insert(decl.name.clone(), id);
    }

    fn register_free_fn(&mut self, decl: &FnDecl) {
        if let Some(previous) = self.fn_by_name.get(&decl.name).copied() {
            self.report_duplicate_fn(decl, self.fns[previous].name_span);
            return;
        }
        if self.generic_fn_by_name.contains_key(&decl.name) {
            self.report_duplicate_fn(decl, decl.name_span);
            return;
        }
        if !decl.generics.is_empty() {
            self.validate_generic_params(&decl.generics);
            let def_id = self.generic_fns.len();
            self.generic_fns.push(GFnDef {
                name: decl.name.clone(),
                name_span: decl.name_span,
                span: decl.span,
                generics: decl.generics.clone(),
                decl: decl.clone(),
                owner: GenericOwner::Free,
                instances: HashMap::new(),
                origin: self.collecting_origin,
                c_name_base: format!("lsaca_fn_{}", sanitize_c_ident(&decl.name)),
                display_base: decl.name.clone(),
            });
            self.generic_fn_by_name.insert(decl.name.clone(), def_id);
            return;
        }
        let id = self.fns.len();
        let c_name = self.alloc_c_name(format!("lsaca_fn_{}", sanitize_c_ident(&decl.name)));
        self.fns.push(FnInfo {
            name: decl.name.clone(),
            display: decl.name.clone(),
            c_name,
            decl: decl.clone(),
            origin: self.collecting_origin,
            generic: None,
            self_ty: None,
            instantiation_depth: 0,
            owner: None,
            self_kind: None,
            sig: None,
            checked: None,
            param_cnames: Vec::new(),
            name_span: decl.name_span,
            span: decl.span,
        });
        self.fn_by_name.insert(decl.name.clone(), id);
    }

    fn report_duplicate_fn(&mut self, decl: &FnDecl, previous_span: Span) {
        let diag = Diagnostic::error(
            format!("the name '{}' is defined multiple times", decl.name),
            decl.name_span,
        )
        .with_code("E0428")
        .with_label(
            previous_span,
            format!("previous definition of '{}' here", decl.name),
        );
        self.diagnostics.push(diag);
    }

    fn register_impl(&mut self, block: &ImplBlock) {
        self.validate_impl_generics(&block.generics);
        self.reject_generic_methods(block);
        if let Some(trait_ref) = &block.trait_name {
            match trait_ref.name.as_str() {
                "Drop" => {
                    if block.generics.is_empty() {
                        if let Some(owner) = self.resolve_impl_target(block) {
                            self.register_drop_impl(block, owner, trait_ref);
                        }
                    } else {
                        self.register_generic_drop_impl(block, trait_ref);
                    }
                }
                "Copy" | "Clone" => self.error_code(
                    format!(
                        "the trait '{}' is derived automatically; remove this impl block",
                        trait_ref.name
                    ),
                    trait_ref.span,
                    "E0119",
                ),
                _ => self.register_trait_impl_block(block, trait_ref),
            }
            return;
        }
        if !block.generics.is_empty() {
            self.register_generic_inherent_impl(block);
            return;
        }
        let owner = match self.resolve_impl_target(block) {
            Some(owner) => owner,
            None => return,
        };
        for func in &block.fns {
            let type_name = self.type_name(owner);
            self.check_method_name_reserved(&type_name, func);
            let key = (owner, func.name.clone());
            if let Some(previous) = self.methods.get(&key) {
                let previous_span = self.fns[*previous].name_span;
                let diag = Diagnostic::error(
                    format!("duplicate definitions with name '{}'", func.name),
                    func.name_span,
                )
                .with_code("E0592")
                .with_label(
                    previous_span,
                    format!("previous definition of '{}' here", func.name),
                );
                self.diagnostics.push(diag);
                continue;
            }
            let display = format!("{}::{}", type_name, func.name);
            let c_name = self.alloc_c_name(format!(
                "lsaca_fn_{}_{}",
                sanitize_c_ident(&type_name),
                sanitize_c_ident(&func.name)
            ));
            let id = self.fns.len();
            self.fns.push(FnInfo {
                name: func.name.clone(),
                display,
                c_name,
                decl: func.clone(),
                origin: self.collecting_origin,
                generic: None,
                self_ty: None,
                instantiation_depth: 0,
                owner: Some(owner),
                self_kind: func.self_kind,
                sig: None,
                checked: None,
                param_cnames: Vec::new(),
                name_span: func.name_span,
                span: func.span,
            });
            self.methods.insert(key, id);
        }
    }

    /// Register a generic inherent impl block; its methods are instantiated on
    /// demand when a method or associated function is used.
    fn register_generic_inherent_impl(&mut self, block: &ImplBlock) {
        if !self.impl_target_is_user_type(block) {
            self.report_invalid_impl_target(block);
            return;
        }
        let impl_id = self.impls.len();
        let mut methods = Vec::new();
        for func in &block.fns {
            self.check_method_name_reserved(&block.ty_name, func);
            let gfn_id = self.register_generic_method(
                impl_id,
                &block.generics,
                func,
                GenericOwner::InherentImpl(impl_id),
                format!(
                    "lsaca_fn_{}_{}",
                    sanitize_c_ident(&block.ty_name),
                    sanitize_c_ident(&func.name)
                ),
                format!("{}::{}", block.ty_name, func.name),
            );
            methods.push(ImplMethod {
                name: func.name.clone(),
                gfn_id,
            });
        }
        self.impls.push(ImplInfo {
            id: impl_id,
            generics: block.generics.clone(),
            self_ty: block.ty.clone(),
            trait_ref: None,
            methods,
            span: block.span,
            late_checked: true,
        });
    }

    /// Register "impl Drop for Generic<T>"; the drop hook is instantiated for
    /// every concrete instance of the generic type.
    fn register_generic_drop_impl(&mut self, block: &ImplBlock, trait_ref: &TraitRef) {
        if !self.impl_target_is_user_type(block) {
            self.report_invalid_impl_target(block);
            return;
        }
        let type_name = block.ty_name.clone();
        if !trait_ref.args.is_empty() {
            self.error_code(
                format!(
                    "trait Drop takes 0 type arguments but {} were supplied",
                    trait_ref.args.len()
                ),
                trait_ref.span,
                "E0107",
            );
            return;
        }
        let method = match block.fns.as_slice() {
            [method] if method.name == "drop" => method,
            _ => {
                self.error_at(
                    format!(
                        "impl Drop for {} must define exactly one method named 'drop'",
                        type_name
                    ),
                    trait_ref.span,
                );
                return;
            }
        };
        let mut valid = true;
        let takes_self = method.self_kind == Some(SelfKind::RefMut)
            && method.params.len() == 1
            && method.params[0].name == "self";
        if !takes_self {
            self.error_at(
                format!(
                    "the 'drop' method must take '&mut self' as its only parameter (impl Drop for {})",
                    type_name
                ),
                method.name_span,
            );
            valid = false;
        }
        if !returns_unit(method) {
            let span = method
                .ret
                .as_ref()
                .map(|ret| ret.span())
                .unwrap_or(method.name_span);
            self.error_at(
                format!(
                    "the 'drop' method must return unit (impl Drop for {})",
                    type_name
                ),
                span,
            );
            valid = false;
        }
        if !valid {
            return;
        }
        let impl_id = self.impls.len();
        let gfn_id = self.register_generic_method(
            impl_id,
            &block.generics,
            method,
            GenericOwner::TraitImpl(impl_id),
            format!("lsaca_fn_Drop_{}", sanitize_c_ident(&type_name)),
            "Drop::drop".to_string(),
        );
        self.impls.push(ImplInfo {
            id: impl_id,
            generics: block.generics.clone(),
            self_ty: block.ty.clone(),
            trait_ref: Some(trait_ref.clone()),
            methods: vec![ImplMethod {
                name: method.name.clone(),
                gfn_id,
            }],
            span: block.span,
            late_checked: true,
        });
    }

    /// Validate "impl Drop for Type" and record the drop hook for the type.
    fn register_drop_impl(&mut self, block: &ImplBlock, owner: TypeKey, trait_ref: &TraitRef) {
        let type_name = self.type_name(owner);
        let method = match block.fns.as_slice() {
            [method] if method.name == "drop" => method,
            _ => {
                self.error_at(
                    format!(
                        "impl Drop for {} must define exactly one method named 'drop'",
                        type_name
                    ),
                    trait_ref.span,
                );
                return;
            }
        };
        let mut valid = true;
        let takes_self = method.self_kind == Some(SelfKind::RefMut)
            && method.params.len() == 1
            && method.params[0].name == "self";
        if !takes_self {
            self.error_at(
                format!(
                    "the 'drop' method must take '&mut self' as its only parameter (impl Drop for {})",
                    type_name
                ),
                method.name_span,
            );
            valid = false;
        }
        if !returns_unit(method) {
            let span = method
                .ret
                .as_ref()
                .map(|ret| ret.span())
                .unwrap_or(method.name_span);
            self.error_at(
                format!(
                    "the 'drop' method must return unit (impl Drop for {})",
                    type_name
                ),
                span,
            );
            valid = false;
        }
        if !valid {
            return;
        }
        let previous_hook = match owner {
            TypeKey::Struct(id) => self.drop_structs[id],
            TypeKey::Enum(id) => self.drop_enums[id],
        };
        if let Some(previous) = previous_hook {
            let previous_span = self.fns[previous].span;
            let diag = Diagnostic::error(
                format!(
                    "conflicting implementations of trait 'Drop' for type '{}'",
                    type_name
                ),
                trait_ref.span,
            )
            .with_code("E0119")
            .with_label(previous_span, "first implementation here".to_string());
            self.diagnostics.push(diag);
            return;
        }
        // The hook is a normal function so its body is checked and lowered, but
        // it is deliberately absent from the method table: user code cannot
        // call it directly.
        let hook = self.push_impl_fn(owner, method);
        match owner {
            TypeKey::Struct(id) => self.drop_structs[id] = Some(hook),
            TypeKey::Enum(id) => self.drop_enums[id] = Some(hook),
        }
    }

    /// Allocate the FnInfo for a function declared in an impl block. Whether it
    /// becomes reachable as a method is up to the caller.
    fn push_impl_fn(&mut self, owner: TypeKey, func: &FnDecl) -> usize {
        let type_name = self.type_name(owner);
        let display = format!("{}::{}", type_name, func.name);
        let c_name = self.alloc_c_name(format!(
            "lsaca_fn_{}_{}",
            sanitize_c_ident(&type_name),
            sanitize_c_ident(&func.name)
        ));
        let id = self.fns.len();
        self.fns.push(FnInfo {
            name: func.name.clone(),
            display,
            c_name,
            decl: func.clone(),
            origin: self.collecting_origin,
            generic: None,
            self_ty: None,
            instantiation_depth: 0,
            owner: Some(owner),
            self_kind: func.self_kind,
            sig: None,
            checked: None,
            param_cnames: Vec::new(),
            name_span: func.name_span,
            span: func.span,
        });
        id
    }

    fn register_const(&mut self, decl: &ast::ConstDecl) {
        if let Some(previous) = self.const_by_name.get(&decl.name) {
            let previous_span = self.consts[*previous].decl.name_span;
            let diag = Diagnostic::error(
                format!("the name '{}' is defined multiple times", decl.name),
                decl.name_span,
            )
            .with_code("E0428")
            .with_label(
                previous_span,
                format!("previous definition of '{}' here", decl.name),
            );
            self.diagnostics.push(diag);
            return;
        }
        let id = self.consts.len();
        let c_name = self.alloc_c_name(format!("lsaca_const_{}", sanitize_c_ident(&decl.name)));
        self.consts.push(ConstItem {
            decl: decl.clone(),
            c_name,
            state: ConstState::Unresolved,
        });
        self.const_by_name.insert(decl.name.clone(), id);
    }

    pub fn alloc_c_name(&mut self, base: String) -> String {
        if self.used_c_names.insert(base.clone()) {
            return base;
        }
        loop {
            self.c_name_counter += 1;
            let candidate = format!("{}_{}", base, self.c_name_counter);
            if self.used_c_names.insert(candidate.clone()) {
                return candidate;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Type name resolution
    // -----------------------------------------------------------------------

    /// Whether the target of an impl block names a user-defined struct or
    /// enum. Builtin types (`String`, `Vec<T>`, tuples, ...) can never be impl
    /// targets: they cannot gain inherent methods or trait implementations.
    pub fn impl_target_is_user_type(&self, block: &ImplBlock) -> bool {
        match &block.ty {
            TypeExpr::Path { name, .. } => {
                self.struct_by_name.contains_key(name)
                    || self.enum_by_name.contains_key(name)
                    || self.generic_struct_by_name.contains_key(name)
                    || self.generic_enum_by_name.contains_key(name)
            }
            _ => false,
        }
    }

    /// Report an impl block whose target is not a user-defined struct or enum.
    pub fn report_invalid_impl_target(&mut self, block: &ImplBlock) {
        self.error_code(
            format!(
                "impl target must be a struct or enum type defined in this program, found {}",
                block.ty.describe()
            ),
            block.ty_span,
            "E0118",
        );
    }

    pub fn resolve_impl_target(&mut self, block: &ImplBlock) -> Option<TypeKey> {
        let resolved = self.resolve_ty(&block.ty, None);
        match resolved {
            Some(ITy::Struct(id)) => Some(TypeKey::Struct(id)),
            Some(ITy::Enum(id)) => Some(TypeKey::Enum(id)),
            Some(_) => {
                self.report_invalid_impl_target(block);
                None
            }
            None => None,
        }
    }

    pub fn resolve_ty(&mut self, ty: &TypeExpr, self_key: Option<TypeKey>) -> Option<ITy> {
        match ty {
            TypeExpr::Path {
                name,
                name_span,
                generics,
            } => self.resolve_type_path(name, *name_span, generics, self_key),
            TypeExpr::Tuple { elems, .. } => {
                let mut resolved = Vec::with_capacity(elems.len());
                for elem in elems {
                    resolved.push(self.resolve_ty(elem, self_key)?);
                }
                Some(ITy::tuple(resolved))
            }
            TypeExpr::Fn { params, ret, .. } => {
                let mut resolved_params = Vec::with_capacity(params.len());
                for param in params {
                    resolved_params.push(self.resolve_ty(param, self_key)?);
                }
                let resolved_ret = self.resolve_ty(ret, self_key)?;
                Some(ITy::function(resolved_params, resolved_ret))
            }
            TypeExpr::Ref { mutable, inner, .. } => {
                let resolved = self.resolve_ty(inner, self_key)?;
                Some(ITy::Ref {
                    mutable: *mutable,
                    inner: Box::new(resolved),
                })
            }
        }
    }

    fn resolve_type_path(
        &mut self,
        name: &str,
        span: Span,
        generics: &[TypeExpr],
        self_key: Option<TypeKey>,
    ) -> Option<ITy> {
        // Generic parameters in scope shadow every type name.
        if name != "Self"
            && let Some(ty) = self.lookup_generic_env(name)
        {
            if !generics.is_empty() {
                self.error_code(
                    format!("generic parameter '{}' does not take type arguments", name),
                    span,
                    "E0107",
                );
            }
            return Some(ty);
        }
        match name {
            "Self" => {
                if let Some(ty) = self.lookup_generic_env("Self") {
                    return Some(ty);
                }
                match self_key {
                    Some(TypeKey::Struct(id)) => Some(ITy::Struct(id)),
                    Some(TypeKey::Enum(id)) => Some(ITy::Enum(id)),
                    None => {
                        self.error_at(
                            "Self is only available inside an impl block".to_string(),
                            span,
                        );
                        None
                    }
                }
            }
            "()" => {
                if generics.is_empty() {
                    Some(ITy::Unit)
                } else {
                    self.arity_error(0, span, generics.len());
                    None
                }
            }
            "bool" => self.simple_type(span, generics, ITy::bool_()),
            "char" => self.simple_type(span, generics, ITy::char_()),
            "String" => self.simple_type(span, generics, ITy::string()),
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => {
                match crate::types::IntTy::from_name(name) {
                    Some(int_ty) => self.simple_type(span, generics, ITy::Int(int_ty)),
                    None => None,
                }
            }
            "f32" | "f64" => match crate::types::FloatTy::from_name(name) {
                Some(float_ty) => self.simple_type(span, generics, ITy::Float(float_ty)),
                None => None,
            },
            "Vec" | "Rc" | "Box" | "Option" => {
                if generics.len() != 1 {
                    self.arity_error(1, span, generics.len());
                    return None;
                }
                let inner = self.resolve_ty(&generics[0], self_key)?;
                match name {
                    "Vec" => Some(ITy::vec(inner)),
                    "Rc" => Some(ITy::rc(inner)),
                    "Box" => Some(ITy::box_(inner)),
                    _ => Some(ITy::option(inner)),
                }
            }
            "Result" => {
                if generics.len() != 2 {
                    self.arity_error(2, span, generics.len());
                    return None;
                }
                let ok = self.resolve_ty(&generics[0], self_key)?;
                let err = self.resolve_ty(&generics[1], self_key)?;
                Some(ITy::result(ok, err))
            }
            "HashMap" => {
                if generics.len() != 2 {
                    self.arity_error(2, span, generics.len());
                    return None;
                }
                let key = self.resolve_ty(&generics[0], self_key)?;
                let value = self.resolve_ty(&generics[1], self_key)?;
                self.validate_hash_map_type(&key, &value, span);
                Some(ITy::hash_map(key, value))
            }
            _ => {
                if let Some(&id) = self.alias_by_name.get(name) {
                    if !generics.is_empty() {
                        self.error_at(
                            format!("type alias '{}' does not take type arguments", name),
                            span,
                        );
                    }
                    return self.ensure_alias(id);
                }
                if let Some(&def_id) = self.generic_struct_by_name.get(name) {
                    return self.instantiate_generic_struct_path(def_id, generics, self_key, span);
                }
                if let Some(&id) = self.struct_by_name.get(name) {
                    if !generics.is_empty() {
                        self.error_at(
                            format!("struct '{}' does not take type arguments", name),
                            span,
                        );
                    }
                    return Some(ITy::Struct(id));
                }
                if let Some(&def_id) = self.generic_enum_by_name.get(name) {
                    return self.instantiate_generic_enum_path(def_id, generics, self_key, span);
                }
                if let Some(&id) = self.enum_by_name.get(name) {
                    if !generics.is_empty() {
                        self.error_at(
                            format!("enum '{}' does not take type arguments", name),
                            span,
                        );
                    }
                    return Some(ITy::Enum(id));
                }
                if self.trait_by_name.contains_key(name) {
                    self.error_code(
                        format!(
                            "trait ({}) cannot be used as a type; trait objects are not supported",
                            name
                        ),
                        span,
                        "E0412",
                    );
                    return None;
                }
                let mut diag =
                    Diagnostic::error(format!("cannot find type '{}' in this scope", name), span)
                        .with_code("E0412");
                if let Some(suggestion) = self.suggest_type_name(name) {
                    diag = diag.with_note(format!("did you mean '{}'?", suggestion));
                }
                self.diagnostics.push(diag);
                None
            }
        }
    }

    fn instantiate_generic_struct_path(
        &mut self,
        def_id: usize,
        generics: &[TypeExpr],
        self_key: Option<TypeKey>,
        span: Span,
    ) -> Option<ITy> {
        let def = self.generic_structs.get(def_id)?.clone();
        let arity = def.generics.len();
        if generics.len() != arity {
            self.error_code(
                format!(
                    "struct {} takes {} type argument{} but {} were supplied",
                    def.name,
                    arity,
                    if arity == 1 { "" } else { "s" },
                    generics.len()
                ),
                span,
                "E0107",
            );
            return None;
        }
        let mut args = Vec::with_capacity(arity);
        let mut unresolved = false;
        for expr in generics {
            let resolved = self.resolve_ty(expr, self_key)?;
            match self.normalize_instance_arg(&resolved) {
                Some(arg) => args.push(arg),
                None => {
                    if self.probing {
                        args.push(self.infer.fresh_var());
                    } else {
                        self.error_code(
                            "type annotations needed".to_string(),
                            expr.span(),
                            "E0282",
                        );
                        unresolved = true;
                    }
                }
            }
        }
        if unresolved {
            return None;
        }
        if self.probing && args.iter().any(|arg| matches!(arg, ITy::Var(_))) {
            return Some(self.infer.fresh_var());
        }
        self.instantiate_struct(def_id, args, span).map(ITy::Struct)
    }

    fn instantiate_generic_enum_path(
        &mut self,
        def_id: usize,
        generics: &[TypeExpr],
        self_key: Option<TypeKey>,
        span: Span,
    ) -> Option<ITy> {
        let def = self.generic_enums.get(def_id)?.clone();
        let arity = def.generics.len();
        if generics.len() != arity {
            self.error_code(
                format!(
                    "enum {} takes {} type argument{} but {} were supplied",
                    def.name,
                    arity,
                    if arity == 1 { "" } else { "s" },
                    generics.len()
                ),
                span,
                "E0107",
            );
            return None;
        }
        let mut args = Vec::with_capacity(arity);
        let mut unresolved = false;
        for expr in generics {
            let resolved = self.resolve_ty(expr, self_key)?;
            match self.normalize_instance_arg(&resolved) {
                Some(arg) => args.push(arg),
                None => {
                    if self.probing {
                        args.push(self.infer.fresh_var());
                    } else {
                        self.error_code(
                            "type annotations needed".to_string(),
                            expr.span(),
                            "E0282",
                        );
                        unresolved = true;
                    }
                }
            }
        }
        if unresolved {
            return None;
        }
        if self.probing && args.iter().any(|arg| matches!(arg, ITy::Var(_))) {
            return Some(self.infer.fresh_var());
        }
        self.instantiate_enum(def_id, args, span).map(ITy::Enum)
    }

    fn arity_error(&mut self, expected: usize, span: Span, found: usize) {
        self.error_at(
            format!(
                "wrong number of type arguments: expected {}, found {}",
                expected, found
            ),
            span,
        );
    }

    fn simple_type(&mut self, span: Span, generics: &[TypeExpr], ty: ITy) -> Option<ITy> {
        if !generics.is_empty() {
            self.arity_error(0, span, generics.len());
            return None;
        }
        Some(ty)
    }

    pub fn suggest_type_name(&self, name: &str) -> Option<String> {
        let mut candidates: Vec<&str> = Vec::new();
        candidates.extend(self.struct_by_name.keys().map(|name| name.as_str()));
        candidates.extend(self.enum_by_name.keys().map(|name| name.as_str()));
        candidates.extend(self.alias_by_name.keys().map(|name| name.as_str()));
        candidates.extend(self.generic_struct_by_name.keys().map(|name| name.as_str()));
        candidates.extend(self.generic_enum_by_name.keys().map(|name| name.as_str()));
        candidates.sort_unstable();
        // Internal standard-library names are not a part of the language the
        // user writes: never suggest one.
        candidates.retain(|candidate| !candidate.starts_with("__lsaca_"));
        closest_name(name, &candidates)
    }

    pub fn ensure_alias(&mut self, id: usize) -> Option<ITy> {
        if let Some(resolved) = &self.aliases[id].resolved {
            return Some(resolved.clone());
        }
        if self.aliases[id].resolving {
            let name = self.aliases[id].name.clone();
            let span = self.aliases[id].span;
            self.error_at(
                format!("cycle detected when expanding type alias '{}'", name),
                span,
            );
            let placeholder = self.infer.fresh_var();
            self.aliases[id].resolved = Some(placeholder.clone());
            return Some(placeholder);
        }
        self.aliases[id].resolving = true;
        let target = self.aliases[id].target.clone();
        let resolved = self.resolve_ty(&target, None);
        self.aliases[id].resolving = false;
        if let Some(ty) = &resolved {
            self.aliases[id].resolved = Some(ty.clone());
        }
        resolved
    }

    pub fn ensure_struct_fields(&mut self, id: usize) {
        if self.structs[id].fields_resolved {
            return;
        }
        self.structs[id].fields_resolved = true;
        let decl_fields = self.structs[id].decl_fields.clone();
        let mut fields: Vec<StructField> = Vec::with_capacity(decl_fields.len());
        let mut seen: HashMap<String, Span> = HashMap::new();
        for field in &decl_fields {
            if let Some(previous) = seen.get(&field.name) {
                let previous = *previous;
                let diag = Diagnostic::error(
                    format!("field '{}' is already declared", field.name),
                    field.name_span,
                )
                .with_code("E0124")
                .with_label(
                    previous,
                    format!("previous declaration of '{}' here", field.name),
                );
                self.diagnostics.push(diag);
                continue;
            }
            seen.insert(field.name.clone(), field.name_span);
            let ty = match self.resolve_ty(&field.ty, None) {
                Some(ty) => ty,
                None => self.infer.fresh_var(),
            };
            self.validate_storage_type(&ty, field.name_span);
            fields.push(StructField {
                name: field.name.clone(),
                name_span: field.name_span,
                ty,
            });
        }
        self.structs[id].fields = fields;
    }

    pub fn ensure_enum_variants(&mut self, id: usize) {
        if self.enums[id].variants_resolved {
            return;
        }
        self.enums[id].variants_resolved = true;
        let decl_variants = self.enums[id].decl_variants.clone();
        let enum_name = self.enums[id].name.clone();
        let mut variants: Vec<VariantInfo> = Vec::with_capacity(decl_variants.len());
        let mut seen: HashMap<String, Span> = HashMap::new();
        for variant in &decl_variants {
            if let Some(previous) = seen.get(&variant.name) {
                let previous = *previous;
                let diag = Diagnostic::error(
                    format!(
                        "variant '{}' is defined multiple times in enum '{}'",
                        variant.name, enum_name
                    ),
                    variant.name_span,
                )
                .with_code("E0428")
                .with_label(
                    previous,
                    format!("previous definition of '{}' here", variant.name),
                );
                self.diagnostics.push(diag);
                continue;
            }
            seen.insert(variant.name.clone(), variant.name_span);
            let mut fields = Vec::with_capacity(variant.fields.len());
            for field in &variant.fields {
                let field_span = field.span();
                match self.resolve_ty(field, None) {
                    Some(ty) => {
                        self.validate_storage_type(&ty, field_span);
                        fields.push(ty);
                    }
                    None => {
                        let placeholder = self.infer.fresh_var();
                        fields.push(placeholder);
                    }
                }
            }
            variants.push(VariantInfo {
                name: variant.name.clone(),
                span: variant.name_span,
                fields,
            });
        }
        self.enums[id].variants = variants;
    }

    pub fn resolve_all_types(&mut self) {
        for id in 0..self.aliases.len() {
            let _ = self.ensure_alias(id);
        }
        for id in 0..self.structs.len() {
            self.ensure_struct_fields(id);
        }
        for id in 0..self.enums.len() {
            self.ensure_enum_variants(id);
        }
        self.check_recursive_types();
        for id in 0..self.fns.len() {
            self.resolve_fn_sig(id);
        }
        self.build_unit_variants();
    }

    pub fn resolve_fn_sig(&mut self, id: usize) {
        if self.fns[id].sig.is_some() {
            return;
        }
        let decl = self.fns[id].decl.clone();
        let owner = self.fns[id].owner;
        let mut params: Vec<ITy> = Vec::with_capacity(decl.params.len());
        let mut seen: HashMap<String, Span> = HashMap::new();
        for param in &decl.params {
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
            let ty = match self.resolve_ty(&param.ty, owner) {
                Some(ty) => ty,
                None => self.infer.fresh_var(),
            };
            params.push(ty);
        }
        for (index, param) in decl.params.iter().enumerate() {
            if let Some(param_ty) = params.get(index).cloned() {
                self.validate_value_type(&param_ty, param.name_span);
            }
        }
        // A method receiver is a reference to the impl type, not the impl
        // type itself: inside the body `self` has type `&Self` or `&mut
        // Self`. Modeling that in the signature keeps `self` behaving like
        // any other reference value (match ergonomics, field auto-deref,
        // copies of `self` being non-owning) instead of pretending it is an
        // owned local.
        if let Some(kind) = decl.self_kind
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
        let ret = match &decl.ret {
            Some(ret_ty) => match self.resolve_ty(ret_ty, owner) {
                Some(ty) => ty,
                None => self.infer.fresh_var(),
            },
            None => ITy::Unit,
        };
        if let Some(ret_ty) = &decl.ret {
            let ret_span = ret_ty.span();
            self.validate_value_type(&ret, ret_span);
        }
        if decl.self_kind.is_some() {
            match decl.params.first() {
                Some(receiver) if receiver.name == "self" => {
                    if let Some(owner_key) = owner {
                        let expected = match owner_key {
                            TypeKey::Struct(struct_id) => ITy::Struct(struct_id),
                            TypeKey::Enum(enum_id) => ITy::Enum(enum_id),
                        };
                        // The receiver is stored as `&Self`/`&mut Self`, so
                        // compare the pointee with the impl type.
                        let actual = self.infer.resolve(&params[0]);
                        let actual = self.peel_refs(&actual);
                        let expected = self.infer.resolve(&expected);
                        if actual != expected {
                            let owner_name = self.type_name(owner_key);
                            self.error_at(
                                format!("self type must match the impl type '{}'", owner_name),
                                receiver.name_span,
                            );
                        }
                    } else {
                        self.error_at(
                            "self parameter is only allowed in methods".to_string(),
                            receiver.name_span,
                        );
                    }
                }
                _ => {
                    self.error_at(
                        "self parameter must be the first parameter".to_string(),
                        decl.name_span,
                    );
                }
            }
        } else if decl.params.iter().any(|param| param.name == "self") {
            self.error_at(
                "self parameter is only allowed in methods".to_string(),
                decl.name_span,
            );
        }
        let mut ret_ref_source: Option<usize> = None;
        let ret_resolved = self.infer.resolve(&ret);
        if let ITy::Ref { mutable, .. } = ret_resolved {
            let mut sources: Vec<usize> = Vec::new();
            // Only `&self`/`&mut self` receivers borrow from a reference. A
            // by-value `self` owns the receiver, so returning a reference
            // derived from it would dangle; such a method must elide its
            // return lifetime from one of its own reference parameters instead.
            if matches!(decl.self_kind, Some(SelfKind::Ref) | Some(SelfKind::RefMut)) {
                sources.push(0);
            } else {
                for (index, param_ty) in params.iter().enumerate() {
                    if matches!(self.infer.resolve(param_ty), ITy::Ref { .. }) {
                        sources.push(index);
                    }
                }
            }
            let ret_span = decl
                .ret
                .as_ref()
                .map(|ty| ty.span())
                .unwrap_or(decl.name_span);
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
                    let from_mutable = if source == 0 && decl.self_kind.is_some() {
                        matches!(decl.self_kind, Some(SelfKind::RefMut))
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
        let param_cnames = (0..params.len())
            .map(|index| format!("p{}", index))
            .collect();
        self.fns[id].sig = Some(FnSig {
            params,
            ret,
            ret_ref_source,
        });
        self.fns[id].param_cnames = param_cnames;
    }

    fn build_unit_variants(&mut self) {
        self.unit_variants.clear();
        for (enum_id, enum_info) in self.enums.iter().enumerate() {
            for (variant_index, variant) in enum_info.variants.iter().enumerate() {
                if variant.fields.is_empty() {
                    self.unit_variants
                        .entry(variant.name.clone())
                        .or_default()
                        .push((enum_id, variant_index));
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Layout checks
    // -----------------------------------------------------------------------

    fn check_recursive_types(&mut self) {
        let struct_count = self.structs.len();
        let total = struct_count + self.enums.len();
        let mut edges: Vec<Vec<usize>> = Vec::with_capacity(total);
        let mut spans: Vec<Span> = Vec::with_capacity(total);
        let mut names: Vec<String> = Vec::with_capacity(total);
        for info in &self.structs {
            spans.push(info.span);
            names.push(info.name.clone());
            let mut targets = Vec::new();
            for field in &info.fields {
                collect_size_edges(&field.ty, &mut targets, struct_count);
            }
            targets.sort_unstable();
            targets.dedup();
            edges.push(targets);
        }
        for info in &self.enums {
            spans.push(info.span);
            names.push(info.name.clone());
            let mut targets = Vec::new();
            for variant in &info.variants {
                for field in &variant.fields {
                    collect_size_edges(field, &mut targets, struct_count);
                }
            }
            targets.sort_unstable();
            targets.dedup();
            edges.push(targets);
        }
        let mut states: Vec<u8> = vec![0; total];
        let mut reported: HashSet<usize> = HashSet::new();
        for start in 0..total {
            if states[start] == 0 {
                self.dfs_recursive(start, &edges, &mut states, &mut reported, &spans, &names);
            }
        }
    }

    fn dfs_recursive(
        &mut self,
        node: usize,
        edges: &[Vec<usize>],
        states: &mut [u8],
        reported: &mut HashSet<usize>,
        spans: &[Span],
        names: &[String],
    ) {
        states[node] = 1;
        for target in &edges[node] {
            match states[*target] {
                0 => self.dfs_recursive(*target, edges, states, reported, spans, names),
                1 if reported.insert(*target) => {
                    let name = names[*target].clone();
                    let span = spans[*target];
                    let diag = Diagnostic::error(
                            format!("recursive type '{}' has infinite size", name),
                            span,
                        )
                        .with_code("E0072")
                        .with_note(
                            "types cannot contain themselves by value; break the cycle with Box<T>, Rc<T> or Vec<T> (all are heap allocated)"
                                .to_string(),
                        );
                    self.diagnostics.push(diag);
                }
                _ => {}
            }
        }
        states[node] = 2;
    }
}

/// Whether a declaration has no return type or returns the unit tuple.
fn returns_unit(method: &FnDecl) -> bool {
    match &method.ret {
        None => true,
        Some(TypeExpr::Tuple { elems, .. }) => elems.is_empty(),
        Some(_) => false,
    }
}

fn collect_size_edges(ty: &ITy, out: &mut Vec<usize>, struct_count: usize) {
    match ty {
        ITy::Struct(id) => out.push(*id),
        ITy::Enum(id) => out.push(struct_count + *id),
        ITy::Tuple(elems) => {
            for elem in elems {
                collect_size_edges(elem, out, struct_count);
            }
        }
        ITy::Option(elem) => collect_size_edges(elem, out, struct_count),
        ITy::Result(ok, err) => {
            collect_size_edges(ok, out, struct_count);
            collect_size_edges(err, out, struct_count);
        }
        _ => {}
    }
}

/// Find the closest candidate name (edit distance <= 2) for diagnostics.
pub fn closest_name<'a>(name: &str, candidates: &[&'a str]) -> Option<String> {
    let mut best: Option<(usize, &'a str)> = None;
    for candidate in candidates {
        if *candidate == name {
            continue;
        }
        let distance = edit_distance(name, candidate);
        if distance <= 2 {
            match best {
                Some((best_distance, _)) if best_distance <= distance => {}
                _ => best = Some((distance, candidate)),
            }
        }
    }
    best.map(|(_, candidate)| candidate.to_string())
}

fn edit_distance(lhs: &str, rhs: &str) -> usize {
    let lhs: Vec<char> = lhs.chars().collect();
    let rhs: Vec<char> = rhs.chars().collect();
    let mut previous: Vec<usize> = (0..=rhs.len()).collect();
    let mut current = vec![0usize; rhs.len() + 1];
    for i in 1..=lhs.len() {
        current[0] = i;
        for j in 1..=rhs.len() {
            let cost = if lhs[i - 1] == rhs[j - 1] { 0 } else { 1 };
            current[j] = (previous[j] + 1)
                .min(current[j - 1] + 1)
                .min(previous[j - 1] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[rhs.len()]
}
