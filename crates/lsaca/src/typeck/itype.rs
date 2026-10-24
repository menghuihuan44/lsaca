//! Inference types and unification for the type checker.
//!
//! The type checker works on [ITy]: a type that may contain unification
//! variables for parts that are not known yet. Variables are either general
//! (any type) or integer/float literal variables, which record that the result
//! must be an integer/float type and remember the literal value so range checks
//! can run once the type is known.

use crate::span::Span;
use crate::types::{FloatTy, IntTy, Ty};

/// A type as seen during type checking.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ITy {
    Unit,
    Bool,
    Int(IntTy),
    Float(FloatTy),
    String,
    Vec(Box<ITy>),
    Option(Box<ITy>),
    Result(Box<ITy>, Box<ITy>),
    Tuple(Vec<ITy>),
    Fn(Vec<ITy>, Box<ITy>),
    Struct(usize),
    Enum(usize),
    /// The type of diverging expressions (return, break, continue). It unifies
    /// with every type and is lowered to unit.
    Never,
    Var(u32),
}

impl ITy {
    pub fn unit() -> ITy {
        ITy::Unit
    }

    pub fn bool_() -> ITy {
        ITy::Bool
    }

    pub fn string() -> ITy {
        ITy::String
    }

    pub fn i64() -> ITy {
        ITy::Int(IntTy::I64)
    }

    pub fn f64() -> ITy {
        ITy::Float(FloatTy::F64)
    }

    pub fn vec(elem: ITy) -> ITy {
        ITy::Vec(Box::new(elem))
    }

    pub fn option(elem: ITy) -> ITy {
        ITy::Option(Box::new(elem))
    }

    pub fn result(ok: ITy, err: ITy) -> ITy {
        ITy::Result(Box::new(ok), Box::new(err))
    }

    pub fn tuple(elems: Vec<ITy>) -> ITy {
        if elems.is_empty() {
            ITy::Unit
        } else {
            ITy::Tuple(elems)
        }
    }

    pub fn never() -> ITy {
        ITy::Never
    }

    pub fn is_never(&self) -> bool {
        matches!(self, ITy::Never)
    }

    pub fn function(params: Vec<ITy>, ret: ITy) -> ITy {
        ITy::Fn(params, Box::new(ret))
    }

    /// Convert a resolved [Ty] into an [ITy].
    pub fn from_ty(ty: &Ty) -> ITy {
        match ty {
            Ty::Unit => ITy::Unit,
            Ty::Bool => ITy::Bool,
            Ty::Int(int_ty) => ITy::Int(*int_ty),
            Ty::Float(float_ty) => ITy::Float(*float_ty),
            Ty::String => ITy::String,
            Ty::Vec(elem) => ITy::vec(ITy::from_ty(elem)),
            Ty::Option(elem) => ITy::option(ITy::from_ty(elem)),
            Ty::Result(ok, err) => ITy::result(ITy::from_ty(ok), ITy::from_ty(err)),
            Ty::Tuple(elems) => ITy::Tuple(elems.iter().map(ITy::from_ty).collect()),
            Ty::Fn(params, ret) => {
                ITy::function(params.iter().map(ITy::from_ty).collect(), ITy::from_ty(ret))
            }
            Ty::Struct(id) => ITy::Struct(*id),
            Ty::Enum(id) => ITy::Enum(*id),
        }
    }
}

/// What a unification variable may be unified with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarKind {
    General,
    IntLiteral,
    FloatLiteral,
}

#[derive(Clone, Debug)]
struct VarState {
    kind: VarKind,
    value: Option<ITy>,
}

/// A deferred integer literal range check.
#[derive(Clone, Debug)]
pub struct IntLitCheck {
    pub ty: ITy,
    pub value: i128,
    pub raw: String,
    pub span: Span,
}

/// A deferred float literal range check (f64 literal used as f32).
#[derive(Clone, Debug)]
pub struct FloatLitCheck {
    pub ty: ITy,
    pub value: f64,
    pub raw: String,
    pub span: Span,
}

#[derive(Clone, Debug, Default)]
pub struct Infer {
    vars: Vec<VarState>,
    int_checks: Vec<IntLitCheck>,
    float_checks: Vec<FloatLitCheck>,
}

impl Infer {
    pub fn new() -> Infer {
        Infer::default()
    }

    pub fn fresh_var(&mut self) -> ITy {
        let id = self.vars.len() as u32;
        self.vars.push(VarState {
            kind: VarKind::General,
            value: None,
        });
        ITy::Var(id)
    }

    pub fn int_literal(&mut self, value: i128, raw: String, span: Span) -> ITy {
        let ty = self.fresh_var();
        if let ITy::Var(id) = ty {
            self.vars[id as usize].kind = VarKind::IntLiteral;
        }
        self.int_checks.push(IntLitCheck {
            ty: ty.clone(),
            value,
            raw,
            span,
        });
        ty
    }

    pub fn float_literal(&mut self, value: f64, raw: String, span: Span) -> ITy {
        let ty = self.fresh_var();
        if let ITy::Var(id) = ty {
            self.vars[id as usize].kind = VarKind::FloatLiteral;
        }
        self.float_checks.push(FloatLitCheck {
            ty: ty.clone(),
            value,
            raw,
            span,
        });
        ty
    }

    pub fn var_kind(&self, id: u32) -> VarKind {
        self.vars
            .get(id as usize)
            .map(|var| var.kind)
            .unwrap_or(VarKind::General)
    }

    /// Resolve every unification variable inside a type.
    pub fn resolve(&self, ty: &ITy) -> ITy {
        match ty {
            ITy::Var(_) => self.resolve_var(ty),
            ITy::Vec(elem) => ITy::vec(self.resolve(elem)),
            ITy::Option(elem) => ITy::option(self.resolve(elem)),
            ITy::Result(ok, err) => ITy::result(self.resolve(ok), self.resolve(err)),
            ITy::Tuple(elems) => ITy::Tuple(elems.iter().map(|ty| self.resolve(ty)).collect()),
            ITy::Fn(params, ret) => ITy::function(
                params.iter().map(|ty| self.resolve(ty)).collect(),
                self.resolve(ret),
            ),
            other => other.clone(),
        }
    }

    fn resolve_var(&self, ty: &ITy) -> ITy {
        let mut id = match ty {
            ITy::Var(id) => *id,
            other => return other.clone(),
        };
        let mut guard = 0;
        loop {
            guard += 1;
            if guard > 100_000 {
                return ITy::Var(id);
            }
            match &self.vars[id as usize].value {
                Some(ITy::Var(next)) => id = *next,
                Some(value) => return self.resolve(value),
                None => return ITy::Var(id),
            }
        }
    }

    /// Unify two types, recording substitutions. Does not report diagnostics:
    /// the caller renders a message when this returns Err.
    pub fn unify(&mut self, lhs: &ITy, rhs: &ITy) -> Result<(), ()> {
        let lhs = self.resolve(lhs);
        let rhs = self.resolve(rhs);
        self.unify_resolved(lhs, rhs)
    }

    fn unify_resolved(&mut self, lhs: ITy, rhs: ITy) -> Result<(), ()> {
        match (lhs, rhs) {
            // A diverging expression is compatible with every type.
            (ITy::Never, _) | (_, ITy::Never) => Ok(()),
            (ITy::Var(a), ITy::Var(b)) => {
                if a == b {
                    return Ok(());
                }
                let kind_a = self.vars[a as usize].kind;
                let kind_b = self.vars[b as usize].kind;
                let merged = match (kind_a, kind_b) {
                    (VarKind::General, other) => other,
                    (this, VarKind::General) => this,
                    (VarKind::IntLiteral, VarKind::IntLiteral) => VarKind::IntLiteral,
                    (VarKind::FloatLiteral, VarKind::FloatLiteral) => VarKind::FloatLiteral,
                    _ => return Err(()),
                };
                self.vars[b as usize].kind = merged;
                self.vars[a as usize].value = Some(ITy::Var(b));
                Ok(())
            }
            (ITy::Var(id), other) => self.bind_var(id, other),
            (other, ITy::Var(id)) => self.bind_var(id, other),
            (ITy::Unit, ITy::Unit) | (ITy::Bool, ITy::Bool) | (ITy::String, ITy::String) => Ok(()),
            (ITy::Int(a), ITy::Int(b)) if a == b => Ok(()),
            (ITy::Float(a), ITy::Float(b)) if a == b => Ok(()),
            (ITy::Vec(a), ITy::Vec(b)) => self.unify_resolved(*a, *b),
            (ITy::Option(a), ITy::Option(b)) => self.unify_resolved(*a, *b),
            (ITy::Result(a_ok, a_err), ITy::Result(b_ok, b_err)) => {
                self.unify_resolved(*a_ok, *b_ok)?;
                self.unify_resolved(*a_err, *b_err)
            }
            (ITy::Tuple(a), ITy::Tuple(b)) => {
                if a.len() != b.len() {
                    return Err(());
                }
                for (x, y) in a.into_iter().zip(b) {
                    self.unify_resolved(x, y)?;
                }
                Ok(())
            }
            (ITy::Fn(a_params, a_ret), ITy::Fn(b_params, b_ret)) => {
                if a_params.len() != b_params.len() {
                    return Err(());
                }
                for (x, y) in a_params.into_iter().zip(b_params) {
                    self.unify_resolved(x, y)?;
                }
                self.unify_resolved(*a_ret, *b_ret)
            }
            (ITy::Struct(a), ITy::Struct(b)) if a == b => Ok(()),
            (ITy::Enum(a), ITy::Enum(b)) if a == b => Ok(()),
            _ => Err(()),
        }
    }

    fn bind_var(&mut self, id: u32, ty: ITy) -> Result<(), ()> {
        if !self.var_kind_allows(id, &ty) {
            return Err(());
        }
        if self.occurs(id, &ty) {
            return Err(());
        }
        self.vars[id as usize].value = Some(ty);
        Ok(())
    }

    fn var_kind_allows(&self, id: u32, ty: &ITy) -> bool {
        match self.vars[id as usize].kind {
            VarKind::General => true,
            VarKind::IntLiteral => self.is_int_like(ty),
            VarKind::FloatLiteral => self.is_float_like(ty),
        }
    }

    fn is_int_like(&self, ty: &ITy) -> bool {
        match ty {
            ITy::Int(_) => true,
            ITy::Var(other) => self.vars[*other as usize].kind != VarKind::FloatLiteral,
            ITy::Unit => false,
            _ => false,
        }
    }

    fn is_float_like(&self, ty: &ITy) -> bool {
        match ty {
            ITy::Float(_) => true,
            ITy::Var(other) => self.vars[*other as usize].kind != VarKind::IntLiteral,
            _ => false,
        }
    }

    fn occurs(&self, id: u32, ty: &ITy) -> bool {
        match ty {
            ITy::Var(other) => {
                if *other == id {
                    return true;
                }
                match &self.vars[*other as usize].value {
                    Some(value) => self.occurs(id, value),
                    None => false,
                }
            }
            ITy::Vec(elem) | ITy::Option(elem) => self.occurs(id, elem),
            ITy::Result(ok, err) => self.occurs(id, ok) || self.occurs(id, err),
            ITy::Tuple(elems) => elems.iter().any(|elem| self.occurs(id, elem)),
            ITy::Fn(params, ret) => {
                params.iter().any(|param| self.occurs(id, param)) || self.occurs(id, ret)
            }
            ITy::Never => false,
            _ => false,
        }
    }

    /// Give every still-unconstrained literal variable its default type
    /// (i64 / f64). Must be called once before finalization.
    pub fn default_literals(&mut self) {
        for state in &mut self.vars {
            if state.value.is_some() {
                continue;
            }
            match state.kind {
                VarKind::General => {}
                VarKind::IntLiteral => state.value = Some(ITy::i64()),
                VarKind::FloatLiteral => state.value = Some(ITy::f64()),
            }
        }
    }

    pub fn int_checks(&self) -> &[IntLitCheck] {
        &self.int_checks
    }

    pub fn float_checks(&self) -> &[FloatLitCheck] {
        &self.float_checks
    }

    /// Finalize one type: resolve variables and convert to [Ty]. Returns Err
    /// when a general inference variable is still unresolved.
    pub fn finalize(&self, ty: &ITy) -> Result<Ty, ()> {
        let ty = self.resolve(ty);
        self.finalize_resolved(&ty)
    }

    fn finalize_resolved(&self, ty: &ITy) -> Result<Ty, ()> {
        match ty {
            ITy::Unit => Ok(Ty::Unit),
            ITy::Bool => Ok(Ty::Bool),
            ITy::Int(int_ty) => Ok(Ty::Int(*int_ty)),
            ITy::Float(float_ty) => Ok(Ty::Float(*float_ty)),
            ITy::String => Ok(Ty::String),
            ITy::Vec(elem) => Ok(Ty::vec(self.finalize_resolved(elem)?)),
            ITy::Option(elem) => Ok(Ty::option(self.finalize_resolved(elem)?)),
            ITy::Result(ok, err) => Ok(Ty::result(
                self.finalize_resolved(ok)?,
                self.finalize_resolved(err)?,
            )),
            ITy::Tuple(elems) => {
                let mut out = Vec::with_capacity(elems.len());
                for elem in elems {
                    out.push(self.finalize_resolved(elem)?);
                }
                Ok(Ty::tuple(out))
            }
            ITy::Fn(params, ret) => {
                let mut out = Vec::with_capacity(params.len());
                for param in params {
                    out.push(self.finalize_resolved(param)?);
                }
                Ok(Ty::function(out, self.finalize_resolved(ret)?))
            }
            ITy::Struct(id) => Ok(Ty::Struct(*id)),
            ITy::Enum(id) => Ok(Ty::Enum(*id)),
            ITy::Never => Ok(Ty::Unit),
            ITy::Var(_) => Err(()),
        }
    }
}
