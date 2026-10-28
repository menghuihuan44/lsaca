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
            ITy::Tuple(elems) => elems
                .iter()
                .any(|elem| self.has_drop_glue_inner(elem, visiting)),
            ITy::Unit
            | ITy::Bool
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

    /// Whether the type may be borrowed mutably in version 1.4.
    ///
    /// In addition to plain values (which imply the old `&mut` support), the
    /// mutable references to heap types that version 1.4 legalises are allowed:
    /// `&mut String`, `&mut Vec<T>`, `&mut Rc<T>` and `&mut Box<T>`.
    pub fn is_mutable_reference_target(&mut self, ty: &ITy) -> bool {
        let ty = self.infer.resolve(ty);
        match &ty {
            ITy::String | ITy::Vec(_) | ITy::Rc(_) | ITy::Box(_) => true,
            _ => self.is_plain_value_type(&ty),
        }
    }
}
