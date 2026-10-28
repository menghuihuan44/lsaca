//! Core of the C emitter: the per-program generator state, the ownership
//! protocol and the expression/statement dispatch.
//!
//! Ownership protocol summary:
//!
//! * every C variable that owns a managed value is initialised with the
//!   zero value of its type, so that releasing it is always safe;
//! * `gen_into(expr, target)` leaves an owned value in the (fresh) lvalue
//!   `target`;
//! * `gen_owned(expr)` allocates a new temporary for the value and returns
//!   the temporary name; temporaries are released at the end of the HIR
//!   statement in which they were created, unless they were moved;
//! * `gen_place(expr)` returns the C lvalue of a place expression;
//! * since v1.4 `program.is_copy(ty)` decides between a shallow C copy
//!   (Copy types: no cleanup) and managed ownership. Every managed variable
//!   carries a trailing `bool <c_name>_live` flag; moving the value out
//!   clears both the variable and the flag, and every release is guarded by
//!   the flag;
//! * consuming contexts use `gen_move_into`, which transfers a movable place
//!   (local/capture) and copies a non-movable one (`Field`/`Index`/`Deref`
//!   are rejected by the move checker as E0507, so only a defensive copy is
//!   emitted for them);
//! * borrowed contexts (format/assert/equality operands, `&` expressions)
//!   read places without taking ownership via `gen_borrowed`; non-place
//!   subexpressions become an owned temporary that is released once at the end
//!   of the enclosing statement;
//! * pattern bindings move out of the scrutinee temporary (copying a Copy
//!   value) so that the temporary never releases the transferred value.

use crate::hir::{self, HExpr, HExprKind, HLit};
use crate::span::Span;
use crate::types::Ty;

use super::CodegenOptions;
use super::c_writer::CWriter;
use super::ops;
use super::types::{TypeNames, TypeRegistry};
use super::util;

/// A named local variable declared in a scope. Managed variables are owned
/// and released when the scope is left.
#[derive(Clone)]
pub(crate) struct BoundVar {
    pub c_name: String,
    pub ty: Ty,
    pub owned: bool,
    /// C `bool` flag that stays true while this variable holds a live owned
    /// value. `None` for Copy values, which never need a release.
    ///
    /// A move clears the C variable together with the flag, so every release
    /// path can be guarded by the flag and a moved-from variable can never be
    /// released twice.
    pub live: Option<String>,
}

/// A compiler temporary holding an owned value. Released at the end of the
/// current HIR statement unless it has been moved into another location.
#[derive(Clone)]
pub(crate) struct Temp {
    pub c_name: String,
    pub ty: Ty,
    pub moved: bool,
}

/// One C lexical scope.
#[derive(Default)]
pub(crate) struct Frame {
    pub vars: Vec<BoundVar>,
    pub temps: Vec<Temp>,
}

/// Bookkeeping for the loop whose body is currently generated.
pub(crate) struct LoopCtx {
    pub break_label: String,
    pub continue_label: String,
    pub body_frame: usize,
    pub used_break: bool,
    pub used_continue: bool,
}

/// The environment of the closure whose body is currently generated.
pub(crate) struct EnvInfo {
    pub c_type: String,
    pub var: String,
}

/// State of one generated C program.
pub(crate) struct Gen<'a> {
    pub program: &'a hir::Program,
    pub registry: &'a TypeRegistry,
    pub options: &'a CodegenOptions,
    pub names: &'a TypeNames,
    pub w: CWriter,
    pub tmp: usize,
    pub label: usize,
    pub frames: Vec<Frame>,
    pub loops: Vec<LoopCtx>,
    pub fn_ret: Ty,
    pub env: Option<EnvInfo>,
    pub block_terminated: bool,
    pub fn_terminated: bool,
    pub debug_checks: bool,
}

impl<'a> Gen<'a> {
    pub(crate) fn new(
        program: &'a hir::Program,
        registry: &'a TypeRegistry,
        options: &'a CodegenOptions,
        w: CWriter,
    ) -> Gen<'a> {
        Gen {
            program,
            registry,
            options,
            names: registry.names(),
            w,
            tmp: 0,
            label: 0,
            frames: Vec::new(),
            loops: Vec::new(),
            fn_ret: Ty::Unit,
            env: None,
            block_terminated: false,
            fn_terminated: false,
            debug_checks: options.debug_checks,
        }
    }

    // ----- names and helpers ------------------------------------------------

    pub(crate) fn ctype(&self, ty: &Ty) -> String {
        self.registry.ctype(ty)
    }

    pub(crate) fn zero_expr(&self, ty: &Ty) -> String {
        self.names.zero_expr(self.program, ty)
    }

    /// A cheap zero initialiser for compiler temporaries: unlike `zero_expr`
    /// it never allocates, so overwriting the temporary with its real value
    /// cannot leak. NULL strings and zeroed aggregates are still safe to
    /// release.
    pub(crate) fn raw_zero_expr(&self, ty: &Ty) -> String {
        match ty {
            Ty::Unit | Ty::Bool | Ty::Int(_) | Ty::Float(_) => self.zero_expr(ty),
            Ty::String => String::from("NULL"),
            _ => format!("(({}){{0}})", self.ctype(ty)),
        }
    }

    pub(crate) fn fresh(&mut self, prefix: &str) -> String {
        self.tmp += 1;
        format!("{}{}", prefix, self.tmp)
    }

    pub(crate) fn fresh_label(&mut self, prefix: &str) -> String {
        self.label += 1;
        format!("lsaca_{}_{}", prefix, self.label)
    }

    pub(crate) fn line(&mut self, text: impl AsRef<str>) {
        self.w.line(text);
    }

    /// Source location arguments for a runtime check or panic call.
    pub(crate) fn loc(&self, span: Span) -> String {
        if !self.options.emit_location_info {
            return "NULL, 0, 0".to_string();
        }
        let (file, line, col) =
            util::source_location(self.options.source.as_ref(), &self.program.name, span);
        format!("{}, {}, {}", file, line, col)
    }

    // ----- temporaries and scopes ------------------------------------------

    pub(crate) fn declare_temp(&mut self, ty: &Ty) -> String {
        let ctype = self.ctype(ty);
        let zero = self.raw_zero_expr(ty);
        let name = self.fresh("t");
        self.w.line(format!("{} {} = {};", ctype, name, zero));
        if let Some(frame) = self.frames.last_mut() {
            frame.temps.push(Temp {
                c_name: name.clone(),
                ty: ty.clone(),
                moved: false,
            });
        }
        name
    }

    pub(crate) fn new_plain(&mut self, ty: &Ty, prefix: &str) -> String {
        let ctype = self.ctype(ty);
        let zero = self.raw_zero_expr(ty);
        let name = self.fresh(prefix);
        self.w.line(format!("{} {} = {};", ctype, name, zero));
        name
    }

    pub(crate) fn new_plain_of(&mut self, ctype: &str, prefix: &str) -> String {
        let name = self.fresh(prefix);
        self.w.line(format!("{} {} = 0;", ctype, name));
        name
    }

    pub(crate) fn mark_moved(&mut self, name: &str) {
        for frame in self.frames.iter_mut().rev() {
            for temp in frame.temps.iter_mut().rev() {
                if temp.c_name == name {
                    temp.moved = true;
                    return;
                }
            }
        }
    }

    /// Liveness flag of a local binding, if it owns a non-Copy value.
    pub(crate) fn live_flag(&self, c_name: &str) -> Option<String> {
        for frame in self.frames.iter().rev() {
            for var in frame.vars.iter().rev() {
                if var.c_name == c_name {
                    return var.live.clone();
                }
            }
        }
        None
    }

    /// Set the liveness flag of a local binding, if it has one.
    pub(crate) fn set_live(&mut self, c_name: &str, live: bool) {
        if let Some(flag) = self.live_flag(c_name) {
            let value = if live { "true" } else { "false" };
            self.w.line(format!("{} = {};", flag, value));
        }
    }

    pub(crate) fn push_frame(&mut self) {
        self.frames.push(Frame::default());
    }

    pub(crate) fn frame_index(&self) -> usize {
        self.frames.len().saturating_sub(1)
    }

    pub(crate) fn pop_frame(&mut self) {
        self.frames.pop();
    }

    /// Release the temps and owned variables of one frame (inner first).
    pub(crate) fn frame_cleanup(&mut self, index: usize) {
        if index >= self.frames.len() {
            return;
        }
        let temps: Vec<Temp> = std::mem::take(&mut self.frames[index].temps);
        for temp in temps.iter().rev() {
            if temp.moved {
                continue;
            }
            self.emit_release(&temp.ty, &temp.c_name);
        }
        let vars: Vec<BoundVar> = std::mem::take(&mut self.frames[index].vars);
        for var in vars.iter().rev() {
            self.release_bound(var);
        }
    }

    /// Release the value owned by one bound variable, honouring its liveness
    /// flag so that a moved-from variable is skipped.
    fn release_bound(&mut self, var: &BoundVar) {
        if !var.owned {
            return;
        }
        self.emit_release_guarded(&var.ty, &var.c_name, var.live.as_deref());
    }

    /// Emit the releases of one frame without consuming its bookkeeping.
    ///
    /// Used by terminating statements (break/continue/return) which leave the
    /// scope from the middle of a nested block. The bookkeeping has to stay
    /// intact because the other branch of a conditional may still reach the
    /// normal scope exit, where the release must be emitted again.
    pub(crate) fn emit_frame_release(&mut self, index: usize) {
        if index >= self.frames.len() {
            return;
        }
        let temps = self.frames[index].temps.clone();
        for temp in temps.iter().rev() {
            if temp.moved {
                continue;
            }
            self.emit_release(&temp.ty, &temp.c_name);
        }
        let vars = self.frames[index].vars.clone();
        for var in vars.iter().rev() {
            self.release_bound(var);
        }
    }

    /// Release the temporaries created by the statement currently generated.
    pub(crate) fn stmt_cleanup(&mut self) {
        if let Some(index) = self.frames.len().checked_sub(1) {
            let temps: Vec<Temp> = std::mem::take(&mut self.frames[index].temps);
            for temp in temps.iter().rev() {
                if temp.moved {
                    continue;
                }
                self.emit_release(&temp.ty, &temp.c_name);
            }
        }
    }

    // ----- value level operations ------------------------------------------

    pub(crate) fn emit_release(&mut self, ty: &Ty, expr: &str) {
        if self.is_trivial_copy(ty) {
            return;
        }
        if let Some(stmt) = ops::release_stmt(self.names, ty, expr) {
            self.w.line(stmt);
        }
    }

    /// Release an owned value, guarded by its C liveness flag when it has one.
    ///
    /// Moved-from variables are cleared and their flag set to false, so the
    /// guarded release is a no-op for them.
    pub(crate) fn emit_release_guarded(&mut self, ty: &Ty, expr: &str, flag: Option<&str>) {
        if self.is_trivial_copy(ty) {
            return;
        }
        let Some(stmt) = ops::release_stmt(self.names, ty, expr) else {
            return;
        };
        match flag {
            Some(flag) => {
                self.w.line(format!("if ({}) {{", flag));
                self.w.line(stmt);
                self.w.line("}");
            }
            None => self.w.line(stmt),
        }
    }

    /// `target = <copy of src>`; target and src must not alias.
    pub(crate) fn emit_copy_into(&mut self, target: &str, ty: &Ty, src: &str) {
        if self.is_trivial_copy(ty) {
            self.w.line(format!("{} = {};", target, src));
            return;
        }
        let value = ops::copy_expr(self.names, ty, src);
        self.w.line(format!("{} = {};", target, value));
    }

    /// True when values of this type are Copy: copying is a plain C
    /// assignment and releasing is a no-op. See [`hir::Program::is_copy`].
    pub(crate) fn is_trivial_copy(&self, ty: &Ty) -> bool {
        self.program.is_copy(ty)
    }

    /// Materialise `expr` into a fresh owned temporary and return its name.
    ///
    /// This is the copying/materialising form: a place expression is copied
    /// (retained) into the temporary. Use [`Gen::gen_owned_move`] in
    /// consuming contexts.
    pub(crate) fn gen_owned(&mut self, expr: &HExpr) -> String {
        let target = self.declare_temp(&expr.ty);
        self.gen_into(expr, &target);
        target
    }

    /// Materialise `expr` into a fresh owned temporary by moving its value.
    ///
    /// The temporary is released at the end of the current HIR statement
    /// unless it is moved on; in a discarded statement that is exactly the
    /// "move out and drop" semantics of a value expression.
    pub(crate) fn gen_owned_move(&mut self, expr: &HExpr) -> String {
        let target = self.declare_temp(&expr.ty);
        self.gen_move_into(expr, &target);
        target
    }

    /// Move `expr` into the owned lvalue `target`.
    ///
    /// Copy values are copied. Owned non-Copy locals and closure captures are
    /// transferred: the value is assigned to `target`, the source is cleared
    /// to its raw zero value and its liveness flag is cleared. Places that may
    /// not be moved out of (`Field`/`Index`/`Deref`) are copied defensively;
    /// the move checker rejects those moves with E0507 before codegen.
    pub(crate) fn gen_move_into(&mut self, expr: &HExpr, target: &str) {
        if self.is_trivial_copy(&expr.ty) {
            self.gen_into(expr, target);
            return;
        }
        match &expr.kind {
            HExprKind::Local { c_name, .. } => {
                let source = self.local_lvalue(c_name);
                self.w.line(format!("{} = {};", target, source));
                let zero = self.raw_zero_expr(&expr.ty);
                self.w.line(format!("{} = {};", source, zero));
                self.set_live(c_name, false);
            }
            HExprKind::Capture { .. } => {
                let source = self.gen_place(expr);
                self.emit_copy_into(target, &expr.ty, &source);
            }
            HExprKind::Field { .. } | HExprKind::Index { .. } => {
                let place = self.gen_place(expr);
                self.emit_copy_into(target, &expr.ty, &place);
            }
            HExprKind::Deref { .. } => {
                let place = self.gen_place(expr);
                let source = if self.is_pointer_type(&expr.ty) {
                    format!("(({})({}))", self.ctype(&expr.ty), place)
                } else {
                    place
                };
                self.emit_copy_into(target, &expr.ty, &source);
            }
            _ => self.gen_into(expr, target),
        }
    }

    /// Read `expr` in a borrowed context without taking ownership.
    ///
    /// Place expressions are read through their C lvalue; the display, debug
    /// and equality helpers take shallow values and never release, so no
    /// retain is needed. Any other expression is materialised into an owned
    /// temporary, which the statement cleanup releases exactly once.
    pub(crate) fn gen_borrowed(&mut self, expr: &HExpr) -> String {
        if expr.is_place() {
            return self.gen_place(expr);
        }
        self.gen_owned(expr)
    }

    /// The C expression that reads a local binding.
    ///
    /// The value of a local is its C variable: owned values are handles or
    /// plain scalars, reference-typed locals are pointers. A method receiver
    /// is no longer special-cased here: since v1.3 `self` has reference type
    /// in the HIR, so its C variable already holds the pointer and every
    /// auto-deref site (`place_through_reference`, `auto_deref_base`) peels
    /// the reference itself rather than the binding.
    pub(crate) fn local_lvalue(&self, c_name: &str) -> String {
        c_name.to_string()
    }

    /// Lvalue reached by dereferencing a reference expression: `(*(ptr))`.
    /// The pointer expression itself is materialised into a temporary.
    pub(crate) fn deref_lvalue(&mut self, expr: &HExpr) -> String {
        let pointer = self.gen_owned(expr);
        format!("(*({}))", pointer)
    }

    /// True when values of this type are represented by a C pointer that is
    /// handed around as-is: String, Rc, Box and the reference types.
    ///
    /// Values read through a shared reference may gain a const qualifier, so
    /// the sites that re-materialise such a value cast it back to its plain C
    /// type.
    pub(crate) fn is_pointer_type(&self, ty: &Ty) -> bool {
        self.ctype(ty).ends_with('*')
    }

    /// Emit `target = &<inner>` for a reference expression of type `ref_ty`.
    ///
    /// Three shapes are handled:
    /// * reborrow / `&mut T` -> `&T` coercion: the pointer value of the inner
    ///   expression is already the address of the pointee, so it is passed
    ///   through unchanged;
    /// * borrow of an `Rc<T>` / `Box<T>` payload cell (optionally through a
    ///   reference): the reference points at the payload inside the cell;
    /// * plain borrow of a place: the address of its lvalue.
    pub(crate) fn gen_ref(&mut self, inner: &HExpr, ref_ty: &Ty, target: &str) {
        let Some(pointee) = ref_ty.ref_inner().cloned() else {
            let place = self.gen_place(inner);
            self.w.line(format!("{} = &({});", target, place));
            return;
        };
        let ctype = self.ctype(ref_ty);
        if let Some(existing) = inner.ty.ref_inner()
            && *existing == pointee
        {
            let pointer = self.gen_owned(inner);
            self.w.line(format!("{} = ({}){};", target, ctype, pointer));
            return;
        }
        let place = self.gen_place(inner);
        if let Some(lvalue) = Self::payload_lvalue(place.clone(), &inner.ty, &pointee) {
            self.w
                .line(format!("{} = ({})&({});", target, ctype, lvalue));
            return;
        }
        self.w
            .line(format!("{} = ({})&({});", target, ctype, place));
    }

    /// The C lvalue of the `Rc`/`Box` payload reachable from an already
    /// materialised place by auto-dereferencing references and cell handles.
    ///
    /// `place` is the lvalue of the inner expression and `ty` its type, so
    /// the walk never evaluates the inner expression a second time (a borrow of
    /// an indexed place must not repeat its bounds check or side effects).
    /// Returns None when the walk does not end at an `Rc`/`Box` payload of
    /// type `payload`, so that a plain borrow of a cell handle itself stays a
    /// borrow of the handle.
    fn payload_lvalue(mut lvalue: String, ty: &Ty, payload: &Ty) -> Option<String> {
        let mut ty = ty.clone();
        let mut crossed_cell = false;
        for _ in 0..8 {
            match &ty {
                Ty::Ref { inner, .. } => {
                    lvalue = format!("(*({}))", lvalue);
                    ty = (**inner).clone();
                }
                Ty::Rc(inner) | Ty::Box(inner) => {
                    lvalue = format!("(({})->payload)", lvalue);
                    ty = (**inner).clone();
                    crossed_cell = true;
                }
                _ => break,
            }
        }
        if crossed_cell && ty == *payload {
            Some(lvalue)
        } else {
            None
        }
    }

    pub(crate) fn capture_lvalue(&self, index: usize) -> String {
        match &self.env {
            Some(env) => format!("(({}*){})->c{}", env.c_type, env.var, index),
            None => format!("/* capture {} outside closure */ 0", index),
        }
    }

    /// Lvalue of a place expression. Non-place expressions are materialised
    /// into an owned temporary (only useful for reads).
    pub(crate) fn gen_place(&mut self, expr: &HExpr) -> String {
        match &expr.kind {
            HExprKind::Local { c_name, .. } => self.local_lvalue(c_name),
            HExprKind::Capture { index, .. } => self.capture_lvalue(*index),
            HExprKind::Field { base, field } => self.field_lvalue(base, *field),
            HExprKind::Index { base, index } => self.index_lvalue(base, index, expr.span),
            HExprKind::Deref { expr: inner } => self.deref_lvalue(inner),
            _ => self.gen_owned(expr),
        }
    }

    /// Lvalue of an expression with every reference layer peeled off.
    ///
    /// Builtin methods (`v.len()`, `s.contains(..)`, ...) operate on a place.
    /// When the front end hands over a receiver of reference type the place is
    /// the referent, so `&Vec<T>` and `&String` readers work without an
    /// explicit dereference in the HIR. `Rc`/`Box` receivers are not
    /// auto-dereferenced here: the front end wraps those in `Deref`.
    pub(crate) fn place_through_reference(&mut self, expr: &HExpr) -> String {
        let mut lvalue = self.gen_place(expr);
        let mut ty = expr.ty.clone();
        while let Ty::Ref { inner, .. } = ty {
            lvalue = format!("(*({}))", lvalue);
            ty = *inner;
        }
        lvalue
    }

    pub(crate) fn literal(&self, lit: &HLit) -> String {
        match lit {
            HLit::Int { value, int_ty } => util::int_literal(*value, *int_ty),
            HLit::Float { value, float_ty } => util::float_literal(*value, *float_ty),
            HLit::Bool(value) => {
                if *value {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            HLit::Str(value) => format!("ls_str_cstr({})", util::c_string_literal(value)),
            HLit::Unit => "((LsUnit){0})".to_string(),
        }
    }

    pub(crate) fn const_expr(&self, c_name: &str, ty: &Ty) -> Option<String> {
        for c in &self.program.consts {
            if c.c_name == c_name {
                return Some(match &c.value {
                    hir::ConstValue::Int(value) => util::int_literal(*value, int_ty_of(ty)),
                    hir::ConstValue::Float(value) => util::float_literal(*value, float_ty_of(ty)),
                    hir::ConstValue::Bool(value) => {
                        if *value {
                            "true".to_string()
                        } else {
                            "false".to_string()
                        }
                    }
                    hir::ConstValue::Str(value) => {
                        format!("ls_str_cstr({})", util::c_string_literal(value))
                    }
                    hir::ConstValue::Unit => "((LsUnit){0})".to_string(),
                });
            }
        }
        None
    }

    // ----- expression dispatch ---------------------------------------------

    pub(crate) fn gen_into(&mut self, expr: &HExpr, target: &str) {
        match &expr.kind {
            HExprKind::Lit(lit) => {
                let value = self.literal(lit);
                self.w.line(format!("{} = {};", target, value));
            }
            HExprKind::Local { c_name, .. } => {
                let source = self.local_lvalue(c_name);
                self.emit_copy_into(target, &expr.ty, &source);
            }
            HExprKind::Capture { index, .. } => {
                let source = self.capture_lvalue(*index);
                self.emit_copy_into(target, &expr.ty, &source);
            }
            HExprKind::ConstRef { c_name } => {
                let value = self
                    .const_expr(c_name, &expr.ty)
                    .unwrap_or_else(|| self.zero_expr(&expr.ty));
                self.w.line(format!("{} = {};", target, value));
            }
            HExprKind::FnRef { fn_id } => self.gen_fn_ref(*fn_id, &expr.ty, target),
            HExprKind::Closure {
                closure_id,
                captures,
            } => self.gen_closure(*closure_id, captures, &expr.ty, target),
            HExprKind::Call { callee, args } => self.gen_call(callee, args, &expr.ty, target),
            HExprKind::DirectCall {
                fn_id,
                receiver,
                args,
            } => self.gen_direct_call(
                *fn_id,
                receiver.as_deref(),
                args,
                &expr.ty,
                target,
                expr.span,
            ),
            HExprKind::BuiltinMethod {
                receiver,
                method,
                args,
            } => self.gen_builtin_method(receiver, *method, args, &expr.ty, target, expr.span),
            HExprKind::BuiltinAssoc {
                func,
                args,
                type_args,
            } => self.gen_builtin_assoc(*func, args, type_args, &expr.ty, target, expr.span),
            HExprKind::BuiltinCall { func, args } => {
                self.gen_builtin_call(*func, args, &expr.ty, target, expr.span)
            }
            HExprKind::Field { base, field } => {
                let place = self.field_lvalue(base, *field);
                self.emit_copy_into(target, &expr.ty, &place);
            }
            HExprKind::Index { base, index } => {
                let place = self.index_lvalue(base, index, expr.span);
                self.emit_copy_into(target, &expr.ty, &place);
            }
            HExprKind::Unary { op, expr: inner } => {
                self.gen_unary(*op, inner, &expr.ty, target, expr.span)
            }
            HExprKind::Binary { op, lhs, rhs } => {
                self.gen_binary(*op, lhs, rhs, &expr.ty, target, expr.span)
            }
            HExprKind::Assign {
                target: place,
                op,
                value,
            } => self.gen_assign(place, *op, value, &expr.ty, target, expr.span),
            HExprKind::Cast { expr: inner } => self.gen_cast(inner, &expr.ty, target, expr.span),
            HExprKind::Block(block) => self.gen_block_value(block, target),
            HExprKind::If {
                cond,
                then_block,
                else_expr,
            } => self.gen_if(
                cond,
                then_block,
                else_expr.as_deref(),
                &expr.ty,
                target,
                expr.span,
            ),
            HExprKind::Match { scrutinee, arms } => {
                self.gen_match(scrutinee, arms, &expr.ty, target)
            }
            HExprKind::Loop { body } => self.gen_loop(body, &expr.ty, target),
            HExprKind::While { cond, body } => self.gen_while(cond, body, &expr.ty, target),
            HExprKind::ForRange {
                pat,
                start,
                end,
                inclusive,
                body,
            } => self.gen_for_range(pat, start, end, *inclusive, body, &expr.ty, target),
            HExprKind::ForVec { pat, vec, body } => {
                self.gen_for_vec(pat, vec, body, &expr.ty, target)
            }
            HExprKind::StructLit { struct_id, fields } => {
                self.gen_struct_lit(*struct_id, fields, &expr.ty, target)
            }
            HExprKind::Tuple(elems) => self.gen_tuple(elems, &expr.ty, target),
            HExprKind::VecLit { elems } => self.gen_vec_lit(elems, &expr.ty, target, expr.span),
            HExprKind::VecRepeat { value, count } => {
                self.gen_vec_repeat(value, count, &expr.ty, target, expr.span)
            }
            HExprKind::EnumLit {
                enum_id,
                variant,
                args,
            } => self.gen_enum_lit(*enum_id, *variant, args, &expr.ty, target),
            HExprKind::OptionSome(inner) => self.gen_option_some(inner, &expr.ty, target),
            HExprKind::OptionNone => {
                let value = self.zero_expr(&expr.ty);
                self.w.line(format!("{} = {};", target, value));
            }
            HExprKind::ResultOk(inner) => self.gen_result_variant(true, inner, &expr.ty, target),
            HExprKind::ResultErr(inner) => self.gen_result_variant(false, inner, &expr.ty, target),
            HExprKind::Format { parts } => self.gen_format(parts, target, expr.span),
            HExprKind::Macro(macro_call) => self.gen_macro(macro_call, target, expr.span),
            HExprKind::Break => self.gen_break(),
            HExprKind::Continue => self.gen_continue(),
            HExprKind::Return(value) => self.gen_return(value.as_deref(), target),
            HExprKind::Ref {
                mutable: _,
                expr: inner,
            } => self.gen_ref(inner, &expr.ty, target),
            HExprKind::Deref { expr: inner } => {
                let place = self.deref_lvalue(inner);
                let source = if self.is_pointer_type(&expr.ty) {
                    // A shared reference to a pointer-shaped value (String,
                    // Rc, Box or another reference) has a const-qualified
                    // handle type; the value itself is not const.
                    format!("(({})({}))", self.ctype(&expr.ty), place)
                } else {
                    place
                };
                self.emit_copy_into(target, &expr.ty, &source);
            }
        }
    }

    // ----- closure and function values -------------------------------------

    fn gen_fn_ref(&mut self, fn_id: usize, ty: &Ty, target: &str) {
        let ctype = self.ctype(ty);
        self.w.line(format!(
            "{} = ({}){{ .env = NULL, .fn_ptr = lsaca_thunk_{} }};",
            target, ctype, fn_id
        ));
    }

    fn gen_closure(&mut self, closure_id: usize, captures: &[HExpr], ty: &Ty, target: &str) {
        let ctype = format!("LsEnv_{}", closure_id);
        let env_name = self.fresh("env");
        self.w.line(format!(
            "{}* {} = ({}*)ls_alloc_managed(sizeof({}), lsaca_env_drop_{}, lsaca_trace_env_{});",
            ctype, env_name, ctype, ctype, closure_id, closure_id
        ));
        for (index, capture) in captures.iter().enumerate() {
            // Copy captures are copied by value; managed captures are moved
            // into the environment (the captured place is cleared).
            let slot = format!("{}->c{}", env_name, index);
            self.gen_move_into(capture, &slot);
        }
        let fn_ctype = self.ctype(ty);
        self.w.line(format!(
            "{} = ({}){{ .env = {}, .fn_ptr = lsaca_closure_{} }};",
            target, fn_ctype, env_name, closure_id
        ));
    }
}

/// Integer type of a constant expression when the HIR type is missing.
pub(crate) fn int_ty_of(ty: &Ty) -> crate::types::IntTy {
    match ty {
        Ty::Int(int_ty) => *int_ty,
        _ => crate::types::IntTy::I64,
    }
}

pub(crate) fn float_ty_of(ty: &Ty) -> crate::types::FloatTy {
    match ty {
        Ty::Float(float_ty) => *float_ty,
        _ => crate::types::FloatTy::F64,
    }
}
