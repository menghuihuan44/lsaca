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
use crate::typeck::tree::{TArm, TBlock, TExpr, TExprKind, TPat, TPatKind, TStmt};

const OPTION_METHODS: [&str; 5] = ["map", "map_or", "and_then", "filter", "ok_or"];
const RESULT_METHODS: [&str; 3] = ["map", "map_err", "and_then"];

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
            other => self.unknown_combinator(receiver, &receiver_ty, other, args, span),
        }
    }
}
