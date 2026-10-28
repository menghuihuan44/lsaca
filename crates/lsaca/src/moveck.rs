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

/// Fully recursive display name of a type.
///
/// `Program::type_name` resolves user type ids for Vec, Option, Result, Tuple
/// and Fn, but it defers Rc, Box and references to `Ty`'s Display impl, which
/// cannot resolve user type ids and prints struct#0. The move checker's notes
/// and messages need source level names at every nesting depth, so this wrapper
/// recurses through those constructors itself.
fn describe_type(program: &hir::Program, ty: &Ty) -> String {
    match ty {
        Ty::Struct(id) => program.struct_name(*id).to_string(),
        Ty::Enum(id) => program.enum_name(*id).to_string(),
        Ty::Vec(inner) => format!("Vec<{}>", describe_type(program, inner)),
        Ty::Rc(inner) => format!("Rc<{}>", describe_type(program, inner)),
        Ty::Box(inner) => format!("Box<{}>", describe_type(program, inner)),
        Ty::Option(inner) => format!("Option<{}>", describe_type(program, inner)),
        Ty::Result(ok, err) => format!(
            "Result<{}, {}>",
            describe_type(program, ok),
            describe_type(program, err)
        ),
        Ty::Tuple(elems) => {
            let inner = elems
                .iter()
                .map(|elem| describe_type(program, elem))
                .collect::<Vec<_>>()
                .join(", ");
            format!("({})", inner)
        }
        Ty::Ref { mutable, inner } => {
            if *mutable {
                format!("&mut {}", describe_type(program, inner))
            } else {
                format!("&{}", describe_type(program, inner))
            }
        }
        Ty::Fn(params, ret) => {
            let params = params
                .iter()
                .map(|param| describe_type(program, param))
                .collect::<Vec<_>>()
                .join(", ");
            format!("fn({}) -> {}", params, describe_type(program, ret))
        }
        other => format!("{}", other),
    }
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
        let type_name = describe_type(self.program, &moved.ty);
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
        let type_name = describe_type(self.program, ty);
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
}
