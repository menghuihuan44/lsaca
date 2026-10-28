//! The semantic type model.
//!
//! This module is the single source of truth for what a type *is* in lsaca.
//! The parser has its own syntactic [crate::ast::TypeExpr]; the type checker
//! resolves those into [Ty] values, and both the HIR and the C code generator
//! consume [Ty].
//!
//! Struct and enum types are referenced by index into
//! [crate::hir::Program::structs] / [crate::hir::Program::enums]. This keeps
//! [Ty] cheap to copy and comparable, at the cost of resolving display names
//! through the surrounding program (see [crate::hir::Program::type_name]).

use std::fmt;

/// The integer types of the language. There is deliberately no pointer-sized
/// integer type: lengths and indices are i64.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum IntTy {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
}

impl IntTy {
    pub const ALL: [IntTy; 8] = [
        IntTy::I8,
        IntTy::I16,
        IntTy::I32,
        IntTy::I64,
        IntTy::U8,
        IntTy::U16,
        IntTy::U32,
        IntTy::U64,
    ];

    pub fn name(self) -> &'static str {
        match self {
            IntTy::I8 => "i8",
            IntTy::I16 => "i16",
            IntTy::I32 => "i32",
            IntTy::I64 => "i64",
            IntTy::U8 => "u8",
            IntTy::U16 => "u16",
            IntTy::U32 => "u32",
            IntTy::U64 => "u64",
        }
    }

    pub fn from_name(name: &str) -> Option<IntTy> {
        Some(match name {
            "i8" => IntTy::I8,
            "i16" => IntTy::I16,
            "i32" => IntTy::I32,
            "i64" => IntTy::I64,
            "u8" => IntTy::U8,
            "u16" => IntTy::U16,
            "u32" => IntTy::U32,
            "u64" => IntTy::U64,
            _ => return None,
        })
    }

    pub fn is_signed(self) -> bool {
        matches!(self, IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64)
    }

    pub fn bits(self) -> u32 {
        match self {
            IntTy::I8 | IntTy::U8 => 8,
            IntTy::I16 | IntTy::U16 => 16,
            IntTy::I32 | IntTy::U32 => 32,
            IntTy::I64 | IntTy::U64 => 64,
        }
    }

    /// The exact-width C integer type used by generated code.
    pub fn c_type(self) -> &'static str {
        match self {
            IntTy::I8 => "int8_t",
            IntTy::I16 => "int16_t",
            IntTy::I32 => "int32_t",
            IntTy::I64 => "int64_t",
            IntTy::U8 => "uint8_t",
            IntTy::U16 => "uint16_t",
            IntTy::U32 => "uint32_t",
            IntTy::U64 => "uint64_t",
        }
    }

    /// Inclusive range of values representable by this type, as i128 so that
    /// both signed and unsigned bounds fit.
    pub fn range(self) -> (i128, i128) {
        match self {
            IntTy::I8 => (i8::MIN as i128, i8::MAX as i128),
            IntTy::I16 => (i16::MIN as i128, i16::MAX as i128),
            IntTy::I32 => (i32::MIN as i128, i32::MAX as i128),
            IntTy::I64 => (i64::MIN as i128, i64::MAX as i128),
            IntTy::U8 => (u8::MIN as i128, u8::MAX as i128),
            IntTy::U16 => (u16::MIN as i128, u16::MAX as i128),
            IntTy::U32 => (u32::MIN as i128, u32::MAX as i128),
            IntTy::U64 => (u64::MIN as i128, u64::MAX as i128),
        }
    }

    pub fn fits(self, value: i128) -> bool {
        let (min, max) = self.range();
        value >= min && value <= max
    }

    /// The unsigned integer type with the same width, used to implement
    /// well-defined wrapping arithmetic in C.
    pub fn unsigned_counterpart(self) -> IntTy {
        match self {
            IntTy::I8 => IntTy::U8,
            IntTy::I16 => IntTy::U16,
            IntTy::I32 => IntTy::U32,
            IntTy::I64 => IntTy::U64,
            IntTy::U8 => IntTy::U8,
            IntTy::U16 => IntTy::U16,
            IntTy::U32 => IntTy::U32,
            IntTy::U64 => IntTy::U64,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FloatTy {
    F32,
    F64,
}

impl FloatTy {
    pub fn name(self) -> &'static str {
        match self {
            FloatTy::F32 => "f32",
            FloatTy::F64 => "f64",
        }
    }

    pub fn from_name(name: &str) -> Option<FloatTy> {
        Some(match name {
            "f32" => FloatTy::F32,
            "f64" => FloatTy::F64,
            _ => return None,
        })
    }

    pub fn c_type(self) -> &'static str {
        match self {
            FloatTy::F32 => "float",
            FloatTy::F64 => "double",
        }
    }
}

/// How a method receives its receiver. Methods are the only place where a
/// reference-like receiver exists; there are no user-visible reference types.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SelfKind {
    /// A by-value receiver, written self. Treated like a borrowed receiver for
    /// code generation: the callee never destroys the caller's value.
    Value,
    /// A shared receiver, written &self. Receives a borrowed pointer to the
    /// value and may not mutate it.
    Ref,
    /// A mutable receiver, written &mut self. Receives a pointer and may
    /// mutate the receiver in place.
    RefMut,
}

/// A resolved semantic type.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Ty {
    /// The unit type, written ().
    Unit,
    Bool,
    Int(IntTy),
    Float(FloatTy),
    /// Owned, reference counted UTF-8 string.
    String,
    /// Builtin growable vector, monomorphized in generated code.
    Vec(Box<Ty>),
    /// Builtin reference counted heap cell, monomorphized in generated code.
    /// `Rc<T>` is the only type whose indirection can close a reference
    /// cycle, so it is also the only place where the ORC cycle collector is
    /// observable; `Box<T>` breaks size recursion but cannot share.
    Rc(Box<Ty>),
    /// Builtin owned heap indirection, monomorphized in generated code.
    /// `Box<T>` owns its payload uniquely; copying a box deep-copies the
    /// payload, so a box never shares state and never takes part in a
    /// reference cycle by itself. Boxes are managed allocations so the cycle
    /// collector can trace through them.
    Box(Box<Ty>),
    /// Builtin optional value, monomorphized in generated code.
    Option(Box<Ty>),
    /// Builtin result value, monomorphized in generated code.
    Result(Box<Ty>, Box<Ty>),
    /// Tuple type; the empty tuple is Ty::Unit.
    Tuple(Vec<Ty>),
    /// Function type: parameter types and return type.
    Fn(Vec<Ty>, Box<Ty>),
    /// A non-owning reference: `&T` when `mutable` is false, `&mut T` when it
    /// is true. References are plain C pointers: they carry no reference count
    /// traffic and are trivially copyable. The borrow checker guarantees that
    /// the referent outlives every live reference.
    Ref {
        mutable: bool,
        inner: Box<Ty>,
    },
    /// A user struct, referenced by index into the HIR struct table.
    Struct(usize),
    /// A user enum, referenced by index into the HIR enum table.
    Enum(usize),
}

impl Ty {
    pub fn unit() -> Ty {
        Ty::Unit
    }

    pub fn bool_() -> Ty {
        Ty::Bool
    }

    pub fn i8() -> Ty {
        Ty::Int(IntTy::I8)
    }

    pub fn i16() -> Ty {
        Ty::Int(IntTy::I16)
    }

    pub fn i32() -> Ty {
        Ty::Int(IntTy::I32)
    }

    pub fn i64() -> Ty {
        Ty::Int(IntTy::I64)
    }

    pub fn u8() -> Ty {
        Ty::Int(IntTy::U8)
    }

    pub fn u16() -> Ty {
        Ty::Int(IntTy::U16)
    }

    pub fn u32() -> Ty {
        Ty::Int(IntTy::U32)
    }

    pub fn u64() -> Ty {
        Ty::Int(IntTy::U64)
    }

    pub fn f32() -> Ty {
        Ty::Float(FloatTy::F32)
    }

    pub fn f64() -> Ty {
        Ty::Float(FloatTy::F64)
    }

    pub fn string() -> Ty {
        Ty::String
    }

    pub fn vec(elem: Ty) -> Ty {
        Ty::Vec(Box::new(elem))
    }

    pub fn rc(elem: Ty) -> Ty {
        Ty::Rc(Box::new(elem))
    }

    pub fn box_(elem: Ty) -> Ty {
        Ty::Box(Box::new(elem))
    }

    pub fn option(elem: Ty) -> Ty {
        Ty::Option(Box::new(elem))
    }

    pub fn result(ok: Ty, err: Ty) -> Ty {
        Ty::Result(Box::new(ok), Box::new(err))
    }

    pub fn tuple(elems: Vec<Ty>) -> Ty {
        if elems.is_empty() {
            Ty::Unit
        } else {
            Ty::Tuple(elems)
        }
    }

    pub fn function(params: Vec<Ty>, ret: Ty) -> Ty {
        Ty::Fn(params, Box::new(ret))
    }

    /// A shared reference type `&T`.
    pub fn ref_to(inner: Ty) -> Ty {
        Ty::Ref {
            mutable: false,
            inner: Box::new(inner),
        }
    }

    /// A mutable reference type `&mut T`.
    pub fn ref_mut_to(inner: Ty) -> Ty {
        Ty::Ref {
            mutable: true,
            inner: Box::new(inner),
        }
    }

    pub fn is_unit(&self) -> bool {
        matches!(self, Ty::Unit)
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Ty::Bool)
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Ty::String)
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Ty::Int(_))
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Ty::Float(_))
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Ty::Int(_) | Ty::Float(_))
    }

    pub fn is_vec(&self) -> bool {
        matches!(self, Ty::Vec(_))
    }

    pub fn is_rc(&self) -> bool {
        matches!(self, Ty::Rc(_))
    }

    pub fn is_box(&self) -> bool {
        matches!(self, Ty::Box(_))
    }

    pub fn is_option(&self) -> bool {
        matches!(self, Ty::Option(_))
    }

    pub fn is_result(&self) -> bool {
        matches!(self, Ty::Result(_, _))
    }

    pub fn is_tuple(&self) -> bool {
        matches!(self, Ty::Tuple(_))
    }

    pub fn is_fn(&self) -> bool {
        matches!(self, Ty::Fn(_, _))
    }

    pub fn is_struct(&self) -> bool {
        matches!(self, Ty::Struct(_))
    }

    pub fn is_enum(&self) -> bool {
        matches!(self, Ty::Enum(_))
    }

    pub fn is_ref(&self) -> bool {
        matches!(self, Ty::Ref { .. })
    }

    pub fn is_ref_mut(&self) -> bool {
        matches!(self, Ty::Ref { mutable: true, .. })
    }

    /// The referent of a reference type.
    pub fn ref_inner(&self) -> Option<&Ty> {
        match self {
            Ty::Ref { inner, .. } => Some(inner),
            _ => None,
        }
    }

    /// True for types with no owned (reference counted) parts. Values of these
    /// types can be copied with a plain C assignment and need no cleanup.
    pub fn is_bitwise_copy(&self) -> bool {
        match self {
            Ty::Unit | Ty::Bool | Ty::Int(_) | Ty::Float(_) => true,
            Ty::Ref { .. } => true,
            Ty::String | Ty::Vec(_) | Ty::Rc(_) | Ty::Box(_) | Ty::Fn(_, _) => false,
            Ty::Option(inner) => inner.is_bitwise_copy(),
            Ty::Result(ok, err) => ok.is_bitwise_copy() && err.is_bitwise_copy(),
            Ty::Tuple(elems) => elems.iter().all(Ty::is_bitwise_copy),
            Ty::Struct(_) | Ty::Enum(_) => false,
        }
    }

    /// True for types whose C representation is a bare value (no pointer or
    /// generated aggregate helpers needed for ownership). This is the same as
    /// is_bitwise_copy for all practical purposes but is kept separate because
    /// user structs/enums are values while being owned.
    pub fn is_scalar(&self) -> bool {
        matches!(
            self,
            Ty::Unit | Ty::Bool | Ty::Int(_) | Ty::Float(_) | Ty::Fn(_, _) | Ty::Ref { .. }
        )
    }

    /// The element type of a Vec, Option or Result type.
    pub fn payload(&self) -> Option<&Ty> {
        match self {
            Ty::Vec(inner) | Ty::Option(inner) => Some(inner),
            Ty::Result(ok, _) => Some(ok),
            _ => None,
        }
    }
}

impl fmt::Display for Ty {
    /// Display is only useful for primitive types here; struct and enum types
    /// need the surrounding program to resolve their names, which the type
    /// checker and code generator handle with their own helpers.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Unit => write!(f, "()"),
            Ty::Bool => write!(f, "bool"),
            Ty::Int(int) => write!(f, "{}", int.name()),
            Ty::Float(float) => write!(f, "{}", float.name()),
            Ty::String => write!(f, "String"),
            Ty::Vec(elem) => write!(f, "Vec<{}>", elem),
            Ty::Rc(elem) => write!(f, "Rc<{}>", elem),
            Ty::Box(elem) => write!(f, "Box<{}>", elem),
            Ty::Option(elem) => write!(f, "Option<{}>", elem),
            Ty::Result(ok, err) => write!(f, "Result<{}, {}>", ok, err),
            Ty::Tuple(elems) => {
                write!(f, "(")?;
                for (index, elem) in elems.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                write!(f, ")")
            }
            Ty::Ref { mutable, inner } => {
                if *mutable {
                    write!(f, "&mut {}", inner)
                } else {
                    write!(f, "&{}", inner)
                }
            }
            Ty::Fn(params, ret) => {
                write!(f, "fn(")?;
                for (index, param) in params.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", param)?;
                }
                write!(f, ") -> {}", ret)
            }
            Ty::Struct(id) => write!(f, "struct#{}", id),
            Ty::Enum(id) => write!(f, "enum#{}", id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_ranges() {
        assert!(IntTy::I8.fits(-128));
        assert!(!IntTy::I8.fits(128));
        assert!(IntTy::U8.fits(255));
        assert!(!IntTy::U8.fits(-1));
        assert!(IntTy::U64.fits(u64::MAX as i128));
    }

    #[test]
    fn copy_classification() {
        assert!(Ty::i64().is_bitwise_copy());
        assert!(Ty::tuple(vec![Ty::bool_(), Ty::f64()]).is_bitwise_copy());
        assert!(!Ty::string().is_bitwise_copy());
        assert!(!Ty::vec(Ty::i64()).is_bitwise_copy());
        assert!(Ty::option(Ty::bool_()).is_bitwise_copy());
        assert!(!Ty::option(Ty::string()).is_bitwise_copy());
    }

    #[test]
    fn empty_tuple_is_unit() {
        assert_eq!(Ty::tuple(vec![]), Ty::Unit);
    }
}
