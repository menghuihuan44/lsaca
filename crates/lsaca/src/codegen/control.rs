//! Statements and control flow: blocks, patterns, branching and loops.

use crate::hir::{HBlock, HExpr, HExprKind, HFormatPartKind, HLit, HPat, HPatKind, HStmt};
use crate::types::Ty;

use super::emit::{BoundVar, Gen, LoopCtx};
use super::types;
use super::util;

// ----- read-only HIR helpers ----------------------------------------------

/// True when the block mentions the local variable `name` anywhere.
fn block_mentions_local(block: &HBlock, name: &str) -> bool {
    block
        .stmts
        .iter()
        .any(|statement| statement_mentions_local(statement, name))
        || block
            .tail
            .as_deref()
            .map(|tail| expr_mentions_local(tail, name))
            .unwrap_or(false)
}

fn statement_mentions_local(statement: &HStmt, name: &str) -> bool {
    match statement {
        HStmt::Let { init, .. } => init
            .as_ref()
            .map(|init| expr_mentions_local(init, name))
            .unwrap_or(false),
        HStmt::Expr(expression) => expr_mentions_local(expression, name),
    }
}

fn expr_mentions_local(expression: &HExpr, name: &str) -> bool {
    match &expression.kind {
        HExprKind::Lit(_)
        | HExprKind::Capture { .. }
        | HExprKind::ConstRef { .. }
        | HExprKind::FnRef { .. }
        | HExprKind::OptionNone
        | HExprKind::Break
        | HExprKind::Continue => false,
        HExprKind::Local { c_name, .. } => c_name == name,
        HExprKind::Closure { captures, .. } => captures
            .iter()
            .any(|capture| expr_mentions_local(capture, name)),
        HExprKind::Call { callee, args } => {
            expr_mentions_local(callee, name)
                || args.iter().any(|arg| expr_mentions_local(arg, name))
        }
        HExprKind::DirectCall { receiver, args, .. } => {
            receiver
                .as_deref()
                .map(|receiver| expr_mentions_local(receiver, name))
                .unwrap_or(false)
                || args.iter().any(|arg| expr_mentions_local(arg, name))
        }
        HExprKind::BuiltinMethod { receiver, args, .. } => {
            expr_mentions_local(receiver, name)
                || args.iter().any(|arg| expr_mentions_local(arg, name))
        }
        HExprKind::BuiltinAssoc { args, .. } | HExprKind::BuiltinCall { args, .. } => {
            args.iter().any(|arg| expr_mentions_local(arg, name))
        }
        HExprKind::Field { base, .. } => expr_mentions_local(base, name),
        HExprKind::Index { base, index } => {
            expr_mentions_local(base, name) || expr_mentions_local(index, name)
        }
        HExprKind::Unary { expr, .. }
        | HExprKind::Cast { expr }
        | HExprKind::OptionSome(expr)
        | HExprKind::ResultOk(expr)
        | HExprKind::ResultErr(expr) => expr_mentions_local(expr, name),
        HExprKind::Binary { lhs, rhs, .. } => {
            expr_mentions_local(lhs, name) || expr_mentions_local(rhs, name)
        }
        HExprKind::Assign { target, value, .. } => {
            expr_mentions_local(target, name) || expr_mentions_local(value, name)
        }
        HExprKind::Block(block) => block_mentions_local(block, name),
        HExprKind::If {
            cond,
            then_block,
            else_expr,
        } => {
            expr_mentions_local(cond, name)
                || block_mentions_local(then_block, name)
                || else_expr
                    .as_deref()
                    .map(|otherwise| expr_mentions_local(otherwise, name))
                    .unwrap_or(false)
        }
        HExprKind::Match { scrutinee, arms } => {
            expr_mentions_local(scrutinee, name)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .map(|guard| expr_mentions_local(guard, name))
                        .unwrap_or(false)
                        || expr_mentions_local(&arm.body, name)
                })
        }
        HExprKind::Loop { body } => block_mentions_local(body, name),
        HExprKind::While { cond, body } => {
            expr_mentions_local(cond, name) || block_mentions_local(body, name)
        }
        HExprKind::ForRange {
            start, end, body, ..
        } => {
            expr_mentions_local(start, name)
                || expr_mentions_local(end, name)
                || block_mentions_local(body, name)
        }
        HExprKind::ForVec { vec, body, .. } => {
            expr_mentions_local(vec, name) || block_mentions_local(body, name)
        }
        HExprKind::StructLit { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_mentions_local(value, name)),
        HExprKind::Tuple(elements) | HExprKind::VecLit { elems: elements } => elements
            .iter()
            .any(|element| expr_mentions_local(element, name)),
        HExprKind::VecRepeat { value, count } => {
            expr_mentions_local(value, name) || expr_mentions_local(count, name)
        }
        HExprKind::EnumLit { args, .. } => args.iter().any(|arg| expr_mentions_local(arg, name)),
        HExprKind::Format { parts } => parts.iter().any(|part| match &part.kind {
            HFormatPartKind::Literal(_) => false,
            HFormatPartKind::Arg { expr, .. } => expr_mentions_local(expr, name),
        }),
        HExprKind::Macro(call) => {
            call.args.iter().any(|arg| expr_mentions_local(arg, name))
                || call.parts.iter().any(|part| match &part.kind {
                    HFormatPartKind::Literal(_) => false,
                    HFormatPartKind::Arg { expr, .. } => expr_mentions_local(expr, name),
                })
        }
        HExprKind::Return(value) => value
            .as_deref()
            .map(|value| expr_mentions_local(value, name))
            .unwrap_or(false),
    }
}

impl<'a> Gen<'a> {
    // ----- statements ------------------------------------------------------

    pub(crate) fn gen_stmts(&mut self, statements: &[HStmt]) {
        for statement in statements {
            if self.block_terminated || self.fn_terminated {
                break;
            }
            self.gen_stmt(statement);
        }
    }

    fn gen_stmt(&mut self, statement: &HStmt) {
        match statement {
            HStmt::Let { pat, init, .. } => {
                if let Some(init) = init {
                    // Write the initialiser straight into the predeclared
                    // local for a single binding: materialising it in a
                    // temporary first would add a full deep copy.
                    if !self.write_bind(pat, init) {
                        let value = self.gen_owned(init);
                        self.bind_pattern(pat, &value);
                    }
                }
            }
            HStmt::Expr(expression) => {
                let ty = expression.ty.clone();
                let target = self.declare_temp(&ty);
                self.gen_into(expression, &target);
                self.w.line(format!("(void){};", target));
            }
        }
        self.stmt_cleanup();
    }

    /// Lower `let <name> = <init>` as a direct write into the local that
    /// [`Gen::predeclare_pattern`] already declared.
    ///
    /// This avoids materialising the value in a temporary and deep-copying it
    /// into the binding. It is sound because a `let` binding is predeclared
    /// exactly once per C block and never rebound; every other pattern still
    /// goes through [`Gen::bind_pattern`].
    fn write_bind(&mut self, pat: &HPat, init: &HExpr) -> bool {
        let HPatKind::Bind { c_name, .. } = &pat.kind else {
            return false;
        };
        let predeclared = self
            .frames
            .last()
            .map(|frame| frame.vars.iter().any(|var| var.c_name == c_name.as_str()))
            .unwrap_or(false);
        if !predeclared {
            return false;
        }
        let name = c_name.clone();
        self.emit_release(&pat.ty, &name);
        self.gen_into(init, &name);
        true
    }

    /// Predeclare every variable bound by a `let` statement of the block, so
    /// that early `break`/`return` can release them safely.
    pub(crate) fn predeclare_block(&mut self, block: &HBlock) {
        for statement in &block.stmts {
            if let HStmt::Let { pat, .. } = statement {
                self.predeclare_pattern(pat);
            }
        }
    }

    fn predeclare_pattern(&mut self, pat: &HPat) {
        match &pat.kind {
            HPatKind::Bind { c_name, .. } => {
                let name = c_name.clone();
                let exists = self
                    .frames
                    .last()
                    .map(|frame| frame.vars.iter().any(|var| var.c_name == name))
                    .unwrap_or(false);
                if exists {
                    return;
                }
                let ctype = self.ctype(&pat.ty);
                let zero = self.zero_expr(&pat.ty);
                self.w.line(format!("{} {} = {};", ctype, name, zero));
                self.w.line(format!("(void){};", name));
                if let Some(frame) = self.frames.last_mut() {
                    frame.vars.push(BoundVar {
                        c_name: name,
                        ty: pat.ty.clone(),
                        owned: true,
                    });
                }
            }
            HPatKind::Tuple(parts) => {
                for part in parts {
                    self.predeclare_pattern(part);
                }
            }
            HPatKind::Struct { fields, .. } => {
                for (_, part) in fields {
                    self.predeclare_pattern(part);
                }
            }
            HPatKind::Enum { args, .. } => {
                for part in args {
                    self.predeclare_pattern(part);
                }
            }
            HPatKind::OptionSome(inner) => self.predeclare_pattern(inner),
            HPatKind::ResultOk(inner) => self.predeclare_pattern(inner),
            HPatKind::ResultErr(inner) => self.predeclare_pattern(inner),
            HPatKind::Or(alternatives) => {
                for alternative in alternatives {
                    self.predeclare_pattern(alternative);
                }
            }
            HPatKind::Wild | HPatKind::Lit(_) | HPatKind::OptionNone => {}
        }
    }

    /// Copy the values selected by a pattern out of `source` into the
    /// predeclared variables.
    fn bind_pattern(&mut self, pat: &HPat, source: &str) {
        match &pat.kind {
            HPatKind::Wild | HPatKind::Lit(_) | HPatKind::OptionNone => {}
            HPatKind::Bind { c_name, .. } => {
                self.emit_release(&pat.ty, c_name);
                self.emit_copy_into(c_name, &pat.ty, source);
            }
            HPatKind::Tuple(parts) => {
                for (index, part) in parts.iter().enumerate() {
                    let sub = format!("({}).{}", source, types::positional_field(index));
                    self.bind_pattern(part, &sub);
                }
            }
            HPatKind::Struct {
                struct_id, fields, ..
            } => {
                for (index, part) in fields {
                    let name = self
                        .program
                        .structs
                        .get(*struct_id)
                        .and_then(|item| item.fields.get(*index))
                        .map(|field| field.name.clone())
                        .unwrap_or_else(|| format!("field{}", index));
                    let sub = format!("({}).{}", source, types::field_cname(&name));
                    self.bind_pattern(part, &sub);
                }
            }
            HPatKind::Enum {
                enum_id,
                variant,
                args,
            } => {
                let name = self
                    .program
                    .enums
                    .get(*enum_id)
                    .and_then(|item| item.variants.get(*variant))
                    .map(|item| item.name.clone())
                    .unwrap_or_else(|| format!("Variant{}", variant));
                for (index, part) in args.iter().enumerate() {
                    let sub = format!(
                        "({}).{}.{}.{}",
                        source,
                        types::UNION_FIELD,
                        types::variant_member(&name),
                        types::positional_field(index)
                    );
                    self.bind_pattern(part, &sub);
                }
            }
            HPatKind::OptionSome(inner) => {
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::OPTION_SOME_MEMBER
                );
                self.bind_pattern(inner, &sub);
            }
            HPatKind::ResultOk(inner) => {
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::RESULT_OK_MEMBER
                );
                self.bind_pattern(inner, &sub);
            }
            HPatKind::ResultErr(inner) => {
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::RESULT_ERR_MEMBER
                );
                self.bind_pattern(inner, &sub);
            }
            HPatKind::Or(alternatives) => {
                // Bind the variables from whichever alternative matched.
                // Each alternative is re-tested here: the pattern tests are
                // side-effect free, and re-testing keeps the binding pass
                // independent from the C scopes created by the test pass.
                for alternative in alternatives {
                    let matched = self.new_plain(&Ty::Bool, "o");
                    self.w.line(format!("{} = true;", matched));
                    self.gen_match_tests(alternative, source, &matched);
                    self.w.line(format!("if ({}) {{", matched));
                    self.w.indent();
                    self.bind_pattern(alternative, source);
                    self.w.dedent();
                    self.w.line("}");
                }
            }
        }
    }

    // ----- blocks and branches ---------------------------------------------

    pub(crate) fn gen_block_value(&mut self, block: &HBlock, target: &str) {
        self.w.line("{");
        self.w.indent();
        let frame = self.frames.len();
        self.push_frame();
        self.predeclare_block(block);
        self.gen_stmts(&block.stmts);
        if !self.block_terminated {
            match &block.tail {
                Some(tail) => self.gen_into(tail, target),
                None => {
                    let zero = self.zero_expr(&Ty::Unit);
                    self.w.line(format!("{} = {};", target, zero));
                }
            }
        }
        self.frame_cleanup(frame);
        self.pop_frame();
        self.w.dedent();
        self.w.line("}");
    }

    pub(crate) fn gen_if(
        &mut self,
        cond: &HExpr,
        then_block: &HBlock,
        else_expr: Option<&HExpr>,
        ty: &Ty,
        target: &str,
        _span: crate::span::Span,
    ) {
        let condition = self.new_plain(&Ty::Bool, "c");
        self.gen_into(cond, &condition);
        if self.fn_terminated {
            self.block_terminated = true;
            return;
        }
        self.w.line(format!("if ({}) {{", condition));
        self.w.indent();
        let frame = self.frames.len();
        self.push_frame();
        self.predeclare_block(then_block);
        self.gen_stmts(&then_block.stmts);
        if !self.block_terminated {
            match &then_block.tail {
                Some(tail) => self.gen_into(tail, target),
                None => {
                    let zero = self.zero_expr(ty);
                    self.w.line(format!("{} = {};", target, zero));
                }
            }
        }
        self.frame_cleanup(frame);
        self.pop_frame();
        self.w.dedent();
        self.w.line("}");
        let then_terminated = self.block_terminated;
        let then_fn_terminated = self.fn_terminated;
        self.block_terminated = false;
        self.fn_terminated = false;
        let mut else_terminated = false;
        let mut else_fn_terminated = false;
        if let Some(other) = else_expr {
            self.w.line("else {");
            self.w.indent();
            let frame = self.frames.len();
            self.push_frame();
            self.gen_into(other, target);
            else_terminated = self.block_terminated;
            else_fn_terminated = self.fn_terminated;
            self.frame_cleanup(frame);
            self.pop_frame();
            self.w.dedent();
            self.w.line("}");
        }
        self.block_terminated = then_terminated && else_terminated;
        self.fn_terminated = then_fn_terminated && else_fn_terminated;
    }

    pub(crate) fn gen_match(
        &mut self,
        scrutinee: &HExpr,
        arms: &[crate::hir::HArm],
        _ty: &Ty,
        target: &str,
    ) {
        let frame = self.frames.len();
        self.push_frame();
        let scrutinee_value = self.declare_temp(&scrutinee.ty);
        self.gen_into(scrutinee, &scrutinee_value);
        if self.fn_terminated {
            self.frame_cleanup(frame);
            self.pop_frame();
            self.block_terminated = true;
            return;
        }
        self.w.line("for (;;) {");
        self.w.indent();
        for arm in arms {
            self.w.line("{");
            self.w.indent();
            let arm_frame = self.frames.len();
            self.push_frame();
            self.predeclare_pattern(&arm.pat);
            let matched = self.new_plain(&Ty::Bool, "m");
            self.w.line(format!("{} = true;", matched));
            // The guard result is tracked in its own flag outside the arm
            // body: a pattern match alone must not skip the remaining arms
            // when the guard evaluates to false.
            let pass = self.new_plain(&Ty::Bool, "g");
            self.w.line(format!("{} = false;", pass));
            self.gen_match_tests(&arm.pat, &scrutinee_value, &matched);
            self.w.line(format!("if ({}) {{", matched));
            self.w.indent();
            self.bind_pattern(&arm.pat, &scrutinee_value);
            let guard_frame = self.frames.len();
            self.push_frame();
            match &arm.guard {
                Some(guard) => self.gen_into(guard, &pass),
                None => self.w.line(format!("{} = true;", pass)),
            }
            self.w.line(format!("if ({}) {{", pass));
            self.w.indent();
            let body_frame = self.frames.len();
            self.push_frame();
            self.gen_into(&arm.body, target);
            self.frame_cleanup(body_frame);
            self.pop_frame();
            self.w.dedent();
            self.w.line("}");
            self.frame_cleanup(guard_frame);
            self.pop_frame();
            self.w.dedent();
            self.w.line("}");
            self.frame_cleanup(arm_frame);
            self.pop_frame();
            // Continue with the next arm unless both the pattern matched
            // and the guard accepted it.
            self.w.line(format!("if ({} && {}) break;", matched, pass));
            self.w.dedent();
            self.w.line("}");
            self.block_terminated = false;
            self.fn_terminated = false;
        }
        self.w.line("break;");
        self.w.dedent();
        self.w.line("}");
        self.frame_cleanup(frame);
        self.pop_frame();
        self.block_terminated = false;
        self.fn_terminated = false;
    }

    fn gen_match_tests(&mut self, pat: &HPat, source: &str, matched: &str) {
        match &pat.kind {
            HPatKind::Wild | HPatKind::Bind { .. } => {}
            HPatKind::Lit(lit) => self.gen_literal_test(lit, source, matched),
            HPatKind::Tuple(parts) => {
                for (index, part) in parts.iter().enumerate() {
                    let sub = format!("({}).{}", source, types::positional_field(index));
                    self.gen_match_tests(part, &sub, matched);
                }
            }
            HPatKind::Struct {
                struct_id, fields, ..
            } => {
                for (index, part) in fields {
                    let name = self
                        .program
                        .structs
                        .get(*struct_id)
                        .and_then(|item| item.fields.get(*index))
                        .map(|field| field.name.clone())
                        .unwrap_or_else(|| format!("field{}", index));
                    let sub = format!("({}).{}", source, types::field_cname(&name));
                    self.gen_match_tests(part, &sub, matched);
                }
            }
            HPatKind::Enum {
                enum_id,
                variant,
                args,
            } => {
                let (variant_name, tag) = match self
                    .program
                    .enums
                    .get(*enum_id)
                    .and_then(|item| item.variants.get(*variant))
                {
                    Some(item) => (item.name.clone(), item.tag),
                    None => (format!("Variant{}", variant), 0),
                };
                self.gen_tag_test(source, tag, matched);
                if !args.is_empty() {
                    self.w.line(format!("if ({}) {{", matched));
                    self.w.indent();
                    for (index, part) in args.iter().enumerate() {
                        let sub = format!(
                            "({}).{}.{}.{}",
                            source,
                            types::UNION_FIELD,
                            types::variant_member(&variant_name),
                            types::positional_field(index)
                        );
                        self.gen_match_tests(part, &sub, matched);
                    }
                    self.w.dedent();
                    self.w.line("}");
                }
            }
            HPatKind::OptionSome(inner) => {
                self.gen_tag_test(source, 1, matched);
                self.w.line(format!("if ({}) {{", matched));
                self.w.indent();
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::OPTION_SOME_MEMBER
                );
                self.gen_match_tests(inner, &sub, matched);
                self.w.dedent();
                self.w.line("}");
            }
            HPatKind::OptionNone => self.gen_tag_test(source, 0, matched),
            HPatKind::ResultOk(inner) => {
                self.gen_tag_test(source, 0, matched);
                self.w.line(format!("if ({}) {{", matched));
                self.w.indent();
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::RESULT_OK_MEMBER
                );
                self.gen_match_tests(inner, &sub, matched);
                self.w.dedent();
                self.w.line("}");
            }
            HPatKind::ResultErr(inner) => {
                self.gen_tag_test(source, 1, matched);
                self.w.line(format!("if ({}) {{", matched));
                self.w.indent();
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::RESULT_ERR_MEMBER
                );
                self.gen_match_tests(inner, &sub, matched);
                self.w.dedent();
                self.w.line("}");
            }
            HPatKind::Or(alternatives) => {
                // Every alternative is tested independently and the arm
                // matches if at least one succeeds. Leaving `matched` at its
                // incoming value would make the arm match unconditionally.
                self.w.line(format!("if ({}) {{", matched));
                self.w.indent();
                let mut alternatives_matched = Vec::with_capacity(alternatives.len());
                for alternative in alternatives {
                    let alternative_matched = self.new_plain(&Ty::Bool, "o");
                    self.w.line(format!("{} = true;", alternative_matched));
                    self.gen_match_tests(alternative, source, &alternative_matched);
                    alternatives_matched.push(alternative_matched);
                }
                self.w.line(format!(
                    "{} = ({});",
                    matched,
                    alternatives_matched.join(" || ")
                ));
                self.w.dedent();
                self.w.line("}");
            }
        }
    }

    fn gen_tag_test(&mut self, source: &str, tag: usize, matched: &str) {
        self.w.line(format!(
            "if (({}).{} != (int32_t){}) {{ {} = false; }}",
            source,
            types::TAG_FIELD,
            tag,
            matched
        ));
    }

    fn gen_literal_test(&mut self, lit: &HLit, source: &str, matched: &str) {
        match lit {
            HLit::Str(text) => {
                let literal = self.new_plain(&Ty::String, "pl");
                self.w.line(format!(
                    "{} = ls_str_cstr({});",
                    literal,
                    util::c_string_literal(text)
                ));
                self.w.line(format!(
                    "if (!ls_str_eq(({}), ({}))) {{ {} = false; }}",
                    source, literal, matched
                ));
                self.w.line(format!("ls_release({});", literal));
            }
            HLit::Int { value, int_ty } => {
                let literal = util::int_literal(*value, *int_ty);
                self.w.line(format!(
                    "if (({}) != ({})) {{ {} = false; }}",
                    source, literal, matched
                ));
            }
            HLit::Float { value, float_ty } => {
                let literal = util::float_literal(*value, *float_ty);
                self.w.line(format!(
                    "if (({}) != ({})) {{ {} = false; }}",
                    source, literal, matched
                ));
            }
            HLit::Bool(value) => {
                let literal = if *value { "true" } else { "false" };
                self.w.line(format!(
                    "if (({}) != {}) {{ {} = false; }}",
                    source, literal, matched
                ));
            }
            HLit::Unit => {}
        }
    }

    // ----- loops -----------------------------------------------------------

    pub(crate) fn gen_loop(&mut self, body: &HBlock, ty: &Ty, target: &str) {
        let break_label = self.fresh_label("loop_end");
        let continue_label = self.fresh_label("loop_next");
        let zero = self.zero_expr(ty);
        self.w.line(format!("{} = {};", target, zero));
        self.loops.push(LoopCtx {
            break_label: break_label.clone(),
            continue_label: continue_label.clone(),
            body_frame: self.frames.len(),
            used_break: false,
            used_continue: false,
        });
        self.w.line("for (;;) {");
        self.w.indent();
        self.gen_loop_body(body, None, &continue_label);
        self.w.dedent();
        self.w.line("}");
        let used_break = self
            .loops
            .last()
            .map(|context| context.used_break)
            .unwrap_or(false);
        self.loops.pop();
        if used_break {
            self.w.line(format!("{}:;", break_label));
        }
        self.block_terminated = false;
        self.fn_terminated = false;
    }

    pub(crate) fn gen_while(&mut self, cond: &HExpr, body: &HBlock, ty: &Ty, target: &str) {
        let break_label = self.fresh_label("while_end");
        let continue_label = self.fresh_label("while_next");
        let zero = self.zero_expr(ty);
        self.w.line(format!("{} = {};", target, zero));
        self.loops.push(LoopCtx {
            break_label: break_label.clone(),
            continue_label: continue_label.clone(),
            body_frame: self.frames.len(),
            used_break: false,
            used_continue: false,
        });
        self.w.line("for (;;) {");
        self.w.indent();
        let cond_frame = self.frames.len();
        self.push_frame();
        let condition = self.new_plain(&Ty::Bool, "c");
        self.gen_into(cond, &condition);
        let cond_terminated = self.fn_terminated;
        self.frame_cleanup(cond_frame);
        self.pop_frame();
        if cond_terminated {
            self.w.line("break;");
            self.w.dedent();
            self.w.line("}");
            self.w.line(format!("{}:;", break_label));
            self.loops.pop();
            self.block_terminated = true;
            self.fn_terminated = true;
            return;
        }
        self.w.line(format!("if (!{}) break;", condition));
        self.gen_loop_body(body, None, &continue_label);
        self.w.dedent();
        self.w.line("}");
        let used_break = self
            .loops
            .last()
            .map(|context| context.used_break)
            .unwrap_or(false);
        self.loops.pop();
        if used_break {
            self.w.line(format!("{}:;", break_label));
        }
        self.block_terminated = false;
        self.fn_terminated = false;
    }

    // One parameter per HIR field keeps this in step with gen_for_vec and the
    // expression dispatch; grouping them into a struct would only hide the shape.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn gen_for_range(
        &mut self,
        pat: &HPat,
        start: &HExpr,
        end: &HExpr,
        inclusive: bool,
        body: &HBlock,
        ty: &Ty,
        target: &str,
    ) {
        let break_label = self.fresh_label("for_end");
        let continue_label = self.fresh_label("for_next");
        let zero = self.zero_expr(ty);
        self.w.line(format!("{} = {};", target, zero));
        let start_value = self.gen_owned(start);
        let end_value = self.gen_owned(end);
        let index = self.new_plain_of("int64_t", "i");
        self.w
            .line(format!("{} = (int64_t)({});", index, start_value));
        self.loops.push(LoopCtx {
            break_label: break_label.clone(),
            continue_label: continue_label.clone(),
            body_frame: self.frames.len(),
            used_break: false,
            used_continue: false,
        });
        self.w.line("for (;;) {");
        self.w.indent();
        let stop = if inclusive {
            format!("{} > (int64_t)({})", index, end_value)
        } else {
            format!("{} >= (int64_t)({})", index, end_value)
        };
        self.w.line(format!("if ({}) break;", stop));
        self.gen_loop_body(body, Some((pat, index.as_str())), &continue_label);
        if inclusive {
            self.w.line(format!("if ({} == INT64_MAX) break;", index));
        }
        self.w.line(format!("{} += 1;", index));
        self.w.dedent();
        self.w.line("}");
        let used_break = self
            .loops
            .last()
            .map(|context| context.used_break)
            .unwrap_or(false);
        self.loops.pop();
        if used_break {
            self.w.line(format!("{}:;", break_label));
        }
        self.block_terminated = false;
        self.fn_terminated = false;
    }

    pub(crate) fn gen_for_vec(
        &mut self,
        pat: &HPat,
        vec: &HExpr,
        body: &HBlock,
        ty: &Ty,
        target: &str,
    ) {
        let break_label = self.fresh_label("for_end");
        let continue_label = self.fresh_label("for_next");
        let zero = self.zero_expr(ty);
        self.w.line(format!("{} = {};", target, zero));
        // A loop over a local vector that its body never mentions can iterate
        // the vector in place: nothing can observe or mutate it during the
        // loop, so the owned snapshot (a full deep copy) is not needed.
        let direct = match &vec.kind {
            HExprKind::Local { c_name, .. } if !block_mentions_local(body, c_name) => {
                Some(self.local_lvalue(c_name))
            }
            _ => None,
        };
        let vec_value = match direct {
            Some(place) => place,
            None => self.gen_owned(vec),
        };
        let index = self.new_plain_of("size_t", "i");
        self.w.line(format!("{} = 0;", index));
        self.loops.push(LoopCtx {
            break_label: break_label.clone(),
            continue_label: continue_label.clone(),
            body_frame: self.frames.len(),
            used_break: false,
            used_continue: false,
        });
        self.w.line("for (;;) {");
        self.w.indent();
        self.w
            .line(format!("if ({} >= ({}).len) break;", index, vec_value));
        let element = format!("({}).data[{}]", vec_value, index);
        self.gen_loop_body(body, Some((pat, element.as_str())), &continue_label);
        self.w.line(format!("{} += 1;", index));
        self.w.dedent();
        self.w.line("}");
        let used_break = self
            .loops
            .last()
            .map(|context| context.used_break)
            .unwrap_or(false);
        self.loops.pop();
        if used_break {
            self.w.line(format!("{}:;", break_label));
        }
        self.block_terminated = false;
        self.fn_terminated = false;
    }

    /// Emit the `{ ... }` block of a loop body, binding the loop pattern (if
    /// any) to `pat_source` before the statements.
    fn gen_loop_body(&mut self, body: &HBlock, pat: Option<(&HPat, &str)>, continue_label: &str) {
        self.w.line("{");
        self.w.indent();
        let body_frame = self.frames.len();
        self.push_frame();
        self.predeclare_block(body);
        if let Some((pattern, source)) = pat {
            self.predeclare_pattern(pattern);
            let source = source.to_string();
            self.bind_pattern(pattern, &source);
        }
        self.gen_stmts(&body.stmts);
        if !self.block_terminated
            && !self.fn_terminated
            && let Some(tail) = &body.tail
        {
            let tail_ty = tail.ty.clone();
            let discard = self.declare_temp(&tail_ty);
            self.gen_into(tail, &discard);
            self.w.line(format!("(void){};", discard));
        }
        let uses_continue = self
            .loops
            .last()
            .map(|context| context.used_continue)
            .unwrap_or(false);
        if uses_continue {
            self.w.line(format!("{}:;", continue_label));
        }
        self.frame_cleanup(body_frame);
        self.pop_frame();
        self.w.dedent();
        self.w.line("}");
    }

    // ----- jump statements -------------------------------------------------

    pub(crate) fn gen_break(&mut self) {
        let context = match self.loops.last() {
            Some(context) => (context.break_label.clone(), context.body_frame),
            None => {
                self.block_terminated = true;
                return;
            }
        };
        let (label, body_frame) = context;
        if let Some(context) = self.loops.last_mut() {
            context.used_break = true;
        }
        let frame_count = self.frames.len();
        let mut index = frame_count;
        while index > body_frame {
            index -= 1;
            self.emit_frame_release(index);
        }
        self.w.line(format!("goto {};", label));
        self.block_terminated = true;
    }

    pub(crate) fn gen_continue(&mut self) {
        let context = match self.loops.last() {
            Some(context) => (context.continue_label.clone(), context.body_frame),
            None => {
                self.block_terminated = true;
                return;
            }
        };
        let (label, body_frame) = context;
        if let Some(context) = self.loops.last_mut() {
            context.used_continue = true;
        }
        let frame_count = self.frames.len();
        let mut index = frame_count;
        while index > body_frame + 1 {
            index -= 1;
            self.emit_frame_release(index);
        }
        self.w.line(format!("goto {};", label));
        self.block_terminated = true;
    }

    pub(crate) fn gen_return(&mut self, value: Option<&HExpr>, _target: &str) {
        let return_ty = self.fn_ret.clone();
        let result = self.new_plain(&return_ty, "r");
        match value {
            Some(value) => self.gen_into(value, &result),
            None => {
                let zero = self.zero_expr(&return_ty);
                self.w.line(format!("{} = {};", result, zero));
            }
        }
        let frame_count = self.frames.len();
        let mut index = frame_count;
        while index > 0 {
            index -= 1;
            self.emit_frame_release(index);
        }
        self.w.line(format!("return {};", result));
        self.block_terminated = true;
        self.fn_terminated = true;
    }
}
