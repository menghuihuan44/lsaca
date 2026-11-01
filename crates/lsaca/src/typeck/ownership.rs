//! Ownership classification helpers shared by the type checker passes.
//!
//! Copy classification, drop hook recording and the reference allow-list live
//! on the [Checker] here so every pass (patterns, expression checking, lowering)
//! observes the same answer. The authoritative classification is
//! [crate::hir::Program::classify_types]; these helpers are the checking-time
//! mirror that does not require a lowered program yet.

use std::collections::HashSet;

use super::check::Checker;
use super::items::TypeKey;
use super::itype::ITy;

impl<'a> Checker<'a> {
    /// Whether releasing a value of this type must run drop glue: either the
    /// type itself has an `impl Drop` hook or one of its owned fields does.
    ///
    /// References never have drop glue (releasing them only releases the
    /// reference). This mirrors [crate::hir::Program::has_drop_glue], but keeps
    /// a visited set so recursive types such as
    /// `struct Node { next: Option<Box<Node>> }` terminate.
    pub fn has_drop_glue(&self, ty: &ITy) -> bool {
        let mut visiting = HashSet::new();
        self.has_drop_glue_inner(ty, &mut visiting)
    }

    fn has_drop_glue_inner(&self, ty: &ITy, visiting: &mut HashSet<TypeKey>) -> bool {
        let ty = self.infer.resolve(ty);
        match &ty {
            ITy::Struct(id) => {
                let key = TypeKey::Struct(*id);
                if !visiting.insert(key) {
                    return false;
                }
                let glued = self.drop_structs.get(*id).and_then(|hook| *hook).is_some()
                    || self
                        .structs
                        .get(*id)
                        .map(|info| {
                            info.fields
                                .iter()
                                .any(|field| self.has_drop_glue_inner(&field.ty, visiting))
                        })
                        .unwrap_or(false);
                visiting.remove(&key);
                glued
            }
            ITy::Enum(id) => {
                let key = TypeKey::Enum(*id);
                if !visiting.insert(key) {
                    return false;
                }
                let glued = self.drop_enums.get(*id).and_then(|hook| *hook).is_some()
                    || self
                        .enums
                        .get(*id)
                        .map(|info| {
                            info.variants.iter().any(|variant| {
                                variant
                                    .fields
                                    .iter()
                                    .any(|field| self.has_drop_glue_inner(field, visiting))
                            })
                        })
                        .unwrap_or(false);
                visiting.remove(&key);
                glued
            }
            ITy::Vec(inner) | ITy::Box(inner) | ITy::Option(inner) => {
                self.has_drop_glue_inner(inner, visiting)
            }
            ITy::Result(ok, err) => {
                self.has_drop_glue_inner(ok, visiting) || self.has_drop_glue_inner(err, visiting)
            }
            ITy::HashMap(key, value) => {
                self.has_drop_glue_inner(key, visiting) || self.has_drop_glue_inner(value, visiting)
            }
            ITy::Tuple(elems) => elems
                .iter()
                .any(|elem| self.has_drop_glue_inner(elem, visiting)),
            ITy::Unit
            | ITy::Bool
            | ITy::Char
            | ITy::Int(_)
            | ITy::Float(_)
            | ITy::String
            | ITy::Rc(_)
            | ITy::Fn(..)
            | ITy::Ref { .. }
            | ITy::Never
            | ITy::Var(_) => false,
        }
    }

    /// Whether the type may be borrowed mutably.
    ///
    /// In addition to plain values, mutable references to heap types are
    /// allowed: `&mut String`, `&mut Vec<T>`, `&mut Rc<T>` and `&mut Box<T>`.
    pub fn is_mutable_reference_target(&mut self, ty: &ITy) -> bool {
        let ty = self.infer.resolve(ty);
        match &ty {
            ITy::String | ITy::Vec(_) | ITy::Rc(_) | ITy::Box(_) => true,
            _ => self.is_plain_value_type(&ty),
        }
    }

    /// Whether a value of this type is copied implicitly (Rust's `Copy`) rather
    /// than moved.
    ///
    /// This mirrors [crate::hir::Program::is_copy] while checking, and keeps a
    /// visited set so recursive types such as
    /// `struct Node { next: Option<Box<Node>> }` terminate. A user type is Copy
    /// when it has no `impl Drop` hook of its own and every field or variant
    /// payload is Copy.
    pub fn is_copy_ty(&mut self, ty: &ITy) -> bool {
        let mut visiting = HashSet::new();
        self.is_copy_ty_inner(ty, &mut visiting)
    }

    fn is_copy_ty_inner(&mut self, ty: &ITy, visiting: &mut HashSet<TypeKey>) -> bool {
        let ty = self.infer.resolve(ty);
        match &ty {
            ITy::Struct(id) => {
                if self.drop_structs.get(*id).and_then(|hook| *hook).is_some() {
                    return false;
                }
                let key = TypeKey::Struct(*id);
                if !visiting.insert(key) {
                    return true;
                }
                self.ensure_struct_fields(*id);
                let fields: Vec<ITy> = self
                    .structs
                    .get(*id)
                    .map(|info| info.fields.iter().map(|field| field.ty.clone()).collect())
                    .unwrap_or_default();
                let copied = fields
                    .iter()
                    .all(|field| self.is_copy_ty_inner(field, visiting));
                visiting.remove(&key);
                copied
            }
            ITy::Enum(id) => {
                if self.drop_enums.get(*id).and_then(|hook| *hook).is_some() {
                    return false;
                }
                let key = TypeKey::Enum(*id);
                if !visiting.insert(key) {
                    return true;
                }
                self.ensure_enum_variants(*id);
                let payloads: Vec<ITy> = self
                    .enums
                    .get(*id)
                    .map(|info| {
                        info.variants
                            .iter()
                            .flat_map(|variant| variant.fields.iter().cloned())
                            .collect()
                    })
                    .unwrap_or_default();
                let copied = payloads
                    .iter()
                    .all(|payload| self.is_copy_ty_inner(payload, visiting));
                visiting.remove(&key);
                copied
            }
            ITy::Option(inner) => self.is_copy_ty_inner(inner, visiting),
            ITy::Result(ok, err) => {
                self.is_copy_ty_inner(ok, visiting) && self.is_copy_ty_inner(err, visiting)
            }
            ITy::Tuple(elems) => elems
                .iter()
                .all(|elem| self.is_copy_ty_inner(elem, visiting)),
            ITy::String
            | ITy::Vec(_)
            | ITy::Rc(_)
            | ITy::Box(_)
            | ITy::HashMap(_, _)
            | ITy::Fn(..) => false,
            ITy::Unit
            | ITy::Bool
            | ITy::Char
            | ITy::Int(_)
            | ITy::Float(_)
            | ITy::Ref { .. }
            | ITy::Never
            | ITy::Var(_) => true,
        }
    }

    /// Whether the type itself (not merely one of its fields) has a user
    /// `impl Drop` hook. This is the Rust rule for "cannot destructure a type
    /// that implements Drop by value".
    pub fn type_implements_drop(&self, ty: &ITy) -> bool {
        match self.infer.resolve(ty) {
            ITy::Struct(id) => self.drop_structs.get(id).and_then(|hook| *hook).is_some(),
            ITy::Enum(id) => self.drop_enums.get(id).and_then(|hook| *hook).is_some(),
            _ => false,
        }
    }
}
