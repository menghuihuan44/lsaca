//! Recognition of standard-library forwarding wrappers.
//!
//! Most standard-library methods are thin wrappers around an intrinsic
//! function:
//!
//! ```text
//! pub fn len(&self) -> i64 { return __str_len(self); }
//! ```
//!
//! Expanding such a wrapper at the call site keeps the generated C free of
//! dead helper functions and lets the intrinsic emitter produce the same
//! specialised code the builtin method arms used to produce. A wrapper is only
//! expanded when the body forwards its own parameters to one intrinsic in
//! declaration order and when the function never escapes as a value (a
//! function pointer, an iterator next function or a vtable entry still needs
//! a real C function).

use std::collections::{HashMap, HashSet};

use crate::hir::{self, HExpr, HExprKind, HStmt};
use crate::span::Span;

use super::CodegenOptions;

/// Call-site expansion decisions for one program.
#[derive(Debug, Default)]
pub(crate) struct InlinePlan {
    /// Wrapper function id -> intrinsic function id it forwards to.
    wrappers: HashMap<usize, usize>,
    /// Functions whose address is taken somewhere, so they must exist as C
    /// functions even when they look like forwarding wrappers.
    escaped: HashSet<usize>,
}

impl InlinePlan {
    pub(crate) fn build(program: &hir::Program, options: &CodegenOptions) -> InlinePlan {
        let mut plan = InlinePlan::default();
        for (index, function) in program.fns.iter().enumerate() {
            if function.intrinsic || !is_standard_library(options, function.span) {
                continue;
            }
            if let Some(target) = forward_target(program, index) {
                plan.wrappers.insert(index, target);
            }
        }
        collect_escapes(program, &mut plan.escaped);
        plan
    }

    /// The intrinsic function a call to `fn_id` expands to, if any.
    pub(crate) fn inline_target(&self, fn_id: usize) -> Option<usize> {
        if self.escaped.contains(&fn_id) {
            return None;
        }
        self.wrappers.get(&fn_id).copied()
    }

    /// True when the function is expanded at every call site and gets no C
    /// function of its own.
    pub(crate) fn is_inlined(&self, fn_id: usize) -> bool {
        self.inline_target(fn_id).is_some()
    }
}

/// True when the span points into a standard-library source file.
fn is_standard_library(options: &CodegenOptions, span: Span) -> bool {
    options
        .sources
        .as_ref()
        .and_then(|sources| sources.location(span))
        .map(|(file, _, _)| file.name().starts_with("std/"))
        .unwrap_or(false)
}

/// The intrinsic function a wrapper body forwards to, if the body is a pure
/// forwarding call.
fn forward_target(program: &hir::Program, index: usize) -> Option<usize> {
    let wrapper = program.fns.get(index)?;
    let call = forwarding_call(&wrapper.body)?;
    let (target, arguments) = match &call.kind {
        HExprKind::DirectCall {
            fn_id,
            receiver: None,
            args,
        } => (*fn_id, args),
        _ => return None,
    };
    if target == index {
        return None;
    }
    let intrinsic = program.fns.get(target)?;
    if !intrinsic.intrinsic {
        return None;
    }
    if arguments.len() != wrapper.params.len() {
        return None;
    }
    for (argument, parameter) in arguments.iter().zip(&wrapper.params) {
        if argument.ty != parameter.ty {
            return None;
        }
        match &argument.kind {
            HExprKind::Local { c_name, .. } if c_name == &parameter.c_name => {}
            _ => return None,
        }
    }
    Some(target)
}

/// The call expression of a body that consists of exactly one call, either a
/// tail expression, a returned expression or a discarded statement.
fn forwarding_call(body: &hir::HBlock) -> Option<&HExpr> {
    match (&body.stmts[..], &body.tail) {
        ([], Some(tail)) => Some(strip_return(tail)),
        ([HStmt::Expr(expr)], None) => Some(strip_return(expr)),
        _ => None,
    }
}

fn strip_return(expr: &HExpr) -> &HExpr {
    match &expr.kind {
        HExprKind::Return(Some(value)) => value,
        _ => expr,
    }
}

/// Record every function whose address is taken, so it is never inlined.
fn collect_escapes(program: &hir::Program, escaped: &mut HashSet<usize>) {
    for function in &program.fns {
        scan_block(&function.body, escaped);
    }
    for closure in &program.closures {
        scan_block(&closure.body, escaped);
    }
    for vtable in &program.vtables {
        for method in &vtable.methods {
            escaped.insert(*method);
        }
    }
}

fn scan_block(block: &hir::HBlock, escaped: &mut HashSet<usize>) {
    for stmt in &block.stmts {
        match stmt {
            HStmt::Let { init, .. } => {
                if let Some(init) = init {
                    scan_expr(init, escaped);
                }
            }
            HStmt::Expr(expr) => scan_expr(expr, escaped),
        }
    }
    if let Some(tail) = &block.tail {
        scan_expr(tail, escaped);
    }
}

fn scan_arm(arm: &hir::HArm, escaped: &mut HashSet<usize>) {
    if let Some(guard) = &arm.guard {
        scan_expr(guard, escaped);
    }
    scan_expr(&arm.body, escaped);
}

fn scan_parts(parts: &[hir::HFormatPart], escaped: &mut HashSet<usize>) {
    for part in parts {
        if let hir::HFormatPartKind::Arg { expr, .. } = &part.kind {
            scan_expr(expr, escaped);
        }
    }
}

fn scan_expr(expr: &HExpr, escaped: &mut HashSet<usize>) {
    match &expr.kind {
        HExprKind::Lit(_)
        | HExprKind::Local { .. }
        | HExprKind::Capture { .. }
        | HExprKind::ConstRef { .. }
        | HExprKind::Break
        | HExprKind::Continue
        | HExprKind::OptionNone => {}
        HExprKind::FnRef { fn_id } => {
            escaped.insert(*fn_id);
        }
        HExprKind::Closure { captures, .. } => {
            for capture in captures {
                scan_expr(capture, escaped);
            }
        }
        HExprKind::Call { callee, args } => {
            scan_expr(callee, escaped);
            for arg in args {
                scan_expr(arg, escaped);
            }
        }
        HExprKind::DirectCall { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                scan_expr(receiver, escaped);
            }
            for arg in args {
                scan_expr(arg, escaped);
            }
        }
        HExprKind::BuiltinMethod { receiver, args, .. } => {
            scan_expr(receiver, escaped);
            for arg in args {
                scan_expr(arg, escaped);
            }
        }
        HExprKind::BuiltinAssoc { args, .. } | HExprKind::BuiltinCall { args, .. } => {
            for arg in args {
                scan_expr(arg, escaped);
            }
        }
        HExprKind::Field { base, .. } => scan_expr(base, escaped),
        HExprKind::Index { base, index } => {
            scan_expr(base, escaped);
            scan_expr(index, escaped);
        }
        HExprKind::Unary { expr, .. } => scan_expr(expr, escaped),
        HExprKind::Binary { lhs, rhs, .. } => {
            scan_expr(lhs, escaped);
            scan_expr(rhs, escaped);
        }
        HExprKind::Assign { target, value, .. } => {
            scan_expr(target, escaped);
            scan_expr(value, escaped);
        }
        HExprKind::Cast { expr } => scan_expr(expr, escaped),
        HExprKind::Block(block) => scan_block(block, escaped),
        HExprKind::If {
            cond,
            then_block,
            else_expr,
        } => {
            scan_expr(cond, escaped);
            scan_block(then_block, escaped);
            if let Some(else_expr) = else_expr {
                scan_expr(else_expr, escaped);
            }
        }
        HExprKind::Match { scrutinee, arms } => {
            scan_expr(scrutinee, escaped);
            for arm in arms {
                scan_arm(arm, escaped);
            }
        }
        HExprKind::Loop { body } => scan_block(body, escaped),
        HExprKind::While { cond, body } => {
            scan_expr(cond, escaped);
            scan_block(body, escaped);
        }
        HExprKind::ForRange {
            start, end, body, ..
        } => {
            scan_expr(start, escaped);
            scan_expr(end, escaped);
            scan_block(body, escaped);
        }
        HExprKind::ForVec { vec, body, .. } => {
            scan_expr(vec, escaped);
            scan_block(body, escaped);
        }
        HExprKind::ForIter {
            iter,
            next_fn,
            body,
            ..
        } => {
            escaped.insert(*next_fn);
            scan_expr(iter, escaped);
            scan_block(body, escaped);
        }
        HExprKind::StructLit { fields, .. } => {
            for (_, field) in fields {
                scan_expr(field, escaped);
            }
        }
        HExprKind::Tuple(elems) | HExprKind::VecLit { elems } => {
            for elem in elems {
                scan_expr(elem, escaped);
            }
        }
        HExprKind::VecRepeat { value, count } => {
            scan_expr(value, escaped);
            scan_expr(count, escaped);
        }
        HExprKind::EnumLit { args, .. } => {
            for arg in args {
                scan_expr(arg, escaped);
            }
        }
        HExprKind::OptionSome(value)
        | HExprKind::ResultOk(value)
        | HExprKind::ResultErr(value)
        | HExprKind::DynCoerce { value, .. }
        | HExprKind::Ref { expr: value, .. }
        | HExprKind::Deref { expr: value } => scan_expr(value, escaped),
        HExprKind::Format { parts } => scan_parts(parts, escaped),
        HExprKind::Macro(call) => {
            scan_parts(&call.parts, escaped);
            for arg in &call.args {
                scan_expr(arg, escaped);
            }
        }
        HExprKind::Return(value) => {
            if let Some(value) = value {
                scan_expr(value, escaped);
            }
        }
        HExprKind::DynCall { object, args, .. } => {
            scan_expr(object, escaped);
            for arg in args {
                scan_expr(arg, escaped);
            }
        }
        HExprKind::Slice {
            base, start, end, ..
        } => {
            scan_expr(base, escaped);
            if let Some(start) = start {
                scan_expr(start, escaped);
            }
            if let Some(end) = end {
                scan_expr(end, escaped);
            }
        }
    }
}
