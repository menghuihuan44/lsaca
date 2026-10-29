//! Value-path resolution and name suggestions.

use crate::ast;
use crate::span::Span;
use crate::typeck::builtins::{builtin_assoc, builtin_fn};
use crate::typeck::check::Checker;
use crate::typeck::items::{TypeKey, closest_name};
use crate::typeck::itype::ITy;
use crate::typeck::tree::{TExpr, TExprKind};

impl<'a> Checker<'a> {
    pub fn check_path_value(
        &mut self,
        path: &ast::PathExpr,
        hint: Option<&ITy>,
        span: Span,
    ) -> TExpr {
        let segments = &path.segments;
        if segments.len() == 1 {
            let name = segments[0].name.clone();
            if !segments[0].generics.is_empty() {
                self.error_code(
                    "generic arguments are not allowed here".to_string(),
                    segments[0].span,
                    "E0107",
                );
            }
            if self.generic_fn_by_name.contains_key(&name) {
                self.error_code(
                    format!(
                        "generic function {} cannot be used as a value; call it with type arguments",
                        name
                    ),
                    span,
                    "E0423",
                );
                return self.never_expr(span);
            }
            return self.check_simple_value(&name, path.span, hint);
        }
        if segments.len() == 2 {
            let type_name = segments[0].name.clone();
            let item = segments[1].name.clone();
            if !segments[1].generics.is_empty() {
                self.error_code(
                    "generic arguments are not allowed here".to_string(),
                    segments[1].span,
                    "E0107",
                );
            }
            if type_name == "Option" {
                match item.as_str() {
                    "None" => return self.check_option_none_value(hint, span),
                    "Some" => {
                        self.error_code(
                            "Some cannot be used as a value; construct it with Some(...)"
                                .to_string(),
                            span,
                            "E0423",
                        );
                        return self.never_expr(span);
                    }
                    _ => {}
                }
            }
            if type_name == "Result" {
                match item.as_str() {
                    "Ok" | "Err" => {
                        self.error_code(
                            format!(
                                "{} cannot be used as a value; construct it with {}(...)",
                                item, item
                            ),
                            span,
                            "E0423",
                        );
                        return self.never_expr(span);
                    }
                    _ => {}
                }
            }
            if builtin_assoc(&type_name, &item).is_some() {
                self.error_code(
                    format!(
                        "associated function {}::{} cannot be used as a value; call it with {}::{}(...)",
                        type_name, item, type_name, item
                    ),
                    span,
                    "E0423",
                );
                return self.never_expr(span);
            }
            // A unit variant of a generic enum needs a concrete instance from
            // the turbofish arguments or from the expected type.
            if self.generic_enum_by_name.contains_key(&type_name) {
                let explicit = segments[0].generics.clone();
                let instance = if !explicit.is_empty() {
                    self.resolve_generic_instance(&type_name, &explicit, segments[0].span)
                } else {
                    self.hinted_instance_of(&type_name, hint)
                };
                if let Some(ITy::Enum(enum_id)) = instance {
                    self.ensure_enum_variants(enum_id);
                    let enum_name = self.enum_name(enum_id);
                    if let Some(variant) = self.find_variant(enum_id, &item) {
                        let fields = self.variant_fields(enum_id, variant);
                        if !fields.is_empty() {
                            self.error_code(
                                format!(
                                    "enum variant {}::{} takes arguments; construct it with {}::{}(...)",
                                    enum_name, item, enum_name, item
                                ),
                                span,
                                "E0532",
                            );
                            return self.never_expr(span);
                        }
                        let ty = ITy::Enum(enum_id);
                        self.apply_hint(&ty, hint);
                        return TExpr::new(
                            ty,
                            TExprKind::EnumLit {
                                enum_id,
                                variant,
                                args: Vec::new(),
                            },
                            span,
                        );
                    }
                    let mut candidates: Vec<String> = Vec::new();
                    for index in 0..self.variant_count(enum_id) {
                        candidates.push(self.variant_name(enum_id, index));
                    }
                    candidates.sort();
                    let refs: Vec<&str> = candidates.iter().map(|name| name.as_str()).collect();
                    let suggestion = closest_name(&item, &refs);
                    self.error_suggest(
                        format!("enum {} has no variant named {}", enum_name, item),
                        suggestion,
                        span,
                        "E0599",
                    );
                    return self.never_expr(span);
                }
                self.error_code("type annotations needed".to_string(), span, "E0282");
                return self.never_expr(span);
            }
            if let Some(enum_id) = self.lookup_enum_for_path(&type_name, segments[0].span) {
                self.ensure_enum_variants(enum_id);
                let enum_name = self.enum_name(enum_id);
                if let Some(variant) = self.find_variant(enum_id, &item) {
                    let fields = self.variant_fields(enum_id, variant);
                    if !fields.is_empty() {
                        self.error_code(
                            format!(
                                "enum variant {}::{} takes arguments; construct it with {}::{}(...)",
                                enum_name, item, enum_name, item
                            ),
                            span,
                            "E0532",
                        );
                        return self.never_expr(span);
                    }
                    let ty = ITy::Enum(enum_id);
                    self.apply_hint(&ty, hint);
                    return TExpr::new(
                        ty,
                        TExprKind::EnumLit {
                            enum_id,
                            variant,
                            args: Vec::new(),
                        },
                        span,
                    );
                }
                let mut candidates: Vec<String> = Vec::new();
                for index in 0..self.variant_count(enum_id) {
                    candidates.push(self.variant_name(enum_id, index));
                }
                candidates.sort();
                let refs: Vec<&str> = candidates.iter().map(|name| name.as_str()).collect();
                let suggestion = closest_name(&item, &refs);
                self.error_suggest(
                    format!("enum {} has no variant named {}", enum_name, item),
                    suggestion,
                    span,
                    "E0599",
                );
                return self.never_expr(span);
            }
            if let Some(owner) = self.lookup_type_key(&type_name, segments[0].span) {
                let owner_name = self.type_name(owner);
                if let Some(fn_id) = self.methods.get(&(owner, item.clone())).copied() {
                    if self.fns[fn_id].is_method() {
                        self.error_code(
                            format!(
                                "method {}::{} cannot be used as a value; call it with a receiver",
                                owner_name, item
                            ),
                            span,
                            "E0599",
                        );
                    } else {
                        self.error_code(
                            format!(
                                "associated function {}::{} cannot be used as a value; call it with {}::{}(...)",
                                owner_name, item, owner_name, item
                            ),
                            span,
                            "E0423",
                        );
                    }
                    return self.never_expr(span);
                }
                let mut candidates: Vec<String> = self
                    .methods
                    .keys()
                    .filter(|entry| entry.0 == owner)
                    .map(|entry| entry.1.clone())
                    .collect();
                candidates.sort();
                let refs: Vec<&str> = candidates.iter().map(|name| name.as_str()).collect();
                let suggestion = closest_name(&item, &refs);
                self.error_suggest(
                    format!(
                        "no function or associated item named {} found for type {}",
                        item, owner_name
                    ),
                    suggestion,
                    span,
                    "E0599",
                );
                return self.never_expr(span);
            }
            let suggestion = self.suggest_type_name(&type_name);
            self.error_suggest(
                format!("cannot find type {} in this scope", type_name),
                suggestion,
                segments[0].span,
                "E0412",
            );
            return self.never_expr(span);
        }
        self.error_code(
            "paths with more than two segments are not supported".to_string(),
            path.span,
            "E0001",
        );
        self.never_expr(span)
    }

    fn check_simple_value(&mut self, name: &str, span: Span, hint: Option<&ITy>) -> TExpr {
        if name == "self" {
            if !self.in_const
                && self.find_local("self").is_some()
                && let Some(expr) = self.resolve_local_name("self", span)
            {
                self.apply_hint(&expr.ty, hint);
                return expr;
            }
            self.error_code(
                "self is only available in methods with a self parameter".to_string(),
                span,
                "E0424",
            );
            return self.never_expr(span);
        }
        if name == "Self" {
            self.error_code("Self is a type, not a value".to_string(), span, "E0423");
            return self.never_expr(span);
        }
        if !self.in_const
            && self.find_local(name).is_some()
            && let Some(expr) = self.resolve_local_name(name, span)
        {
            self.apply_hint(&expr.ty, hint);
            return expr;
        }
        if let Some(id) = self.const_by_name.get(name).copied() {
            self.ensure_const(id);
            if let Some((ty, _)) = self.const_value(id) {
                let ity = ITy::from_ty(&ty);
                self.apply_hint(&ity, hint);
                return TExpr::new(ity, TExprKind::ConstRef { const_id: id }, span);
            }
            return self.never_expr(span);
        }
        match name {
            "Some" | "Ok" | "Err" => {
                self.error_code(
                    format!(
                        "{} cannot be used as a value; construct it with {}(...)",
                        name, name
                    ),
                    span,
                    "E0423",
                );
                return self.never_expr(span);
            }
            "None" => return self.check_option_none_value(hint, span),
            _ => {}
        }
        if let Some(entries) = self.unit_variants.get(name).cloned() {
            let mut chosen: Option<(usize, usize)> = None;
            if let Some(hint) = hint
                && let ITy::Enum(enum_id) = self.infer.resolve(hint)
            {
                chosen = entries
                    .iter()
                    .find(|(variant_enum, _)| *variant_enum == enum_id)
                    .copied();
            }
            let (enum_id, variant) = match chosen {
                Some(pair) => pair,
                None => {
                    if entries.len() == 1 {
                        entries[0]
                    } else {
                        self.error_code(
                            format!(
                                "variant {} is ambiguous; qualify it with the enum name",
                                name
                            ),
                            span,
                            "E0659",
                        );
                        return self.never_expr(span);
                    }
                }
            };
            let fields = self.variant_fields(enum_id, variant);
            if !fields.is_empty() {
                let enum_name = self.enum_name(enum_id);
                self.error_code(
                    format!(
                        "enum variant {}::{} takes arguments; construct it with {}::{}(...)",
                        enum_name, name, enum_name, name
                    ),
                    span,
                    "E0532",
                );
                return self.never_expr(span);
            }
            let ty = ITy::Enum(enum_id);
            self.apply_hint(&ty, hint);
            return TExpr::new(
                ty,
                TExprKind::EnumLit {
                    enum_id,
                    variant,
                    args: Vec::new(),
                },
                span,
            );
        }
        if let Some(fn_id) = self.fn_by_name.get(name).copied() {
            let sig = self.fns[fn_id].sig.clone();
            if let Some(sig) = sig {
                let ty = ITy::function(sig.params, sig.ret);
                self.apply_hint(&ty, hint);
                return TExpr::new(ty, TExprKind::FnRef { fn_id }, span);
            }
        }
        if builtin_fn(name).is_some() {
            self.error_code(
                format!(
                    "builtin function {} cannot be used as a value; call it with {}()",
                    name, name
                ),
                span,
                "E0425",
            );
            return self.never_expr(span);
        }
        let suggestion = self.suggest_value(name);
        self.error_suggest(
            format!("cannot find value {} in this scope", name),
            suggestion,
            span,
            "E0425",
        );
        self.never_expr(span)
    }

    pub fn lookup_enum_for_path(&mut self, name: &str, span: Span) -> Option<usize> {
        if name == "Self" {
            // Generic impl methods keep their concrete self type in the generic
            // environment only, so consult that first for enum instances.
            if let Some(ITy::Enum(enum_id)) = self.current_self_ty() {
                return Some(enum_id);
            }
            if let Some(ITy::Struct(_)) = self.current_self_ty() {
                self.error_code(
                    "expected an enum, found a struct".to_string(),
                    span,
                    "E0532",
                );
                return None;
            }
            return match self.current_self_key() {
                Some(TypeKey::Enum(enum_id)) => Some(enum_id),
                Some(TypeKey::Struct(_)) => {
                    self.error_code(
                        "expected an enum, found a struct".to_string(),
                        span,
                        "E0532",
                    );
                    None
                }
                None => {
                    self.error_code(
                        "Self is only available inside an impl block".to_string(),
                        span,
                        "E0411",
                    );
                    None
                }
            };
        }
        self.enum_by_name.get(name).copied()
    }

    pub fn lookup_type_key(&mut self, name: &str, span: Span) -> Option<TypeKey> {
        if name == "Self" {
            // Generic impl methods carry their concrete self type in the
            // generic environment rather than in the methods map.
            if let Some(self_ty) = self.current_self_ty() {
                match self_ty {
                    ITy::Struct(struct_id) => return Some(TypeKey::Struct(struct_id)),
                    ITy::Enum(enum_id) => return Some(TypeKey::Enum(enum_id)),
                    _ => {}
                }
            }
            return match self.current_self_key() {
                Some(key) => Some(key),
                None => {
                    self.error_code(
                        "Self is only available inside an impl block".to_string(),
                        span,
                        "E0411",
                    );
                    None
                }
            };
        }
        if let Some(id) = self.struct_by_name.get(name).copied() {
            return Some(TypeKey::Struct(id));
        }
        self.enum_by_name.get(name).copied().map(TypeKey::Enum)
    }

    pub fn find_variant(&self, enum_id: usize, item: &str) -> Option<usize> {
        self.ensure_enum_variants_view(enum_id);
        self.enums[enum_id]
            .variants
            .iter()
            .position(|variant| variant.name == item)
    }

    fn ensure_enum_variants_view(&self, _enum_id: usize) {}

    pub fn suggest_value(&self, name: &str) -> Option<String> {
        let mut candidates: Vec<String> = Vec::new();
        for frame in &self.frames {
            for scope in &frame.scopes {
                candidates.extend(scope.keys().cloned());
            }
        }
        candidates.extend(self.consts.iter().map(|item| item.decl.name.clone()));
        candidates.extend(self.unit_variants.keys().cloned());
        candidates.extend(
            self.fns
                .iter()
                .filter(|function| function.owner.is_none())
                .map(|function| function.name.clone()),
        );
        candidates.sort();
        candidates.dedup();
        let refs: Vec<&str> = candidates
            .iter()
            .map(|name| name.as_str())
            .filter(|name| !name.starts_with("__lsaca_"))
            .collect();
        closest_name(name, &refs)
    }

    pub fn suggest_callable(&self, name: &str) -> Option<String> {
        let mut candidates: Vec<String> = Vec::new();
        for frame in &self.frames {
            for scope in &frame.scopes {
                candidates.extend(scope.keys().cloned());
            }
        }
        candidates.extend(
            self.fns
                .iter()
                .filter(|function| function.owner.is_none())
                .map(|function| function.name.clone()),
        );
        candidates.extend(self.consts.iter().map(|item| item.decl.name.clone()));
        candidates.sort();
        candidates.dedup();
        let refs: Vec<&str> = candidates
            .iter()
            .map(|name| name.as_str())
            .filter(|name| !name.starts_with("__lsaca_"))
            .collect();
        closest_name(name, &refs)
    }
}
