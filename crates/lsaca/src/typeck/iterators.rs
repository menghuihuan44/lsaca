//! Type checking for the iterator method sugar (version 1.5).
//!
//! The language ships a small prelude written in lsaca itself
//! (see `crates/lsaca/src/prelude.ls`) with the `Iterator<T>` trait and the
//! lazy adapters. The checker lowers the method sugar listed in DESIGN 5.3
//! directly to calls of the corresponding `__lsaca_it_*` prelude functions, so
//! the adapters are ordinary generic code that codegen can compile without
//! special cases.
//!
//! Sugar is tried after inherent and trait methods (a user type that implements
//! `Iterator<T>` and defines its own `count` keeps that method) and before the
//! builtin method table. Every sugar method consumes its receiver by value.

use super::check::Checker;
use super::itype::ITy;
use super::tree::{TExpr, TExprKind};
use crate::ast;
use crate::span::Span;

/// The method names resolved by the iterator sugar.
pub const SUGAR_METHODS: [&str; 13] = [
    "iter",
    "map",
    "filter",
    "take",
    "skip",
    "enumerate",
    "collect",
    "fold",
    "count",
    "sum",
    "any",
    "all",
    "find",
];

/// Whether the method name is one of the iterator sugar names.
pub fn is_iterator_sugar_name(name: &str) -> bool {
    SUGAR_METHODS.contains(&name)
}

impl<'a> Checker<'a> {
    fn prelude_fn(&self, name: &str) -> Option<usize> {
        self.generic_fn_by_name.get(name).copied()
    }

    /// The item type produced by a receiver that implements `Iterator<T>`.
    fn iterator_item_type(&mut self, receiver_ty: &ITy, span: Span) -> Option<ITy> {
        let (impl_id, impl_args) = self.find_trait_impl(receiver_ty, "Iterator")?;
        self.check_impl_coherence(impl_id, span);
        let info = self.impls.get(impl_id)?.clone();
        self.validate_generic_bounds(&info.generics, &impl_args, span);
        let trait_args = self.trait_impl_args(impl_id, &impl_args)?;
        trait_args.first().cloned()
    }

    /// Check an argument list for a sugar call. Reports the usual E0061 arity
    /// error and returns `None` when the arity does not match.
    fn sugar_args(
        &mut self,
        expected: &[ITy],
        args: &[ast::Expr],
        span: Span,
    ) -> Option<Vec<TExpr>> {
        if expected.len() != args.len() {
            self.error_code(
                format!(
                    "this function takes {} argument(s) but {} were supplied",
                    expected.len(),
                    args.len()
                ),
                span,
                "E0061",
            );
            for arg in args {
                let _ = self.check_expr(arg, None);
            }
            return None;
        }
        Some(self.check_call_args(expected, args, span))
    }

    /// Finish a sugar call: apply the expected-type hint and build the direct
    /// call to the instantiated prelude function.
    fn finish_sugar_call(
        &mut self,
        fn_id: usize,
        receiver: TExpr,
        args: Vec<TExpr>,
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let ret = self.fns[fn_id].ret();
        self.apply_hint(&ret, hint);
        let mut call_args = Vec::with_capacity(args.len() + 1);
        call_args.push(receiver);
        call_args.extend(args);
        TExpr::new(
            ret,
            TExprKind::DirectCall {
                fn_id,
                receiver: None,
                args: call_args,
            },
            span,
        )
    }

    /// Instantiate and call a prelude helper. Reports an internal E0283 (the
    /// prelude is compiler code, so unresolved arguments indicate a bug in the
    /// sugar lowering) and returns a never expression when instantiation fails.
    fn instantiate_sugar_fn(
        &mut self,
        def_id: usize,
        args: Vec<ITy>,
        receiver: TExpr,
        extra: Vec<TExpr>,
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        match self.instantiate_generic_fn(def_id, args, span) {
            Some(fn_id) => self.finish_sugar_call(fn_id, receiver, extra, span, hint),
            None => self.never_expr(span),
        }
    }

    /// Resolve a sugar method call. Returns `None` when the name is not sugar
    /// or the receiver cannot provide the required item type; the caller then
    /// continues with the builtin method table and eventually E0599.
    #[allow(clippy::too_many_arguments)]
    pub fn check_iterator_sugar(
        &mut self,
        receiver_expr: &TExpr,
        receiver_ty: &ITy,
        method: &str,
        method_span: Span,
        turbofish: &[ast::TypeExpr],
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> Option<TExpr> {
        if !is_iterator_sugar_name(method) {
            return None;
        }
        // Only .collect() accepts a type argument (the target Vec<T>).
        if !turbofish.is_empty() && method != "collect" {
            self.error_code(
                format!("generic arguments are not supported for .{}()", method),
                method_span,
                "E0107",
            );
            for arg in args {
                let _ = self.check_expr(arg, None);
            }
            return Some(self.never_expr(span));
        }
        let receiver_ty = self.infer.resolve(receiver_ty);
        let receiver_expr = self.wrap_derefs(receiver_expr.clone());

        // .iter() consumes a Vec<T> and produces the VecIter adapter.
        if method == "iter" {
            let elem = match &receiver_ty {
                ITy::Vec(elem) => (**elem).clone(),
                _ => return None,
            };
            let def_id = self.prelude_fn("__lsaca_vec_iter")?;
            if self.sugar_args(&[], args, span).is_none() {
                return Some(self.never_expr(span));
            }
            return Some(self.instantiate_sugar_fn(
                def_id,
                vec![elem],
                receiver_expr,
                Vec::new(),
                span,
                hint,
            ));
        }

        // Every other sugar method needs a receiver that implements
        // Iterator<T>; otherwise this is an ordinary unknown method.
        let item = self.iterator_item_type(&receiver_ty, method_span)?;

        match method {
            "map" => {
                let def_id = self.prelude_fn("__lsaca_it_map")?;
                let u = self.infer.fresh_var();
                let closure_ty = ITy::function(vec![item.clone()], u.clone());
                let extra = self.sugar_args(std::slice::from_ref(&closure_ty), args, span)?;
                Some(self.instantiate_sugar_fn(
                    def_id,
                    vec![item, u, receiver_ty, closure_ty],
                    receiver_expr,
                    extra,
                    span,
                    hint,
                ))
            }
            "filter" | "any" | "all" | "find" => {
                let name = match method {
                    "filter" => "__lsaca_it_filter",
                    "any" => "__lsaca_it_any",
                    "all" => "__lsaca_it_all",
                    _ => "__lsaca_it_find",
                };
                let def_id = self.prelude_fn(name)?;
                let pred_ty = ITy::function(
                    vec![ITy::Ref {
                        mutable: false,
                        inner: Box::new(item.clone()),
                    }],
                    ITy::bool_(),
                );
                let extra = self.sugar_args(std::slice::from_ref(&pred_ty), args, span)?;
                Some(self.instantiate_sugar_fn(
                    def_id,
                    vec![item, receiver_ty, pred_ty],
                    receiver_expr,
                    extra,
                    span,
                    hint,
                ))
            }
            "take" | "skip" => {
                let name = if method == "take" {
                    "__lsaca_it_take"
                } else {
                    "__lsaca_it_skip"
                };
                let def_id = self.prelude_fn(name)?;
                let extra = self.sugar_args(&[ITy::i64()], args, span)?;
                Some(self.instantiate_sugar_fn(
                    def_id,
                    vec![item, receiver_ty],
                    receiver_expr,
                    extra,
                    span,
                    hint,
                ))
            }
            "enumerate" => {
                let def_id = self.prelude_fn("__lsaca_it_enumerate")?;
                if self.sugar_args(&[], args, span).is_none() {
                    return Some(self.never_expr(span));
                }
                Some(self.instantiate_sugar_fn(
                    def_id,
                    vec![item, receiver_ty],
                    receiver_expr,
                    Vec::new(),
                    span,
                    hint,
                ))
            }
            "count" => {
                let def_id = self.prelude_fn("__lsaca_it_count")?;
                if self.sugar_args(&[], args, span).is_none() {
                    return Some(self.never_expr(span));
                }
                Some(self.instantiate_sugar_fn(
                    def_id,
                    vec![item, receiver_ty],
                    receiver_expr,
                    Vec::new(),
                    span,
                    hint,
                ))
            }
            "collect" => {
                let def_id = self.prelude_fn("__lsaca_it_collect_vec")?;
                if !turbofish.is_empty() && turbofish.len() != 1 {
                    self.error_code(
                        "generic arguments are not supported for .collect()".to_string(),
                        method_span,
                        "E0107",
                    );
                    for arg in args {
                        let _ = self.check_expr(arg, None);
                    }
                    return Some(self.never_expr(span));
                }
                if self.sugar_args(&[], args, span).is_none() {
                    return Some(self.never_expr(span));
                }
                let target = if let Some(expr) = turbofish.first() {
                    self.resolve_ty(expr, self.current_self_key())
                } else {
                    hint.map(|hint| self.infer.resolve(hint))
                };
                let elem = match target {
                    Some(ITy::Vec(elem)) => (*elem).clone(),
                    _ => {
                        self.error_code(
                            "type annotations needed: .collect() requires a known Vec<T> result type"
                                .to_string(),
                            method_span,
                            "E0282",
                        );
                        return Some(self.never_expr(span));
                    }
                };
                if self.infer.unify(&elem, &item).is_err() {
                    self.report_mismatch(&elem, &item, method_span);
                }
                Some(self.instantiate_sugar_fn(
                    def_id,
                    vec![item, receiver_ty],
                    receiver_expr,
                    Vec::new(),
                    span,
                    hint,
                ))
            }
            "fold" => {
                let def_id = self.prelude_fn("__lsaca_it_fold")?;
                if args.len() != 2 {
                    self.error_code(
                        format!(
                            "this function takes 2 argument(s) but {} were supplied",
                            args.len()
                        ),
                        span,
                        "E0061",
                    );
                    for arg in args {
                        let _ = self.check_expr(arg, None);
                    }
                    return Some(self.never_expr(span));
                }
                let init = self.check_expr(&args[0], None);
                let u = init.ty.clone();
                let closure_ty = ITy::function(vec![u.clone(), item.clone()], u.clone());
                let closure = self.check_expr(&args[1], Some(&closure_ty));
                let closure = self.coerce_to(closure, &closure_ty, args[1].span);
                let extra = vec![init, closure];
                Some(self.instantiate_sugar_fn(
                    def_id,
                    vec![item, u, receiver_ty, closure_ty],
                    receiver_expr,
                    extra,
                    span,
                    hint,
                ))
            }
            "sum" => {
                let resolved_item = self.infer.resolve(&item);
                let name = match &resolved_item {
                    ITy::Int(crate::types::IntTy::I64) => "__lsaca_it_sum_i64",
                    ITy::Float(crate::types::FloatTy::F64) => "__lsaca_it_sum_f64",
                    _ => {
                        let described = self.describe_resolved(&resolved_item);
                        self.error_code(
                            format!(
                                "cannot sum an iterator of type {}; .sum() is only supported for iterators of i64 or f64",
                                described
                            ),
                            method_span,
                            "E0277",
                        );
                        for arg in args {
                            let _ = self.check_expr(arg, None);
                        }
                        return Some(self.never_expr(span));
                    }
                };
                let def_id = self.prelude_fn(name)?;
                if self.sugar_args(&[], args, span).is_none() {
                    return Some(self.never_expr(span));
                }
                Some(self.instantiate_sugar_fn(
                    def_id,
                    vec![receiver_ty],
                    receiver_expr,
                    Vec::new(),
                    span,
                    hint,
                ))
            }
            _ => None,
        }
    }
}
