//! Typing rules for the builtin HashMap<K, V> type (version 1.5).
//!
//! The key type is restricted to the four key types the runtime knows how to
//! hash and compare (i64, u64, bool and String). Values may be any storable
//! value type. The methods follow the standard library reference in
//! docs/language/15-standard-library.md: insert, get, get_with, contains_key,
//! remove, len, is_empty, clear, keys and values.

use super::builtins::BuiltinMethodInfo;
use super::check::Checker;
use super::itype::ITy;
use crate::hir::HBuiltinMethod;
use crate::span::Span;

fn method(
    method: HBuiltinMethod,
    params: Vec<ITy>,
    ret: ITy,
    mutable_receiver: bool,
) -> BuiltinMethodInfo {
    BuiltinMethodInfo {
        method,
        params,
        ret,
        mutable_receiver,
    }
}

impl<'a> Checker<'a> {
    /// Whether ty is one of the accepted HashMap key types.
    pub fn hash_map_key_supported(&self, ty: &ITy) -> bool {
        matches!(
            self.infer.resolve(ty),
            ITy::Int(crate::types::IntTy::I64)
                | ITy::Int(crate::types::IntTy::U64)
                | ITy::Bool
                | ITy::String
        )
    }

    /// Validate the key and value types of a HashMap<K, V> before it is
    /// created. Unresolved inference variables are left for a later check.
    pub fn validate_hash_map_type(&mut self, key: &ITy, value: &ITy, span: Span) {
        let resolved_key = self.infer.resolve(key);
        let resolved_value = self.infer.resolve(value);
        if matches!(resolved_key, ITy::Var(_)) || matches!(resolved_value, ITy::Var(_)) {
            return;
        }
        if !self.hash_map_key_supported(&resolved_key) {
            // The same HashMap<K, V> type is validated both at its annotation
            // (where the type is written) and where the value is created. Keep
            // the first diagnostic only: it points at the annotation.
            let described = self.describe_resolved(&resolved_key);
            if self.reported_hash_map_key_errors.insert(described.clone()) {
                self.error_code_note(
                    format!(
                        "HashMap keys must be one of i64, u64, bool or String, found {}",
                        described
                    ),
                    span,
                    "E0277",
                    "only the listed key types are supported; use a struct field or a String key instead",
                );
            }
        }
        self.validate_storage_type(&resolved_value, span);
    }

    /// Resolve a builtin method on HashMap<K, V>.
    ///
    /// Reports the clone-related E0277 errors itself because they depend on
    /// the concrete value type: get and values clone every value they return,
    /// which is impossible for types with drop glue.
    pub fn hash_map_method(
        &mut self,
        key: &ITy,
        value: &ITy,
        name: &str,
        span: Span,
    ) -> Option<BuiltinMethodInfo> {
        use HBuiltinMethod::*;
        let key = key.clone();
        let value = value.clone();
        let info = match name {
            // insert moves both the key and the value into the map and returns
            // the previous value, which is moved out of the slot.
            "insert" => method(
                MapInsert,
                vec![key, value.clone()],
                ITy::option(value),
                true,
            ),
            "get" => {
                self.require_cloneable_hash_map_value(&value, "get", span);
                method(MapGet, vec![key], ITy::option(value), false)
            }
            "get_with" => {
                let result = self.infer.fresh_var();
                let closure = ITy::Fn(
                    vec![ITy::Ref {
                        mutable: false,
                        inner: Box::new(value.clone()),
                    }],
                    Box::new(result.clone()),
                );
                method(MapGetWith, vec![key, closure], ITy::option(result), false)
            }
            "contains_key" => method(MapContainsKey, vec![key], ITy::bool_(), false),
            "remove" => method(MapRemove, vec![key], ITy::option(value), true),
            "len" => method(MapLen, vec![], ITy::i64(), false),
            "is_empty" => method(MapIsEmpty, vec![], ITy::bool_(), false),
            "clear" => method(MapClear, vec![], ITy::Unit, true),
            // keys()/values() build a new Vec and therefore clone each entry.
            "keys" => method(MapKeys, vec![], ITy::vec(key), false),
            "values" => {
                self.require_cloneable_hash_map_value(&value, "values", span);
                method(MapValues, vec![], ITy::vec(value), false)
            }
            _ => return None,
        };
        Some(info)
    }

    fn require_cloneable_hash_map_value(&mut self, value: &ITy, method_name: &str, span: Span) {
        if !self.has_drop_glue(value) {
            return;
        }
        let described = self.describe_resolved(value);
        self.error_code_note(
            format!(
                "HashMap::{} cannot clone values of type {} because it implements Drop; use get_with(...) instead",
                method_name, described
            ),
            span,
            "E0277",
            "get_with borrows the stored value, so it works for types with drop glue",
        );
    }
}
