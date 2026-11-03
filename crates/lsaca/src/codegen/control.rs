//! Statements and control flow: blocks, patterns, branching and loops.

use crate::hir::{HBlock, HExpr, HExprKind, HLit, HPat, HPatKind, HStmt};
use crate::types::Ty;

use super::emit::{BoundVar, Gen, LoopCtx};
use super::types;
use super::util;

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
                    if !pattern_binds_any(pat) {
                        // A binding-free pattern such as `let _ = <init>;`:
                        // the initialiser is still evaluated for its side
                        // effects, but the value is dropped immediately.
                        self.gen_discard(init);
                    } else if !self.write_bind(pat, init) {
                        // Write the initialiser straight into the predeclared
                        // local for a single binding: materialising it in a
                        // temporary first would add a full deep copy.
                        //
                        // A destructuring pattern whose initialiser is a place
                        // of a type with a Drop hook must read the fields
                        // straight out of that place. Copying the value into a
                        // temporary would run the destructor on the copy (and
                        // the original would still be dropped), so a Drop hook
                        // would observe two drops for one value. The type
                        // checker only admits Copy bindings under a Drop-hook
                        // aggregate (E0509 for everything else), so reading the
                        // place here never transfers ownership.
                        if init.is_place() && self.program.drop_hook(&init.ty).is_some() {
                            let place = self.gen_place(init);
                            self.bind_let_pattern(pat, init, &place);
                        } else if self.let_pattern_consumes_owned_value(pat, init) {
                            // A destructuring pattern that binds a Drop-hook
                            // payload owns the initialiser: move it into a
                            // temporary, steal the payload fields and mark the
                            // temporary moved. Copying would run the Drop hook
                            // for the binding and for the temporary.
                            let value = self.gen_owned_move(init);
                            self.bind_pattern_consume(pat, &value);
                            self.mark_moved(&value);
                        } else {
                            let value = self.gen_owned(init);
                            self.bind_let_pattern(pat, init, &value);
                        }
                    }
                }
            }
            HStmt::Expr(expression) => {
                let ty = expression.ty.clone();
                // A discarded expression statement consumes its value: the
                // value is transferred out of any owned place and the
                // temporary is released (dropped) at the end of the statement.
                let target = if self.is_trivial_copy(&ty) {
                    let target = self.declare_temp(&ty);
                    self.gen_into(expression, &target);
                    target
                } else {
                    self.gen_owned_move(expression)
                };
                self.w.line(format!("(void){};", target));
            }
        }
        self.stmt_cleanup();
    }

    /// Evaluate `expr` only for its side effects, discarding its value.
    ///
    /// Used for binding-free patterns such as `let _ = <init>;`: the
    /// initialiser is still evaluated (calls run, indexing checks fire), but
    /// no binding exists, so the value is dropped immediately. Like Rust's
    /// wildcard pattern, reading a place does not move out of it; a
    /// materialised temporary is released exactly once by
    /// [`Gen::stmt_cleanup`]. The `(void)` use keeps the generated C free of
    /// unused-variable and unused-but-set warnings.
    fn gen_discard(&mut self, expr: &HExpr) {
        let value = if expr.is_place() {
            self.gen_place(expr)
        } else {
            self.gen_owned(expr)
        };
        self.w.line(format!("(void)({});", value));
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
        if self.is_trivial_copy(&pat.ty) {
            self.emit_release(&pat.ty, &name);
            self.gen_into(init, &name);
        } else {
            // A plain let initialiser is a consuming context: release a
            // previous value first, transfer the initialiser into the binding
            // and mark the binding live for cleanup.
            self.emit_release_guarded(&pat.ty, &name, self.live_flag(&name).as_deref());
            self.gen_move_into(init, &name);
            self.set_live(&name, true);
        }
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
                // Predeclared bindings must start at a cheap zero value: writing the
                // real initialiser cannot leak a heap allocation made for the placeholder.
                let zero = self.raw_zero_expr(&pat.ty);
                self.w.line(format!("{} {} = {};", ctype, name, zero));
                self.w.line(format!("(void){};", name));
                let live = if self.is_trivial_copy(&pat.ty) {
                    None
                } else {
                    let flag = format!("{}_live", name);
                    self.w.line(format!("bool {} = false;", flag));
                    Some(flag)
                };
                if let Some(frame) = self.frames.last_mut() {
                    frame.vars.push(BoundVar {
                        c_name: name,
                        ty: pat.ty.clone(),
                        owned: true,
                        live,
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
        self.bind_pattern_mode(pat, source, false, false);
    }

    /// Bind the variables of a `let` pattern whose initialiser may be a
    /// reference.
    ///
    /// This mirrors the scrutinee peeling done by `gen_match`: when the
    /// initialiser is a reference, match ergonomics make every binding a
    /// reference to the selected place, and structural sub-patterns read
    /// through the dereferenced pointer instead of the pointer itself.
    fn bind_let_pattern(&mut self, pat: &HPat, init: &HExpr, value: &str) {
        let (source, _, ref_mode) = peel_match_references(value.to_string(), &init.ty);
        self.bind_pattern_mode(pat, &source, ref_mode, false);
    }

    /// Bind the variables of a pattern.
    ///
    /// With `ref_mode` set (the scrutinee is a reference, match ergonomics)
    /// every binding takes the address of the place selected by the pattern
    /// instead of copying its value: the binding itself has reference type.
    /// Test paths and binding paths read through the same dereferenced source,
    /// so nested structural patterns work unchanged.
    ///
    /// With `move_bindings` set the pattern belongs to a consuming, unguarded
    /// match arm: a by-value binding steals the selected value out of the
    /// scrutinee instead of copying it (Rust pattern bindings move). The
    /// selected place is raw-zeroed and the binding becomes its owner; see
    /// [`Gen::zero_release_is_inert`] for when that is sound. Guarded arms
    /// keep the copying form because the bindings are created before the guard
    /// is evaluated and a rejecting guard would have to move the values back.
    fn bind_pattern_mode(&mut self, pat: &HPat, source: &str, ref_mode: bool, move_bindings: bool) {
        match &pat.kind {
            HPatKind::Wild | HPatKind::Lit(_) | HPatKind::OptionNone => {}
            HPatKind::Bind { c_name, .. } => {
                if ref_mode && pat.ty.is_ref() {
                    // Match ergonomics: the binding borrows the place that the
                    // pattern selected. References are plain addresses, so the
                    // binding is the address of the selected field, never a
                    // retain of the pointee.
                    self.w.line(format!(
                        "{} = ({})(&({}));",
                        c_name,
                        self.ctype(&pat.ty),
                        source
                    ));
                } else if move_bindings
                    && !self.is_trivial_copy(&pat.ty)
                    && self.zero_release_is_inert(&pat.ty)
                {
                    // A by-value binding in a consuming, unguarded match arm
                    // moves the value out of the scrutinee: Rust pattern
                    // bindings move, they do not clone. Stealing the value is
                    // sound because releasing the raw-zeroed place it leaves
                    // behind is a no-op, and the scrutinee owner is released
                    // exactly once at the end of the match.
                    self.emit_release_guarded(&pat.ty, c_name, self.live_flag(c_name).as_deref());
                    self.w.line(format!("{} = {};", c_name, source));
                    self.w
                        .line(format!("{} = {};", source, self.raw_zero_expr(&pat.ty)));
                    self.set_live(c_name, true);
                } else {
                    // Pattern bindings read their value out of the scrutinee
                    // temporary: copy Copy values and retain managed ones.
                    self.emit_release_guarded(&pat.ty, c_name, self.live_flag(c_name).as_deref());
                    self.emit_copy_into(c_name, &pat.ty, source);
                    if !self.is_trivial_copy(&pat.ty) {
                        self.set_live(c_name, true);
                    }
                }
            }
            HPatKind::Tuple(parts) => {
                for (index, part) in parts.iter().enumerate() {
                    let sub = format!("({}).{}", source, types::positional_field(index));
                    self.bind_pattern_mode(part, &sub, ref_mode, move_bindings);
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
                    self.bind_pattern_mode(part, &sub, ref_mode, move_bindings);
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
                    self.bind_pattern_mode(part, &sub, ref_mode, move_bindings);
                }
            }
            HPatKind::OptionSome(inner) => {
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::OPTION_SOME_MEMBER
                );
                self.bind_pattern_mode(inner, &sub, ref_mode, move_bindings);
            }
            HPatKind::ResultOk(inner) => {
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::RESULT_OK_MEMBER
                );
                self.bind_pattern_mode(inner, &sub, ref_mode, move_bindings);
            }
            HPatKind::ResultErr(inner) => {
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::RESULT_ERR_MEMBER
                );
                self.bind_pattern_mode(inner, &sub, ref_mode, move_bindings);
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
                    self.bind_pattern_mode(alternative, source, ref_mode, move_bindings);
                    self.w.dedent();
                    self.w.line("}");
                }
            }
        }
    }

    /// True when the pattern binds a non-Copy value whose zero image would
    /// still run a user Drop hook when released.
    ///
    /// Such a binding must move its payload: copying would deep-copy (or
    /// retain) the payload into the binding while the source aggregate still
    /// drops the original, running the Drop hook twice. The zero-release
    /// property is exactly what [`Gen::zero_release_is_inert`] decides; every
    /// container is walked so a hook buried in an enum variant or tuple field
    /// is found as well.
    fn pattern_moves_drop_payload(&self, pat: &HPat) -> bool {
        match &pat.kind {
            HPatKind::Bind { .. } => {
                !self.is_trivial_copy(&pat.ty) && !self.zero_release_is_inert(&pat.ty)
            }
            HPatKind::Tuple(parts) => parts
                .iter()
                .any(|part| self.pattern_moves_drop_payload(part)),
            HPatKind::Struct { fields, .. } => fields
                .iter()
                .any(|(_, part)| self.pattern_moves_drop_payload(part)),
            HPatKind::Enum { args, .. } => args
                .iter()
                .any(|part| self.pattern_moves_drop_payload(part)),
            HPatKind::OptionSome(inner)
            | HPatKind::ResultOk(inner)
            | HPatKind::ResultErr(inner) => self.pattern_moves_drop_payload(inner),
            HPatKind::Or(alternatives) => alternatives
                .iter()
                .any(|alternative| self.pattern_moves_drop_payload(alternative)),
            HPatKind::Wild | HPatKind::Lit(_) | HPatKind::OptionNone => false,
        }
    }

    /// Whether `let <pat> = <init>` must bind by consuming the initialiser.
    ///
    /// Only whole-value initialisers can be consumed: a local variable is
    /// moved out of with its liveness flag cleared, and a constructor is
    /// materialised directly inside the temporary. Aggregate places (fields,
    /// indexes, dereferences) and references are rejected here: moving out of
    /// them is E0507, and the defensive copy emitted for those places would
    /// leave two owners of the same payload.
    fn let_pattern_consumes_owned_value(&self, pat: &HPat, init: &HExpr) -> bool {
        if matches!(&init.ty, Ty::Ref { .. }) {
            return false;
        }
        if init.is_place() && !matches!(&init.kind, HExprKind::Local { .. }) {
            return false;
        }
        self.program.drop_hook(&init.ty).is_none() && self.pattern_moves_drop_payload(pat)
    }
    /// Move the value selected by a wildcard or ignored field into a released
    /// temporary, clearing the source slot.
    ///
    /// Consuming patterns neutralise the source aggregate instead of releasing
    /// it, so a part the pattern does not bind would otherwise leak. Moving it
    /// into a temporary keeps the normal scope cleanup responsible: the value
    /// is dropped exactly once, and an early `return` still releases it.
    fn move_place_into_temp(&mut self, ty: &Ty, place: &str) {
        let temp = self.declare_temp(ty);
        self.w.line(format!("{} = {};", temp, place));
        self.w
            .line(format!("{} = {};", place, self.raw_zero_expr(ty)));
    }

    /// Bind the variables of a consuming pattern by moving the selected values.
    ///
    /// Rust pattern bindings move. Unlike [`Gen::bind_pattern_mode`] this path
    /// takes ownership of every selected value instead of copying it, so a
    /// Drop-hook payload is bound exactly once and never copied:
    ///
    /// * a non-Copy binding steals the value, clears the source slot and marks
    ///   the binding live;
    /// * Copy bindings copy;
    /// * values selected by wildcards, literal tests or by aggregate fields
    ///   the pattern does not name move into temporaries so they are still
    ///   dropped exactly once.
    ///
    /// The caller must not release the source aggregate afterwards: nothing is
    /// left to release. See the `let` and `match` callers.
    fn bind_pattern_consume(&mut self, pat: &HPat, source: &str) {
        match &pat.kind {
            HPatKind::Wild | HPatKind::Lit(_) => {
                if !self.is_trivial_copy(&pat.ty) {
                    self.move_place_into_temp(&pat.ty, source);
                }
            }
            HPatKind::Bind { c_name, .. } => {
                if self.is_trivial_copy(&pat.ty) {
                    self.w.line(format!("{} = {};", c_name, source));
                } else {
                    self.emit_release_guarded(&pat.ty, c_name, self.live_flag(c_name).as_deref());
                    self.w.line(format!("{} = {};", c_name, source));
                    self.w
                        .line(format!("{} = {};", source, self.raw_zero_expr(&pat.ty)));
                    self.set_live(c_name, true);
                }
            }
            HPatKind::Tuple(parts) => {
                for (index, part) in parts.iter().enumerate() {
                    let sub = format!("({}).{}", source, types::positional_field(index));
                    self.bind_pattern_consume(part, &sub);
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
                    self.bind_pattern_consume(part, &sub);
                }
                // Fields the pattern does not name still belong to the value
                // being consumed (`Struct { field, .. }`).
                if let Some(item) = self.program.structs.get(*struct_id) {
                    let named: Vec<usize> = fields.iter().map(|(index, _)| *index).collect();
                    let remaining: Vec<(String, Ty)> = item
                        .fields
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| !named.contains(index))
                        .map(|(_, field)| (field.name.clone(), field.ty.clone()))
                        .collect();
                    for (name, ty) in remaining {
                        if self.is_trivial_copy(&ty) {
                            continue;
                        }
                        let sub = format!("({}).{}", source, types::field_cname(&name));
                        self.move_place_into_temp(&ty, &sub);
                    }
                }
            }
            HPatKind::Enum {
                enum_id,
                variant,
                args,
            } => {
                let matched_variant = self
                    .program
                    .enums
                    .get(*enum_id)
                    .and_then(|item| item.variants.get(*variant))
                    .cloned();
                let variant_name = matched_variant
                    .as_ref()
                    .map(|item| item.name.clone())
                    .unwrap_or_else(|| format!("Variant{}", variant));
                for (index, part) in args.iter().enumerate() {
                    let sub = format!(
                        "({}).{}.{}.{}",
                        source,
                        types::UNION_FIELD,
                        types::variant_member(&variant_name),
                        types::positional_field(index)
                    );
                    self.bind_pattern_consume(part, &sub);
                }
                // Payload fields the pattern does not name (`Variant(..)`).
                if let Some(item) = matched_variant {
                    for (index, ty) in item.fields.iter().enumerate() {
                        if index < args.len() || self.is_trivial_copy(ty) {
                            continue;
                        }
                        let sub = format!(
                            "({}).{}.{}.{}",
                            source,
                            types::UNION_FIELD,
                            types::variant_member(&variant_name),
                            types::positional_field(index)
                        );
                        self.move_place_into_temp(ty, &sub);
                    }
                }
            }
            HPatKind::OptionSome(inner) => {
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::OPTION_SOME_MEMBER
                );
                self.bind_pattern_consume(inner, &sub);
            }
            HPatKind::OptionNone => {}
            HPatKind::ResultOk(inner) => {
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::RESULT_OK_MEMBER
                );
                self.bind_pattern_consume(inner, &sub);
            }
            HPatKind::ResultErr(inner) => {
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::RESULT_ERR_MEMBER
                );
                self.bind_pattern_consume(inner, &sub);
            }
            HPatKind::Or(alternatives) => {
                for alternative in alternatives {
                    let matched = self.new_plain(&Ty::Bool, "o");
                    self.w.line(format!("{} = true;", matched));
                    self.gen_match_tests(alternative, source, &matched);
                    self.w.line(format!("if ({}) {{", matched));
                    self.w.indent();
                    self.bind_pattern_consume(alternative, source);
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
                Some(tail) => self.gen_move_into(tail, target),
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
                Some(tail) => self.gen_move_into(tail, target),
                None => {
                    // A unit completion value is never observed, and a
                    // diverging `if` in a value position must not store a
                    // unit aggregate into a differently typed slot.
                    if !ty.is_unit() {
                        let zero = self.zero_expr(ty);
                        self.w.line(format!("{} = {};", target, zero));
                    }
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
            self.gen_move_into(other, target);
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

    /// Bind the values selected by a pattern without taking ownership of them.
    ///
    /// Guarded match arms need their bindings to be readable while the guard
    /// runs but must not own the values yet: when the guard rejects the arm the
    /// remaining arms still see an untouched scrutinee, so nothing may be
    /// released. The bindings therefore only receive shallow aliases and stay
    /// not live; once the guard accepts the arm `bind_pattern_consume` steals
    /// the payloads for real.
    fn bind_pattern_borrow(&mut self, pat: &HPat, source: &str) {
        match &pat.kind {
            HPatKind::Wild | HPatKind::Lit { .. } | HPatKind::OptionNone => {}
            HPatKind::Bind { c_name, .. } => {
                self.w.line(format!("{} = {};", c_name, source));
            }
            HPatKind::Tuple(parts) => {
                for (index, part) in parts.iter().enumerate() {
                    let sub = format!("({}).{}", source, types::positional_field(index));
                    self.bind_pattern_borrow(part, &sub);
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
                    self.bind_pattern_borrow(part, &sub);
                }
            }
            HPatKind::Enum {
                enum_id,
                variant,
                args,
            } => {
                let variant_name = self
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
                        types::variant_member(&variant_name),
                        types::positional_field(index)
                    );
                    self.bind_pattern_borrow(part, &sub);
                }
            }
            HPatKind::OptionSome(inner) => {
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::OPTION_SOME_MEMBER
                );
                self.bind_pattern_borrow(inner, &sub);
            }
            HPatKind::ResultOk(inner) => {
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::RESULT_OK_MEMBER
                );
                self.bind_pattern_borrow(inner, &sub);
            }
            HPatKind::ResultErr(inner) => {
                let sub = format!(
                    "({}).{}.{}",
                    source,
                    types::UNION_FIELD,
                    types::RESULT_ERR_MEMBER
                );
                self.bind_pattern_borrow(inner, &sub);
            }
            HPatKind::Or(alternatives) => {
                for alternative in alternatives {
                    let matched = self.new_plain(&Ty::Bool, "o");
                    self.w.line(format!("{} = true;", matched));
                    self.gen_match_tests(alternative, source, &matched);
                    self.w.line(format!("if ({}) {{", matched));
                    self.w.indent();
                    self.bind_pattern_borrow(alternative, source);
                    self.w.dedent();
                    self.w.line("}");
                }
            }
        }
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
        // The scrutinee of a match or if let is a consuming context: the
        // value is transferred into a local that owns it for the arms.
        //
        // The owning local carries a liveness flag instead of being a plain
        // temporary: a whole-value binding moves the value out of it (setting
        // the flag to false), and a guard that rejects an arm moves the value
        // back (setting the flag to true again). The flag is honoured by both
        // the normal scope exit and the jump-statement cleanups, so the value
        // is always dropped exactly once.
        let scrutinee_value = self.fresh("t");
        let scrutinee_ctype = self.ctype(&scrutinee.ty);
        let scrutinee_zero = self.raw_zero_expr(&scrutinee.ty);
        self.w.line(format!(
            "{} {} = {};",
            scrutinee_ctype, scrutinee_value, scrutinee_zero
        ));
        let scrutinee_live = if self.is_trivial_copy(&scrutinee.ty) {
            None
        } else {
            let flag = format!("{}_live", scrutinee_value);
            self.w.line(format!("bool {} = true;", flag));
            Some(flag)
        };
        if let Some(frame) = self.frames.last_mut() {
            frame.vars.push(BoundVar {
                c_name: scrutinee_value.clone(),
                ty: scrutinee.ty.clone(),
                owned: true,
                live: scrutinee_live,
            });
        }
        self.gen_move_into(scrutinee, &scrutinee_value);
        // Match ergonomics: when the scrutinee is a reference the tests and
        // bindings read through the pointer, and every binding becomes the
        // address of the matched place (see bind_pattern_mode). Nested
        // references are peeled one level per structure, which makes
        // `&&Option<T>` and `Option<T>` behave identically.
        let (source, scrutinee_ty, ref_mode) =
            peel_match_references(scrutinee_value.clone(), &scrutinee.ty);
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
            self.gen_match_tests(&arm.pat, &source, &matched);
            self.w.line(format!("if ({}) {{", matched));
            self.w.indent();
            // A whole-value binding of a non-Copy scrutinee moves the value out
            // of the owning local above. Copying instead would release the value
            // twice (and run a Drop hook twice) because the scrutinee local is
            // released at the end of the match. This mirrors Rust: a by-value
            // binding in a pattern moves, it does not clone.
            let moved_whole = match &arm.pat.kind {
                HPatKind::Bind { c_name, .. }
                    if !ref_mode && !self.is_trivial_copy(&arm.pat.ty) =>
                {
                    self.w.line(format!("{} = {};", c_name, source));
                    self.w.line(format!(
                        "{} = {};",
                        source,
                        self.raw_zero_expr(&scrutinee_ty)
                    ));
                    self.set_live(c_name, true);
                    self.set_live(&scrutinee_value, false);
                    Some(c_name.clone())
                }
                _ => None,
            };
            let mut guard_consume = false;
            if moved_whole.is_none() {
                // A pattern that binds a Drop-hook payload must move it out of
                // the scrutinee: copying would leave the payload owned by the
                // binding and by the aggregate at the same time, so the Drop
                // hook would run twice (a File-like handle would close twice).
                // Such patterns consume every selected value and the scrutinee
                // local is marked moved, so its release helper does not drop the
                // payload a second time.
                let drop_hook_container = self.program.drop_hook(&scrutinee_ty).is_none();
                let moves_payload = self.pattern_moves_drop_payload(&arm.pat);
                let consume =
                    !ref_mode && arm.guard.is_none() && drop_hook_container && moves_payload;
                // A guarded arm cannot consume yet: the guard must be able to
                // read the bindings first, and when it rejects the arm the
                // payloads belong to the remaining arms. The arm binds shallow
                // aliases now and steals the payloads once the guard accepts.
                guard_consume =
                    !ref_mode && arm.guard.is_some() && drop_hook_container && moves_payload;
                if guard_consume {
                    self.bind_pattern_borrow(&arm.pat, &source);
                } else if consume {
                    self.bind_pattern_consume(&arm.pat, &source);
                    self.set_live(&scrutinee_value, false);
                } else {
                    // A consuming arm without a guard binds by move: Rust
                    // pattern bindings take ownership of the matched values
                    // instead of cloning them. Guarded arms keep the copying
                    // form because their bindings exist before the guard
                    // result is known.
                    self.bind_pattern_mode(
                        &arm.pat,
                        &source,
                        ref_mode,
                        !ref_mode && arm.guard.is_none(),
                    );
                }
            }
            let guard_frame = self.frames.len();
            self.push_frame();
            match &arm.guard {
                Some(guard) => self.gen_into(guard, &pass),
                None => self.w.line(format!("{} = true;", pass)),
            }
            self.w.line(format!("if ({}) {{", pass));
            self.w.indent();
            if guard_consume {
                // The guard accepted the arm, so the payloads can be moved
                // into the bindings now. The scrutinee local gives up its
                // ownership; nothing is released on the rejected path above.
                self.bind_pattern_consume(&arm.pat, &source);
                self.set_live(&scrutinee_value, false);
            }
            let body_frame = self.frames.len();
            self.push_frame();
            self.gen_move_into(&arm.body, target);
            self.frame_cleanup(body_frame);
            self.pop_frame();
            self.w.dedent();
            self.w.line("}");
            if let Some(c_name) = &moved_whole
                && arm.guard.is_some()
            {
                // The guard rejected the arm: give the value back to the
                // scrutinee local so the remaining arms can still match it.
                // The binding is cleared and therefore not dropped.
                self.w.line(format!("if (!{}) {{", pass));
                self.w.indent();
                self.w.line(format!("{} = {};", source, c_name));
                self.w.line(format!(
                    "{} = {};",
                    c_name,
                    self.raw_zero_expr(&scrutinee_ty)
                ));
                self.set_live(c_name, false);
                self.set_live(&scrutinee_value, true);
                self.w.dedent();
                self.w.line("}");
            }
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
                // A string literal pattern compares against the immortal
                // interned copy: no allocation, no release, and embedded NUL
                // bytes compare exactly like the occurrence of the literal.
                let literal = self.interned_address(text);
                self.w.line(format!(
                    "if (!ls_str_eq(({}), ({}))) {{ {} = false; }}",
                    source, literal, matched
                ));
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
            HLit::Char(value) => {
                let literal = util::char_literal(*value);
                self.w.line(format!(
                    "if (({}) != ({})) {{ {} = false; }}",
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
        // A loop completion value of unit type is never observed; storing it
        // would also be ill-typed when the enclosing slot has another type
        // (for example a diverging loop used as a function tail).
        if !ty.is_unit() {
            let zero = self.zero_expr(ty);
            self.w.line(format!("{} = {};", target, zero));
        }
        self.loops.push(LoopCtx {
            break_label: break_label.clone(),
            continue_label: continue_label.clone(),
            body_frame: self.frames.len(),
            used_break: false,
            used_continue: false,
        });
        self.w.line("for (;;) {");
        self.w.indent();
        self.gen_loop_body(body, None, &continue_label, None);
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
        // A loop completion value of unit type is never observed; storing it
        // would also be ill-typed when the enclosing slot has another type
        // (for example a diverging loop used as a function tail).
        if !ty.is_unit() {
            let zero = self.zero_expr(ty);
            self.w.line(format!("{} = {};", target, zero));
        }
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
        self.gen_loop_body(body, None, &continue_label, None);
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
        // A loop completion value of unit type is never observed; storing it
        // would also be ill-typed when the enclosing slot has another type
        // (for example a diverging loop used as a function tail).
        if !ty.is_unit() {
            let zero = self.zero_expr(ty);
            self.w.line(format!("{} = {};", target, zero));
        }
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
        self.gen_loop_body(
            body,
            Some((pat, index.as_str(), false)),
            &continue_label,
            None,
        );
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
        // A loop completion value of unit type is never observed; storing it
        // would also be ill-typed when the enclosing slot has another type
        // (for example a diverging loop used as a function tail).
        if !ty.is_unit() {
            let zero = self.zero_expr(ty);
            self.w.line(format!("{} = {};", target, zero));
        }
        // The iterable is consumed: the vector is moved into an owned
        // temporary and each non-Copy element is transferred out of it into
        // the loop binding. When the loop ends the shell is emptied so that
        // releasing it only frees the element buffer.
        let vec_value = self.declare_temp(&vec.ty);
        self.gen_move_into(vec, &vec_value);
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
        self.gen_loop_body(
            body,
            Some((pat, element.as_str(), true)),
            &continue_label,
            None,
        );
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
            // A break leaves the elements after the current index untransferred
            // in the consumed vector. Release them here so that they are
            // dropped exactly once; transferred slots were already zeroed, so
            // start after the current element. When the loop finishes normally
            // the index equals the length and the loop body executes zero
            // times.
            let element_ty = match &vec.ty {
                Ty::Vec(inner) => (**inner).clone(),
                _ => Ty::Unit,
            };
            if !self.is_trivial_copy(&element_ty) {
                let counter = self.fresh("k");
                self.w.line(format!(
                    "for (size_t {} = {} + 1; {} < ({}).len; {} += 1) {{",
                    counter, index, counter, vec_value, counter
                ));
                self.w.indent();
                let slot = format!("({}).data[{}]", vec_value, counter);
                self.emit_release(&element_ty, &slot);
                let zero = self.raw_zero_expr(&element_ty);
                self.w.line(format!("{} = {};", slot, zero));
                self.w.dedent();
                self.w.line("}");
            }
        }
        self.w.line(format!("({}).len = 0;", vec_value));
        self.block_terminated = false;
        self.fn_terminated = false;
    }

    /// Emit the `{ ... }` block of a loop body, binding the loop pattern (if
    /// any) to `pat_source` before the statements.
    /// Emit a lazy `for ... in iterator` loop.
    ///
    /// The iterator value is evaluated once, moved into a local temporary and
    /// polled through the instantiated `next` method. The iterator and the
    /// polled option live in a dedicated frame so that every exit path releases
    /// the iterator exactly once: a normal exit and a `break` both fall through
    /// to the frame cleanup after the loop, a `continue` keeps the frame alive,
    /// and an early `return` releases every frame including this one. Panics
    /// abort the process and need no cleanup.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn gen_for_iter(
        &mut self,
        iter: &HExpr,
        next_fn: usize,
        pat: &HPat,
        body: &HBlock,
        ty: &Ty,
        target: &str,
    ) {
        let break_label = self.fresh_label("for_end");
        let continue_label = self.fresh_label("for_next");
        // A loop completion value of unit type is never observed; storing it
        // would also be ill-typed when the enclosing slot has another type
        // (for example a diverging loop used as a function tail).
        if !ty.is_unit() {
            let zero = self.zero_expr(ty);
            self.w.line(format!("{} = {};", target, zero));
        }
        let Some((next_c_name, option_ty, param_ty)) = self.program.fns.get(next_fn).map(|item| {
            (
                item.c_name.clone(),
                item.ret.clone(),
                item.params.first().map(|param| param.ty.clone()),
            )
        }) else {
            return;
        };
        self.push_frame();
        let frame = self.frames.len() - 1;
        let iterator = self.declare_temp(&iter.ty);
        self.gen_move_into(iter, &iterator);
        let polled = self.declare_temp(&option_ty);
        let self_ty = match param_ty {
            Some(param_ty) => param_ty.ref_inner().cloned().unwrap_or(param_ty),
            None => iter.ty.clone(),
        };
        let pointer_ctype = format!("{}*", self.ctype(&self_ty));
        self.loops.push(LoopCtx {
            break_label: break_label.clone(),
            continue_label: continue_label.clone(),
            body_frame: self.frames.len(),
            used_break: false,
            used_continue: false,
        });
        self.w.line("for (;;) {");
        self.w.indent();
        self.w.line(format!(
            "{} = {}(({})(&({})));",
            polled, next_c_name, pointer_ctype, iterator
        ));
        let tag = format!("({}).{}", polled, types::TAG_FIELD);
        let payload = format!(
            "({}).{}.{}",
            polled,
            types::UNION_FIELD,
            types::OPTION_SOME_MEMBER
        );
        self.w.line(format!("if ({} == 0) break;", tag));
        self.gen_loop_body(
            body,
            Some((pat, payload.as_str(), true)),
            &continue_label,
            Some(tag.as_str()),
        );
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
        self.frame_cleanup(frame);
        self.pop_frame();
        self.block_terminated = false;
        self.fn_terminated = false;
    }
    pub(crate) fn gen_loop_body(
        &mut self,
        body: &HBlock,
        pat: Option<(&HPat, &str, bool)>,
        continue_label: &str,
        clear_slot: Option<&str>,
    ) {
        self.w.line("{");
        self.w.indent();
        let body_frame = self.frames.len();
        self.push_frame();
        self.predeclare_block(body);
        if let Some((pattern, source, owned_transfer)) = pat {
            self.predeclare_pattern(pattern);
            let source = source.to_string();
            if owned_transfer && !self.is_trivial_copy(&pattern.ty) {
                self.bind_owned_pattern(pattern, &source);
            } else {
                self.bind_pattern(pattern, &source);
            }
            // A lazy loop scrutinee (the Option returned by the instantiated
            // next method) transferred its payload into the binding above.
            // Clearing the discriminant keeps the scrutinee from releasing the
            // payload a second time when its scope ends.
            if let Some(slot) = clear_slot {
                self.w.line(format!("{} = 0;", slot));
            }
        }
        self.gen_stmts(&body.stmts);
        if !self.block_terminated
            && !self.fn_terminated
            && let Some(tail) = &body.tail
        {
            let tail_ty = tail.ty.clone();
            let discard = self.declare_temp(&tail_ty);
            self.gen_move_into(tail, &discard);
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

    /// Bind a pattern to a place whose value is transferred out of an owned
    /// aggregate (the element of a consumed for-loop vector).
    ///
    /// A plain binding takes ownership of the selected value and the place is
    /// zeroed so that releasing the aggregate cannot release it a second time.
    /// Structural patterns copy their parts out of the place and then release
    /// the whole value (the parts retain their own references).
    fn bind_owned_pattern(&mut self, pat: &HPat, source: &str) {
        if let HPatKind::Bind { c_name, .. } = &pat.kind {
            let name = c_name.clone();
            self.emit_release_guarded(&pat.ty, &name, self.live_flag(&name).as_deref());
            self.w.line(format!("{} = {};", name, source));
            let zero = self.raw_zero_expr(&pat.ty);
            self.w.line(format!("{} = {};", source, zero));
            self.set_live(&name, true);
        } else {
            self.bind_pattern(pat, source);
            self.emit_release(&pat.ty, source);
            let zero = self.raw_zero_expr(&pat.ty);
            self.w.line(format!("{} = {};", source, zero));
        }
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
            Some(value) => self.gen_move_into(value, &result),
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

/// Whether `pat` binds at least one local variable.
///
/// A pattern that binds nothing (for example `_` or `(_, _)`) still evaluates
/// its scrutinee, but produces no binding to clean up, so the initialiser can
/// be evaluated and discarded as a whole.
fn pattern_binds_any(pat: &HPat) -> bool {
    match &pat.kind {
        HPatKind::Wild | HPatKind::Lit(_) | HPatKind::OptionNone => false,
        HPatKind::Bind { .. } => true,
        HPatKind::Tuple(parts) | HPatKind::Or(parts) => parts.iter().any(pattern_binds_any),
        HPatKind::Struct { fields, .. } => fields.iter().any(|(_, part)| pattern_binds_any(part)),
        HPatKind::Enum { args, .. } => args.iter().any(pattern_binds_any),
        HPatKind::OptionSome(inner) | HPatKind::ResultOk(inner) | HPatKind::ResultErr(inner) => {
            pattern_binds_any(inner)
        }
    }
}

/// Peel the reference layers between a match scrutinee (or a let initialiser)
/// and the place a pattern reads.
///
/// Match ergonomics make every binding a reference to the selected place, and
/// nested structures peel one reference layer each. References to unsized
/// referents stop the peeling: their fat values are already stored by value.
fn peel_match_references(mut source: String, ty: &Ty) -> (String, Ty, bool) {
    let mut scrutinee_ty = ty.clone();
    let mut ref_mode = false;
    while let Ty::Ref { inner, .. } = scrutinee_ty.clone() {
        if inner.is_unsized() {
            break;
        }
        source = format!("(*({}))", source);
        scrutinee_ty = *inner;
        ref_mode = true;
    }
    (source, scrutinee_ty, ref_mode)
}
