//! Option and Result combinator methods.
//!
//! Every combinator is desugared into a match over the receiver (plus a block
//! holding the eagerly evaluated arguments for map_or and ok_or), so codegen
//! needs no new HIR nodes. The receiver is evaluated first; eager arguments
//! follow, which matches the evaluation order of a method call.

use crate::ast;
use crate::span::Span;
use crate::typeck::check::Checker;
use crate::typeck::itype::ITy;
use crate::typeck::tree::{TArm, TBlock, TExpr, TExprKind, TLit, TPat, TPatKind, TStmt};

const OPTION_METHODS: [&str; 12] = [
    "map",
    "map_or",
    "map_or_else",
    "and_then",
    "or_else",
    "filter",
    "ok_or",
    "ok_or_else",
    "unwrap_or_else",
    "is_some_and",
    "is_none_or",
    "and",
];
const RESULT_METHODS: [&str; 12] = [
    "map",
    "map_or",
    "map_or_else",
    "map_err",
    "and_then",
    "or",
    "or_else",
    "is_ok_and",
    "is_err_and",
    "inspect",
    "inspect_err",
    "unwrap_or_else",
];

/// Whether the combinator desugaring handles this method on this receiver type.
pub fn is_combinator_method(receiver: &ITy, method: &str) -> bool {
    match receiver {
        ITy::Option(_) => OPTION_METHODS.contains(&method),
        ITy::Result(_, _) => RESULT_METHODS.contains(&method),
        _ => false,
    }
}

fn call_expr(callee: TExpr, args: Vec<TExpr>, ret: &ITy, span: Span) -> TExpr {
    TExpr::new(
        ret.clone(),
        TExprKind::Call {
            callee: Box::new(callee),
            args,
        },
        span,
    )
}

fn match_expr(scrutinee: TExpr, arms: Vec<TArm>, ty: &ITy, span: Span) -> TExpr {
    TExpr::new(
        ty.clone(),
        TExprKind::Match {
            scrutinee: Box::new(scrutinee),
            arms,
        },
        span,
    )
}

fn block_expr(stmts: Vec<TStmt>, tail: TExpr, span: Span) -> TExpr {
    let ty = tail.ty.clone();
    TExpr::new(
        ty,
        TExprKind::Block(TBlock {
            stmts,
            tail: Some(Box::new(tail)),
            span,
        }),
        span,
    )
}

fn arm(pat: TPat, body: TExpr) -> TArm {
    TArm {
        pat,
        guard: None,
        body,
    }
}

fn some_expr(value: TExpr, elem: &ITy, span: Span) -> TExpr {
    TExpr::new(
        ITy::option(elem.clone()),
        TExprKind::OptionSome(Box::new(value)),
        span,
    )
}

fn none_expr(elem: &ITy, span: Span) -> TExpr {
    TExpr::new(ITy::option(elem.clone()), TExprKind::OptionNone, span)
}

fn ok_expr(value: TExpr, ok: &ITy, err: &ITy, span: Span) -> TExpr {
    TExpr::new(
        ITy::result(ok.clone(), err.clone()),
        TExprKind::ResultOk(Box::new(value)),
        span,
    )
}

fn err_expr(value: TExpr, ok: &ITy, err: &ITy, span: Span) -> TExpr {
    TExpr::new(
        ITy::result(ok.clone(), err.clone()),
        TExprKind::ResultErr(Box::new(value)),
        span,
    )
}

fn bool_expr(value: bool, span: Span) -> TExpr {
    TExpr::new(ITy::Bool, TExprKind::Lit(TLit::Bool(value)), span)
}

impl<'a> Checker<'a> {
    /// Check a method call that names an Option or Result combinator.
    pub fn check_combinator_method(
        &mut self,
        receiver: TExpr,
        method: &str,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let receiver_ty = self.infer.resolve(&receiver.ty);
        match receiver_ty {
            ITy::Option(payload) => {
                self.check_option_combinator(receiver, *payload, method, args, span, hint)
            }
            ITy::Result(ok, err) => {
                self.check_result_combinator(receiver, *ok, *err, method, args, span, hint)
            }
            other => self.unknown_combinator(receiver, &other, method, args, span),
        }
    }

    fn unknown_combinator(
        &mut self,
        _receiver: TExpr,
        receiver_ty: &ITy,
        method: &str,
        args: &[ast::Expr],
        span: Span,
    ) -> TExpr {
        let described = self.describe_resolved(receiver_ty);
        self.error_code(
            format!(
                "no method named {} found for type {} in the current scope",
                method, described
            ),
            span,
            "E0599",
        );
        for arg in args {
            let _ = self.check_expr(arg, None);
        }
        self.never_expr(span)
    }

    fn combinator_arity(
        &mut self,
        method: &str,
        expected: usize,
        args: &[ast::Expr],
        span: Span,
    ) -> bool {
        if args.len() == expected {
            return true;
        }
        self.error_code(
            format!(
                "the method {} takes {} argument(s) but {} were supplied",
                method,
                expected,
                args.len()
            ),
            span,
            "E0061",
        );
        for arg in args {
            let _ = self.check_expr(arg, None);
        }
        false
    }

    /// Create a fresh binding pattern plus an expression that reads it.
    fn bind_value(&mut self, name: &str, ty: &ITy, span: Span) -> (TPat, TExpr) {
        let c_name = self.fresh_local_c_name();
        let pat = TPat::new(
            ty.clone(),
            TPatKind::Bind {
                name: name.to_string(),
                c_name: c_name.clone(),
                mutable: false,
            },
            span,
        );
        let read = TExpr::new(
            ty.clone(),
            TExprKind::Local {
                name: name.to_string(),
                c_name,
                mutable: false,
            },
            span,
        );
        (pat, read)
    }

    /// Check a closure argument expected to have the given parameter and return types.
    fn closure_arg(&mut self, arg: &ast::Expr, param_ty: &ITy, ret_ty: &ITy) -> (TExpr, ITy, ITy) {
        let expected = ITy::function(vec![param_ty.clone()], ret_ty.clone());
        let expr = self.check_expr(arg, Some(&expected));
        if !self.unify_or_report(&expected, &expr.ty, arg.span) {
            return (expr, param_ty.clone(), ret_ty.clone());
        }
        match self.infer.resolve(&expr.ty) {
            ITy::Fn(params, ret) if params.len() == 1 => (expr, params[0].clone(), *ret),
            _ => (expr, param_ty.clone(), ret_ty.clone()),
        }
    }

    /// Check a closure argument expected to take no parameters.
    fn closure_arg0(&mut self, arg: &ast::Expr, ret_ty: &ITy) -> (TExpr, ITy) {
        let expected = ITy::function(Vec::new(), ret_ty.clone());
        let expr = self.check_expr(arg, Some(&expected));
        if !self.unify_or_report(&expected, &expr.ty, arg.span) {
            return (expr, ret_ty.clone());
        }
        match self.infer.resolve(&expr.ty) {
            ITy::Fn(params, ret) if params.is_empty() => (expr, *ret),
            _ => (expr, ret_ty.clone()),
        }
    }

    fn hint_option_inner(&mut self, hint: Option<&ITy>) -> ITy {
        if let Some(resolved) = hint.map(|hint| self.infer.resolve(hint))
            && let ITy::Option(inner) = resolved
        {
            return *inner;
        }
        self.infer.fresh_var()
    }

    fn hint_result_parts(&mut self, hint: Option<&ITy>) -> (ITy, ITy) {
        if let Some(resolved) = hint.map(|hint| self.infer.resolve(hint))
            && let ITy::Result(ok, err) = resolved
        {
            return (*ok, *err);
        }
        (self.infer.fresh_var(), self.infer.fresh_var())
    }

    fn hint_expected(&mut self, hint: Option<&ITy>) -> ITy {
        match hint.map(|hint| self.infer.resolve(hint)) {
            Some(resolved) => resolved,
            None => self.infer.fresh_var(),
        }
    }

    fn check_option_combinator(
        &mut self,
        receiver: TExpr,
        payload: ITy,
        method: &str,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let receiver_ty = ITy::option(payload.clone());
        match method {
            "map" => {
                if !self.combinator_arity("map", 1, args, span) {
                    return self.never_expr(span);
                }
                let inner = self.hint_option_inner(hint);
                let (closure, _, ret) = self.closure_arg(&args[0], &payload, &inner);
                let result_ty = ITy::option(ret.clone());
                self.apply_hint(&result_ty, hint);
                let (pat, read) = self.bind_value("value", &payload, span);
                let call = call_expr(closure, vec![read], &ret, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::OptionSome(Box::new(pat)),
                            span,
                        ),
                        some_expr(call, &ret, span),
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::OptionNone, span),
                        none_expr(&ret, span),
                    ),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            "map_or" => {
                if !self.combinator_arity("map_or", 2, args, span) {
                    return self.never_expr(span);
                }
                let result_ty = self.hint_expected(hint);
                let default = self.check_expr(&args[0], Some(&result_ty));
                let _ = self.unify_or_report(&result_ty, &default.ty, args[0].span);
                let expected_ret = self.infer.resolve(&result_ty);
                let (closure, _, ret) = self.closure_arg(&args[1], &payload, &expected_ret);
                let _ = self.unify_quiet(&result_ty, &ret);
                let result_ty = self.infer.resolve(&result_ty);
                self.apply_hint(&result_ty, hint);
                let (receiver_pat, receiver_read) = self.bind_value("receiver", &receiver_ty, span);
                let (fallback_pat, fallback_read) = self.bind_value("fallback", &result_ty, span);
                let (pat, read) = self.bind_value("value", &payload, span);
                let call = call_expr(closure, vec![read], &ret, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::OptionSome(Box::new(pat)),
                            span,
                        ),
                        call,
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::OptionNone, span),
                        fallback_read,
                    ),
                ];
                let mat = match_expr(receiver_read, arms, &result_ty, span);
                let stmts = vec![
                    TStmt::Let {
                        pat: receiver_pat,
                        init: Some(receiver),
                        span,
                    },
                    TStmt::Let {
                        pat: fallback_pat,
                        init: Some(default),
                        span,
                    },
                ];
                block_expr(stmts, mat, span)
            }
            "and_then" => {
                if !self.combinator_arity("and_then", 1, args, span) {
                    return self.never_expr(span);
                }
                let inner = self.hint_option_inner(hint);
                let expected_ret = ITy::option(inner);
                let (closure, _, ret) = self.closure_arg(&args[0], &payload, &expected_ret);
                let (result_ty, call_ret) = match self.infer.resolve(&ret) {
                    ITy::Option(inner) => (ITy::option((*inner).clone()), *inner),
                    _ => {
                        let fresh = self.infer.fresh_var();
                        (ITy::option(fresh.clone()), fresh)
                    }
                };
                self.apply_hint(&result_ty, hint);
                let (pat, read) = self.bind_value("value", &payload, span);
                let call = call_expr(closure, vec![read], &result_ty, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::OptionSome(Box::new(pat)),
                            span,
                        ),
                        call,
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::OptionNone, span),
                        none_expr(&call_ret, span),
                    ),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            "filter" => {
                if !self.combinator_arity("filter", 1, args, span) {
                    return self.never_expr(span);
                }
                let result_ty = ITy::option(payload.clone());
                self.apply_hint(&result_ty, hint);
                // Rust's Option::filter takes `FnOnce(&T) -> bool`, so the
                // predicate borrows the payload. Consuming the payload for the
                // predicate call would move the binding twice (once into the
                // predicate, once into the returned Some), which the move
                // checker reports as E0382 for every non-Copy payload.
                let borrowed = ITy::Ref {
                    mutable: false,
                    inner: Box::new(payload.clone()),
                };
                let (predicate, _, _) = self.closure_arg(&args[0], &borrowed, &ITy::Bool);
                let (pat, read) = self.bind_value("value", &payload, span);
                // Bind the borrow in its own statement so its loan ends at the
                // predicate call, before the payload is moved into the returned
                // `Some`. Passing `&value` inline would keep the loan live until
                // the end of the enclosing statement and the borrow checker would
                // reject moving the payload afterwards (E0505/E0597).
                let reference_c_name = self.fresh_local_c_name();
                let reference_pat = TPat::new(
                    borrowed.clone(),
                    TPatKind::Bind {
                        name: "reference".to_string(),
                        c_name: reference_c_name.clone(),
                        mutable: false,
                    },
                    span,
                );
                let reference_read = TExpr::new(
                    borrowed.clone(),
                    TExprKind::Local {
                        name: "reference".to_string(),
                        c_name: reference_c_name,
                        mutable: false,
                    },
                    span,
                );
                let reference_init = TExpr::new(
                    borrowed,
                    TExprKind::Ref {
                        mutable: false,
                        expr: Box::new(read.clone()),
                    },
                    span,
                );
                let borrow_stmt = TStmt::Let {
                    pat: reference_pat,
                    init: Some(reference_init),
                    span,
                };
                let condition = call_expr(predicate, vec![reference_read], &ITy::Bool, span);
                let keep = some_expr(read, &payload, span);
                let drop = none_expr(&payload, span);
                let condition_expr = TExpr::new(
                    result_ty.clone(),
                    TExprKind::If {
                        cond: Box::new(condition),
                        then_block: TBlock {
                            stmts: Vec::new(),
                            tail: Some(Box::new(keep)),
                            span,
                        },
                        else_expr: Some(Box::new(drop)),
                    },
                    span,
                );
                let some_body = TExpr::new(
                    result_ty.clone(),
                    TExprKind::Block(TBlock {
                        stmts: vec![borrow_stmt],
                        tail: Some(Box::new(condition_expr)),
                        span,
                    }),
                    span,
                );
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::OptionSome(Box::new(pat)),
                            span,
                        ),
                        some_body,
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::OptionNone, span),
                        none_expr(&payload, span),
                    ),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            "ok_or" => {
                if !self.combinator_arity("ok_or", 1, args, span) {
                    return self.never_expr(span);
                }
                let err_hint = self.hint_result_parts(hint).1;
                let error = self.check_expr(&args[0], Some(&err_hint));
                let _ = self.unify_or_report(&err_hint, &error.ty, args[0].span);
                let err_ty = self.infer.resolve(&err_hint);
                let result_ty = ITy::result(payload.clone(), err_ty.clone());
                self.apply_hint(&result_ty, hint);
                let (receiver_pat, receiver_read) = self.bind_value("receiver", &receiver_ty, span);
                let (error_pat, error_read) = self.bind_value("error", &err_ty, span);
                let (pat, read) = self.bind_value("value", &payload, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::OptionSome(Box::new(pat)),
                            span,
                        ),
                        ok_expr(read, &payload, &err_ty, span),
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::OptionNone, span),
                        err_expr(error_read, &payload, &err_ty, span),
                    ),
                ];
                let mat = match_expr(receiver_read, arms, &result_ty, span);
                let stmts = vec![
                    TStmt::Let {
                        pat: receiver_pat,
                        init: Some(receiver),
                        span,
                    },
                    TStmt::Let {
                        pat: error_pat,
                        init: Some(error),
                        span,
                    },
                ];
                block_expr(stmts, mat, span)
            }
            "or_else" => {
                if !self.combinator_arity("or_else", 1, args, span) {
                    return self.never_expr(span);
                }
                let result_ty = ITy::option(payload.clone());
                self.apply_hint(&result_ty, hint);
                let (closure, ret) = self.closure_arg0(&args[0], &result_ty);
                let _ = self.unify_quiet(&result_ty, &ret);
                let (pat, read) = self.bind_value("value", &payload, span);
                let call = call_expr(closure, Vec::new(), &result_ty, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::OptionSome(Box::new(pat)),
                            span,
                        ),
                        some_expr(read, &payload, span),
                    ),
                    arm(TPat::new(receiver_ty, TPatKind::OptionNone, span), call),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            "map_or_else" => {
                if !self.combinator_arity("map_or_else", 2, args, span) {
                    return self.never_expr(span);
                }
                let result_ty = self.hint_expected(hint);
                let (default, default_ret) = self.closure_arg0(&args[0], &result_ty);
                let _ = self.unify_quiet(&result_ty, &default_ret);
                let expected_ret = self.infer.resolve(&result_ty);
                let (map, _, ret) = self.closure_arg(&args[1], &payload, &expected_ret);
                let _ = self.unify_quiet(&result_ty, &ret);
                let result_ty = self.infer.resolve(&result_ty);
                self.apply_hint(&result_ty, hint);
                let (pat, read) = self.bind_value("value", &payload, span);
                let map_call = call_expr(map, vec![read], &result_ty, span);
                let default_call = call_expr(default, Vec::new(), &result_ty, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::OptionSome(Box::new(pat)),
                            span,
                        ),
                        map_call,
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::OptionNone, span),
                        default_call,
                    ),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            "ok_or_else" => {
                if !self.combinator_arity("ok_or_else", 1, args, span) {
                    return self.never_expr(span);
                }
                let err_hint = self.hint_result_parts(hint).1;
                let (closure, ret) = self.closure_arg0(&args[0], &err_hint);
                let _ = self.unify_quiet(&err_hint, &ret);
                let err_ty = self.infer.resolve(&err_hint);
                let result_ty = ITy::result(payload.clone(), err_ty.clone());
                self.apply_hint(&result_ty, hint);
                let (pat, read) = self.bind_value("value", &payload, span);
                let call = call_expr(closure, Vec::new(), &err_ty, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::OptionSome(Box::new(pat)),
                            span,
                        ),
                        ok_expr(read, &payload, &err_ty, span),
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::OptionNone, span),
                        err_expr(call, &payload, &err_ty, span),
                    ),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            "unwrap_or_else" => {
                if !self.combinator_arity("unwrap_or_else", 1, args, span) {
                    return self.never_expr(span);
                }
                let result_ty = self.infer.resolve(&payload);
                let (closure, ret) = self.closure_arg0(&args[0], &result_ty);
                let _ = self.unify_quiet(&result_ty, &ret);
                let result_ty = self.infer.resolve(&result_ty);
                self.apply_hint(&result_ty, hint);
                let (pat, read) = self.bind_value("value", &payload, span);
                let call = call_expr(closure, Vec::new(), &result_ty, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::OptionSome(Box::new(pat)),
                            span,
                        ),
                        read,
                    ),
                    arm(TPat::new(receiver_ty, TPatKind::OptionNone, span), call),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            "is_some_and" => {
                if !self.combinator_arity("is_some_and", 1, args, span) {
                    return self.never_expr(span);
                }
                let (predicate, _, _) = self.closure_arg(&args[0], &payload, &ITy::Bool);
                let (pat, read) = self.bind_value("value", &payload, span);
                let call = call_expr(predicate, vec![read], &ITy::Bool, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::OptionSome(Box::new(pat)),
                            span,
                        ),
                        call,
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::OptionNone, span),
                        bool_expr(false, span),
                    ),
                ];
                match_expr(receiver, arms, &ITy::Bool, span)
            }
            "is_none_or" => {
                if !self.combinator_arity("is_none_or", 1, args, span) {
                    return self.never_expr(span);
                }
                let (predicate, _, _) = self.closure_arg(&args[0], &payload, &ITy::Bool);
                let (pat, read) = self.bind_value("value", &payload, span);
                let call = call_expr(predicate, vec![read], &ITy::Bool, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::OptionSome(Box::new(pat)),
                            span,
                        ),
                        call,
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::OptionNone, span),
                        bool_expr(true, span),
                    ),
                ];
                match_expr(receiver, arms, &ITy::Bool, span)
            }
            "and" => {
                if !self.combinator_arity("and", 1, args, span) {
                    return self.never_expr(span);
                }
                let other_inner = self.hint_option_inner(hint);
                let expected_other = ITy::option(other_inner.clone());
                let other = self.check_expr(&args[0], Some(&expected_other));
                let _ = self.unify_or_report(&expected_other, &other.ty, args[0].span);
                let other_ty = self.infer.resolve(&other.ty);
                let result_inner = match &other_ty {
                    ITy::Option(inner) => (**inner).clone(),
                    _ => self.infer.resolve(&other_inner),
                };
                let result_ty = ITy::option(result_inner.clone());
                self.apply_hint(&result_ty, hint);
                let (receiver_pat, receiver_read) = self.bind_value("receiver", &receiver_ty, span);
                let (other_pat, other_read) = self.bind_value("other", &result_ty, span);
                let wild = TPat::new(payload.clone(), TPatKind::Wild, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::OptionSome(Box::new(wild)),
                            span,
                        ),
                        other_read,
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::OptionNone, span),
                        none_expr(&result_inner, span),
                    ),
                ];
                let mat = match_expr(receiver_read, arms, &result_ty, span);
                let stmts = vec![
                    TStmt::Let {
                        pat: receiver_pat,
                        init: Some(receiver),
                        span,
                    },
                    TStmt::Let {
                        pat: other_pat,
                        init: Some(other),
                        span,
                    },
                ];
                block_expr(stmts, mat, span)
            }
            other => self.unknown_combinator(receiver, &receiver_ty, other, args, span),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check_result_combinator(
        &mut self,
        receiver: TExpr,
        ok: ITy,
        err: ITy,
        method: &str,
        args: &[ast::Expr],
        span: Span,
        hint: Option<&ITy>,
    ) -> TExpr {
        let receiver_ty = ITy::result(ok.clone(), err.clone());
        match method {
            "map" => {
                if !self.combinator_arity("map", 1, args, span) {
                    return self.never_expr(span);
                }
                let inner = self.hint_result_parts(hint).0;
                let (closure, _, ret) = self.closure_arg(&args[0], &ok, &inner);
                let result_ty = ITy::result(ret.clone(), err.clone());
                self.apply_hint(&result_ty, hint);
                let (ok_pat, ok_read) = self.bind_value("value", &ok, span);
                let (err_pat, err_read) = self.bind_value("error", &err, span);
                let call = call_expr(closure, vec![ok_read], &ret, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::ResultOk(Box::new(ok_pat)),
                            span,
                        ),
                        ok_expr(call, &ret, &err, span),
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::ResultErr(Box::new(err_pat)), span),
                        err_expr(err_read, &ret, &err, span),
                    ),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            "map_err" => {
                if !self.combinator_arity("map_err", 1, args, span) {
                    return self.never_expr(span);
                }
                let mapped = self.hint_result_parts(hint).1;
                let (closure, _, ret) = self.closure_arg(&args[0], &err, &mapped);
                let result_ty = ITy::result(ok.clone(), ret.clone());
                self.apply_hint(&result_ty, hint);
                let (ok_pat, ok_read) = self.bind_value("value", &ok, span);
                let (err_pat, err_read) = self.bind_value("error", &err, span);
                let call = call_expr(closure, vec![err_read], &ret, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::ResultOk(Box::new(ok_pat)),
                            span,
                        ),
                        ok_expr(ok_read, &ok, &ret, span),
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::ResultErr(Box::new(err_pat)), span),
                        err_expr(call, &ok, &ret, span),
                    ),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            "and_then" => {
                if !self.combinator_arity("and_then", 1, args, span) {
                    return self.never_expr(span);
                }
                let inner = self.infer.fresh_var();
                let expected_ret = ITy::result(inner, err.clone());
                let (closure, _, ret) = self.closure_arg(&args[0], &ok, &expected_ret);
                let result_ty = match self.infer.resolve(&ret) {
                    ITy::Result(ok, err) => ITy::result(*ok, *err),
                    ITy::Never => expected_ret.clone(),
                    _ => expected_ret.clone(),
                };
                self.apply_hint(&result_ty, hint);
                let (ok_pat, ok_read) = self.bind_value("value", &ok, span);
                let (err_pat, err_read) = self.bind_value("error", &err, span);
                let call = call_expr(closure, vec![ok_read], &result_ty, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::ResultOk(Box::new(ok_pat)),
                            span,
                        ),
                        call,
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::ResultErr(Box::new(err_pat)), span),
                        err_expr(err_read, &ok, &err, span),
                    ),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            "map_or" => {
                if !self.combinator_arity("map_or", 2, args, span) {
                    return self.never_expr(span);
                }
                let result_ty = self.hint_expected(hint);
                let default = self.check_expr(&args[0], Some(&result_ty));
                let _ = self.unify_or_report(&result_ty, &default.ty, args[0].span);
                let expected_ret = self.infer.resolve(&result_ty);
                let (map, _, ret) = self.closure_arg(&args[1], &ok, &expected_ret);
                let _ = self.unify_quiet(&result_ty, &ret);
                let result_ty = self.infer.resolve(&result_ty);
                self.apply_hint(&result_ty, hint);
                let (receiver_pat, receiver_read) = self.bind_value("receiver", &receiver_ty, span);
                let (fallback_pat, fallback_read) = self.bind_value("fallback", &result_ty, span);
                let (ok_pat, ok_read) = self.bind_value("value", &ok, span);
                let call = call_expr(map, vec![ok_read], &result_ty, span);
                let wild = TPat::new(err.clone(), TPatKind::Wild, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::ResultOk(Box::new(ok_pat)),
                            span,
                        ),
                        call,
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::ResultErr(Box::new(wild)), span),
                        fallback_read,
                    ),
                ];
                let mat = match_expr(receiver_read, arms, &result_ty, span);
                let stmts = vec![
                    TStmt::Let {
                        pat: receiver_pat,
                        init: Some(receiver),
                        span,
                    },
                    TStmt::Let {
                        pat: fallback_pat,
                        init: Some(default),
                        span,
                    },
                ];
                block_expr(stmts, mat, span)
            }
            "map_or_else" => {
                if !self.combinator_arity("map_or_else", 2, args, span) {
                    return self.never_expr(span);
                }
                let result_ty = self.hint_expected(hint);
                let (default, _, default_ret) = self.closure_arg(&args[0], &err, &result_ty);
                let _ = self.unify_quiet(&result_ty, &default_ret);
                let expected_ret = self.infer.resolve(&result_ty);
                let (map, _, ret) = self.closure_arg(&args[1], &ok, &expected_ret);
                let _ = self.unify_quiet(&result_ty, &ret);
                let result_ty = self.infer.resolve(&result_ty);
                self.apply_hint(&result_ty, hint);
                let (ok_pat, ok_read) = self.bind_value("value", &ok, span);
                let (err_pat, err_read) = self.bind_value("error", &err, span);
                let map_call = call_expr(map, vec![ok_read], &result_ty, span);
                let default_call = call_expr(default, vec![err_read], &result_ty, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::ResultOk(Box::new(ok_pat)),
                            span,
                        ),
                        map_call,
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::ResultErr(Box::new(err_pat)), span),
                        default_call,
                    ),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            "or" => {
                if !self.combinator_arity("or", 1, args, span) {
                    return self.never_expr(span);
                }
                let other_err = self.hint_result_parts(hint).1;
                let expected_other = ITy::result(ok.clone(), other_err.clone());
                let other = self.check_expr(&args[0], Some(&expected_other));
                let _ = self.unify_or_report(&expected_other, &other.ty, args[0].span);
                let other_ty = self.infer.resolve(&other.ty);
                let (result_ok, result_err) = match &other_ty {
                    ITy::Result(ok, err) => ((**ok).clone(), (**err).clone()),
                    _ => (ok.clone(), self.infer.resolve(&other_err)),
                };
                let result_ty = ITy::result(result_ok.clone(), result_err.clone());
                self.apply_hint(&result_ty, hint);
                let (receiver_pat, receiver_read) = self.bind_value("receiver", &receiver_ty, span);
                let (other_pat, other_read) = self.bind_value("other", &result_ty, span);
                let (ok_pat, ok_read) = self.bind_value("value", &result_ok, span);
                let wild = TPat::new(result_err.clone(), TPatKind::Wild, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::ResultOk(Box::new(ok_pat)),
                            span,
                        ),
                        ok_expr(ok_read, &result_ok, &result_err, span),
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::ResultErr(Box::new(wild)), span),
                        other_read,
                    ),
                ];
                let mat = match_expr(receiver_read, arms, &result_ty, span);
                let stmts = vec![
                    TStmt::Let {
                        pat: receiver_pat,
                        init: Some(receiver),
                        span,
                    },
                    TStmt::Let {
                        pat: other_pat,
                        init: Some(other),
                        span,
                    },
                ];
                block_expr(stmts, mat, span)
            }
            "or_else" => {
                if !self.combinator_arity("or_else", 1, args, span) {
                    return self.never_expr(span);
                }
                let new_err = self.hint_result_parts(hint).1;
                let expected_ret = ITy::result(ok.clone(), new_err.clone());
                let (closure, _, ret) = self.closure_arg(&args[0], &err, &expected_ret);
                let result_ty = match self.infer.resolve(&ret) {
                    ITy::Result(ok, err) => ITy::result(*ok, *err),
                    ITy::Never => expected_ret.clone(),
                    _ => expected_ret.clone(),
                };
                self.apply_hint(&result_ty, hint);
                let (result_ok, result_err) = match &result_ty {
                    ITy::Result(ok, err) => ((**ok).clone(), (**err).clone()),
                    _ => (ok.clone(), self.infer.resolve(&new_err)),
                };
                let (ok_pat, ok_read) = self.bind_value("value", &ok, span);
                let (err_pat, err_read) = self.bind_value("error", &err, span);
                let call = call_expr(closure, vec![err_read], &result_ty, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::ResultOk(Box::new(ok_pat)),
                            span,
                        ),
                        ok_expr(ok_read, &result_ok, &result_err, span),
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::ResultErr(Box::new(err_pat)), span),
                        call,
                    ),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            "is_ok_and" => {
                if !self.combinator_arity("is_ok_and", 1, args, span) {
                    return self.never_expr(span);
                }
                let (predicate, _, _) = self.closure_arg(&args[0], &ok, &ITy::Bool);
                let (ok_pat, ok_read) = self.bind_value("value", &ok, span);
                let (err_pat, _) = self.bind_value("error", &err, span);
                let call = call_expr(predicate, vec![ok_read], &ITy::Bool, span);
                let wild = TPat::new(err_pat.ty.clone(), TPatKind::Wild, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::ResultOk(Box::new(ok_pat)),
                            span,
                        ),
                        call,
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::ResultErr(Box::new(wild)), span),
                        bool_expr(false, span),
                    ),
                ];
                match_expr(receiver, arms, &ITy::Bool, span)
            }
            "is_err_and" => {
                if !self.combinator_arity("is_err_and", 1, args, span) {
                    return self.never_expr(span);
                }
                let (predicate, _, _) = self.closure_arg(&args[0], &err, &ITy::Bool);
                let (err_pat, err_read) = self.bind_value("error", &err, span);
                let (ok_pat, _) = self.bind_value("value", &ok, span);
                let call = call_expr(predicate, vec![err_read], &ITy::Bool, span);
                let wild = TPat::new(ok_pat.ty.clone(), TPatKind::Wild, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::ResultOk(Box::new(wild)),
                            span,
                        ),
                        bool_expr(false, span),
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::ResultErr(Box::new(err_pat)), span),
                        call,
                    ),
                ];
                match_expr(receiver, arms, &ITy::Bool, span)
            }
            "inspect" => {
                if !self.combinator_arity("inspect", 1, args, span) {
                    return self.never_expr(span);
                }
                let borrowed = ITy::Ref {
                    mutable: false,
                    inner: Box::new(ok.clone()),
                };
                let (closure, _, _) = self.closure_arg(&args[0], &borrowed, &ITy::unit());
                let result_ty = ITy::result(ok.clone(), err.clone());
                self.apply_hint(&result_ty, hint);
                let (ok_pat, ok_read) = self.bind_value("value", &ok, span);
                let (err_pat, err_read) = self.bind_value("error", &err, span);
                let reference_c_name = self.fresh_local_c_name();
                let reference_pat = TPat::new(
                    borrowed.clone(),
                    TPatKind::Bind {
                        name: "reference".to_string(),
                        c_name: reference_c_name.clone(),
                        mutable: false,
                    },
                    span,
                );
                let reference_read = TExpr::new(
                    borrowed.clone(),
                    TExprKind::Local {
                        name: "reference".to_string(),
                        c_name: reference_c_name,
                        mutable: false,
                    },
                    span,
                );
                let reference_init = TExpr::new(
                    borrowed,
                    TExprKind::Ref {
                        mutable: false,
                        expr: Box::new(ok_read.clone()),
                    },
                    span,
                );
                let borrow_stmt = TStmt::Let {
                    pat: reference_pat,
                    init: Some(reference_init),
                    span,
                };
                let call = call_expr(closure, vec![reference_read], &ITy::unit(), span);
                let keep = ok_expr(ok_read, &ok, &err, span);
                let some_body = block_expr(vec![borrow_stmt, TStmt::Expr(call)], keep, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::ResultOk(Box::new(ok_pat)),
                            span,
                        ),
                        some_body,
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::ResultErr(Box::new(err_pat)), span),
                        err_expr(err_read, &ok, &err, span),
                    ),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            "inspect_err" => {
                if !self.combinator_arity("inspect_err", 1, args, span) {
                    return self.never_expr(span);
                }
                let borrowed = ITy::Ref {
                    mutable: false,
                    inner: Box::new(err.clone()),
                };
                let (closure, _, _) = self.closure_arg(&args[0], &borrowed, &ITy::unit());
                let result_ty = ITy::result(ok.clone(), err.clone());
                self.apply_hint(&result_ty, hint);
                let (ok_pat, ok_read) = self.bind_value("value", &ok, span);
                let (err_pat, err_read) = self.bind_value("error", &err, span);
                let reference_c_name = self.fresh_local_c_name();
                let reference_pat = TPat::new(
                    borrowed.clone(),
                    TPatKind::Bind {
                        name: "reference".to_string(),
                        c_name: reference_c_name.clone(),
                        mutable: false,
                    },
                    span,
                );
                let reference_read = TExpr::new(
                    borrowed.clone(),
                    TExprKind::Local {
                        name: "reference".to_string(),
                        c_name: reference_c_name,
                        mutable: false,
                    },
                    span,
                );
                let reference_init = TExpr::new(
                    borrowed,
                    TExprKind::Ref {
                        mutable: false,
                        expr: Box::new(err_read.clone()),
                    },
                    span,
                );
                let borrow_stmt = TStmt::Let {
                    pat: reference_pat,
                    init: Some(reference_init),
                    span,
                };
                let call = call_expr(closure, vec![reference_read], &ITy::unit(), span);
                let keep = err_expr(err_read, &ok, &err, span);
                let err_body = block_expr(vec![borrow_stmt, TStmt::Expr(call)], keep, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::ResultOk(Box::new(ok_pat)),
                            span,
                        ),
                        ok_expr(ok_read, &ok, &err, span),
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::ResultErr(Box::new(err_pat)), span),
                        err_body,
                    ),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            "unwrap_or_else" => {
                if !self.combinator_arity("unwrap_or_else", 1, args, span) {
                    return self.never_expr(span);
                }
                let result_ty = self.infer.resolve(&ok);
                let (closure, _, ret) = self.closure_arg(&args[0], &err, &result_ty);
                let _ = self.unify_quiet(&result_ty, &ret);
                let result_ty = self.infer.resolve(&result_ty);
                self.apply_hint(&result_ty, hint);
                let (ok_pat, ok_read) = self.bind_value("value", &ok, span);
                let (err_pat, err_read) = self.bind_value("error", &err, span);
                let call = call_expr(closure, vec![err_read], &result_ty, span);
                let arms = vec![
                    arm(
                        TPat::new(
                            receiver_ty.clone(),
                            TPatKind::ResultOk(Box::new(ok_pat)),
                            span,
                        ),
                        ok_read,
                    ),
                    arm(
                        TPat::new(receiver_ty, TPatKind::ResultErr(Box::new(err_pat)), span),
                        call,
                    ),
                ];
                match_expr(receiver, arms, &result_ty, span)
            }
            other => self.unknown_combinator(receiver, &receiver_ty, other, args, span),
        }
    }
}
