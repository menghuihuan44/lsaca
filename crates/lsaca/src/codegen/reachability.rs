//! Reachability of generated C functions.
//!
//! The HIR program contains every function of the user crate and of the
//! standard library prelude. Most prelude functions are never used by a given
//! program, and a user function that the entry point cannot reach does not need
//! a C definition either (this is a single binary compilation model). Pruning
//! the unreachable functions keeps the generated C small and deterministic.
//!
//! The analysis is conservative: whenever a call goes through a function value
//! that this walker cannot resolve, every function is kept.

use std::collections::HashSet;

use crate::hir::{self, HExpr, HExprKind, HStmt};
use crate::span::Span;

use super::CodegenOptions;
use super::types::TypeRegistry;

/// The functions of a program that must be emitted.
#[derive(Debug, Default)]
pub(crate) struct Reachability {
    reachable_fns: Vec<bool>,
    reachable_closures: Vec<bool>,
    keep_all: bool,
}

impl Reachability {
    /// Compute the reachable functions for a program.
    ///
    /// Roots are the program entry point, every vtable method (a vtable exists
    /// only because the program used a trait object), every extra root the
    /// caller requests and every drop hook of a user type. Drop hooks of
    /// standard-library types are only pulled in when a type that they belong
    /// to is actually emitted; [Reachability::needed_drop_hooks] reports those
    /// hooks so the generator can iterate to a fixpoint without dragging the
    /// whole prelude into a program that does not use it.
    pub(crate) fn compute(
        program: &hir::Program,
        options: &CodegenOptions,
        extra_roots: &[usize],
    ) -> Reachability {
        let mut marks = Marks::default();
        marks.fns.insert(program.main_fn);
        for vtable in &program.vtables {
            for method in &vtable.methods {
                marks.fns.insert(*method);
            }
        }
        for root in extra_roots {
            marks.fns.insert(*root);
        }
        for hook in program.drop_structs.iter().flatten() {
            if is_user_drop_hook(program, options, *hook) {
                marks.fns.insert(*hook);
            }
        }
        for hook in program.drop_enums.iter().flatten() {
            if is_user_drop_hook(program, options, *hook) {
                marks.fns.insert(*hook);
            }
        }
        let mut pending_fns: Vec<usize> = marks.fns.iter().copied().collect();
        let mut pending_closures: Vec<usize> = marks.closures.iter().copied().collect();
        let mut visited_fns: HashSet<usize> = HashSet::new();
        let mut visited_closures: HashSet<usize> = HashSet::new();
        while !marks.keep_all {
            while let Some(fn_id) = pending_fns.pop() {
                if !visited_fns.insert(fn_id) {
                    continue;
                }
                if let Some(function) = program.fns.get(fn_id) {
                    scan_block(&function.body, &mut marks);
                }
            }
            while let Some(closure_id) = pending_closures.pop() {
                if !visited_closures.insert(closure_id) {
                    continue;
                }
                if let Some(closure) = program.closures.get(closure_id) {
                    scan_block(&closure.body, &mut marks);
                }
            }
            let mut progressed = false;
            for index in marks.fns.iter().copied() {
                if !visited_fns.contains(&index) {
                    pending_fns.push(index);
                    progressed = true;
                }
            }
            for index in marks.closures.iter().copied() {
                if !visited_closures.contains(&index) {
                    pending_closures.push(index);
                    progressed = true;
                }
            }
            if !progressed {
                break;
            }
        }
        if marks.keep_all {
            return Reachability {
                reachable_fns: vec![true; program.fns.len()],
                reachable_closures: vec![true; program.closures.len()],
                keep_all: true,
            };
        }
        let mut reachable_fns = vec![false; program.fns.len()];
        for index in marks.fns.iter().copied() {
            if let Some(slot) = reachable_fns.get_mut(index) {
                *slot = true;
            }
        }
        let mut reachable_closures = vec![false; program.closures.len()];
        for index in marks.closures.iter().copied() {
            if let Some(slot) = reachable_closures.get_mut(index) {
                *slot = true;
            }
        }
        Reachability {
            reachable_fns,
            reachable_closures,
            keep_all: false,
        }
    }

    /// True when the C function of this HIR function must be emitted.
    pub(crate) fn is_reachable(&self, fn_id: usize) -> bool {
        self.keep_all || self.reachable_fns.get(fn_id).copied().unwrap_or(true)
    }

    /// True when the C function of this closure must be emitted.
    pub(crate) fn is_closure_reachable(&self, closure_id: usize) -> bool {
        self.keep_all
            || self
                .reachable_closures
                .get(closure_id)
                .copied()
                .unwrap_or(true)
    }

    /// Drop hooks of emitted types that no emitted C helper would otherwise
    /// define yet.
    ///
    /// A release helper calls the drop hook of the type by name, so the hook
    /// must be emitted whenever its type is registered, even when the hook
    /// itself is only reachable through drop glue. The generator iterates:
    /// compute reachability, collect types, ask for the missing hooks, and
    /// recompute until this returns nothing.
    pub(crate) fn needed_drop_hooks(
        &self,
        program: &hir::Program,
        registry: &TypeRegistry,
    ) -> Vec<usize> {
        let mut hooks = Vec::new();
        for ty in registry.order() {
            let Some(hook) = program.drop_hook(ty) else {
                continue;
            };
            if !self.is_reachable(hook) && !hooks.contains(&hook) {
                hooks.push(hook);
            }
        }
        hooks
    }

    /// True when the analysis gave up and every function is kept.
    #[cfg(test)]
    pub(crate) fn keeps_everything(&self) -> bool {
        self.keep_all
    }
}

/// True when the drop hook is implemented by the user rather than by the
/// embedded standard library.
fn is_user_drop_hook(program: &hir::Program, options: &CodegenOptions, hook: usize) -> bool {
    match program.fns.get(hook) {
        Some(function) => !is_standard_library(options, function.span),
        None => true,
    }
}

/// True when a span resolves into the embedded standard library crate.
pub(crate) fn is_standard_library(options: &CodegenOptions, span: Span) -> bool {
    options
        .sources
        .as_ref()
        .and_then(|sources| sources.location(span))
        .map(|(file, _, _)| file.name().starts_with("std/"))
        .unwrap_or(false)
}

#[derive(Debug, Default)]
struct Marks {
    fns: HashSet<usize>,
    closures: HashSet<usize>,
    keep_all: bool,
}

fn scan_block(block: &hir::HBlock, marks: &mut Marks) {
    if marks.keep_all {
        return;
    }
    for stmt in &block.stmts {
        match stmt {
            HStmt::Let { init, .. } => {
                if let Some(init) = init {
                    scan_expr(init, marks);
                }
            }
            HStmt::Expr(expr) => scan_expr(expr, marks),
        }
    }
    if let Some(tail) = &block.tail {
        scan_expr(tail, marks);
    }
}

fn scan_arm(arm: &hir::HArm, marks: &mut Marks) {
    if let Some(guard) = &arm.guard {
        scan_expr(guard, marks);
    }
    scan_expr(&arm.body, marks);
}

fn scan_parts(parts: &[hir::HFormatPart], marks: &mut Marks) {
    for part in parts {
        if let hir::HFormatPartKind::Arg { expr, .. } = &part.kind {
            scan_expr(expr, marks);
        }
    }
}

fn scan_expr(expr: &HExpr, marks: &mut Marks) {
    if marks.keep_all {
        return;
    }
    match &expr.kind {
        HExprKind::Lit(_)
        | HExprKind::Local { .. }
        | HExprKind::Capture { .. }
        | HExprKind::ConstRef { .. }
        | HExprKind::Break
        | HExprKind::Continue
        | HExprKind::OptionNone => {}
        HExprKind::FnRef { fn_id } => {
            marks.fns.insert(*fn_id);
        }
        HExprKind::Closure {
            closure_id,
            captures,
        } => {
            marks.closures.insert(*closure_id);
            for capture in captures {
                scan_expr(capture, marks);
            }
        }
        HExprKind::Call { callee, args } => {
            // A call through anything but a literal function or closure value
            // has an unknown target: keep every function.
            match &callee.kind {
                HExprKind::FnRef { .. } | HExprKind::Closure { .. } => {}
                _ => marks.keep_all = true,
            }
            scan_expr(callee, marks);
            for arg in args {
                scan_expr(arg, marks);
            }
        }
        HExprKind::DirectCall {
            fn_id,
            receiver,
            args,
        } => {
            marks.fns.insert(*fn_id);
            if let Some(receiver) = receiver {
                scan_expr(receiver, marks);
            }
            for arg in args {
                scan_expr(arg, marks);
            }
        }
        HExprKind::BuiltinMethod { receiver, args, .. } => {
            scan_expr(receiver, marks);
            for arg in args {
                scan_expr(arg, marks);
            }
        }
        HExprKind::BuiltinAssoc { args, .. } | HExprKind::BuiltinCall { args, .. } => {
            for arg in args {
                scan_expr(arg, marks);
            }
        }
        HExprKind::Field { base, .. } => scan_expr(base, marks),
        HExprKind::Index { base, index } => {
            scan_expr(base, marks);
            scan_expr(index, marks);
        }
        HExprKind::Unary { expr, .. } => scan_expr(expr, marks),
        HExprKind::Binary { lhs, rhs, .. } => {
            scan_expr(lhs, marks);
            scan_expr(rhs, marks);
        }
        HExprKind::Assign { target, value, .. } => {
            scan_expr(target, marks);
            scan_expr(value, marks);
        }
        HExprKind::Cast { expr } => scan_expr(expr, marks),
        HExprKind::Block(block) => scan_block(block, marks),
        HExprKind::If {
            cond,
            then_block,
            else_expr,
        } => {
            scan_expr(cond, marks);
            scan_block(then_block, marks);
            if let Some(else_expr) = else_expr {
                scan_expr(else_expr, marks);
            }
        }
        HExprKind::Match { scrutinee, arms } => {
            scan_expr(scrutinee, marks);
            for arm in arms {
                scan_arm(arm, marks);
            }
        }
        HExprKind::Loop { body } => scan_block(body, marks),
        HExprKind::While { cond, body } => {
            scan_expr(cond, marks);
            scan_block(body, marks);
        }
        HExprKind::ForRange {
            start, end, body, ..
        } => {
            scan_expr(start, marks);
            scan_expr(end, marks);
            scan_block(body, marks);
        }
        HExprKind::ForVec { vec, body, .. } => {
            scan_expr(vec, marks);
            scan_block(body, marks);
        }
        HExprKind::ForIter {
            iter,
            next_fn,
            body,
            ..
        } => {
            marks.fns.insert(*next_fn);
            scan_expr(iter, marks);
            scan_block(body, marks);
        }
        HExprKind::StructLit { fields, .. } => {
            for (_, field) in fields {
                scan_expr(field, marks);
            }
        }
        HExprKind::Tuple(elems) | HExprKind::VecLit { elems } => {
            for elem in elems {
                scan_expr(elem, marks);
            }
        }
        HExprKind::VecRepeat { value, count } => {
            scan_expr(value, marks);
            scan_expr(count, marks);
        }
        HExprKind::EnumLit { args, .. } => {
            for arg in args {
                scan_expr(arg, marks);
            }
        }
        HExprKind::OptionSome(value)
        | HExprKind::ResultOk(value)
        | HExprKind::ResultErr(value)
        | HExprKind::DynCoerce { value, .. }
        | HExprKind::Ref { expr: value, .. }
        | HExprKind::Deref { expr: value } => scan_expr(value, marks),
        HExprKind::Format { parts } => scan_parts(parts, marks),
        HExprKind::Macro(call) => {
            scan_parts(&call.parts, marks);
            for arg in &call.args {
                scan_expr(arg, marks);
            }
        }
        HExprKind::Return(value) => {
            if let Some(value) = value {
                scan_expr(value, marks);
            }
        }
        HExprKind::DynCall { object, args, .. } => {
            scan_expr(object, marks);
            for arg in args {
                scan_expr(arg, marks);
            }
        }
        HExprKind::Slice {
            base, start, end, ..
        } => {
            scan_expr(base, marks);
            if let Some(start) = start {
                scan_expr(start, marks);
            }
            if let Some(end) = end {
                scan_expr(end, marks);
            }
        }
    }
}
