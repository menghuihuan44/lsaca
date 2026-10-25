//! Item collection, hoisting, type name resolution and layout checks.
//!
//! All items are collected before any body is checked (items are hoisted and
//! visible throughout the program). Items nested in blocks are hoisted too.

use std::collections::{HashMap, HashSet};

use super::check::Checker;
use super::itype::ITy;
use super::tree::TBlock;
use crate::ast::{
    self, Block, EnumDecl, Expr, ExprKind, FieldDef, FnDecl, ImplBlock, Item, LetStmt, Program,
    Stmt, StructDecl, TypeAliasDecl, TypeExpr, VariantDef, VecLit,
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
}

#[derive(Clone, Debug)]
pub struct FnInfo {
    pub name: String,
    pub display: String,
    pub c_name: String,
    pub decl: FnDecl,
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
    pub fn collect_items(&mut self, program: &Program) {
        let items = gather_items(program);
        for item in &items {
            match item {
                Item::Struct(decl) => self.register_struct(decl),
                Item::Enum(decl) => self.register_enum(decl),
                Item::TypeAlias(decl) => self.register_alias(decl),
                _ => {}
            }
        }
        for item in &items {
            match item {
                Item::Fn(decl) => self.register_free_fn(decl),
                Item::Impl(block) => self.register_impl(block),
                Item::Const(decl) => self.register_const(decl),
                _ => {}
            }
        }
        self.resolve_all_types();
    }

    fn type_name_conflict(&self, name: &str) -> Option<Span> {
        if let Some(id) = self.struct_by_name.get(name) {
            return self.structs.get(*id).map(|info| info.span);
        }
        if let Some(id) = self.enum_by_name.get(name) {
            return self.enums.get(*id).map(|info| info.span);
        }
        if let Some(id) = self.alias_by_name.get(name) {
            return self.aliases.get(*id).map(|info| info.span);
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
        let id = self.structs.len();
        self.structs.push(StructInfo {
            name: decl.name.clone(),
            span: decl.span,
            decl_fields: decl.fields.clone(),
            fields: Vec::new(),
            fields_resolved: false,
        });
        self.struct_by_name.insert(decl.name.clone(), id);
    }

    fn register_enum(&mut self, decl: &EnumDecl) {
        if self.type_name_conflict(&decl.name).is_some() {
            self.report_duplicate_type(&decl.name, decl.name_span);
            return;
        }
        let id = self.enums.len();
        self.enums.push(EnumInfo {
            name: decl.name.clone(),
            span: decl.span,
            decl_variants: decl.variants.clone(),
            variants: Vec::new(),
            variants_resolved: false,
        });
        self.enum_by_name.insert(decl.name.clone(), id);
    }

    fn register_alias(&mut self, decl: &TypeAliasDecl) {
        if self.type_name_conflict(&decl.name).is_some() {
            self.report_duplicate_type(&decl.name, decl.name_span);
            return;
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
        if self.fn_by_name.contains_key(&decl.name) {
            let previous = self.fn_by_name[&decl.name];
            let previous_span = self.fns[previous].name_span;
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
        let id = self.fns.len();
        let c_name = self.alloc_c_name(format!("lsaca_fn_{}", sanitize_c_ident(&decl.name)));
        self.fns.push(FnInfo {
            name: decl.name.clone(),
            display: decl.name.clone(),
            c_name,
            decl: decl.clone(),
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

    fn register_impl(&mut self, block: &ImplBlock) {
        let owner = match self.resolve_impl_target(block) {
            Some(owner) => owner,
            None => return,
        };
        for func in &block.fns {
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

    pub fn resolve_impl_target(&mut self, block: &ImplBlock) -> Option<TypeKey> {
        let resolved = self.resolve_ty(&block.ty, None);
        match resolved {
            Some(ITy::Struct(id)) => Some(TypeKey::Struct(id)),
            Some(ITy::Enum(id)) => Some(TypeKey::Enum(id)),
            Some(_) => {
                self.error_at(
                    "impl target must be a struct or enum type".to_string(),
                    block.ty_span,
                );
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
        }
    }

    fn resolve_type_path(
        &mut self,
        name: &str,
        span: Span,
        generics: &[TypeExpr],
        self_key: Option<TypeKey>,
    ) -> Option<ITy> {
        match name {
            "Self" => match self_key {
                Some(TypeKey::Struct(id)) => Some(ITy::Struct(id)),
                Some(TypeKey::Enum(id)) => Some(ITy::Enum(id)),
                None => {
                    self.error_at(
                        "Self is only available inside an impl block".to_string(),
                        span,
                    );
                    None
                }
            },
            "()" => {
                if generics.is_empty() {
                    Some(ITy::Unit)
                } else {
                    self.arity_error(0, span, generics.len());
                    None
                }
            }
            "bool" => self.simple_type(span, generics, ITy::bool_()),
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
                if let Some(&id) = self.struct_by_name.get(name) {
                    if !generics.is_empty() {
                        self.error_at(
                            format!("struct '{}' does not take type arguments", name),
                            span,
                        );
                    }
                    return Some(ITy::Struct(id));
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
        candidates.sort_unstable();
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
                match self.resolve_ty(field, None) {
                    Some(ty) => fields.push(ty),
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
        let ret = match &decl.ret {
            Some(ret_ty) => match self.resolve_ty(ret_ty, owner) {
                Some(ty) => ty,
                None => self.infer.fresh_var(),
            },
            None => ITy::Unit,
        };
        if decl.self_kind.is_some() {
            match decl.params.first() {
                Some(receiver) if receiver.name == "self" => {
                    if let Some(owner_key) = owner {
                        let expected = match owner_key {
                            TypeKey::Struct(struct_id) => ITy::Struct(struct_id),
                            TypeKey::Enum(enum_id) => ITy::Enum(enum_id),
                        };
                        let actual = self.infer.resolve(&params[0]);
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
        let param_cnames = (0..params.len())
            .map(|index| format!("p{}", index))
            .collect();
        self.fns[id].sig = Some(FnSig { params, ret });
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
