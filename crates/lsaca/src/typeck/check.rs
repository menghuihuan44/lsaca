//! Checker core: state, scopes, diagnostics helpers and function checking.

use std::collections::{HashMap, HashSet};

use super::items::{
    AliasInfo, ConstItem, ConstState, ConstVal, EnumInfo, FnInfo, StructInfo, TypeKey,
};
use super::itype::{ITy, Infer};
use super::refs::Provenance;
use super::tree::{TBlock, TClosure, TExpr, TStmt};
use crate::diag::{Diagnostic, Diagnostics};
use crate::span::{SourceFile, Span};
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
    /// Kept so callers can render diagnostics against the source file.
    #[allow(dead_code)]
    pub file: &'a SourceFile,
    pub structs: Vec<StructInfo>,
    pub enums: Vec<EnumInfo>,
    pub aliases: Vec<AliasInfo>,
    pub consts: Vec<ConstItem>,
    pub fns: Vec<FnInfo>,
    pub struct_by_name: HashMap<String, usize>,
    pub enum_by_name: HashMap<String, usize>,
    pub alias_by_name: HashMap<String, usize>,
    pub const_by_name: HashMap<String, usize>,
    pub fn_by_name: HashMap<String, usize>,
    /// Methods and associated functions keyed by (owner type, name).
    pub methods: HashMap<(TypeKey, String), usize>,
    /// Unit variant names in scope for pattern resolution.
    pub unit_variants: HashMap<String, Vec<(usize, usize)>>,
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
    /// How many compound pattern layers (tuple/struct/variant/or) enclose the
    /// pattern currently being checked. A by-value binding of a type with drop
    /// glue below such a layer is E0509.
    pub pattern_nesting: usize,
}

impl<'a> Checker<'a> {
    pub fn new(file: &'a SourceFile) -> Checker<'a> {
        Checker {
            file,
            structs: Vec::new(),
            enums: Vec::new(),
            aliases: Vec::new(),
            consts: Vec::new(),
            fns: Vec::new(),
            struct_by_name: HashMap::new(),
            enum_by_name: HashMap::new(),
            alias_by_name: HashMap::new(),
            const_by_name: HashMap::new(),
            fn_by_name: HashMap::new(),
            methods: HashMap::new(),
            unit_variants: HashMap::new(),
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
            pattern_nesting: 0,
        }
    }

    // -- diagnostics --------------------------------------------------------

    pub fn error_at(&mut self, message: String, span: Span) {
        self.diagnostics.push(Diagnostic::error(message, span));
    }

    pub fn error_code(&mut self, message: String, span: Span, code: &'static str) {
        self.diagnostics
            .push(Diagnostic::error(message, span).with_code(code));
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

    pub fn struct_name(&self, id: usize) -> String {
        self.structs
            .get(id)
            .map(|info| info.name.clone())
            .unwrap_or_else(|| "?".to_string())
    }

    pub fn enum_name(&self, id: usize) -> String {
        self.enums
            .get(id)
            .map(|info| info.name.clone())
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
                "references cannot be captured by closures in v1.3".to_string(),
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

    /// Check every function body in source order.
    pub fn check_all_fns(&mut self) {
        for id in 0..self.fns.len() {
            self.check_fn(id);
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
        let param_cnames = self.fns[id].param_cnames.clone();
        let self_kind = self.fns[id].self_kind;
        self.current_fn = id;
        self.ref_local_sources.clear();
        let mut frame = Frame::new(sig.ret.clone());
        for (index, param) in decl.params.iter().enumerate() {
            let ty = sig.params.get(index).cloned().unwrap_or(ITy::Unit);
            let mut mutable = false;
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
        let mut body = self.check_block(&decl.body, Some(&sig.ret));
        self.frames.pop();
        // Coercions apply to return expressions, and the implicit return is the
        // tail expression of the body block.
        if let Some(tail) = body.tail.take() {
            let tail_span = tail.span;
            let coerced = self.coerce_to(*tail, &sig.ret, tail_span);
            body.tail = Some(Box::new(coerced));
        }
        let body_ty = block_type(&body);
        if !self.unify_or_report(&sig.ret, &body_ty, decl.body.span) {
            // error already reported
        }
        if let Some(tail) = &body.tail {
            let source = sig.ret_ref_source;
            let tail = (**tail).clone();
            self.check_returned_reference(&tail, source, tail.span);
        }
        self.fns[id].checked = Some(body);
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
