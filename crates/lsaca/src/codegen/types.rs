//! Monomorphized type registry: C names, structure definitions, dependency
//! ordering and zero values.
//!
//! The registry is filled by a deterministic pre-pass over the HIR (structs,
//! enums, functions, closures and constants, in program order), so generated
//! type definitions never depend on hash map iteration order.

use std::collections::HashMap;

use crate::hir::{self, HExprKind, HFormatPartKind, HPatKind};
use crate::types::Ty;

/// Names of the user defined types, indexed by StructId / EnumId.
#[derive(Debug, Clone, Default)]
pub struct TypeNames {
    pub structs: Vec<String>,
    pub enums: Vec<String>,
}

impl TypeNames {
    pub fn from_program(program: &hir::Program) -> TypeNames {
        TypeNames {
            structs: program.structs.iter().map(|s| s.name.clone()).collect(),
            enums: program.enums.iter().map(|e| e.name.clone()).collect(),
        }
    }

    fn user_struct(&self, index: usize) -> String {
        match self.structs.get(index) {
            Some(name) => crate::codegen::util::sanitize_ident(name),
            None => format!("Struct{}", index),
        }
    }

    fn user_enum(&self, index: usize) -> String {
        match self.enums.get(index) {
            Some(name) => crate::codegen::util::sanitize_ident(name),
            None => format!("Enum{}", index),
        }
    }

    /// The Rust-like mangled name of a type, without the leading Ls prefix.
    pub fn mangle(&self, ty: &Ty) -> String {
        match ty {
            Ty::Unit => String::from("unit"),
            Ty::Bool => String::from("bool"),
            Ty::Int(int_ty) => int_ty.name().to_string(),
            Ty::Float(float_ty) => float_ty.name().to_string(),
            Ty::String => String::from("String"),
            Ty::Vec(elem) => format!("Vec_{}", self.mangle(elem)),
            Ty::Option(elem) => format!("Opt_{}", self.mangle(elem)),
            Ty::Result(ok, err) => format!("Res_{}_{}", self.mangle(ok), self.mangle(err)),
            Ty::Tuple(elems) => {
                let mut out = format!("Tup{}", elems.len());
                for elem in elems {
                    out.push('_');
                    out.push_str(&self.mangle(elem));
                }
                out
            }
            Ty::Fn(params, ret) => {
                let mut out = format!("Fn{}", params.len());
                for param in params {
                    out.push('_');
                    out.push_str(&self.mangle(param));
                }
                out.push_str("_to_");
                out.push_str(&self.mangle(ret));
                out
            }
            Ty::Struct(index) => self.user_struct(*index),
            Ty::Enum(index) => self.user_enum(*index),
        }
    }

    /// The C type used to store a value of this type.
    pub fn ctype(&self, ty: &Ty) -> String {
        match ty {
            Ty::Unit => String::from("LsUnit"),
            Ty::Bool => String::from("bool"),
            Ty::Int(int_ty) => int_ty.c_type().to_string(),
            Ty::Float(float_ty) => float_ty.c_type().to_string(),
            Ty::String => String::from("LsString*"),
            _ => format!("Ls{}", self.mangle(ty)),
        }
    }
}

/// What kind of C definition a monomorphized type needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDef {
    /// Nothing is emitted; scalars and the unit type are plain C values.
    Scalar,
    /// LsString comes from the runtime header.
    Frozen,
    Struct(usize),
    Enum(usize),
    Vec(Ty),
    Option(Ty),
    Result(Ty, Ty),
    Tuple(Vec<Ty>),
    Fn(Vec<Ty>, Ty),
}

impl TypeDef {
    pub fn is_emitted(&self) -> bool {
        !matches!(self, TypeDef::Scalar | TypeDef::Frozen)
    }
}

/// All monomorphized types of one program, in deterministic discovery order.
#[derive(Debug, Clone)]
pub struct TypeRegistry {
    names: TypeNames,
    order: Vec<Ty>,
    defs: HashMap<Ty, TypeDef>,
}

impl TypeRegistry {
    pub fn collect(program: &hir::Program) -> TypeRegistry {
        let mut registry = TypeRegistry {
            names: TypeNames::from_program(program),
            order: Vec::new(),
            defs: HashMap::new(),
        };
        {
            let mut collector = Collector {
                program,
                registry: &mut registry,
            };
            collector.run();
        }
        registry
    }

    pub fn names(&self) -> &TypeNames {
        &self.names
    }

    pub fn order(&self) -> &[Ty] {
        &self.order
    }

    pub fn get(&self, ty: &Ty) -> Option<&TypeDef> {
        self.defs.get(ty)
    }

    pub fn ctype(&self, ty: &Ty) -> String {
        self.names.ctype(ty)
    }

    /// True for types whose definition is emitted by codegen (not scalars and
    /// not the frozen runtime instantiations).
    pub fn needs_definition(&self, ty: &Ty) -> bool {
        match self.defs.get(ty) {
            Some(def) => def.is_emitted(),
            None => false,
        }
    }

    /// True for the instantiations whose struct layout comes from the runtime
    /// header (LsVec_String, LsOpt_String, LsRes_String_String) and for
    /// LsString itself.
    pub fn is_frozen(&self, ty: &Ty) -> bool {
        matches!(
            ty,
            Ty::String | Ty::Vec(_) | Ty::Option(_) | Ty::Result(_, _)
        ) && self.frozen_name(ty)
    }

    fn frozen_name(&self, ty: &Ty) -> bool {
        // Unwrap the generic shape before testing, so that the check works even
        // for types that were not registered.
        match ty {
            Ty::Vec(elem) => **elem == Ty::String,
            Ty::Option(elem) => **elem == Ty::String,
            Ty::Result(ok, err) => **ok == Ty::String && **err == Ty::String,
            _ => false,
        }
    }

    /// Return the emitted type definitions in dependency order: a type whose
    /// fields are stored by value comes after those field types. Cycles (which
    /// a checked program cannot contain) are broken deterministically in
    /// discovery order.
    pub fn topological_order(&self, program: &hir::Program) -> Vec<Ty> {
        let mut order = Vec::new();
        let mut state: HashMap<Ty, u8> = HashMap::new();
        for ty in &self.order {
            if !self.needs_definition(ty) {
                continue;
            }
            self.visit_type(program, ty, &mut state, &mut order);
        }
        order
    }

    fn visit_type(
        &self,
        program: &hir::Program,
        ty: &Ty,
        state: &mut HashMap<Ty, u8>,
        order: &mut Vec<Ty>,
    ) {
        match state.get(ty).copied() {
            Some(2) => return,
            Some(1) => return, // dependency cycle: broken deterministically.
            _ => {}
        }
        state.insert(ty.clone(), 1);
        for dep in self.complete_dependencies(program, ty) {
            if self.needs_definition(&dep) {
                self.visit_type(program, &dep, state, order);
            }
        }
        state.insert(ty.clone(), 2);
        order.push(ty.clone());
    }

    /// Types that must be complete before this type can be defined. Heap
    /// indirection (vector buffers, function environments) does not need the
    /// pointee to be complete.
    pub fn complete_dependencies(&self, program: &hir::Program, ty: &Ty) -> Vec<Ty> {
        let mut deps = Vec::new();
        match ty {
            Ty::Struct(index) => {
                if let Some(item) = program.structs.get(*index) {
                    for field in &item.fields {
                        push_if_compound(&mut deps, &field.ty);
                    }
                }
            }
            Ty::Enum(index) => {
                if let Some(item) = program.enums.get(*index) {
                    for variant in &item.variants {
                        for field in &variant.fields {
                            push_if_compound(&mut deps, field);
                        }
                    }
                }
            }
            Ty::Option(elem) => push_if_compound(&mut deps, elem),
            Ty::Result(ok, err) => {
                push_if_compound(&mut deps, ok);
                push_if_compound(&mut deps, err);
            }
            Ty::Tuple(elems) => {
                for elem in elems {
                    push_if_compound(&mut deps, elem);
                }
            }
            Ty::Fn(params, ret) => {
                for param in params {
                    push_if_compound(&mut deps, param);
                }
                push_if_compound(&mut deps, ret);
            }
            Ty::Vec(_) => {}
            _ => {}
        }
        deps
    }
}

fn push_if_compound(deps: &mut Vec<Ty>, ty: &Ty) {
    if !ty.is_bitwise_copy() && !deps.contains(ty) {
        deps.push(ty.clone());
    }
}

struct Collector<'a> {
    program: &'a hir::Program,
    registry: &'a mut TypeRegistry,
}

impl<'a> Collector<'a> {
    fn run(&mut self) {
        let program = self.program;
        for (index, item) in program.structs.iter().enumerate() {
            self.visit_ty(&Ty::Struct(index));
            for field in &item.fields {
                self.visit_ty(&field.ty);
            }
        }
        for (index, item) in program.enums.iter().enumerate() {
            self.visit_ty(&Ty::Enum(index));
            for variant in &item.variants {
                for field in &variant.fields {
                    self.visit_ty(field);
                }
            }
        }
        for konst in &program.consts {
            self.visit_ty(&konst.ty);
        }
        for function in &program.fns {
            self.visit_ty(&function.ret);
            for param in &function.params {
                self.visit_ty(&param.ty);
            }
            self.visit_block(&function.body);
        }
        for closure in &program.closures {
            self.visit_ty(&closure.ret);
            for param in &closure.params {
                self.visit_ty(&param.ty);
            }
            for capture in &closure.captures {
                self.visit_ty(&capture.ty);
            }
            self.visit_block(&closure.body);
        }
    }

    fn visit_ty(&mut self, ty: &Ty) {
        if !self.registry.insert_ty(ty) {
            return;
        }
        match ty {
            Ty::Vec(elem) | Ty::Option(elem) => self.visit_ty(elem),
            Ty::Result(ok, err) => {
                self.visit_ty(ok);
                self.visit_ty(err);
            }
            Ty::Tuple(elems) => {
                for elem in elems {
                    self.visit_ty(elem);
                }
            }
            Ty::Fn(params, ret) => {
                for param in params {
                    self.visit_ty(param);
                }
                self.visit_ty(ret);
            }
            _ => {}
        }
    }

    fn visit_block(&mut self, block: &hir::HBlock) {
        for stmt in &block.stmts {
            match stmt {
                hir::HStmt::Let { pat, init, .. } => {
                    self.visit_pat(pat);
                    if let Some(init) = init {
                        self.visit_expr(init);
                    }
                }
                hir::HStmt::Expr(expr) => self.visit_expr(expr),
            }
        }
        if let Some(tail) = &block.tail {
            self.visit_expr(tail);
        }
    }

    fn visit_pat(&mut self, pat: &hir::HPat) {
        self.visit_ty(&pat.ty);
        match &pat.kind {
            HPatKind::Wild | HPatKind::Bind { .. } | HPatKind::Lit(_) => {}
            HPatKind::Tuple(parts) => {
                for part in parts {
                    self.visit_pat(part);
                }
            }
            HPatKind::Struct { fields, .. } => {
                for (_, part) in fields {
                    self.visit_pat(part);
                }
            }
            HPatKind::Enum { args, .. } => {
                for part in args {
                    self.visit_pat(part);
                }
            }
            HPatKind::OptionSome(inner)
            | HPatKind::ResultOk(inner)
            | HPatKind::ResultErr(inner) => {
                self.visit_pat(inner);
            }
            HPatKind::OptionNone => {}
            HPatKind::Or(parts) => {
                for part in parts {
                    self.visit_pat(part);
                }
            }
        }
    }

    fn visit_expr(&mut self, expr: &hir::HExpr) {
        self.visit_ty(&expr.ty);
        match &expr.kind {
            HExprKind::Lit(_) | HExprKind::ConstRef { .. } => {}
            HExprKind::Local { .. } | HExprKind::Capture { .. } | HExprKind::FnRef { .. } => {}
            HExprKind::Closure { captures, .. } => {
                for capture in captures {
                    self.visit_expr(capture);
                }
            }
            HExprKind::Call { callee, args } => {
                self.visit_expr(callee);
                for arg in args {
                    self.visit_expr(arg);
                }
            }
            HExprKind::DirectCall { receiver, args, .. } => {
                if let Some(receiver) = receiver {
                    self.visit_expr(receiver);
                }
                for arg in args {
                    self.visit_expr(arg);
                }
            }
            HExprKind::BuiltinMethod { receiver, args, .. } => {
                self.visit_expr(receiver);
                for arg in args {
                    self.visit_expr(arg);
                }
            }
            HExprKind::BuiltinAssoc {
                args, type_args, ..
            } => {
                for arg in args {
                    self.visit_expr(arg);
                }
                for ty in type_args {
                    self.visit_ty(ty);
                }
            }
            HExprKind::BuiltinCall { args, .. } => {
                for arg in args {
                    self.visit_expr(arg);
                }
            }
            HExprKind::Field { base, .. } => self.visit_expr(base),
            HExprKind::Index { base, index } => {
                self.visit_expr(base);
                self.visit_expr(index);
            }
            HExprKind::Unary { expr, .. } => self.visit_expr(expr),
            HExprKind::Binary { lhs, rhs, .. } => {
                self.visit_expr(lhs);
                self.visit_expr(rhs);
            }
            HExprKind::Assign { target, value, .. } => {
                self.visit_expr(target);
                self.visit_expr(value);
            }
            HExprKind::Cast { expr } => self.visit_expr(expr),
            HExprKind::Block(block) => self.visit_block(block),
            HExprKind::If {
                cond,
                then_block,
                else_expr,
            } => {
                self.visit_expr(cond);
                self.visit_block(then_block);
                if let Some(else_expr) = else_expr {
                    self.visit_expr(else_expr);
                }
            }
            HExprKind::Match { scrutinee, arms } => {
                self.visit_expr(scrutinee);
                for arm in arms {
                    self.visit_pat(&arm.pat);
                    if let Some(guard) = &arm.guard {
                        self.visit_expr(guard);
                    }
                    self.visit_expr(&arm.body);
                }
            }
            HExprKind::Loop { body } => self.visit_block(body),
            HExprKind::While { cond, body } => {
                self.visit_expr(cond);
                self.visit_block(body);
            }
            HExprKind::ForRange {
                pat,
                start,
                end,
                body,
                ..
            } => {
                self.visit_pat(pat);
                self.visit_expr(start);
                self.visit_expr(end);
                self.visit_block(body);
            }
            HExprKind::ForVec { pat, vec, body } => {
                self.visit_pat(pat);
                self.visit_expr(vec);
                self.visit_block(body);
            }
            HExprKind::StructLit { fields, .. } => {
                for (_, value) in fields {
                    self.visit_expr(value);
                }
            }
            HExprKind::Tuple(elems) | HExprKind::VecLit { elems } => {
                for elem in elems {
                    self.visit_expr(elem);
                }
            }
            HExprKind::VecRepeat { value, count } => {
                self.visit_expr(value);
                self.visit_expr(count);
            }
            HExprKind::EnumLit { args, .. } => {
                for arg in args {
                    self.visit_expr(arg);
                }
            }
            HExprKind::OptionSome(inner)
            | HExprKind::ResultOk(inner)
            | HExprKind::ResultErr(inner) => self.visit_expr(inner),
            HExprKind::OptionNone => {}
            HExprKind::Format { parts } => {
                for part in parts {
                    if let HFormatPartKind::Arg { expr, .. } = &part.kind {
                        self.visit_expr(expr);
                    }
                }
            }
            HExprKind::Macro(call) => {
                for part in &call.parts {
                    if let HFormatPartKind::Arg { expr, .. } = &part.kind {
                        self.visit_expr(expr);
                    }
                }
                for arg in &call.args {
                    self.visit_expr(arg);
                }
            }
            HExprKind::Break | HExprKind::Continue => {}
            HExprKind::Return(value) => {
                if let Some(value) = value {
                    self.visit_expr(value);
                }
            }
        }
    }
}

// Type registration lives in its own impl block so the struct definition
// above stays readable.
impl TypeRegistry {
    fn insert_ty(&mut self, ty: &Ty) -> bool {
        if self.defs.contains_key(ty) {
            return false;
        }
        let def = match ty {
            Ty::Unit | Ty::Bool | Ty::Int(_) | Ty::Float(_) => TypeDef::Scalar,
            Ty::String => TypeDef::Frozen,
            Ty::Struct(index) => TypeDef::Struct(*index),
            Ty::Enum(index) => TypeDef::Enum(*index),
            Ty::Vec(elem) => TypeDef::Vec((**elem).clone()),
            Ty::Option(elem) => TypeDef::Option((**elem).clone()),
            Ty::Result(ok, err) => TypeDef::Result((**ok).clone(), (**err).clone()),
            Ty::Tuple(elems) => TypeDef::Tuple(elems.clone()),
            Ty::Fn(params, ret) => TypeDef::Fn(params.clone(), (**ret).clone()),
        };
        self.order.push(ty.clone());
        self.defs.insert(ty.clone(), def);
        true
    }
}

/// C name of a struct field (also used for enum payload structs).
pub fn field_cname(name: &str) -> String {
    format!("f_{}", crate::codegen::util::sanitize_ident(name))
}

/// C name of the payload aggregate of an enum variant inside the enum union.
pub fn variant_member(name: &str) -> String {
    format!("v_{}", crate::codegen::util::sanitize_ident(name))
}

/// C name of the n-th positional payload field of a variant or tuple.
pub fn positional_field(index: usize) -> String {
    format!("f{}", index)
}

/// Filler field emitted for otherwise empty structs (C has no empty structs).
pub const EMPTY_STRUCT_FIELD: &str = "_empty";

/// Layout names shared with the runtime header.
pub const TAG_FIELD: &str = "tag";
pub const UNION_FIELD: &str = "u";
pub const OPTION_SOME_MEMBER: &str = "some";
pub const RESULT_OK_MEMBER: &str = "ok";
pub const RESULT_ERR_MEMBER: &str = "err";

/// Function value fields.
pub const FN_ENV_FIELD: &str = "env";
pub const FN_PTR_FIELD: &str = "fn_ptr";

impl TypeNames {
    /// A valid zero value of the given type, as a C expression. Used for
    /// pre-declared locals and block result temporaries; every managed value
    /// produced here owns its resources and must be released normally.
    pub fn zero_expr(&self, program: &hir::Program, ty: &Ty) -> String {
        match ty {
            Ty::Unit => String::from("((LsUnit){0})"),
            Ty::Bool => String::from("false"),
            Ty::Int(int_ty) => crate::codegen::util::int_literal(0, *int_ty),
            Ty::Float(float_ty) => crate::codegen::util::float_literal(0.0, *float_ty),
            Ty::String => String::from("ls_str_new()"),
            Ty::Vec(_) => format!("(({}){{0, 0, NULL}})", self.ctype(ty)),
            Ty::Option(elem) => {
                if **elem == Ty::String {
                    String::from("ls_opt_string_none()")
                } else {
                    format!("(({}){{0}})", self.ctype(ty))
                }
            }
            Ty::Result(ok, err) => {
                if **ok == Ty::String && **err == Ty::String {
                    String::from("ls_res_string_ok(ls_str_new())")
                } else {
                    format!(
                        "(({}){{ .tag = 0, .u = {{ .ok = {} }} }})",
                        self.ctype(ty),
                        self.zero_expr(program, ok)
                    )
                }
            }
            Ty::Tuple(elems) => {
                if elems.is_empty() {
                    return format!("(({}){{}})", self.ctype(ty));
                }
                let fields: Vec<String> = elems
                    .iter()
                    .enumerate()
                    .map(|(index, elem)| {
                        format!(
                            ".{} = {}",
                            positional_field(index),
                            self.zero_expr(program, elem)
                        )
                    })
                    .collect();
                format!("(({}){{ {} }})", self.ctype(ty), fields.join(", "))
            }
            Ty::Fn(_, _) => format!(
                "(({}){{ .{} = NULL, .{} = NULL }})",
                self.ctype(ty),
                FN_ENV_FIELD,
                FN_PTR_FIELD
            ),
            Ty::Struct(index) => {
                let fields = match program.structs.get(*index) {
                    Some(item) => &item.fields,
                    None => return format!("(({}){{0}})", self.ctype(ty)),
                };
                if fields.is_empty() {
                    return format!("(({}){{ .{} = 0 }})", self.ctype(ty), EMPTY_STRUCT_FIELD);
                }
                let inits: Vec<String> = fields
                    .iter()
                    .map(|field| {
                        format!(
                            ".{} = {}",
                            field_cname(&field.name),
                            self.zero_expr(program, &field.ty)
                        )
                    })
                    .collect();
                format!("(({}){{ {} }})", self.ctype(ty), inits.join(", "))
            }
            Ty::Enum(index) => {
                let item = match program.enums.get(*index) {
                    Some(item) => item,
                    None => return format!("(({}){{0}})", self.ctype(ty)),
                };
                let variant = match item.variants.first() {
                    Some(variant) => variant,
                    None => return format!("(({}){{0}})", self.ctype(ty)),
                };
                let tag = variant.tag as i64;
                if variant.fields.is_empty() {
                    format!("(({}){{ .tag = ((int32_t){}) }})", self.ctype(ty), tag)
                } else {
                    let fields: Vec<String> = variant
                        .fields
                        .iter()
                        .enumerate()
                        .map(|(field_index, field_ty)| {
                            format!(
                                ".{} = {}",
                                positional_field(field_index),
                                self.zero_expr(program, field_ty)
                            )
                        })
                        .collect();
                    format!(
                        "(({}){{ .tag = ((int32_t){}), .u = {{ .{} = {{ {} }} }} }})",
                        self.ctype(ty),
                        tag,
                        variant_member(&variant.name),
                        fields.join(", ")
                    )
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TypeNames;
    use crate::types::{FloatTy, IntTy, Ty};

    #[test]
    fn mangles_types() {
        let mut names = TypeNames::default();
        names.structs.push(String::from("Point"));
        assert_eq!(names.mangle(&Ty::i64()), "i64");
        assert_eq!(names.mangle(&Ty::string()), "String");
        assert_eq!(names.mangle(&Ty::vec(Ty::i64())), "Vec_i64");
        assert_eq!(names.mangle(&Ty::option(Ty::string())), "Opt_String");
        assert_eq!(
            names.mangle(&Ty::result(Ty::i64(), Ty::string())),
            "Res_i64_String"
        );
        assert_eq!(names.mangle(&Ty::Struct(0)), "Point");
        assert_eq!(names.ctype(&Ty::Struct(0)), "LsPoint");
        assert_eq!(names.ctype(&Ty::string()), "LsString*");
        assert_eq!(names.ctype(&Ty::vec(Ty::string())), "LsVec_String");
        assert_eq!(names.ctype(&Ty::bool_()), "bool");
        assert_eq!(names.ctype(&Ty::Float(FloatTy::F32)), "float");
        assert_eq!(names.ctype(&Ty::Int(IntTy::U32)), "uint32_t");
    }
}
