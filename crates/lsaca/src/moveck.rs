//! The move checker: a dataflow pass that rejects use of moved values.
//!
//! The checker walks every function and closure body with an abstract state
//! that records which local bindings (parameters, let bindings, pattern
//! bindings and closure captures) have been moved. Copy types are never moved,
//! so only bindings of move-only types ever enter the moved set.
//!
//! Consuming contexts mark a binding as moved and produce E0382 when the
//! binding is not live (a second consume or a read of a moved value). A move
//! out of a place that only holds a borrow (a field, an indexed element, a
//! dereference, a closure capture, or a match guard) produces E0507 and
//! suggests calling clone() to make a copy.
//!
//! Branches merge with a union of their moved states, and loop bodies are
//! analyzed to a fixed point so a value moved on one iteration is moved at the
//! loop head on later iterations. Code after a terminating statement (return,
//! break, continue) is unreachable and produces no diagnostics.
//!
//! Two related diagnostics are owned by other passes: E0505 (cannot move while
//! borrowed) belongs to the borrow checker and E0509 (moving out of a value
//! with drop glue through a pattern binding) belongs to the type checker. This
//! module intentionally emits neither.

use std::collections::BTreeMap;

use crate::diag::{Diagnostic, Diagnostics, sort_and_dedup};
use crate::hir::{
    self, HBlock, HBuiltinAssoc, HBuiltinMethod, HExpr, HExprKind, HPat, HPatKind, HStmt,
};
use crate::span::Span;
use crate::types::{SelfKind, Ty};

/// Upper bound for the number of fixed point iterations of a loop body. A body
/// can only add bindings to the moved set, so this is a safety valve and not a
/// limit that well formed programs reach.
const MAX_LOOP_ITERATIONS: usize = 256;

/// Run the move checker over every function and closure body in the program.
pub fn check(program: &hir::Program) -> Diagnostics {
    let mut diagnostics = Diagnostics::new();
    for function in &program.fns {
        let mut checker = FnChecker::new(program);
        checker.walk_block(&function.body, Ctx::Consume);
        diagnostics.extend(checker.finish());
    }
    for closure in &program.closures {
        let mut checker = FnChecker::new(program);
        checker.walk_block(&closure.body, Ctx::Consume);
        diagnostics.extend(checker.finish());
    }
    sort_and_dedup(&mut diagnostics);
    diagnostics
}

/// How the value of an expression reached by the walk is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ctx {
    /// The value is consumed: moving out of a binding is a real move.
    Consume,
    /// The value is only read (a borrowed context): no move happens, so the
    /// source of the value stays live.
    Borrow,
}

/// A moved binding, remembering the binding name, the type and the span of the
/// move that made it unavailable (reported as a label on E0382).
#[derive(Clone, Debug, PartialEq)]
struct MovedValue {
    name: String,
    ty: Ty,
    span: Span,
}

/// Abstract state of one function or closure body: the set of moved bindings.
#[derive(Clone, Debug, Default, PartialEq)]
struct State {
    moved: BTreeMap<String, MovedValue>,
}

impl State {
    /// Merge two states: a binding is moved if it is moved on either path. The
    /// earliest move span is kept so the label points at the first move.
    fn join(&self, other: &State) -> State {
        let mut joined = self.clone();
        for (binding, moved) in &other.moved {
            match joined.moved.get(binding) {
                Some(existing) => {
                    let span = if moved.span.start < existing.span.start {
                        moved.span
                    } else {
                        existing.span
                    };
                    joined.moved.insert(
                        binding.clone(),
                        MovedValue {
                            name: existing.name.clone(),
                            ty: existing.ty.clone(),
                            span,
                        },
                    );
                }
                None => {
                    joined.moved.insert(binding.clone(), moved.clone());
                }
            }
        }
        joined
    }
}

struct LoopFrame {
    breaks: Vec<State>,
    continues: Vec<State>,
}

struct FnChecker<'a> {
    program: &'a hir::Program,
    diagnostics: Diagnostics,
    state: State,
    /// Greater than zero while walking a match arm guard; consuming a value
    /// there is a move out of the guard (E0507).
    guard_depth: u32,
    loops: Vec<LoopFrame>,
}

impl<'a> FnChecker<'a> {
    fn new(program: &'a hir::Program) -> FnChecker<'a> {
        FnChecker {
            program,
            diagnostics: Diagnostics::new(),
            state: State::default(),
            guard_depth: 0,
            loops: Vec::new(),
        }
    }

    fn finish(self) -> Diagnostics {
        self.diagnostics
    }

    // ------------------------------------------------------------------
    // Diagnostics
    // ------------------------------------------------------------------

    fn report_moved_use(&mut self, span: Span, moved: &MovedValue) {
        let type_name = self.program.type_name(&moved.ty);
        self.diagnostics.push(
            Diagnostic::error(format!("use of moved value: {}", moved.name), span)
                .with_code("E0382")
                .with_label(moved.span, "value moved here")
                .with_note(format!(
                    "{} has type {}, which is not Copy, so the value is moved whenever it is used by value; use .clone() to copy it first",
                    moved.name, type_name
                )),
        );
    }

    fn report_move_out_of(&mut self, span: Span, place: &str, ty: &Ty) {
        let type_name = self.program.type_name(ty);
        self.diagnostics.push(
            Diagnostic::error(
                format!("cannot move out of {} of type {}", place, type_name),
                span,
            )
            .with_code("E0507")
            .with_note("use .clone() to copy the value before consuming it"),
        );
    }

    // ------------------------------------------------------------------
    // Reading and writing places
    // ------------------------------------------------------------------

    /// Read a binding. A moved binding is an error (E0382); Copy bindings are
    /// never in the moved set.
    fn require_live(&mut self, binding: &str, span: Span) {
        if let Some(moved) = self.state.moved.get(binding).cloned() {
            self.report_moved_use(span, &moved);
        }
    }

    /// Consume a binding: mark it moved, or report a use of an already moved
    /// value.
    fn consume_local(&mut self, name: &str, binding: &str, ty: &Ty, span: Span) {
        if self.program.is_copy(ty) {
            // Copying never moves the source; this is a read.
            return;
        }
        if self.guard_depth > 0 {
            self.diagnostics.push(
                Diagnostic::error(format!("cannot move out of {} in a match guard", name), span)
                    .with_code("E0507")
                    .with_note(
                        "guard expressions cannot take ownership of a value; move it before the match or borrow it instead",
                    ),
            );
            return;
        }
        if let Some(moved) = self.state.moved.get(binding).cloned() {
            self.report_moved_use(span, &moved);
            return;
        }
        self.state.moved.insert(
            binding.to_string(),
            MovedValue {
                name: name.to_string(),
                ty: ty.clone(),
                span,
            },
        );
    }

    /// A place expression read in a borrowed context.
    fn read_place(&mut self, expr: &HExpr) {
        match &expr.kind {
            HExprKind::Local { c_name, .. } => self.require_live(c_name, expr.span),
            HExprKind::Capture { .. } => {}
            HExprKind::Field { base, .. } => self.read_place(base),
            HExprKind::Index { base, index } => {
                self.read_place(base);
                self.walk_expr(index, Ctx::Borrow);
            }
            HExprKind::Deref { expr: inner } => {
                self.walk_expr(inner, Ctx::Borrow);
            }
            _ => {
                self.walk_expr(expr, Ctx::Borrow);
            }
        }
    }

    /// A place expression written by an assignment target. Only the local root
    /// of the place becomes live again; no ownership is transferred by a write.
    fn write_place(&mut self, expr: &HExpr) {
        match &expr.kind {
            HExprKind::Local { c_name, .. } => {
                self.state.moved.remove(c_name);
            }
            HExprKind::Capture { .. } => {}
            HExprKind::Field { base, .. } => self.read_place(base),
            HExprKind::Index { base, index } => {
                self.read_place(base);
                self.walk_expr(index, Ctx::Borrow);
            }
            HExprKind::Deref { expr: inner } => {
                self.walk_expr(inner, Ctx::Borrow);
            }
            _ => {
                self.walk_expr(expr, Ctx::Borrow);
            }
        }
    }

    // ------------------------------------------------------------------
    // Bindings and patterns
    // ------------------------------------------------------------------

    /// Pattern bindings are fresh values: they shadow the moved state and are
    /// live after the pattern was matched. The type checker owns E0509 for
    /// bindings of drop-glue types, so no ownership diagnostics are emitted
    /// here.
    fn bind_pat(&mut self, pat: &HPat) {
        match &pat.kind {
            HPatKind::Wild | HPatKind::Lit(_) => {}
            HPatKind::Bind { c_name, .. } => {
                self.state.moved.remove(c_name);
            }
            HPatKind::Tuple(patterns) => {
                for pattern in patterns {
                    self.bind_pat(pattern);
                }
            }
            HPatKind::Struct { fields, .. } => {
                for (_, pattern) in fields {
                    self.bind_pat(pattern);
                }
            }
            HPatKind::Enum { args, .. } => {
                for pattern in args {
                    self.bind_pat(pattern);
                }
            }
            HPatKind::OptionSome(pattern) => self.bind_pat(pattern),
            HPatKind::OptionNone => {}
            HPatKind::ResultOk(pattern) | HPatKind::ResultErr(pattern) => self.bind_pat(pattern),
            HPatKind::Or(patterns) => {
                for pattern in patterns {
                    self.bind_pat(pattern);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Bodies and statements
    // ------------------------------------------------------------------

    /// Walk a block. Returns whether execution can fall through the block.
    fn walk_block(&mut self, block: &HBlock, ctx: Ctx) -> bool {
        let mut falls = true;
        for stmt in &block.stmts {
            if !falls {
                // The remainder of the block is unreachable; walking it could
                // only produce spurious diagnostics.
                break;
            }
            match stmt {
                HStmt::Let { pat, init, .. } => {
                    if let Some(init) = init {
                        // A plain identifier binding moves the initializer; a
                        // compound pattern binds copies (the type checker rejects
                        // drop-glue bindings).
                        let init_ctx = if matches!(&pat.kind, HPatKind::Bind { .. }) {
                            Ctx::Consume
                        } else {
                            Ctx::Borrow
                        };
                        self.walk_expr(init, init_ctx);
                    }
                    self.bind_pat(pat);
                }
                HStmt::Expr(expr) => {
                    // A discarded expression statement consumes the value and
                    // releases it at the end of the statement.
                    falls = self.walk_expr(expr, Ctx::Consume);
                }
            }
        }
        if !falls {
            return false;
        }
        match &block.tail {
            Some(tail) => self.walk_expr(tail, ctx),
            None => true,
        }
    }

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

    /// Walk an expression and report whether it (and therefore the enclosing
    /// statement) can fall through.
    fn walk_expr(&mut self, expr: &HExpr, ctx: Ctx) -> bool {
        match &expr.kind {
            HExprKind::Lit(_) | HExprKind::ConstRef { .. } | HExprKind::FnRef { .. } => true,
            HExprKind::Local { name, c_name } => {
                match ctx {
                    Ctx::Consume => self.consume_local(name, c_name, &expr.ty, expr.span),
                    Ctx::Borrow => self.require_live(c_name, expr.span),
                }
                true
            }
            HExprKind::Capture { name, .. } => {
                if ctx == Ctx::Consume && !self.program.is_copy(&expr.ty) {
                    self.report_move_out_of(
                        expr.span,
                        &format!("the captured variable {}", name),
                        &expr.ty,
                    );
                }
                true
            }
            HExprKind::Field { base, .. } => {
                if ctx == Ctx::Consume && !self.program.is_copy(&expr.ty) {
                    self.report_move_out_of(expr.span, "a field", &expr.ty);
                } else {
                    self.read_place(base);
                }
                true
            }
            HExprKind::Index { base, index } => {
                if ctx == Ctx::Consume && !self.program.is_copy(&expr.ty) {
                    self.report_move_out_of(expr.span, "an indexed element", &expr.ty);
                } else {
                    self.read_place(base);
                    self.walk_expr(index, Ctx::Borrow);
                }
                true
            }
            HExprKind::Deref { expr: inner } => {
                if ctx == Ctx::Consume && !self.program.is_copy(&expr.ty) {
                    self.report_move_out_of(expr.span, "a dereferenced value", &expr.ty);
                } else {
                    self.walk_expr(inner, Ctx::Borrow);
                }
                true
            }
            HExprKind::Closure { captures, .. } => {
                // Creating the closure moves its captures regardless of how the
                // closure value itself is used.
                for capture in captures {
                    self.walk_capture(capture, Ctx::Consume);
                }
                true
            }
            HExprKind::Call { callee, args } => {
                self.walk_call(callee, args);
                true
            }
            HExprKind::DirectCall {
                fn_id,
                receiver,
                args,
            } => {
                self.walk_direct_call(*fn_id, receiver.as_deref(), args);
                true
            }
            HExprKind::BuiltinMethod {
                receiver,
                method,
                args,
            } => {
                self.walk_builtin_method(receiver, method, args);
                true
            }
            HExprKind::BuiltinAssoc { func, args, .. } => {
                self.walk_builtin_assoc(func, args);
                true
            }
            HExprKind::BuiltinCall { args, .. } => {
                self.walk_args(args, Ctx::Borrow);
                true
            }
            HExprKind::Unary { expr: operand, .. } => {
                self.walk_expr(operand, Ctx::Borrow);
                true
            }
            HExprKind::Binary { lhs, rhs, .. } => {
                // Comparison and arithmetic operands are borrowed reads; String
                // concatenation is rewritten by the code generator to clone its
                // operands, so the sources stay live.
                self.walk_expr(lhs, Ctx::Borrow);
                self.walk_expr(rhs, Ctx::Borrow);
                true
            }
            HExprKind::Assign { target, op, value } => {
                self.walk_expr(value, Ctx::Consume);
                if op.is_some() {
                    // Compound assignment also reads the target place.
                    self.read_place(target);
                }
                self.write_place(target);
                true
            }
            HExprKind::Cast { expr: operand } => {
                self.walk_expr(operand, Ctx::Borrow);
                true
            }
            HExprKind::Block(block) => self.walk_block(block, ctx),
            HExprKind::If {
                cond,
                then_block,
                else_expr,
            } => self.walk_if(cond, then_block, else_expr.as_deref(), ctx),
            HExprKind::Match { scrutinee, arms } => self.walk_match(scrutinee, arms, ctx),
            HExprKind::Loop { body } => {
                self.walk_loop(body);
                true
            }
            HExprKind::While { cond, body } => {
                self.walk_while(cond, body);
                true
            }
            HExprKind::ForRange {
                pat,
                start,
                end,
                body,
                ..
            } => {
                self.walk_for_range(pat, start, end, body);
                true
            }
            HExprKind::ForVec { pat, vec, body } => {
                self.walk_for_vec(pat, vec, body);
                true
            }
            HExprKind::ForIter {
                iter, pat, body, ..
            } => {
                self.walk_for_iter(pat, iter, body);
                true
            }
            HExprKind::StructLit { fields, .. } => {
                // The fields of a literal are values that are moved into the
                // aggregate, even when the literal itself is only borrowed.
                for (_, value) in fields {
                    self.walk_expr(value, Ctx::Consume);
                }
                true
            }
            HExprKind::Tuple(elements) => {
                for element in elements {
                    self.walk_expr(element, Ctx::Consume);
                }
                true
            }
            HExprKind::VecLit { elems } => {
                for element in elems {
                    self.walk_expr(element, Ctx::Consume);
                }
                true
            }
            HExprKind::VecRepeat { value, count } => {
                // The repeated element is copied into every slot, so it is only
                // read; the count is a plain read as well.
                self.walk_expr(value, Ctx::Borrow);
                self.walk_expr(count, Ctx::Borrow);
                true
            }
            HExprKind::EnumLit { args, .. } => {
                for arg in args {
                    self.walk_expr(arg, Ctx::Consume);
                }
                true
            }
            HExprKind::OptionSome(inner)
            | HExprKind::ResultOk(inner)
            | HExprKind::ResultErr(inner) => {
                self.walk_expr(inner, Ctx::Consume);
                true
            }
            HExprKind::OptionNone => true,
            HExprKind::Format { parts } => {
                self.walk_parts(parts);
                true
            }
            HExprKind::Macro(call) => {
                // Format arguments of the printing/assertion macros are borrowed;
                // assert conditions and assert_eq operands are read-only too.
                self.walk_parts(&call.parts);
                self.walk_args(&call.args, Ctx::Borrow);
                true
            }
            HExprKind::Break => {
                self.record_loop_exit(true);
                false
            }
            HExprKind::Continue => {
                self.record_loop_exit(false);
                false
            }
            HExprKind::Return(value) => {
                if let Some(value) = value {
                    self.walk_expr(value, Ctx::Consume);
                }
                false
            }
            HExprKind::Ref { expr: inner, .. } => {
                self.walk_expr(inner, Ctx::Borrow);
                true
            }
        }
    }

    fn walk_args(&mut self, args: &[HExpr], ctx: Ctx) {
        for arg in args {
            self.walk_expr(arg, ctx);
        }
    }

    fn walk_parts(&mut self, parts: &[hir::HFormatPart]) {
        for part in parts {
            if let hir::HFormatPartKind::Arg { expr, .. } = &part.kind {
                self.walk_expr(expr, Ctx::Borrow);
            }
        }
    }

    /// Closure capture initialisers are evaluated in the enclosing body. Only a
    /// capture of a plain local transfers ownership; the other forms the
    /// frontend produces are copied by the code generator, which reports its own
    /// E0507 for user-written moves out of a capture.
    fn walk_capture(&mut self, capture: &HExpr, ctx: Ctx) {
        match &capture.kind {
            HExprKind::Local { .. } => {
                self.walk_expr(capture, ctx);
            }
            HExprKind::Break => {}
            _ => {
                self.walk_expr(capture, Ctx::Borrow);
            }
        }
    }

    /// Arguments of a call through a function value: by-value parameters consume
    /// their argument, reference parameters only borrow it. Calling does not
    /// consume the function value itself.
    fn walk_call(&mut self, callee: &HExpr, args: &[HExpr]) {
        self.walk_expr(callee, Ctx::Borrow);
        let params = match &callee.ty {
            Ty::Fn(params, _) => params.clone(),
            _ => Vec::new(),
        };
        for (index, arg) in args.iter().enumerate() {
            let arg_ctx = match params.get(index) {
                Some(param) if !param.is_ref() => Ctx::Consume,
                _ => Ctx::Borrow,
            };
            self.walk_expr(arg, arg_ctx);
        }
    }

    /// Arguments of a statically resolved call. A by-value receiver (including a
    /// by-value self) consumes the receiver place; every other receiver kind
    /// borrows it.
    fn walk_direct_call(&mut self, fn_id: hir::FnId, receiver: Option<&HExpr>, args: &[HExpr]) {
        let (self_kind, param_refs) = match self.program.fns.get(fn_id) {
            Some(function) => (
                function.self_kind,
                function
                    .params
                    .iter()
                    .map(|param| param.ty.is_ref())
                    .collect::<Vec<_>>(),
            ),
            None => return,
        };
        let offset = usize::from(receiver.is_some());
        if let Some(receiver) = receiver {
            let receiver_ctx = if self_kind == Some(SelfKind::Value) {
                Ctx::Consume
            } else {
                Ctx::Borrow
            };
            self.walk_expr(receiver, receiver_ctx);
        }
        for (index, arg) in args.iter().enumerate() {
            let arg_ctx = match param_refs.get(offset + index) {
                Some(true) | None => Ctx::Borrow,
                Some(false) => Ctx::Consume,
            };
            self.walk_expr(arg, arg_ctx);
        }
    }

    /// Builtin methods: only the self-consuming ones consume the receiver;
    /// payload parameters of the consuming builtins consume their argument.
    ///
    /// None of the HashMap methods consume the map itself: they all take
    /// shared or mutable references to it, so reading a moved map as the
    /// receiver still reports the move. insert moves both the key and the
    /// value into the table, while get, get_with, contains_key and remove copy
    /// the key. get_with takes its closure by value like any by-value
    /// argument.
    fn walk_builtin_method(&mut self, receiver: &HExpr, method: &HBuiltinMethod, args: &[HExpr]) {
        let consumes_receiver = matches!(
            method,
            HBuiltinMethod::OptUnwrap
                | HBuiltinMethod::OptUnwrapOr
                | HBuiltinMethod::OptExpect
                | HBuiltinMethod::ResUnwrap
                | HBuiltinMethod::ResUnwrapOr
                | HBuiltinMethod::ResExpect
        );
        self.walk_expr(
            receiver,
            if consumes_receiver {
                Ctx::Consume
            } else {
                Ctx::Borrow
            },
        );
        for (index, arg) in args.iter().enumerate() {
            let consumes_arg = match method {
                HBuiltinMethod::OptUnwrapOr | HBuiltinMethod::ResUnwrapOr => index == 0,
                HBuiltinMethod::VecPush | HBuiltinMethod::VecExtend => index == 0,
                HBuiltinMethod::VecInsert => index == 1,
                HBuiltinMethod::RcSet | HBuiltinMethod::BoxSet => index == 0,
                HBuiltinMethod::MapInsert => index <= 1,
                HBuiltinMethod::MapGetWith => index == 1,
                _ => false,
            };
            self.walk_expr(
                arg,
                if consumes_arg {
                    Ctx::Consume
                } else {
                    Ctx::Borrow
                },
            );
        }
    }

    /// Builtin associated functions: Rc::new and Box::new take ownership of
    /// their payload, everything else only borrows its arguments.
    fn walk_builtin_assoc(&mut self, func: &HBuiltinAssoc, args: &[HExpr]) {
        for (index, arg) in args.iter().enumerate() {
            let consumes =
                matches!(func, HBuiltinAssoc::RcNew | HBuiltinAssoc::BoxNew) && index == 0;
            self.walk_expr(arg, if consumes { Ctx::Consume } else { Ctx::Borrow });
        }
    }

    // ------------------------------------------------------------------
    // Control flow
    // ------------------------------------------------------------------

    fn walk_if(
        &mut self,
        cond: &HExpr,
        then_block: &HBlock,
        else_expr: Option<&HExpr>,
        ctx: Ctx,
    ) -> bool {
        self.walk_expr(cond, Ctx::Borrow);
        let pre_if = self.state.clone();
        let then_falls = self.walk_block(then_block, ctx);
        let then_state = self.state.clone();
        self.state = pre_if;
        let (else_falls, else_state) = match else_expr {
            Some(else_expr) => {
                let falls = self.walk_expr(else_expr, ctx);
                (falls, self.state.clone())
            }
            None => (true, self.state.clone()),
        };
        let mut joined: Option<State> = None;
        if then_falls {
            joined = Some(then_state);
        }
        if else_falls {
            joined = Some(match joined {
                Some(current) => current.join(&else_state),
                None => else_state,
            });
        }
        self.state = joined.unwrap_or_default();
        then_falls || else_falls
    }

    fn walk_match(&mut self, scrutinee: &HExpr, arms: &[hir::HArm], ctx: Ctx) -> bool {
        // When the scrutinee is non-Copy it is moved into a temporary for the
        // duration of the match.
        self.walk_expr(scrutinee, Ctx::Consume);
        let pre_match = self.state.clone();
        let mut joined: Option<State> = None;
        let mut falls_any = false;
        for arm in arms {
            self.state = pre_match.clone();
            self.bind_pat(&arm.pat);
            if let Some(guard) = &arm.guard {
                self.guard_depth += 1;
                self.walk_expr(guard, Ctx::Borrow);
                self.guard_depth -= 1;
            }
            let falls = self.walk_expr(&arm.body, ctx);
            if falls {
                falls_any = true;
                joined = Some(match joined {
                    Some(current) => current.join(&self.state),
                    None => self.state.clone(),
                });
            }
        }
        self.state = joined.unwrap_or(pre_match);
        falls_any
    }

    fn record_loop_exit(&mut self, is_break: bool) {
        let state = self.state.clone();
        if let Some(frame) = self.loops.last_mut() {
            if is_break {
                frame.breaks.push(state);
            } else {
                frame.continues.push(state);
            }
        }
    }

    /// Analyze a loop body to a fixed point. The iteration callback walks one
    /// iteration (condition and/or pattern bindings included) and returns
    /// whether the iteration can fall through to the next one.
    ///
    /// The fixed point is the state at the loop head: it records every value
    /// that may have been moved by an earlier iteration, so body code that
    /// consumes such a value reports E0382 on the iteration that reaches it.
    ///
    /// A while/for loop can also exit at the loop head (the condition fails or
    /// the iterator is exhausted), so the loop-head state is part of the state
    /// after the loop. A plain loop() only exits through a break.
    fn analyze_loop<F>(&mut self, exits_at_head: bool, mut iteration: F) -> State
    where
        F: FnMut(&mut FnChecker<'a>) -> bool,
    {
        let pre = self.state.clone();
        let mut entry = pre.clone();
        let mut breaks: Vec<State> = Vec::new();
        for _ in 0..MAX_LOOP_ITERATIONS {
            self.loops.push(LoopFrame {
                breaks: Vec::new(),
                continues: Vec::new(),
            });
            self.state = entry.clone();
            let falls = iteration(self);
            let frame = match self.loops.pop() {
                Some(frame) => frame,
                None => break,
            };
            let mut iter_out: Option<State> = None;
            for state in frame.continues {
                iter_out = Some(match iter_out {
                    Some(current) => current.join(&state),
                    None => state,
                });
            }
            if falls {
                iter_out = Some(match iter_out {
                    Some(current) => current.join(&self.state),
                    None => self.state.clone(),
                });
            }
            breaks = frame.breaks;
            let next_entry = match iter_out {
                Some(iter_out) => pre.join(&iter_out),
                None => pre.clone(),
            };
            let converged = next_entry == entry;
            entry = next_entry;
            if converged {
                break;
            }
        }
        let mut post = if exits_at_head {
            entry
        } else if let Some(first) = breaks.first() {
            first.clone()
        } else {
            // A loop that never breaks cannot fall through; keep the incoming
            // state so trailing unreachable code does not invent diagnostics.
            pre
        };
        for state in &breaks {
            post = post.join(state);
        }
        post
    }

    fn walk_loop(&mut self, body: &HBlock) {
        let post = self.analyze_loop(false, |checker| checker.walk_block(body, Ctx::Consume));
        self.state = post;
    }

    fn walk_while(&mut self, cond: &HExpr, body: &HBlock) {
        let post = self.analyze_loop(true, |checker| {
            checker.walk_expr(cond, Ctx::Borrow);
            checker.walk_block(body, Ctx::Consume)
        });
        self.state = post;
    }

    fn walk_for_range(&mut self, pat: &HPat, start: &HExpr, end: &HExpr, body: &HBlock) {
        self.walk_expr(start, Ctx::Borrow);
        self.walk_expr(end, Ctx::Borrow);
        let post = self.analyze_loop(true, |checker| {
            checker.bind_pat(pat);
            checker.walk_block(body, Ctx::Consume)
        });
        self.state = post;
    }

    fn walk_for_vec(&mut self, pat: &HPat, vec: &HExpr, body: &HBlock) {
        // A for loop over a vector moves the collection into the loop; each
        // iteration transfers one element into the pattern binding.
        self.walk_expr(vec, Ctx::Consume);
        let post = self.analyze_loop(true, |checker| {
            checker.bind_pat(pat);
            checker.walk_block(body, Ctx::Consume)
        });
        self.state = post;
    }

    fn walk_for_iter(&mut self, pat: &HPat, iter: &HExpr, body: &HBlock) {
        // A for loop over an iterator consumes the iterator value: the hidden
        // loop owns it and polls next_fn until it yields None. The pattern is
        // rebound on every iteration, so moving a binding inside the body is
        // fine, while moving an enclosing local is reported on the next
        // iteration by the loop-head fixed point.
        self.walk_expr(iter, Ctx::Consume);
        let post = self.analyze_loop(true, |checker| {
            checker.bind_pat(pat);
            checker.walk_block(body, Ctx::Consume)
        });
        self.state = post;
    }
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::diag::Diagnostics;
    use crate::span::SourceFile;
    use crate::{hir, lexer, parser, typeck};

    fn messages(diagnostics: &Diagnostics) -> String {
        diagnostics
            .as_slice()
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// Parse and type check a source file, returning the HIR if the front end
    /// accepted it.
    fn check_source(source: &str) -> Result<hir::Program, Diagnostics> {
        let file = SourceFile::new("test.ls", source);
        let (tokens, lex_diagnostics) = lexer::tokenize(source);
        assert!(
            !lex_diagnostics.has_errors(),
            "lexer errors: {}",
            messages(&lex_diagnostics)
        );
        let (program, parse_diagnostics) = parser::parse(&tokens, &file);
        assert!(
            !parse_diagnostics.has_errors(),
            "parser errors: {}",
            messages(&parse_diagnostics)
        );
        typeck::check(&program, &file)
    }

    /// Collect the move checker diagnostics for a program. When the type
    /// checking pipeline rejects the program its diagnostics already include
    /// the move checker output (the pipeline runs it as its last step).
    fn diagnostics_for(source: &str) -> Diagnostics {
        match check_source(source) {
            Ok(program) => check(&program),
            Err(diagnostics) => diagnostics,
        }
    }

    fn expect_accepted(source: &str) {
        let program = match check_source(source) {
            Ok(program) => program,
            Err(diagnostics) => {
                panic!("valid program was rejected: {}", messages(&diagnostics))
            }
        };
        let diagnostics = check(&program);
        assert!(
            !diagnostics.has_errors(),
            "move checker rejected valid program: {}",
            messages(&diagnostics)
        );
    }

    fn expect_rejected(source: &str, code: &'static str) {
        let diagnostics = diagnostics_for(source);
        let found = diagnostics
            .as_slice()
            .iter()
            .any(|diagnostic| diagnostic.code == Some(code));
        assert!(
            found,
            "expected a {} diagnostic, got: {}",
            code,
            messages(&diagnostics)
        );
    }

    fn expect_code<'a>(diagnostics: &'a Diagnostics, code: &str) -> &'a crate::diag::Diagnostic {
        diagnostics
            .as_slice()
            .iter()
            .find(|diagnostic| diagnostic.code == Some(code))
            .unwrap_or_else(|| {
                panic!(
                    "expected a {} diagnostic, got: {}",
                    code,
                    messages(diagnostics)
                )
            })
    }

    // -- binding moves (E0382) ------------------------------------------

    #[test]
    fn accepts_copy_values() {
        expect_accepted(
            r#"
fn main() {
    let number = 42;
    let copy = number;
    println!("{} {}", number, copy);
}
"#,
        );
    }

    #[test]
    fn accepts_copy_struct_bindings() {
        expect_accepted(
            r#"
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let point = Point { x: 1, y: 2 };
    let copy = point;
    println!("{} {}", point.x, copy.y);
}
"#,
        );
    }

    #[test]
    fn rejects_use_after_move() {
        expect_rejected(
            r#"
fn main() {
    let text = String::from("hello");
    let moved = text;
    println!("{}", text);
}
"#,
            "E0382",
        );
    }

    #[test]
    fn rejects_use_after_move_without_intervening_statement() {
        expect_rejected(
            r#"
fn main() {
    let text = String::from("hello");
    let moved = text;
    let length = text.len();
    println!("{} {}", moved, length);
}
"#,
            "E0382",
        );
    }

    #[test]
    fn rejects_second_move() {
        expect_rejected(
            r#"
fn main() {
    let text = String::from("hello");
    let first = text;
    let second = text;
    println!("{} {}", first, second);
}
"#,
            "E0382",
        );
    }

    #[test]
    fn move_after_use_is_allowed() {
        expect_accepted(
            r#"
fn main() {
    let text = String::from("hello");
    println!("{}", text);
    let moved = text;
    println!("{}", moved);
}
"#,
        );
    }

    #[test]
    fn accepts_borrowed_format_arguments() {
        expect_accepted(
            r#"
fn main() {
    let text = String::from("hello");
    println!("{}", text);
    println!("{:?}", text);
    let line = format!("text is {}", text);
    println!("{}", line);
}
"#,
        );
    }

    #[test]
    fn accepts_reinitialization_after_move() {
        expect_accepted(
            r#"
fn main() {
    let mut text = String::from("hello");
    let moved = text;
    text = String::from("again");
    println!("{} {}", text, moved);
}
"#,
        );
    }

    #[test]
    fn accepts_assignment_in_both_branches() {
        expect_accepted(
            r#"
fn main() {
    let mut text = String::from("hello");
    let moved = text;
    let flag = true;
    if flag {
        text = String::from("left");
    } else {
        text = String::from("right");
    }
    println!("{} {}", text, moved);
}
"#,
        );
    }

    #[test]
    fn rejects_use_after_move_in_branch() {
        expect_rejected(
            r#"
fn main() {
    let text = String::from("hello");
    let flag = true;
    if flag {
        let moved = text;
        println!("{}", moved);
    }
    println!("{}", text);
}
"#,
            "E0382",
        );
    }

    #[test]
    fn rejects_move_on_one_match_arm() {
        expect_rejected(
            r#"
fn main() {
    let text = String::from("hello");
    let value = 1;
    match value {
        1 => {
            let moved = text;
            println!("{}", moved);
        }
        _ => println!("other"),
    }
    println!("{}", text);
}
"#,
            "E0382",
        );
    }

    #[test]
    fn accepts_match_without_move() {
        expect_accepted(
            r#"
fn main() {
    let text = String::from("hello");
    let value = 2;
    match value {
        1 => println!("one"),
        _ => println!("{}", text),
    }
}
"#,
        );
    }

    #[test]
    fn rejects_move_in_loop_body() {
        expect_rejected(
            r#"
fn main() {
    let text = String::from("hello");
    let mut index = 0;
    while index < 3 {
        let moved = text;
        println!("{}", moved);
        index = index + 1;
    }
    println!("{}", text);
}
"#,
            "E0382",
        );
    }

    #[test]
    fn accepts_fresh_binding_in_each_loop_iteration() {
        expect_accepted(
            r#"
fn main() {
    let mut index = 0;
    while index < 3 {
        let sample = String::from("hello");
        println!("{}", sample);
        index = index + 1;
    }
    println!("{}", index);
}
"#,
        );
    }

    // -- moves out of places (E0507) ------------------------------------

    #[test]
    fn rejects_move_out_of_field() {
        expect_rejected(
            r#"
struct Wrapper {
    text: String,
}

fn main() {
    let wrapper = Wrapper { text: String::from("hello") };
    let moved = wrapper.text;
    println!("{}", moved);
}
"#,
            "E0507",
        );
    }

    #[test]
    fn rejects_move_out_of_index() {
        expect_rejected(
            r#"
fn main() {
    let items: Vec<String> = vec![String::from("hello"), String::from("world")];
    let moved = items[0];
    println!("{}", moved);
}
"#,
            "E0507",
        );
    }

    #[test]
    fn rejects_move_out_of_dereference() {
        expect_rejected(
            r#"
fn main() {
    let text = String::from("hello");
    let reference = &text;
    let moved = *reference;
    println!("{}", moved);
}
"#,
            "E0507",
        );
    }

    #[test]
    fn rejects_move_out_of_closure_capture() {
        expect_rejected(
            r#"
fn main() {
    let text = String::from("hello");
    let closure = || {
        let moved = text;
        println!("{}", moved);
    };
    closure();
}
"#,
            "E0507",
        );
    }

    #[test]
    fn rejects_use_after_closure_capture_moves() {
        expect_rejected(
            r#"
fn main() {
    let text = String::from("hello");
    let closure = || println!("{}", text);
    closure();
    println!("{}", text);
}
"#,
            "E0382",
        );
    }

    #[test]
    fn accepts_calling_closure_twice() {
        expect_accepted(
            r#"
fn main() {
    let text = String::from("hello");
    let closure = || println!("{}", text);
    closure();
    closure();
}
"#,
        );
    }

    #[test]
    fn accepts_copy_capture_in_closure() {
        expect_accepted(
            r#"
fn main() {
    let number = 7;
    let closure = || println!("{}", number);
    closure();
    println!("{}", number);
}
"#,
        );
    }

    #[test]
    fn rejects_move_out_of_match_scrutinee() {
        expect_rejected(
            r#"
fn main() {
    let option: Option<String> = Some(String::from("hello"));
    match option {
        Some(text) => println!("{}", text),
        None => println!("none"),
    }
    println!("{}", option.is_some());
}
"#,
            "E0382",
        );
    }

    #[test]
    fn rejects_discarded_expression_moves() {
        expect_rejected(
            r#"
fn main() {
    let text = String::from("hello");
    text;
    println!("{}", text);
}
"#,
            "E0382",
        );
    }

    // -- consuming call arguments ---------------------------------------

    #[test]
    fn rejects_use_after_by_value_argument() {
        expect_rejected(
            r#"
fn take(text: String) {
    println!("{}", text);
}

fn main() {
    let text = String::from("hello");
    take(text);
    println!("{}", text);
}
"#,
            "E0382",
        );
    }

    #[test]
    fn accepts_by_reference_argument() {
        expect_accepted(
            r#"
fn describe(text: &String) -> String {
    format!("{}", text)
}

fn main() {
    let text = String::from("hello");
    let line = describe(&text);
    println!("{} {}", line, text);
}
"#,
        );
    }

    #[test]
    fn rejects_use_after_vec_push_payload() {
        expect_rejected(
            r#"
fn main() {
    let mut items: Vec<String> = Vec::new();
    let text = String::from("hello");
    items.push(text);
    println!("{}", text);
}
"#,
            "E0382",
        );
    }

    #[test]
    fn rejects_use_after_unwrap_receiver() {
        expect_rejected(
            r#"
fn main() {
    let option: Option<String> = Some(String::from("hello"));
    let value = option.unwrap();
    let again = option.unwrap();
    println!("{} {}", value, again);
}
"#,
            "E0382",
        );
    }

    // -- by-value and borrowed self receivers ---------------------------

    #[test]
    fn accepts_by_value_self_receiver() {
        expect_accepted(
            r#"
struct Counter {
    count: i64,
    label: String,
}

impl Counter {
    fn consume(self) -> i64 {
        self.count
    }
}

fn main() {
    let counter = Counter { count: 3, label: String::from("x") };
    let value = counter.consume();
    println!("{}", value);
}
"#,
        );
    }

    #[test]
    fn rejects_use_after_by_value_self_receiver() {
        expect_rejected(
            r#"
struct Counter {
    count: i64,
    label: String,
}

impl Counter {
    fn consume(self) -> i64 {
        self.count
    }
}

fn main() {
    let counter = Counter { count: 3, label: String::from("x") };
    let value = counter.consume();
    println!("{} {}", value, counter.count);
}
"#,
            "E0382",
        );
    }

    #[test]
    fn accepts_borrowed_self_receiver() {
        expect_accepted(
            r#"
struct Counter {
    count: i64,
}

impl Counter {
    fn total(&self) -> i64 {
        self.count
    }
}

fn main() {
    let counter = Counter { count: 3 };
    let first = counter.total();
    let second = counter.total();
    println!("{} {} {}", first, second, counter.count);
}
"#,
        );
    }

    // -- literals and constructors --------------------------------------

    #[test]
    fn rejects_move_into_some_payload() {
        expect_rejected(
            r#"
fn main() {
    let text = String::from("hello");
    let option: Option<String> = Some(text);
    println!("{}", text);
}
"#,
            "E0382",
        );
    }

    #[test]
    fn rejects_move_into_struct_literal_field() {
        expect_rejected(
            r#"
struct Wrapper {
    text: String,
}

fn main() {
    let text = String::from("hello");
    let wrapper = Wrapper { text: text };
    println!("{}", text);
}
"#,
            "E0382",
        );
    }

    #[test]
    fn rejects_move_into_tuple_element() {
        expect_rejected(
            r#"
fn main() {
    let text = String::from("hello");
    let pair = (text, 1);
    println!("{}", text);
}
"#,
            "E0382",
        );
    }

    #[test]
    fn rejects_move_into_enum_payload() {
        expect_rejected(
            r#"
enum Message {
    Text(String),
    Number(i64),
}

fn main() {
    let text = String::from("hello");
    let message = Message::Text(text);
    println!("{}", text);
    match message {
        Message::Text(inner) => println!("{}", inner),
        Message::Number(value) => println!("{}", value),
    }
}
"#,
            "E0382",
        );
    }

    // -- loops over collections -----------------------------------------

    #[test]
    fn rejects_use_of_vector_after_for_loop() {
        expect_rejected(
            r#"
fn main() {
    let items: Vec<String> = vec![String::from("hello"), String::from("world")];
    for item in items {
        println!("{}", item);
    }
    println!("{}", items.len());
}
"#,
            "E0382",
        );
    }

    #[test]
    fn accepts_for_loop_without_later_use() {
        expect_accepted(
            r#"
fn main() {
    let items: Vec<String> = vec![String::from("hello")];
    for item in items {
        println!("{}", item);
    }
}
"#,
        );
    }

    // -- returns and block tails ----------------------------------------

    #[test]
    fn accepts_tail_expression_move() {
        expect_accepted(
            r#"
fn build() -> String {
    let text = String::from("hello");
    text
}

fn main() {
    let result = build();
    println!("{}", result);
}
"#,
        );
    }

    #[test]
    fn rejects_use_after_return_value_move() {
        expect_rejected(
            r#"
fn build() -> String {
    let text = String::from("hello");
    let moved = text;
    text
}

fn main() {
    let result = build();
    println!("{}", result);
}
"#,
            "E0382",
        );
    }

    // -- clones ---------------------------------------------------------

    #[test]
    fn accepts_clone_keeps_source_live() {
        expect_accepted(
            r#"
fn main() {
    let text = String::from("hello");
    let copy = text.clone();
    println!("{} {}", text, copy);
}
"#,
        );
    }

    // -- while-let and match guards -------------------------------------

    #[test]
    fn accepts_while_let_with_reassignment() {
        expect_accepted(
            r#"
fn main() {
    let mut next: Option<String> = Some(String::from("hello"));
    while let Some(text) = next {
        next = None;
        println!("{}", text);
    }
}
"#,
        );
    }

    #[test]
    fn rejects_move_into_while_let_without_reassignment() {
        expect_rejected(
            r#"
fn main() {
    let mut next: Option<String> = Some(String::from("hello"));
    while let Some(text) = next {
        println!("{}", text);
    }
}
"#,
            "E0382",
        );
    }

    #[test]
    fn rejects_move_out_of_match_guard() {
        expect_rejected(
            r#"
fn main() {
    let text = String::from("hello");
    let value = 2;
    match value {
        number if {
            let moved = text;
            moved.len() > 0
        } => println!("{}", number),
        _ => println!("other"),
    }
}
"#,
            "E0507",
        );
    }

    #[test]
    fn accepts_reading_in_match_guard() {
        expect_accepted(
            r#"
fn main() {
    let text = String::from("hello");
    let value = 2;
    match value {
        number if number > 0 => println!("{} {}", number, text),
        _ => println!("other"),
    }
}
"#,
        );
    }

    #[test]
    fn accepts_match_binding_read_twice() {
        expect_accepted(
            r#"
fn main() {
    let option: Option<String> = Some(String::from("hello"));
    match option {
        Some(text) => {
            println!("{}", text);
            println!("{}", text);
        }
        None => println!("none"),
    }
}
"#,
        );
    }

    #[test]
    fn accepts_borrow_then_use() {
        expect_accepted(
            r#"
fn main() {
    let text = String::from("hello");
    let reference = &text;
    let line = format!("{}", reference);
    println!("{} {}", text, line);
}
"#,
        );
    }

    // -- diagnostic shape ----------------------------------------------

    #[test]
    fn use_after_move_diagnostic_names_the_move_site() {
        let diagnostics = diagnostics_for(
            r#"
fn main() {
    let text = String::from("hello");
    let moved = text;
    println!("{} {}", text, moved);
}
"#,
        );
        let diagnostic = expect_code(&diagnostics, "E0382");
        assert!(
            diagnostic
                .labels
                .iter()
                .any(|label| label.message.as_deref() == Some("value moved here")),
            "E0382 should label the move site: {:?}",
            diagnostic.labels
        );
        assert!(
            diagnostic.notes.iter().any(|note| note.contains("moved")),
            "E0382 should carry a note: {:?}",
            diagnostic.notes
        );
    }

    #[test]
    fn move_out_of_place_suggests_clone() {
        let diagnostics = diagnostics_for(
            r#"
struct Wrapper {
    text: String,
}

fn main() {
    let wrapper = Wrapper { text: String::from("hello") };
    let moved = wrapper.text;
    println!("{}", moved);
}
"#,
        );
        let diagnostic = expect_code(&diagnostics, "E0507");
        assert!(
            diagnostic
                .notes
                .iter()
                .any(|note| note.contains(".clone()")),
            "E0507 should suggest .clone(): {:?}",
            diagnostic.notes
        );
    }

    #[test]
    fn names_user_types_nested_in_box_and_option() {
        let source = r#"
struct Node {
    value: i64,
    next: Option<Box<Node>>,
}

fn detach(head: Node) -> Option<Box<Node>> {
    head.next
}

fn main() {
    let node = Node { value: 1, next: None };
    let detached = detach(node);
    if detached.is_some() {
        println!("some");
    }
}
"#;
        let diagnostics = diagnostics_for(source);
        let diagnostic = expect_code(&diagnostics, "E0507");
        assert!(
            diagnostic.message.contains("Option<Box<Node>>"),
            "E0507 should name nested user types: {}",
            diagnostic.message
        );
        assert!(
            !diagnostic.message.contains("struct#"),
            "E0507 should not leak raw type ids: {}",
            diagnostic.message
        );
        assert!(
            diagnostic
                .notes
                .iter()
                .any(|note| note.contains(".clone()")),
            "E0507 should suggest .clone(): {:?}",
            diagnostic.notes
        );
    }

    #[test]
    fn diagnostics_are_deterministic() {
        let source = r#"
fn main() {
    let text = String::from("hello");
    let moved = text;
    println!("{} {}", text, moved);
    println!("{}", text);
}
"#;
        let first = messages(&diagnostics_for(source));
        let second = messages(&diagnostics_for(source));
        assert_eq!(first, second, "move checker output should be deterministic");
    }

    // -- v1.5 integration through the real front end --------------------
    //
    // These programs go through the lexer, parser and type checker together
    // with the standard library prelude (the house style of this checker: the
    // hand built HIR tests pin the rules, these pin the lowering). The move
    // checker is then run on the lowered HIR, which also contains the
    // instantiated prelude adapters (their filter/find bodies borrow an item
    // before returning it, a sequence that must stay accepted).

    /// Parse and type check a source file together with the standard library
    /// prelude, which provides the Iterator trait and the HashMap builtin.
    fn check_source_with_prelude(source: &str) -> Result<hir::Program, Diagnostics> {
        let file = SourceFile::new("test.ls", source);
        let (tokens, lex_diagnostics) = lexer::tokenize(source);
        assert!(
            !lex_diagnostics.has_errors(),
            "lexer errors: {}",
            messages(&lex_diagnostics)
        );
        let (program, parse_diagnostics) = parser::parse(&tokens, &file);
        assert!(
            !parse_diagnostics.has_errors(),
            "parser errors: {}",
            messages(&parse_diagnostics)
        );
        let prelude = match crate::prelude::program() {
            Ok(prelude) => prelude,
            Err(diagnostics) => {
                panic!(
                    "the embedded prelude must parse: {}",
                    messages(&diagnostics)
                )
            }
        };
        typeck::check_with_prelude(&program, Some(prelude), &file)
    }

    /// The diagnostics for a v1.5 program.
    ///
    /// [typeck::check_with_prelude] already runs the borrow checker and the
    /// move checker as post-lowering passes, so a program rejected by either
    /// of them carries those diagnostics in the error result.
    fn diagnostics_with_prelude(source: &str) -> Diagnostics {
        match check_source_with_prelude(source) {
            Ok(program) => check(&program),
            Err(diagnostics) => diagnostics,
        }
    }

    fn expect_accepted_with_prelude(source: &str) {
        match check_source_with_prelude(source) {
            Ok(program) => {
                let diagnostics = check(&program);
                assert!(
                    !diagnostics.has_errors(),
                    "move checker rejected a valid v1.5 program: {}",
                    messages(&diagnostics)
                );
            }
            Err(diagnostics) => panic!(
                "the front end rejected a valid v1.5 program: {}",
                messages(&diagnostics)
            ),
        }
    }

    /// Expect an error with the given code and return its message.
    fn expect_moveck_error(source: &str, code: &'static str) -> String {
        let diagnostics = diagnostics_with_prelude(source);
        let diagnostic = expect_code(&diagnostics, code);
        diagnostic.message.clone()
    }

    #[test]
    fn accepts_an_iterator_chain_through_the_prelude() {
        expect_accepted_with_prelude(
            r#"
fn main() {
    let numbers: Vec<i64> = vec![1, 2, 3, 4, 5];
    let doubled: Vec<i64> = numbers.iter().map(|value| value * 2).collect();
    println!("{:?}", doubled);
}
"#,
        );
    }

    #[test]
    fn accepts_an_iterator_for_loop_through_the_prelude() {
        expect_accepted_with_prelude(
            r#"
fn main() {
    let numbers: Vec<i64> = vec![1, 2, 3, 4, 5];
    let mut total: i64 = 0;
    for value in numbers.iter().map(|n| n * 3).filter(|n| *n % 2 == 0) {
        total = total + value;
    }
    println!("total {}", total);
}
"#,
        );
    }

    #[test]
    fn rejects_a_second_for_loop_over_a_moved_iterator() {
        let message = expect_moveck_error(
            r#"
fn main() {
    let numbers: Vec<i64> = vec![1, 2, 3];
    let iter = numbers.iter();
    for value in iter {
        println!("{}", value);
    }
    for again in iter {
        println!("{}", again);
    }
}
"#,
            "E0382",
        );
        assert!(
            message.contains("iter"),
            "E0382 should name the moved iterator: {}",
            message
        );
    }

    #[test]
    fn hash_map_insert_moves_the_key() {
        let message = expect_moveck_error(
            r#"
fn main() {
    let mut counts: HashMap<String, i64> = HashMap::new();
    let key = String::from("a");
    counts.insert(key, 1);
    println!("{}", key);
}
"#,
            "E0382",
        );
        assert!(
            message.contains("key"),
            "E0382 should name the moved key: {}",
            message
        );
    }

    #[test]
    fn hash_map_reads_copy_the_key() {
        expect_accepted_with_prelude(
            r#"
fn main() {
    let mut counts: HashMap<String, i64> = HashMap::new();
    let key = String::from("a");
    counts.insert(key.clone(), 1);
    let found = counts.get(key.clone());
    if counts.contains_key(key.clone()) {
        counts.remove(key.clone());
    }
    println!("{} {:?} {}", key, found, counts.len());
}
"#,
        );
    }

    #[test]
    fn hash_map_get_with_moves_only_the_closure_argument() {
        expect_accepted_with_prelude(
            r#"
fn main() {
    let mut counts: HashMap<String, i64> = HashMap::new();
    counts.insert(String::from("a"), 2);
    let doubled = counts.get_with(String::from("a"), |value| *value * 2);
    println!("{}", doubled);
}
"#,
        );
    }

    #[test]
    fn accepts_find_on_non_copy_items_through_the_prelude() {
        // find moves the matched item out of the iterator after borrowing it
        // for the predicate: the same borrow-then-move shape as filter, but
        // with a non-Copy payload (String) that must not be reported as a
        // moved value when the iterator is consumed by the adapter itself.
        expect_accepted_with_prelude(
            r#"
fn main() {
    let words: Vec<String> = vec![String::from("hi"), String::from("hello")];
    let found: Option<String> = words.iter().find(|word| word.len() > 3);
    println!("{:?}", found);
}
"#,
        );
    }

    #[test]
    fn hash_map_keys_and_values_borrow_the_map() {
        expect_accepted_with_prelude(
            r#"
fn main() {
    let mut counts: HashMap<String, i64> = HashMap::new();
    counts.insert(String::from("a"), 1);
    counts.insert(String::from("b"), 2);
    let keys: Vec<String> = counts.keys();
    let values: Vec<i64> = counts.values();
    println!("{:?} {:?} {}", keys, values, counts.len());
}
"#,
        );
    }

    #[test]
    fn accepts_borrow_then_move_through_a_closure_predicate() {
        // The shape used by the lazy adapters in the prelude (filter, find,
        // any, all): a shared borrow of a value feeds a predicate, and the
        // value is moved afterwards once the borrow is over. This is also the
        // shape of the closure_ref_local fixture.
        expect_accepted_with_prelude(
            r#"
fn is_long(value: &String) -> bool {
    value.len() > 3
}

fn main() {
    let text = String::from("lsaca");
    let keep = |value: &String| -> bool { value.len() > 3 };
    if keep(&text) {
        let moved = text;
        println!("{}", moved);
    }

    let local = String::from("local");
    if is_long(&local) {
        println!("{}", local);
    }
}
"#,
        );
    }

    #[test]
    fn fn_value_in_a_struct_field_is_called_without_moving_the_field() {
        expect_accepted_with_prelude(
            r#"
fn double(value: i64) -> i64 {
    value * 2
}

struct Handler {
    name: String,
    apply: fn(i64) -> i64,
}

fn main() {
    let handler = Handler { name: String::from("h"), apply: double };
    let first = (handler.apply)(2);
    let second = (handler.apply)(3);
    println!("{} {} {}", handler.name, first, second);
}
"#,
        );
    }

    #[test]
    fn moving_a_fn_value_into_a_struct_literal_is_a_move() {
        let message = expect_moveck_error(
            r#"
fn double(value: i64) -> i64 {
    value * 2
}

struct Handler {
    apply: fn(i64) -> i64,
}

fn main() {
    let apply: fn(i64) -> i64 = double;
    let handler = Handler { apply: apply };
    let value = apply(1);
    println!("{} {}", value, (handler.apply)(2));
}
"#,
            "E0382",
        );
        assert!(
            message.contains("apply"),
            "E0382 should name the moved function value: {}",
            message
        );
    }
}

// ---------------------------------------------------------------------------
// Hand built HIR tests for the v1.5 constructs
// ---------------------------------------------------------------------------
//
// The v1.5 constructs (user iterators and the HashMap builtin) are lowered to
// HIR by the type checker, which is developed in parallel with this pass. The
// tests below build the HIR directly so the move rules for the new nodes are
// pinned independently of the front end, mirroring the approach of the code
// generation reference tests. The types attached to the hand built nodes are
// only the ones the move checker reads: whether a value is Copy and, for
// ForIter, that the iterator value is an owned value. next_fn is opaque here;
// only the code generator uses it.
#[cfg(test)]
mod hir_tests {
    use super::check;
    use crate::diag::Diagnostics;
    use crate::hir::{
        HArm, HBlock, HBuiltinMethod, HExpr, HExprKind, HField, HFieldRef, HFn, HLit, HPat,
        HPatKind, HStmt, HStruct, Program,
    };
    use crate::span::Span;
    use crate::types::{IntTy, SelfKind, Ty};

    fn sp() -> Span {
        Span { start: 0, end: 0 }
    }

    fn messages(diagnostics: &Diagnostics) -> String {
        diagnostics
            .as_slice()
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn expect_code<'a>(diagnostics: &'a Diagnostics, code: &str) -> &'a crate::diag::Diagnostic {
        diagnostics
            .as_slice()
            .iter()
            .find(|diagnostic| diagnostic.code == Some(code))
            .unwrap_or_else(|| {
                panic!(
                    "expected a {} diagnostic, got: {}",
                    code,
                    messages(diagnostics)
                )
            })
    }

    // -- program construction -------------------------------------------

    fn local(name: &str, ty: Ty) -> HExpr {
        HExpr::new(
            ty,
            HExprKind::Local {
                name: name.to_string(),
                c_name: name.to_string(),
            },
            sp(),
        )
    }

    fn int_lit(value: i128) -> HExpr {
        HExpr::new(
            Ty::i64(),
            HExprKind::Lit(HLit::Int {
                value,
                int_ty: IntTy::I64,
            }),
            sp(),
        )
    }

    fn str_lit(value: &str) -> HExpr {
        HExpr::new(
            Ty::String,
            HExprKind::Lit(HLit::Str(value.to_string())),
            sp(),
        )
    }

    fn pat_bind(name: &str, ty: Ty) -> HPat {
        HPat {
            ty,
            kind: HPatKind::Bind {
                c_name: name.to_string(),
                name: name.to_string(),
            },
            span: sp(),
        }
    }

    fn let_stmt(name: &str, init: HExpr) -> HStmt {
        HStmt::Let {
            pat: pat_bind(name, init.ty.clone()),
            init: Some(init),
            span: sp(),
        }
    }

    fn expr_stmt(expr: HExpr) -> HStmt {
        HStmt::Expr(expr)
    }

    fn block(stmts: Vec<HStmt>) -> HBlock {
        HBlock {
            stmts,
            tail: None,
            span: sp(),
        }
    }

    fn main_fn(body: HBlock) -> HFn {
        HFn {
            name: "main".to_string(),
            c_name: "lsaca_fn_main".to_string(),
            params: Vec::new(),
            ret: Ty::unit(),
            self_kind: None,
            body,
            span: sp(),
        }
    }

    fn base_program(fns: Vec<HFn>, structs: Vec<HStruct>) -> Program {
        let mut program = Program {
            name: "test".to_string(),
            structs,
            enums: Vec::new(),
            fns,
            consts: Vec::new(),
            closures: Vec::new(),
            main_fn: 0,
            copy_structs: Vec::new(),
            copy_enums: Vec::new(),
            drop_structs: Vec::new(),
            drop_enums: Vec::new(),
        };
        program.classify_types(Vec::new(), Vec::new());
        program
    }

    fn program(body: HBlock, structs: Vec<HStruct>) -> Program {
        base_program(vec![main_fn(body)], structs)
    }

    fn run(body: HBlock, structs: Vec<HStruct>) -> Diagnostics {
        check(&program(body, structs))
    }

    fn accept(body: HBlock, structs: Vec<HStruct>) {
        let diagnostics = run(body, structs);
        assert!(
            !diagnostics.has_errors(),
            "move checker rejected valid HIR: {}",
            messages(&diagnostics)
        );
    }

    // -- user iterators --------------------------------------------------

    const ITER_STRUCT: usize = 0;

    /// A value of the hand written iterator struct: it only needs to be a
    /// non-Copy owned value so the move checker records the move of the
    /// iterator expression.
    fn iterator_type() -> Ty {
        Ty::Struct(ITER_STRUCT)
    }

    fn iterator_struct() -> HStruct {
        HStruct {
            name: "VecIter".to_string(),
            c_name: "VecIter".to_string(),
            fields: vec![HField {
                name: "items".to_string(),
                ty: Ty::vec(Ty::i64()),
            }],
            span: sp(),
        }
    }

    fn iterator_value() -> HExpr {
        HExpr::new(
            iterator_type(),
            HExprKind::StructLit {
                struct_id: ITER_STRUCT,
                fields: vec![(
                    0,
                    HExpr::new(
                        Ty::vec(Ty::i64()),
                        HExprKind::VecLit {
                            elems: vec![int_lit(1)],
                        },
                        sp(),
                    ),
                )],
            },
            sp(),
        )
    }

    fn for_iter(iter: HExpr, item_name: &str, item_ty: Ty, body: HBlock) -> HExpr {
        HExpr::new(
            Ty::unit(),
            HExprKind::ForIter {
                iter: Box::new(iter),
                next_fn: 0,
                pat: pat_bind(item_name, item_ty),
                body,
            },
            sp(),
        )
    }

    #[test]
    fn for_iter_consumes_the_iterator() {
        let body = block(vec![
            let_stmt("iter", iterator_value()),
            expr_stmt(for_iter(
                local("iter", iterator_type()),
                "item",
                Ty::i64(),
                block(Vec::new()),
            )),
            expr_stmt(local("iter", iterator_type())),
        ]);
        let diagnostics = run(body, vec![iterator_struct()]);
        let diagnostic = expect_code(&diagnostics, "E0382");
        assert_eq!(diagnostic.message, "use of moved value: iter");
        assert!(!diagnostic.notes.is_empty(), "E0382 should carry a note");
    }

    #[test]
    fn for_iter_accepted_when_the_iterator_is_not_reused() {
        let body = block(vec![
            let_stmt("iter", iterator_value()),
            expr_stmt(for_iter(
                local("iter", iterator_type()),
                "item",
                Ty::i64(),
                block(Vec::new()),
            )),
        ]);
        accept(body, vec![iterator_struct()]);
    }

    #[test]
    fn for_iter_pattern_binding_is_fresh_each_iteration() {
        let body = block(vec![
            let_stmt("iter", iterator_value()),
            expr_stmt(for_iter(
                local("iter", iterator_type()),
                "item",
                Ty::String,
                block(vec![let_stmt("moved", local("item", Ty::String))]),
            )),
        ]);
        accept(body, vec![iterator_struct()]);
    }

    #[test]
    fn for_iter_reports_move_of_an_enclosing_local_on_the_next_iteration() {
        let body = block(vec![
            let_stmt("text", str_lit("hello")),
            let_stmt("iter", iterator_value()),
            expr_stmt(for_iter(
                local("iter", iterator_type()),
                "item",
                Ty::i64(),
                block(vec![let_stmt("moved", local("text", Ty::String))]),
            )),
        ]);
        let diagnostics = run(body, vec![iterator_struct()]);
        let diagnostic = expect_code(&diagnostics, "E0382");
        assert_eq!(diagnostic.message, "use of moved value: text");
    }

    // -- HashMap builtin -------------------------------------------------

    fn string_map() -> Ty {
        Ty::hash_map(Ty::String, Ty::i64())
    }

    fn hash_map_new(key: Ty, value: Ty) -> HExpr {
        HExpr::new(
            Ty::hash_map(key.clone(), value.clone()),
            HExprKind::BuiltinAssoc {
                func: crate::hir::HBuiltinAssoc::HashMapNew,
                args: Vec::new(),
                type_args: vec![key, value],
            },
            sp(),
        )
    }

    fn builtin_method(receiver: HExpr, method: HBuiltinMethod, args: Vec<HExpr>, ty: Ty) -> HExpr {
        HExpr::new(
            ty,
            HExprKind::BuiltinMethod {
                receiver: Box::new(receiver),
                method,
                args,
            },
            sp(),
        )
    }

    #[test]
    fn map_insert_consumes_the_key_and_the_value() {
        let map_ty = Ty::hash_map(Ty::String, Ty::String);
        let body = block(vec![
            let_stmt("map", hash_map_new(Ty::String, Ty::String)),
            let_stmt("key", str_lit("key")),
            let_stmt("value", str_lit("value")),
            expr_stmt(builtin_method(
                local("map", map_ty),
                HBuiltinMethod::MapInsert,
                vec![local("key", Ty::String), local("value", Ty::String)],
                Ty::option(Ty::String),
            )),
            expr_stmt(local("key", Ty::String)),
            expr_stmt(local("value", Ty::String)),
        ]);
        let diagnostics = run(body, Vec::new());
        let moved_arguments = diagnostics
            .as_slice()
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == Some("E0382")
                    && (diagnostic.message == "use of moved value: key"
                        || diagnostic.message == "use of moved value: value")
            })
            .count();
        assert_eq!(
            moved_arguments,
            2,
            "insert should move both the key and the value: {}",
            messages(&diagnostics)
        );
    }

    #[test]
    fn map_get_copies_the_key() {
        let map_ty = string_map();
        let body = block(vec![
            let_stmt("map", hash_map_new(Ty::String, Ty::i64())),
            let_stmt("key", str_lit("key")),
            expr_stmt(builtin_method(
                local("map", map_ty.clone()),
                HBuiltinMethod::MapGet,
                vec![local("key", Ty::String)],
                Ty::option(Ty::i64()),
            )),
            expr_stmt(local("map", map_ty)),
            expr_stmt(local("key", Ty::String)),
        ]);
        accept(body, Vec::new());
    }

    #[test]
    fn map_contains_key_and_remove_copy_the_key() {
        let map_ty = string_map();
        let body = block(vec![
            let_stmt("map", hash_map_new(Ty::String, Ty::i64())),
            let_stmt("key", str_lit("key")),
            expr_stmt(builtin_method(
                local("map", map_ty.clone()),
                HBuiltinMethod::MapContainsKey,
                vec![local("key", Ty::String)],
                Ty::Bool,
            )),
            expr_stmt(builtin_method(
                local("map", map_ty.clone()),
                HBuiltinMethod::MapRemove,
                vec![local("key", Ty::String)],
                Ty::option(Ty::i64()),
            )),
            expr_stmt(local("map", map_ty)),
            expr_stmt(local("key", Ty::String)),
        ]);
        accept(body, Vec::new());
    }

    fn callback_type() -> Ty {
        Ty::function(Vec::new(), Ty::i64())
    }

    fn fn_ref() -> HExpr {
        HExpr::new(callback_type(), HExprKind::FnRef { fn_id: 0 }, sp())
    }

    #[test]
    fn map_get_with_moves_the_closure_but_copies_the_key() {
        let map_ty = string_map();
        let callback = callback_type();
        let body = block(vec![
            let_stmt("map", hash_map_new(Ty::String, Ty::i64())),
            let_stmt("key", str_lit("key")),
            let_stmt("callback", fn_ref()),
            expr_stmt(builtin_method(
                local("map", map_ty.clone()),
                HBuiltinMethod::MapGetWith,
                vec![
                    local("key", Ty::String),
                    local("callback", callback.clone()),
                ],
                Ty::option(Ty::i64()),
            )),
            expr_stmt(local("key", Ty::String)),
            expr_stmt(local("callback", callback)),
        ]);
        let diagnostics = run(body, Vec::new());
        let diagnostic = expect_code(&diagnostics, "E0382");
        assert_eq!(diagnostic.message, "use of moved value: callback");
        let key_was_moved = diagnostics
            .as_slice()
            .iter()
            .any(|diagnostic| diagnostic.message == "use of moved value: key");
        assert!(!key_was_moved, "get_with should copy the key");
    }

    #[test]
    fn map_methods_borrow_the_receiver() {
        let map_ty = string_map();
        let body = block(vec![
            let_stmt("map", hash_map_new(Ty::String, Ty::i64())),
            let_stmt("key", str_lit("key")),
            expr_stmt(builtin_method(
                local("map", map_ty.clone()),
                HBuiltinMethod::MapInsert,
                vec![local("key", Ty::String), int_lit(1)],
                Ty::option(Ty::i64()),
            )),
            expr_stmt(builtin_method(
                local("map", map_ty.clone()),
                HBuiltinMethod::MapLen,
                Vec::new(),
                Ty::i64(),
            )),
            expr_stmt(builtin_method(
                local("map", map_ty.clone()),
                HBuiltinMethod::MapIsEmpty,
                Vec::new(),
                Ty::Bool,
            )),
            expr_stmt(builtin_method(
                local("map", map_ty.clone()),
                HBuiltinMethod::MapClear,
                Vec::new(),
                Ty::unit(),
            )),
            expr_stmt(builtin_method(
                local("map", map_ty.clone()),
                HBuiltinMethod::MapKeys,
                Vec::new(),
                Ty::vec(Ty::String),
            )),
            expr_stmt(builtin_method(
                local("map", map_ty.clone()),
                HBuiltinMethod::MapValues,
                Vec::new(),
                Ty::vec(Ty::i64()),
            )),
            expr_stmt(local("map", map_ty)),
        ]);
        accept(body, Vec::new());
    }

    #[test]
    fn map_use_after_move_reports_e0382_and_names_the_map_type() {
        let map_ty = string_map();
        let body = block(vec![
            let_stmt("map", hash_map_new(Ty::String, Ty::i64())),
            let_stmt("moved", local("map", map_ty.clone())),
            expr_stmt(builtin_method(
                local("map", map_ty),
                HBuiltinMethod::MapLen,
                Vec::new(),
                Ty::i64(),
            )),
        ]);
        let diagnostics = run(body, Vec::new());
        let diagnostic = expect_code(&diagnostics, "E0382");
        assert_eq!(diagnostic.message, "use of moved value: map");
        assert!(
            diagnostic
                .notes
                .iter()
                .any(|note| note.contains("HashMap<String, i64>")),
            "E0382 should name the HashMap type: {:?}",
            diagnostic.notes
        );
        assert!(
            !diagnostic.notes.iter().any(|note| note.contains("struct#")),
            "E0382 should not leak raw type ids: {:?}",
            diagnostic.notes
        );
    }

    #[test]
    fn hash_map_is_never_copy() {
        let program = program(block(Vec::new()), Vec::new());
        let map_ty = string_map();
        assert!(!program.is_copy(&map_ty), "HashMap is never Copy");
        assert!(
            !program.is_copy(&Ty::option(map_ty.clone())),
            "a compound type containing a HashMap is never Copy either"
        );
        assert!(
            !program.is_copy(&Ty::vec(map_ty)),
            "a Vec of HashMaps is never Copy either"
        );
    }

    const OWNED_STRUCT: usize = 0;

    fn owned_struct() -> HStruct {
        HStruct {
            name: "Owned".to_string(),
            c_name: "Owned".to_string(),
            fields: vec![HField {
                name: "text".to_string(),
                ty: Ty::String,
            }],
            span: sp(),
        }
    }

    #[test]
    fn hash_map_with_a_drop_type_needs_drop_glue() {
        // Drop::drop on the mapped value means a copy of the map would run the
        // hook twice, so the drop glue must propagate through HashMap. (An
        // owning String payload alone does not: the v1.3 clone semantics give
        // it a real deep copy, which is why a HashMap<String, i64> is move only
        // but not drop-glue-bearing.)
        let owned_ty = Ty::Struct(OWNED_STRUCT);
        let map_ty = Ty::hash_map(Ty::String, owned_ty.clone());
        let mut program = program(block(Vec::new()), vec![owned_struct()]);
        program.classify_types(vec![Some(0)], Vec::new());
        assert!(!program.is_copy(&map_ty), "HashMap is never Copy");
        assert!(
            program.has_drop_glue(&map_ty),
            "the drop glue must reach a mapped value that implements Drop"
        );
        assert!(
            program.has_drop_glue(&Ty::option(map_ty.clone())),
            "the drop glue must pass through Option"
        );
        assert!(
            program.has_drop_glue(&Ty::vec(map_ty)),
            "the drop glue must pass through Vec"
        );
    }

    // -- function values stored in aggregates ----------------------------

    const NODE_STRUCT: usize = 0;
    const HOLDER_STRUCT: usize = 1;

    fn node_struct() -> HStruct {
        HStruct {
            name: "Node".to_string(),
            c_name: "Node".to_string(),
            fields: vec![HField {
                name: "text".to_string(),
                ty: Ty::String,
            }],
            span: sp(),
        }
    }

    fn map_holder_struct() -> HStruct {
        HStruct {
            name: "Holder".to_string(),
            c_name: "Holder".to_string(),
            fields: vec![HField {
                name: "entries".to_string(),
                ty: Ty::hash_map(Ty::String, Ty::Struct(NODE_STRUCT)),
            }],
            span: sp(),
        }
    }

    #[test]
    fn moving_a_hash_map_field_names_nested_user_types() {
        let holder_ty = Ty::Struct(HOLDER_STRUCT);
        let entries_ty = Ty::hash_map(Ty::String, Ty::Struct(NODE_STRUCT));
        let body = block(vec![
            let_stmt(
                "holder",
                HExpr::new(
                    holder_ty.clone(),
                    HExprKind::StructLit {
                        struct_id: HOLDER_STRUCT,
                        fields: vec![(0, hash_map_new(Ty::String, Ty::Struct(NODE_STRUCT)))],
                    },
                    sp(),
                ),
            ),
            let_stmt(
                "entries",
                HExpr::new(
                    entries_ty,
                    HExprKind::Field {
                        base: Box::new(local("holder", holder_ty)),
                        field: HFieldRef::Struct {
                            struct_id: HOLDER_STRUCT,
                            index: 0,
                        },
                    },
                    sp(),
                ),
            ),
        ]);
        let diagnostics = run(body, vec![node_struct(), map_holder_struct()]);
        let diagnostic = expect_code(&diagnostics, "E0507");
        assert!(
            diagnostic.message.contains("HashMap<String, Node>"),
            "E0507 should name nested user types: {}",
            diagnostic.message
        );
        assert!(
            !diagnostic.message.contains("struct#"),
            "E0507 should not leak raw type ids: {}",
            diagnostic.message
        );
    }

    const CALLBACK_STRUCT: usize = 0;

    fn callback_holder_struct() -> HStruct {
        HStruct {
            name: "CallbackHolder".to_string(),
            c_name: "CallbackHolder".to_string(),
            fields: vec![HField {
                name: "callback".to_string(),
                ty: callback_type(),
            }],
            span: sp(),
        }
    }

    fn callback_holder_value() -> HExpr {
        HExpr::new(
            Ty::Struct(CALLBACK_STRUCT),
            HExprKind::StructLit {
                struct_id: CALLBACK_STRUCT,
                fields: vec![(0, local("callback", callback_type()))],
            },
            sp(),
        )
    }

    fn callback_field(holder: HExpr) -> HExpr {
        HExpr::new(
            callback_type(),
            HExprKind::Field {
                base: Box::new(holder),
                field: HFieldRef::Struct {
                    struct_id: CALLBACK_STRUCT,
                    index: 0,
                },
            },
            sp(),
        )
    }

    fn call(callee: HExpr, args: Vec<HExpr>) -> HExpr {
        let ret = match &callee.ty {
            Ty::Fn(_, ret) => ret.as_ref().clone(),
            _ => Ty::unit(),
        };
        HExpr::new(
            ret,
            HExprKind::Call {
                callee: Box::new(callee),
                args,
            },
            sp(),
        )
    }

    #[test]
    fn fn_value_used_as_a_struct_literal_field_is_moved() {
        let body = block(vec![
            let_stmt("callback", fn_ref()),
            let_stmt("holder", callback_holder_value()),
            expr_stmt(local("callback", callback_type())),
        ]);
        let diagnostics = run(body, vec![callback_holder_struct()]);
        let diagnostic = expect_code(&diagnostics, "E0382");
        assert_eq!(diagnostic.message, "use of moved value: callback");
    }

    #[test]
    fn calling_a_fn_field_twice_only_borrows_the_field() {
        let body = block(vec![
            let_stmt("callback", fn_ref()),
            let_stmt("holder", callback_holder_value()),
            expr_stmt(call(
                callback_field(local("holder", Ty::Struct(CALLBACK_STRUCT))),
                Vec::new(),
            )),
            expr_stmt(call(
                callback_field(local("holder", Ty::Struct(CALLBACK_STRUCT))),
                Vec::new(),
            )),
        ]);
        accept(body, vec![callback_holder_struct()]);
    }

    #[test]
    fn moving_a_fn_field_out_is_e0507() {
        let body = block(vec![
            let_stmt("callback", fn_ref()),
            let_stmt("holder", callback_holder_value()),
            let_stmt(
                "moved",
                callback_field(local("holder", Ty::Struct(CALLBACK_STRUCT))),
            ),
        ]);
        let diagnostics = run(body, vec![callback_holder_struct()]);
        let diagnostic = expect_code(&diagnostics, "E0507");
        assert!(
            diagnostic.message.contains("fn() -> i64"),
            "E0507 should name the function type: {}",
            diagnostic.message
        );
    }

    // -- borrow then move -------------------------------------------------

    fn fn_ref_with(params: Vec<Ty>, ret: Ty) -> HExpr {
        HExpr::new(
            Ty::function(params, ret),
            HExprKind::FnRef { fn_id: 0 },
            sp(),
        )
    }

    fn borrow(expr: HExpr) -> HExpr {
        let inner = expr.ty.clone();
        HExpr::new(
            Ty::ref_to(inner),
            HExprKind::Ref {
                mutable: false,
                expr: Box::new(expr),
            },
            sp(),
        )
    }

    fn return_some(expr: HExpr) -> HExpr {
        let payload = HExpr::new(
            Ty::option(Ty::String),
            HExprKind::OptionSome(Box::new(expr)),
            sp(),
        );
        HExpr::new(Ty::unit(), HExprKind::Return(Some(Box::new(payload))), sp())
    }

    #[test]
    fn borrow_then_move_in_a_straight_line_is_accepted() {
        // The v1.5 prelude takes a shared reference to an element and then
        // moves the element when it is kept (filter/find). A borrow does not
        // move the value, so the later move is legal.
        let body = block(vec![
            let_stmt("item", str_lit("hello")),
            let_stmt("borrowed", borrow(local("item", Ty::String))),
            let_stmt("moved", local("item", Ty::String)),
        ]);
        accept(body, Vec::new());
    }

    #[test]
    fn borrow_then_move_through_a_predicate_call_is_accepted() {
        // This is the body of filter/find: if (predicate)(&item) { return
        // Some(item); } The predicate only borrows the item, and moving the
        // item in the taken branch is not a use of a borrowed value.
        let predicate_ty = Ty::function(vec![Ty::ref_to(Ty::String)], Ty::Bool);
        let condition = call(
            local("predicate", predicate_ty),
            vec![borrow(local("item", Ty::String))],
        );
        let body = block(vec![
            let_stmt("item", str_lit("hello")),
            let_stmt(
                "predicate",
                fn_ref_with(vec![Ty::ref_to(Ty::String)], Ty::Bool),
            ),
            expr_stmt(HExpr::new(
                Ty::unit(),
                HExprKind::If {
                    cond: Box::new(condition),
                    then_block: block(vec![expr_stmt(return_some(local("item", Ty::String)))]),
                    else_expr: None,
                },
                sp(),
            )),
        ]);
        accept(body, Vec::new());
    }

    #[test]
    fn calling_a_function_value_in_a_loop_does_not_consume_it() {
        // The prelude calls a stored function value once per iteration
        // ((self.f)(item)); the callee is read, so the value stays live.
        let consumer_ty = Ty::function(vec![Ty::String], Ty::i64());
        let body = block(vec![
            let_stmt("iter", iterator_value()),
            let_stmt("callback", fn_ref_with(vec![Ty::String], Ty::i64())),
            expr_stmt(for_iter(
                local("iter", iterator_type()),
                "item",
                Ty::String,
                block(vec![expr_stmt(call(
                    local("callback", consumer_ty.clone()),
                    vec![local("item", Ty::String)],
                ))]),
            )),
            expr_stmt(local("callback", consumer_ty)),
        ]);
        accept(body, vec![iterator_struct()]);
    }

    // -- prelude shapes ---------------------------------------------------

    const FILTER_STRUCT: usize = 0;

    fn filter_predicate() -> Ty {
        Ty::function(vec![Ty::ref_to(Ty::String)], Ty::Bool)
    }

    fn filter_struct() -> HStruct {
        HStruct {
            name: "FilterIter".to_string(),
            c_name: "FilterIter".to_string(),
            fields: vec![
                HField {
                    name: "inner".to_string(),
                    ty: Ty::vec(Ty::String),
                },
                HField {
                    name: "pred".to_string(),
                    ty: filter_predicate(),
                },
            ],
            span: sp(),
        }
    }

    fn self_field(index: usize, ty: Ty) -> HExpr {
        HExpr::new(
            ty,
            HExprKind::Field {
                base: Box::new(local("self", Ty::ref_to(Ty::Struct(FILTER_STRUCT)))),
                field: HFieldRef::Struct {
                    struct_id: FILTER_STRUCT,
                    index,
                },
            },
            sp(),
        )
    }

    fn return_none() -> HExpr {
        HExpr::new(
            Ty::unit(),
            HExprKind::Return(Some(Box::new(HExpr::new(
                Ty::option(Ty::String),
                HExprKind::OptionNone,
                sp(),
            )))),
            sp(),
        )
    }

    #[test]
    fn filter_iterator_next_body_shape_is_accepted() {
        // Hand built HIR of __lsaca_FilterIter::next from
        // crates/lsaca/src/prelude.ls:
        //
        // loop {
        //     match self.inner.next() {
        //         Some(item) => { if (self.pred)(&item) { return Some(item); } }
        //         None => { return None; }
        //     }
        // }
        //
        // The predicate only borrows the item and the move in the taken
        // branch is the last use, so the prelude body must be accepted.
        let pulled = HExpr::new(
            Ty::option(Ty::String),
            HExprKind::DirectCall {
                fn_id: 1,
                receiver: Some(Box::new(self_field(0, Ty::ref_to(Ty::vec(Ty::String))))),
                args: Vec::new(),
            },
            sp(),
        );
        let condition = call(
            self_field(1, filter_predicate()),
            vec![borrow(local("item", Ty::String))],
        );
        let some_arm = HArm {
            pat: HPat {
                ty: Ty::option(Ty::String),
                kind: HPatKind::OptionSome(Box::new(pat_bind("item", Ty::String))),
                span: sp(),
            },
            guard: None,
            body: HExpr::new(
                Ty::unit(),
                HExprKind::If {
                    cond: Box::new(condition),
                    then_block: block(vec![expr_stmt(return_some(local("item", Ty::String)))]),
                    else_expr: None,
                },
                sp(),
            ),
        };
        let none_arm = HArm {
            pat: HPat {
                ty: Ty::option(Ty::String),
                kind: HPatKind::OptionNone,
                span: sp(),
            },
            guard: None,
            body: return_none(),
        };
        let match_expr = HExpr::new(
            Ty::unit(),
            HExprKind::Match {
                scrutinee: Box::new(pulled),
                arms: vec![some_arm, none_arm],
            },
            sp(),
        );
        let loop_expr = HExpr::new(
            Ty::unit(),
            HExprKind::Loop {
                body: block(vec![expr_stmt(match_expr)]),
            },
            sp(),
        );
        let next = HFn {
            name: "__lsaca_FilterIter::next".to_string(),
            c_name: "lsaca_fn___lsaca_FilterIter_next".to_string(),
            params: Vec::new(),
            ret: Ty::option(Ty::String),
            self_kind: Some(SelfKind::RefMut),
            body: block(vec![expr_stmt(loop_expr)]),
            span: sp(),
        };
        let program = base_program(
            vec![main_fn(block(Vec::new())), next],
            vec![filter_struct()],
        );
        let diagnostics = check(&program);
        assert!(
            !diagnostics.has_errors(),
            "prelude FilterIter::next shape was rejected: {}",
            messages(&diagnostics)
        );
    }
}
