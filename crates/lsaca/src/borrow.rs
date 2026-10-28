//! The NLL-lite borrow checker for references.
//!
//! lsaca manages memory with ORC (ownership plus reference counting): every
//! owned value is reference counted, values are freely copyable and no value
//! is ever moved out from under its owner. The borrow checker therefore does
//! *not* have to prove memory safety - the runtime owns every string, vector
//! and aggregate. What it proves instead is the aliasing discipline that makes
//! references sound and predictable:
//!
//! * a shared borrow may coexist with any number of other shared borrows;
//! * a mutable borrow is exclusive: no other borrow and no direct access to
//!   the borrowed place may happen while it is live;
//! * writes through a mutable reference are exclusive: a reference derived
//!   from it may not be live at the same time;
//! * a reference may never outlive the value it points at;
//! * a function may not return a reference to one of its own locals, and a
//!   reference returned from a call is attributed to the parameter the return
//!   lifetime was elided from.
//!
//! The analysis is a single forward pass over the HIR per function and per
//! closure. Every borrow expression creates a loan over a Target: a local, a
//! borrowed parameter or a captured variable, plus a projection path of
//! struct/tuple fields. A monotonically increasing event index describes the
//! program point of every loan, read and write. A loan of a reference-typed
//! local is adopted by the binding, so its region is the local live range (up
//! to its last use, NLL-style); references that are copied out of other
//! references derive a new loan whose parent is the source loan, which gives
//! us reborrow families for the exclusivity rules.
//!
//! This is deliberately a conservative approximation of Rust NLL:
//!
//! * dynamic indexing collapses to the container root target;
//! * control flow joins conservatively (a reference that may come from one of
//!   several places loses its provenance);
//! * lifetimes are regions only, not generic parameters.
//!
//! The reported codes follow rustc: E0499 (two mutable borrows), E0502
//! (mutable borrow while a shared borrow is live, or a read through a mutable
//! borrow), E0506 (assignment to a borrowed value), E0515 (returned reference
//! to a local or to the wrong parameter) and E0597 (borrow outlives its
//! referent).

use std::collections::HashMap;

use crate::diag::{Diagnostic, Diagnostics, sort_and_dedup};
use crate::hir::{
    HArm, HBlock, HBuiltinMethod, HClosure, HExpr, HExprKind, HFieldRef, HFn, HFormatPartKind,
    HPat, HPatKind, HStmt, Program,
};
use crate::span::Span;
use crate::types::{SelfKind, Ty};

/// Stable id for a variable name inside one function body.
type NameId = usize;
/// Index into the loan table of the current function.
type LoanId = usize;

/// The place a loan covers.
///
/// Local is a plain local variable (including by-value parameters), Param is a
/// parameter that is itself a reference (&T / &mut T, and the &self / &mut
/// self receiver) and Capture is a captured variable inside a closure body.
/// The Vec of u32 is a projection path of struct/tuple field indices; an empty
/// path means the whole place. Dynamic indexing is deliberately not
/// represented: borrowing v[i] covers all of v.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Target {
    Local(NameId, Vec<u32>),
    Param(NameId, Vec<u32>),
    Capture(usize),
    Opaque,
}

impl Target {
    fn opaque() -> Target {
        Target::Opaque
    }
}

/// Append a field projection to a target. Captures and opaque targets have no
/// usable projection, so they stay conservative.
fn push_field(target: &mut Target, field: u32) {
    match target {
        Target::Local(_, path) | Target::Param(_, path) => path.push(field),
        Target::Capture(_) | Target::Opaque => {}
    }
}

/// Two targets overlap when they denote the same place or one is a prefix of
/// the other (borrowing a whole struct conflicts with borrowing a field of it,
/// but two different fields do not conflict).
fn targets_overlap(a: &Target, b: &Target) -> bool {
    match (a, b) {
        (Target::Local(x, px), Target::Local(y, py)) => x == y && paths_prefix(px, py),
        (Target::Param(x, px), Target::Param(y, py)) => x == y && paths_prefix(px, py),
        (Target::Capture(x), Target::Capture(y)) => x == y,
        _ => false,
    }
}

fn paths_prefix(a: &[u32], b: &[u32]) -> bool {
    let len = a.len().min(b.len());
    a[..len] == b[..len]
}

/// A region of code during which a place is borrowed.
#[derive(Clone, Debug)]
struct Loan {
    target: Target,
    mutable: bool,
    /// First event index of the loan (the borrow expression or the parameter).
    start: usize,
    /// Last event index of the loan; usize::MAX until finalization.
    end: usize,
    /// The local that holds the reference, when the loan is owned by one.
    owner: Option<NameId>,
    /// The loan this one was derived from, if any (reborrow family).
    parent: Option<LoanId>,
    span: Span,
}

/// One write to a place.
#[derive(Clone, Debug)]
struct Mutation {
    target: Target,
    /// When the write goes through a reference, the local holding it.
    via: Option<NameId>,
    idx: usize,
    span: Span,
}

/// One read of a place (a value use, not a read through a reference).
#[derive(Clone, Debug)]
struct Read {
    target: Target,
    idx: usize,
    span: Span,
}

/// What a reference value points at.
#[derive(Clone, Debug)]
struct Provenance {
    target: Target,
    mutable: bool,
    /// The loan that owns this reference value, when known.
    loan: Option<LoanId>,
}

impl Provenance {
    /// A child provenance for the given projection path, keeping the loan.
    fn project(&self, path: &[u32]) -> Provenance {
        let mut target = self.target.clone();
        for code in path {
            push_field(&mut target, *code);
        }
        Provenance {
            target,
            mutable: self.mutable,
            loan: self.loan,
        }
    }
}

struct FnChecker<'a> {
    program: &'a Program,
    ret: Ty,
    self_kind: Option<SelfKind>,
    /// Name id of the receiver when this is a method.
    receiver: Option<NameId>,
    names: Vec<String>,
    name_ids: HashMap<String, NameId>,
    idx: usize,
    loans: Vec<Loan>,
    mutations: Vec<Mutation>,
    reads: Vec<Read>,
    last_use: HashMap<NameId, usize>,
    provenance: HashMap<NameId, Provenance>,
    /// Loan of every reference-typed local.
    ref_loan: HashMap<NameId, LoanId>,
    /// Loan created by every borrow expression, keyed by its HIR address.
    ptr_loan: HashMap<usize, LoanId>,
    /// Scope end event and declaration span of every local.
    scope_end: HashMap<NameId, (usize, Span)>,
    decl_span: HashMap<NameId, Span>,
    /// Inclusive event ranges of loops, used to extend loans that live across
    /// an iteration boundary.
    loop_ranges: Vec<(usize, usize)>,
    decl_stack: Vec<Vec<NameId>>,
    /// Reference-typed parameters of the current function or closure.
    ref_params: Vec<NameId>,
    /// By-value parameters of the current function or closure (for messages).
    by_value_params: Vec<NameId>,
    diagnostics: Diagnostics,
}

/// Check the borrow discipline of a whole HIR program.
///
/// The pass is read-only over the HIR and never panics on malformed input: it
/// reports errors through the returned Diagnostics.
pub fn check(program: &Program) -> Diagnostics {
    let mut diagnostics = Diagnostics::new();
    for function in &program.fns {
        let mut checker = FnChecker::new(program);
        checker.check_function(function);
        diagnostics.extend(checker.finish());
    }
    for closure in &program.closures {
        let mut checker = FnChecker::new(program);
        checker.check_closure(closure);
        diagnostics.extend(checker.finish());
    }
    sort_and_dedup(&mut diagnostics);
    diagnostics
}

impl<'a> FnChecker<'a> {
    fn new(program: &'a Program) -> FnChecker<'a> {
        FnChecker {
            program,
            ret: Ty::Unit,
            self_kind: None,
            receiver: None,
            names: Vec::new(),
            name_ids: HashMap::new(),
            idx: 0,
            loans: Vec::new(),
            mutations: Vec::new(),
            reads: Vec::new(),
            last_use: HashMap::new(),
            provenance: HashMap::new(),
            ref_loan: HashMap::new(),
            ptr_loan: HashMap::new(),
            scope_end: HashMap::new(),
            decl_span: HashMap::new(),
            loop_ranges: Vec::new(),
            decl_stack: Vec::new(),
            ref_params: Vec::new(),
            by_value_params: Vec::new(),
            diagnostics: Diagnostics::new(),
        }
    }

    fn finish(self) -> Diagnostics {
        self.diagnostics
    }

    // -- small utilities ---------------------------------------------------

    fn tick(&mut self) -> usize {
        let current = self.idx;
        self.idx = self.idx.saturating_add(1);
        current
    }

    fn intern(&mut self, c_name: &str) -> NameId {
        if let Some(id) = self.name_ids.get(c_name) {
            return *id;
        }
        let id = self.names.len();
        self.names.push(c_name.to_string());
        self.name_ids.insert(c_name.to_string(), id);
        id
    }

    /// Intern a variable by its unique C name and remember the source-level
    /// name used in diagnostics. Internal names such as `v1` must never leak
    /// into a message shown to the user.
    fn register(&mut self, c_name: &str, name: &str) -> NameId {
        let id = self.intern(c_name);
        if let Some(slot) = self.names.get_mut(id) {
            *slot = name.to_string();
        }
        id
    }

    fn lookup(&self, name: &str) -> Option<NameId> {
        self.name_ids.get(name).copied()
    }

    fn name(&self, id: NameId) -> &str {
        self.names
            .get(id)
            .map(String::as_str)
            .unwrap_or("<variable>")
    }

    fn use_local(&mut self, name: &str, idx: usize) {
        if let Some(id) = self.lookup(name) {
            let entry = self.last_use.entry(id).or_insert(idx);
            if *entry < idx {
                *entry = idx;
            }
        }
    }

    fn declare(&mut self, id: NameId, span: Span) {
        if let Some(scope) = self.decl_stack.last_mut() {
            scope.push(id);
        }
        self.decl_span.entry(id).or_insert(span);
    }

    fn push_scope(&mut self) {
        self.decl_stack.push(Vec::new());
    }

    fn pop_scope(&mut self, end: usize) {
        let names = self.decl_stack.pop().unwrap_or_default();
        for id in names {
            let span = self.decl_span.get(&id).copied().unwrap_or_else(empty_span);
            self.scope_end.insert(id, (end, span));
        }
    }

    fn push_loan(
        &mut self,
        target: Target,
        mutable: bool,
        span: Span,
        owner: Option<NameId>,
        parent: Option<LoanId>,
        start: usize,
    ) -> LoanId {
        let id = self.loans.len();
        self.loans.push(Loan {
            target,
            mutable,
            start,
            end: usize::MAX,
            owner,
            parent,
            span,
        });
        id
    }

    /// Close an inline loan at the end of the statement that created it.
    fn finalize_inline(&mut self, first: usize, end: usize) {
        for loan in self.loans.iter_mut().skip(first) {
            if loan.end == usize::MAX && loan.owner.is_none() {
                loan.end = end;
            }
        }
    }

    fn finish_loan(&mut self, id: LoanId, end: usize) {
        if let Some(loan) = self.loans.get_mut(id)
            && loan.end == usize::MAX
        {
            loan.end = end;
        }
    }

    fn loan_of(&self, id: NameId) -> Option<LoanId> {
        self.ref_loan.get(&id).copied()
    }

    fn error(&mut self, span: Span, message: String, code: &'static str) {
        self.diagnostics
            .push(Diagnostic::error(message, span).with_code(code));
    }

    // -- entry points ------------------------------------------------------

    fn check_function(&mut self, function: &HFn) {
        self.ret = function.ret.clone();
        self.self_kind = function.self_kind;
        let span = function.span;
        let start = self.idx;
        for (index, param) in function.params.iter().enumerate() {
            let id = self.register(&param.c_name, &param.name);
            if index == 0 && function.self_kind.is_some() {
                self.receiver = Some(id);
            }
            let borrowed = param.ty.is_ref()
                || (index == 0
                    && matches!(
                        function.self_kind,
                        Some(SelfKind::Ref) | Some(SelfKind::RefMut)
                    ));
            if borrowed {
                self.ref_params.push(id);
                let mutable = param.ty.is_ref_mut()
                    || (index == 0 && function.self_kind == Some(SelfKind::RefMut));
                let loan = self.push_loan(
                    Target::Param(id, Vec::new()),
                    mutable,
                    span,
                    Some(id),
                    None,
                    start,
                );
                self.provenance.insert(
                    id,
                    Provenance {
                        target: Target::Param(id, Vec::new()),
                        mutable,
                        loan: Some(loan),
                    },
                );
                self.ref_loan.insert(id, loan);
            } else {
                self.by_value_params.push(id);
            }
        }
        self.walk_block(&function.body);
        let end = self.tick();
        for param in &function.params {
            if let Some(id) = self.lookup(&param.c_name) {
                self.scope_end
                    .insert(id, (end, self.decl_span.get(&id).copied().unwrap_or(span)));
            }
        }
        if let Some(tail) = &function.body.tail {
            self.check_escape(tail);
        }
        self.finish_analysis();
    }

    fn check_closure(&mut self, closure: &HClosure) {
        self.ret = closure.ret.clone();
        let start = self.idx;
        for (index, capture) in closure.captures.iter().enumerate() {
            let id = self.intern(&capture.name);
            if capture.ty.is_ref() {
                let loan = self.push_loan(
                    Target::Capture(index),
                    capture.ty.is_ref_mut(),
                    closure.span,
                    Some(id),
                    None,
                    start,
                );
                self.provenance.insert(
                    id,
                    Provenance {
                        target: Target::Capture(index),
                        mutable: capture.ty.is_ref_mut(),
                        loan: Some(loan),
                    },
                );
                self.ref_loan.insert(id, loan);
            }
        }
        for param in &closure.params {
            let id = self.register(&param.c_name, &param.name);
            if param.ty.is_ref() {
                self.ref_params.push(id);
                let loan = self.push_loan(
                    Target::Param(id, Vec::new()),
                    param.ty.is_ref_mut(),
                    closure.span,
                    Some(id),
                    None,
                    start,
                );
                self.provenance.insert(
                    id,
                    Provenance {
                        target: Target::Param(id, Vec::new()),
                        mutable: param.ty.is_ref_mut(),
                        loan: Some(loan),
                    },
                );
                self.ref_loan.insert(id, loan);
            } else {
                self.by_value_params.push(id);
            }
        }
        self.walk_block(&closure.body);
        let end = self.tick();
        for param in &closure.params {
            if let Some(id) = self.lookup(&param.c_name) {
                self.scope_end.insert(
                    id,
                    (
                        end,
                        self.decl_span.get(&id).copied().unwrap_or(closure.span),
                    ),
                );
            }
        }
        if let Some(tail) = &closure.body.tail {
            self.check_escape(tail);
        }
        self.finish_analysis();
    }

    fn finish_analysis(&mut self) {
        self.finalize_loan_ends();
        let mut diagnostics = Vec::new();
        diagnostics.extend(self.scope_diagnostics());
        diagnostics.extend(self.pair_diagnostics());
        diagnostics.extend(self.mutation_diagnostics());
        diagnostics.extend(self.read_diagnostics());
        for diagnostic in diagnostics {
            self.diagnostics.push(diagnostic);
        }
    }

    // -- loan finalization -------------------------------------------------

    fn finalize_loan_ends(&mut self) {
        let loans_len = self.loans.len();
        for index in 0..loans_len {
            let mut end = self.loans[index].end;
            if end == usize::MAX {
                end = match self.loans[index].owner {
                    Some(owner) => {
                        let used = self.last_use.get(&owner).copied().unwrap_or(0);
                        used.max(self.loans[index].start)
                    }
                    None => self.loans[index].start,
                };
            }
            for &(loop_start, loop_end) in &self.loop_ranges {
                if self.loans[index].start <= loop_start && end >= loop_start {
                    end = end.max(loop_end);
                }
            }
            self.loans[index].end = end;
        }
    }

    fn interval_overlaps(a: &Loan, b: &Loan) -> bool {
        a.start <= b.end && b.start <= a.end
    }

    fn is_ancestor(&self, ancestor: LoanId, loan: LoanId) -> bool {
        let mut current = self.loans.get(loan).and_then(|item| item.parent);
        while let Some(parent) = current {
            if parent == ancestor {
                return true;
            }
            current = self.loans.get(parent).and_then(|item| item.parent);
        }
        false
    }

    // -- diagnostics -------------------------------------------------------

    fn describe_target(&self, target: &Target) -> String {
        match target {
            Target::Local(id, _) | Target::Param(id, _) => self.name(*id).to_string(),
            Target::Capture(_) => "captured variable".to_string(),
            Target::Opaque => "value".to_string(),
        }
    }

    fn scope_diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for loan in &self.loans {
            let name_id = match &loan.target {
                Target::Local(id, _) => *id,
                _ => continue,
            };
            let Some(&(scope_end, decl_span)) = self.scope_end.get(&name_id) else {
                continue;
            };
            if scope_end == usize::MAX || loan.end <= scope_end {
                continue;
            }
            let name = self.name(name_id);
            let diagnostic =
                Diagnostic::error(format!("{} does not live long enough", name), loan.span)
                    .with_code("E0597")
                    .with_label(decl_span, format!("{} is dropped here", name))
                    .with_note(
                        "the reference must not outlive the value it points to; \
                         keep the referent alive longer or copy the value"
                            .to_string(),
                    );
            diagnostics.push(diagnostic);
        }
        diagnostics
    }

    fn pair_diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for i in 0..self.loans.len() {
            for j in (i + 1)..self.loans.len() {
                let first = &self.loans[i];
                let second = &self.loans[j];
                if !targets_overlap(&first.target, &second.target) {
                    continue;
                }
                if !Self::interval_overlaps(first, second) {
                    continue;
                }
                if self.is_ancestor(i, j) || self.is_ancestor(j, i) {
                    continue;
                }
                match (first.mutable, second.mutable) {
                    (true, true) => {
                        let (earlier, later) = if first.start <= second.start {
                            (i, j)
                        } else {
                            (j, i)
                        };
                        let name = self.describe_target(&self.loans[later].target);
                        diagnostics.push(
                            Diagnostic::error(
                                format!(
                                    "cannot borrow {} as mutable more than once at a time",
                                    name
                                ),
                                self.loans[later].span,
                            )
                            .with_code("E0499")
                            .with_label(
                                self.loans[earlier].span,
                                "first mutable borrow occurs here",
                            ),
                        );
                    }
                    (true, false) | (false, true) => {
                        let (mutable, shared) = if first.mutable { (i, j) } else { (j, i) };
                        let name = self.describe_target(&self.loans[mutable].target);
                        let later = if self.loans[mutable].start > self.loans[shared].start {
                            mutable
                        } else {
                            shared
                        };
                        diagnostics.push(
                            Diagnostic::error(
                                format!(
                                    "cannot borrow {} as immutable because it is also \
                                     borrowed as mutable",
                                    name
                                ),
                                self.loans[later].span,
                            )
                            .with_code("E0502")
                            .with_label(self.loans[mutable].span, "mutable borrow occurs here"),
                        );
                    }
                    (false, false) => {}
                }
            }
        }
        diagnostics
    }

    fn mutation_diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for mutation in &self.mutations {
            if matches!(mutation.target, Target::Opaque) {
                continue;
            }
            let mut conflict: Option<LoanId> = None;
            for (index, loan) in self.loans.iter().enumerate() {
                if !targets_overlap(&mutation.target, &loan.target) {
                    continue;
                }
                if mutation.idx < loan.start || mutation.idx > loan.end {
                    continue;
                }
                // Writing through a reference is a self-write of the loan
                // that reference owns, and of every loan that loan is a
                // reborrow of (the reference may have been derived from a
                // `&mut self` receiver or a `&mut` parameter).
                let allowed = match mutation.via {
                    Some(via) => match self.loan_of(via) {
                        Some(via_loan) => {
                            via_loan == index
                                || self.is_ancestor(via_loan, index)
                                || self.is_ancestor(index, via_loan)
                        }
                        None => false,
                    },
                    None => false,
                };
                if !allowed {
                    conflict = Some(index);
                    break;
                }
            }
            let Some(loan_id) = conflict else {
                continue;
            };
            let loan = &self.loans[loan_id];
            let name = self.describe_target(&mutation.target);
            let label = if loan.mutable {
                "mutable borrow occurs here"
            } else {
                "shared borrow occurs here"
            };
            diagnostics.push(
                Diagnostic::error(
                    format!("cannot assign to {} because it is borrowed", name),
                    mutation.span,
                )
                .with_code("E0506")
                .with_label(loan.span, label),
            );
        }
        diagnostics
    }

    fn read_diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for read in &self.reads {
            if matches!(read.target, Target::Opaque) {
                continue;
            }
            for loan in &self.loans {
                if !loan.mutable {
                    continue;
                }
                if !targets_overlap(&read.target, &loan.target) {
                    continue;
                }
                if read.idx < loan.start || read.idx > loan.end {
                    continue;
                }
                let name = self.describe_target(&read.target);
                diagnostics.push(
                    Diagnostic::error(
                        format!(
                            "cannot borrow {} as immutable because it is also \
                             borrowed as mutable",
                            name
                        ),
                        read.span,
                    )
                    .with_code("E0502")
                    .with_label(loan.span, "mutable borrow occurs here"),
                );
                break;
            }
        }
        diagnostics
    }

    // -- escape checking ---------------------------------------------------

    fn expected_return_target(&self) -> Option<NameId> {
        match self.self_kind {
            Some(SelfKind::Ref) | Some(SelfKind::RefMut) => self.receiver,
            Some(SelfKind::Value) | None => {
                if self.ref_params.len() == 1 {
                    self.ref_params.first().copied()
                } else {
                    None
                }
            }
        }
    }

    fn check_escape(&mut self, expr: &HExpr) {
        if !self.ret.is_ref() {
            return;
        }
        // Every result position must be checked: the returned value may come
        // from any branch of a control-flow expression, while
        // provenance_value() only reports the first branch with a known
        // provenance. Checking the branches themselves keeps a reference to a
        // block-local from escaping through the branch it does not report.
        match &expr.kind {
            HExprKind::Block(block) => {
                if let Some(tail) = &block.tail {
                    self.check_escape(tail);
                }
                return;
            }
            HExprKind::If {
                then_block,
                else_expr,
                ..
            } => {
                if let Some(tail) = &then_block.tail {
                    self.check_escape(tail);
                }
                if let Some(else_expr) = else_expr {
                    self.check_escape(else_expr);
                }
                return;
            }
            HExprKind::Match { arms, .. } => {
                for arm in arms {
                    self.check_escape(&arm.body);
                }
                return;
            }
            _ => {}
        }
        let Some(expected) = self.expected_return_target() else {
            return;
        };
        let provenance = self.provenance_value(expr);
        match &provenance.target {
            Target::Local(id, _) => {
                let name = self.name(*id).to_string();
                let kind = if self.by_value_params.contains(id) {
                    "function parameter"
                } else {
                    "local variable"
                };
                self.error(
                    expr.span,
                    format!("cannot return a reference to {} {}", kind, name),
                    "E0515",
                );
            }
            Target::Capture(_) => {
                self.error(
                    expr.span,
                    "cannot return a reference to a captured variable".to_string(),
                    "E0515",
                );
            }
            Target::Param(id, _) => {
                if *id != expected {
                    let returned = self.name(*id).to_string();
                    let wanted = self.name(expected).to_string();
                    self.error(
                        expr.span,
                        format!(
                            "cannot return a reference derived from {} when the return \
                             type borrows from {}",
                            returned, wanted
                        ),
                        "E0515",
                    );
                }
            }
            Target::Opaque => {}
        }
    }

    // -- place analysis ----------------------------------------------------

    fn place_root(&self, id: NameId) -> Target {
        let is_ref_receiver = self.receiver == Some(id)
            && matches!(self.self_kind, Some(SelfKind::Ref) | Some(SelfKind::RefMut));
        if is_ref_receiver || self.ref_params.contains(&id) {
            Target::Param(id, Vec::new())
        } else {
            Target::Local(id, Vec::new())
        }
    }

    fn target_of_place(&self, expr: &HExpr) -> Target {
        match &expr.kind {
            HExprKind::Local { c_name, .. } => match self.lookup(c_name) {
                Some(id) => self.place_root(id),
                None => Target::opaque(),
            },
            HExprKind::Capture { index, .. } => Target::Capture(*index),
            HExprKind::Field { base, field } => {
                let mut target = self.target_of_place(base);
                push_field(&mut target, field_code(field));
                target
            }
            HExprKind::Index { base, .. } => self.target_of_place(base),
            HExprKind::Deref { expr } => self.provenance_value(expr).target,
            _ => {
                if expr.ty.is_ref() {
                    self.provenance_value(expr).target
                } else {
                    Target::opaque()
                }
            }
        }
    }

    fn mutation_of_place(&self, place: &HExpr) -> (Target, Option<NameId>) {
        match &place.kind {
            HExprKind::Local { c_name, .. } => {
                let Some(id) = self.lookup(c_name) else {
                    return (Target::opaque(), None);
                };
                let mut via = None;
                if self.receiver == Some(id) && self.self_kind == Some(SelfKind::RefMut) {
                    via = Some(id);
                }
                (self.place_root(id), via)
            }
            HExprKind::Capture { index, .. } => (Target::Capture(*index), None),
            HExprKind::Field { base, field } => {
                let (mut target, via) = self.mutation_of_place(base);
                push_field(&mut target, field_code(field));
                (target, via.or_else(|| self.root_reference(base)))
            }
            HExprKind::Index { base, .. } => {
                let (target, via) = self.mutation_of_place(base);
                (target, via.or_else(|| self.root_reference(base)))
            }
            HExprKind::Deref { expr } => {
                let provenance = self.provenance_value(expr);
                (provenance.target, self.root_reference(expr))
            }
            _ => (Target::opaque(), None),
        }
    }

    /// The reference-typed local through which a place is reached, if any.
    fn root_reference(&self, expr: &HExpr) -> Option<NameId> {
        match &expr.kind {
            HExprKind::Local { c_name, .. } => self.lookup(c_name),
            HExprKind::Field { base, .. } | HExprKind::Index { base, .. } => {
                self.root_reference(base)
            }
            HExprKind::Deref { expr } => self.root_reference(expr),
            _ => None,
        }
    }

    // -- provenance --------------------------------------------------------

    fn provenance_value(&self, expr: &HExpr) -> Provenance {
        let opaque = Provenance {
            target: Target::opaque(),
            mutable: false,
            loan: None,
        };
        match &expr.kind {
            HExprKind::Local { c_name, .. } => match self.lookup(c_name) {
                Some(id) => self.provenance.get(&id).cloned().unwrap_or(opaque),
                None => opaque,
            },
            HExprKind::Ref {
                mutable,
                expr: inner,
            } => {
                let loan = self.ptr_loan.get(&expr_address(expr)).copied();
                let target = self.target_of_place(inner);
                Provenance {
                    target,
                    mutable: *mutable,
                    loan,
                }
            }
            HExprKind::Block(block) => match &block.tail {
                Some(tail) => self.provenance_value(tail),
                None => opaque,
            },
            HExprKind::If {
                then_block,
                else_expr,
                ..
            } => {
                let mut provenances = Vec::new();
                if let Some(tail) = &then_block.tail {
                    provenances.push(self.provenance_value(tail));
                }
                if let Some(else_expr) = else_expr {
                    provenances.push(self.provenance_value(else_expr));
                }
                Self::merge_provenances(&provenances)
            }
            HExprKind::Match { arms, .. } => {
                let provenances: Vec<Provenance> = arms
                    .iter()
                    .map(|arm| self.provenance_value(&arm.body))
                    .collect();
                Self::merge_provenances(&provenances)
            }
            HExprKind::DirectCall {
                fn_id,
                receiver,
                args,
            } => self.call_return_provenance(*fn_id, receiver.as_deref(), args),
            _ => opaque,
        }
    }

    /// Combine the provenances of the result positions of a control-flow
    /// expression. A returned value may come from any of them, so a target
    /// that could be a local or captured variable wins over a parameter: the
    /// escape check must reject a reference that may point into a block-local.
    fn merge_provenances(provenances: &[Provenance]) -> Provenance {
        let mut local = None;
        let mut parameter = None;
        for provenance in provenances {
            match &provenance.target {
                Target::Local(..) | Target::Capture(_) => {
                    if local.is_none() {
                        local = Some(provenance.clone());
                    }
                }
                Target::Param(..) => {
                    if parameter.is_none() {
                        parameter = Some(provenance.clone());
                    }
                }
                Target::Opaque => {}
            }
        }
        local.or(parameter).unwrap_or(Provenance {
            target: Target::opaque(),
            mutable: false,
            loan: None,
        })
    }

    fn call_return_provenance(
        &self,
        fn_id: usize,
        receiver: Option<&HExpr>,
        args: &[HExpr],
    ) -> Provenance {
        let opaque = Provenance {
            target: Target::opaque(),
            mutable: false,
            loan: None,
        };
        let Some(function) = self.program.fns.get(fn_id) else {
            return opaque;
        };
        if !function.ret.is_ref() {
            return opaque;
        }
        let Some(source_index) = return_source_param(function) else {
            return opaque;
        };
        let source = if function.is_method() {
            if source_index == 0 {
                receiver
            } else {
                args.get(source_index - 1)
            }
        } else {
            args.get(source_index)
        };
        let Some(source) = source else {
            return opaque;
        };
        let mut provenance = if source.ty.is_ref() {
            self.provenance_value(source)
        } else {
            Provenance {
                target: self.target_of_place(source),
                mutable: false,
                loan: self.origin_loan(source),
            }
        };
        provenance.mutable = function.ret.is_ref_mut();
        provenance
    }

    fn origin_loan(&self, expr: &HExpr) -> Option<LoanId> {
        match &expr.kind {
            HExprKind::Ref { .. } => self.ptr_loan.get(&expr_address(expr)).copied(),
            HExprKind::Local { c_name, .. } => self.lookup(c_name).and_then(|id| self.loan_of(id)),
            // A field or element projection reborrows from the place it is
            // projected out of: either the loan behind an auto-dereferenced
            // reference, or the implicit loan of a `&mut self` receiver.
            HExprKind::Field { base, .. } | HExprKind::Index { base, .. } => self.origin_loan(base),
            HExprKind::Deref { expr } if expr.ty.is_ref() => self.origin_loan(expr),
            HExprKind::Block(block) => block
                .tail
                .as_deref()
                .and_then(|tail| self.origin_loan(tail)),
            HExprKind::If {
                then_block,
                else_expr,
                ..
            } => then_block
                .tail
                .as_deref()
                .and_then(|tail| self.origin_loan(tail))
                .or_else(|| else_expr.as_deref().and_then(|expr| self.origin_loan(expr))),
            HExprKind::Match { arms, .. } => {
                arms.iter().find_map(|arm| self.origin_loan(&arm.body))
            }
            HExprKind::DirectCall {
                fn_id,
                receiver,
                args,
            } => {
                let function = self.program.fns.get(*fn_id)?;
                let source_index = return_source_param(function)?;
                let source = if function.is_method() {
                    if source_index == 0 {
                        receiver.as_deref()
                    } else {
                        args.get(source_index - 1)
                    }
                } else {
                    args.get(source_index)
                };
                source.and_then(|source| self.origin_loan(source))
            }
            _ => None,
        }
    }

    // -- walking -----------------------------------------------------------

    fn walk_block(&mut self, block: &HBlock) {
        self.push_scope();
        for stmt in &block.stmts {
            self.walk_stmt(stmt);
        }
        if let Some(tail) = &block.tail {
            let _start = self.tick();
            let pending = self.loans.len();
            self.walk_expr(tail);
            let end = self.tick();
            self.finalize_inline(pending, end);
        }
        let end = self.tick();
        self.pop_scope(end);
    }

    fn walk_stmt(&mut self, stmt: &HStmt) {
        let _start = self.tick();
        let pending = self.loans.len();
        match stmt {
            HStmt::Let { pat, init, span } => {
                if let Some(init) = init {
                    self.walk_expr(init);
                }
                // Bindings the initializer produces start existing after it is
                // evaluated; without the extra tick a reference copied out of
                // the initializer would appear to be borrowed at the same
                // instant as a mutation performed while evaluating it.
                let _ = self.tick();
                self.declare_let(pat, init.as_ref(), *span);
            }
            HStmt::Expr(expr) => self.walk_expr(expr),
        }
        let end = self.tick();
        self.finalize_inline(pending, end);
    }

    fn declare_let(&mut self, pat: &HPat, init: Option<&HExpr>, span: Span) {
        if let HPatKind::Bind { c_name, name } = &pat.kind {
            let id = self.register(c_name, name);
            self.declare(id, pat.span);
            if pat.ty.is_ref() {
                self.bind_ref_local(id, &pat.ty, init, span);
            }
            return;
        }
        self.declare_pattern(pat, None);
    }

    /// Declare the bindings of a pattern, creating loans for reference-typed
    /// bindings that come from ref-mode match ergonomics.
    fn declare_pattern(&mut self, pat: &HPat, base: Option<&Provenance>) {
        match &pat.kind {
            HPatKind::Bind { c_name, name } => {
                let id = self.register(c_name, name);
                let already_declared = self
                    .decl_stack
                    .last()
                    .is_some_and(|scope| scope.contains(&id));
                if already_declared {
                    return;
                }
                self.declare(id, pat.span);
                if pat.ty.is_ref() {
                    let target = match base {
                        Some(base) => base.target.clone(),
                        None => Target::opaque(),
                    };
                    let mutable = pat.ty.is_ref_mut();
                    let parent = base.and_then(|base| base.loan);
                    let loan = self.push_loan(
                        target.clone(),
                        mutable,
                        pat.span,
                        Some(id),
                        parent,
                        self.idx,
                    );
                    self.provenance.insert(
                        id,
                        Provenance {
                            target,
                            mutable,
                            loan: Some(loan),
                        },
                    );
                    self.ref_loan.insert(id, loan);
                } else {
                    self.provenance.remove(&id);
                    self.ref_loan.remove(&id);
                }
            }
            HPatKind::Wild | HPatKind::Lit(_) | HPatKind::OptionNone => {}
            HPatKind::Tuple(elements) => {
                for (index, element) in elements.iter().enumerate() {
                    let child = base.map(|base| base.project(&[index as u32]));
                    self.declare_pattern(element, child.as_ref());
                }
            }
            HPatKind::Enum { variant, args, .. } => {
                for (index, element) in args.iter().enumerate() {
                    let path = [*variant as u32, index as u32];
                    let child = base.map(|base| base.project(&path));
                    self.declare_pattern(element, child.as_ref());
                }
            }
            HPatKind::OptionSome(inner) => {
                let child = base.map(|base| base.project(&[1, 0]));
                self.declare_pattern(inner, child.as_ref());
            }
            HPatKind::ResultOk(inner) => {
                let child = base.map(|base| base.project(&[0, 0]));
                self.declare_pattern(inner, child.as_ref());
            }
            HPatKind::ResultErr(inner) => {
                let child = base.map(|base| base.project(&[1, 0]));
                self.declare_pattern(inner, child.as_ref());
            }
            HPatKind::Struct { fields, .. } => {
                for (index, field) in fields {
                    let child = base.map(|base| base.project(&[*index as u32]));
                    self.declare_pattern(field, child.as_ref());
                }
            }
            HPatKind::Or(alternatives) => {
                for alternative in alternatives {
                    self.declare_pattern(alternative, base);
                }
            }
        }
    }

    fn bind_ref_local(&mut self, id: NameId, ty: &Ty, init: Option<&HExpr>, span: Span) {
        let provenance = match init {
            Some(init) => self.provenance_value(init),
            None => Provenance {
                target: Target::opaque(),
                mutable: false,
                loan: None,
            },
        };
        let mutable = ty.is_ref_mut();
        let adopted = match init {
            Some(init) => match &init.kind {
                HExprKind::Ref { .. } => self.ptr_loan.get(&expr_address(init)).copied(),
                _ => None,
            },
            None => None,
        };
        let loan = if let Some(loan_id) = adopted {
            if let Some(loan) = self.loans.get_mut(loan_id) {
                loan.owner = Some(id);
                loan.mutable = mutable;
            }
            loan_id
        } else {
            let parent = init.and_then(|init| self.origin_loan(init));
            self.push_loan(
                provenance.target.clone(),
                mutable,
                span,
                Some(id),
                parent,
                self.idx,
            )
        };
        self.provenance.insert(
            id,
            Provenance {
                target: provenance.target,
                mutable,
                loan: Some(loan),
            },
        );
        self.ref_loan.insert(id, loan);
    }

    /// Rebind an existing reference-typed local on assignment.
    fn rebind_local(&mut self, id: NameId, ty: &Ty, value: &HExpr) {
        if let Some(old) = self.loan_of(id) {
            let end = self.idx;
            self.finish_loan(old, end);
            if let Some(loan) = self.loans.get_mut(old) {
                loan.owner = None;
            }
        }
        let provenance = self.provenance_value(value);
        let mutable = ty.is_ref_mut();
        let parent = self.origin_loan(value);
        let loan = self.push_loan(
            provenance.target.clone(),
            mutable,
            value.span,
            Some(id),
            parent,
            self.idx,
        );
        self.provenance.insert(
            id,
            Provenance {
                target: provenance.target,
                mutable,
                loan: Some(loan),
            },
        );
        self.ref_loan.insert(id, loan);
    }

    fn walk_expr(&mut self, expr: &HExpr) {
        match &expr.kind {
            HExprKind::Lit(_) | HExprKind::ConstRef { .. } | HExprKind::FnRef { .. } => {}
            HExprKind::OptionNone | HExprKind::Break | HExprKind::Continue => {}
            HExprKind::Local { name, c_name } => {
                self.register(c_name, name);
                self.use_local(c_name, self.idx);
                let is_receiver = self.receiver.is_some() && self.receiver == self.lookup(c_name);
                if !expr.ty.is_ref()
                    && !is_receiver
                    && let Some(id) = self.lookup(c_name)
                {
                    let target = self.place_root(id);
                    self.reads.push(Read {
                        target,
                        idx: self.idx,
                        span: expr.span,
                    });
                }
            }
            HExprKind::Capture { index, .. } => {
                self.reads.push(Read {
                    target: Target::Capture(*index),
                    idx: self.idx,
                    span: expr.span,
                });
            }
            HExprKind::Closure { captures, .. } => {
                for capture in captures {
                    self.walk_expr(capture);
                }
            }
            HExprKind::Call { callee, args } => {
                self.walk_expr(callee);
                for arg in args {
                    self.walk_expr(arg);
                }
            }
            HExprKind::DirectCall {
                fn_id,
                receiver,
                args,
            } => {
                if let Some(receiver) = receiver {
                    self.walk_place(receiver);
                    if self.method_takes_mut_receiver(*fn_id) {
                        self.record_mutation(receiver);
                    }
                }
                for arg in args {
                    self.walk_expr(arg);
                }
            }
            HExprKind::BuiltinMethod {
                receiver,
                method,
                args,
            } => {
                if builtin_method_mutates(*method) {
                    self.walk_place(receiver);
                    self.record_mutation(receiver);
                } else {
                    self.walk_expr(receiver);
                }
                for arg in args {
                    self.walk_expr(arg);
                }
            }
            HExprKind::BuiltinAssoc { args, .. } | HExprKind::BuiltinCall { args, .. } => {
                for arg in args {
                    self.walk_expr(arg);
                }
            }
            HExprKind::Field { base, .. } => self.walk_expr(base),
            HExprKind::Index { base, index } => {
                self.walk_expr(base);
                self.walk_expr(index);
            }
            HExprKind::Unary { expr, .. } | HExprKind::Cast { expr } => self.walk_expr(expr),
            HExprKind::Binary { lhs, rhs, .. } => {
                self.walk_expr(lhs);
                self.walk_expr(rhs);
            }
            HExprKind::Assign { target, value, .. } => {
                self.walk_place(target);
                self.walk_expr(value);
                if let HExprKind::Local { c_name, .. } = &target.kind
                    && target.ty.is_ref()
                    && let Some(id) = self.lookup(c_name)
                {
                    self.rebind_local(id, &target.ty, value);
                }
                self.record_mutation(target);
            }
            HExprKind::Block(block) => self.walk_block(block),
            HExprKind::If {
                cond,
                then_block,
                else_expr,
            } => {
                self.walk_expr(cond);
                self.walk_block(then_block);
                if let Some(else_expr) = else_expr {
                    self.walk_expr(else_expr);
                }
            }
            HExprKind::Match { scrutinee, arms } => self.walk_match(scrutinee, arms),
            HExprKind::Loop { body } => {
                let start = self.idx;
                self.walk_block(body);
                let end = self.tick();
                self.loop_ranges.push((start, end));
            }
            HExprKind::While { cond, body } => {
                let start = self.idx;
                self.walk_expr(cond);
                self.walk_block(body);
                let end = self.tick();
                self.loop_ranges.push((start, end));
            }
            HExprKind::ForRange {
                pat,
                start,
                end,
                body,
                ..
            } => {
                self.walk_expr(start);
                self.walk_expr(end);
                self.walk_for(pat, body);
            }
            HExprKind::ForVec { pat, vec, body } => {
                self.walk_expr(vec);
                self.walk_for(pat, body);
            }
            HExprKind::StructLit { fields, .. } => {
                for (_, value) in fields {
                    self.walk_expr(value);
                }
            }
            HExprKind::Tuple(elements) | HExprKind::VecLit { elems: elements } => {
                for element in elements {
                    self.walk_expr(element);
                }
            }
            HExprKind::VecRepeat { value, count } => {
                self.walk_expr(value);
                self.walk_expr(count);
            }
            HExprKind::EnumLit { args, .. } => {
                for arg in args {
                    self.walk_expr(arg);
                }
            }
            HExprKind::OptionSome(inner)
            | HExprKind::ResultOk(inner)
            | HExprKind::ResultErr(inner) => self.walk_expr(inner),
            HExprKind::Format { parts } => {
                for part in parts {
                    if let HFormatPartKind::Arg { expr, .. } = &part.kind {
                        self.walk_expr(expr);
                    }
                }
            }
            HExprKind::Macro(call) => {
                for part in &call.parts {
                    if let HFormatPartKind::Arg { expr, .. } = &part.kind {
                        self.walk_expr(expr);
                    }
                }
                for arg in &call.args {
                    self.walk_expr(arg);
                }
            }
            HExprKind::Return(Some(value)) => {
                self.walk_expr(value);
                self.check_escape(value);
            }
            HExprKind::Return(None) => {}
            HExprKind::Ref {
                mutable,
                expr: inner,
            } => {
                self.walk_place(inner);
                let target = self.target_of_place(inner);
                let parent = self.origin_loan(inner);
                let loan = self.push_loan(target, *mutable, expr.span, None, parent, self.idx);
                self.ptr_loan.insert(expr_address(expr), loan);
            }
            HExprKind::Deref { expr } => self.walk_expr(expr),
        }
    }

    fn walk_place(&mut self, expr: &HExpr) {
        match &expr.kind {
            HExprKind::Local { name, c_name } => {
                self.register(c_name, name);
                self.use_local(c_name, self.idx);
            }
            HExprKind::Capture { .. } => {}
            HExprKind::Field { base, .. } => self.walk_place(base),
            HExprKind::Index { base, index } => {
                self.walk_place(base);
                self.walk_expr(index);
            }
            HExprKind::Deref { expr } => self.walk_expr(expr),
            _ => self.walk_expr(expr),
        }
    }

    fn walk_for(&mut self, pat: &HPat, body: &HBlock) {
        let start = self.idx;
        self.push_scope();
        self.declare_pattern(pat, None);
        self.walk_block(body);
        let end = self.tick();
        self.pop_scope(end);
        self.loop_ranges.push((start, end));
    }

    fn walk_match(&mut self, scrutinee: &HExpr, arms: &[HArm]) {
        self.walk_expr(scrutinee);
        let base = if scrutinee.ty.is_ref() {
            Some(self.provenance_value(scrutinee))
        } else {
            None
        };
        for arm in arms {
            self.push_scope();
            self.declare_pattern(&arm.pat, base.as_ref());
            if let Some(guard) = &arm.guard {
                self.walk_expr(guard);
            }
            self.walk_expr(&arm.body);
            let end = self.tick();
            self.pop_scope(end);
        }
    }

    fn method_takes_mut_receiver(&self, fn_id: usize) -> bool {
        self.program
            .fns
            .get(fn_id)
            .map(|function| function.self_kind == Some(SelfKind::RefMut))
            .unwrap_or(false)
    }

    fn record_mutation(&mut self, place: &HExpr) {
        let (target, via) = self.mutation_of_place(place);
        self.mutations.push(Mutation {
            target,
            via,
            idx: self.idx,
            span: place.span,
        });
    }
}

fn empty_span() -> Span {
    Span::empty(0)
}

fn expr_address(expr: &HExpr) -> usize {
    expr as *const HExpr as usize
}

fn field_code(field: &HFieldRef) -> u32 {
    match field {
        HFieldRef::Struct { index, .. } | HFieldRef::Tuple(index) => *index as u32,
    }
}

fn return_source_param(function: &HFn) -> Option<usize> {
    if matches!(
        function.self_kind,
        Some(SelfKind::Ref) | Some(SelfKind::RefMut)
    ) {
        return Some(0);
    }
    let mut found = None;
    for (index, param) in function.params.iter().enumerate() {
        if !param.ty.is_ref() {
            continue;
        }
        if found.replace(index).is_some() {
            return None;
        }
    }
    found
}

fn builtin_method_mutates(method: HBuiltinMethod) -> bool {
    matches!(
        method,
        HBuiltinMethod::StrPushStr
            | HBuiltinMethod::StrClear
            | HBuiltinMethod::VecPush
            | HBuiltinMethod::VecPop
            | HBuiltinMethod::VecInsert
            | HBuiltinMethod::VecRemove
            | HBuiltinMethod::VecClear
            | HBuiltinMethod::VecExtend
            | HBuiltinMethod::VecReverse
            | HBuiltinMethod::RcSet
            | HBuiltinMethod::BoxSet
    )
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

    /// Front end and type check a program. The borrow checker is part of the
    /// type checking pipeline, so rejection of an invalid program surfaces as
    /// an error result here.
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

    fn expect_accepted(source: &str) {
        let program = match check_source(source) {
            Ok(program) => program,
            Err(diagnostics) => panic!("valid program was rejected: {}", messages(&diagnostics)),
        };
        // The pipeline has already run the borrow checker; running it again
        // on the accepted HIR pins down that it is a pure read-only pass.
        let diagnostics = check(&program);
        assert!(
            !diagnostics.has_errors(),
            "borrow checker rejected valid program: {}",
            messages(&diagnostics)
        );
    }

    fn expect_rejected(source: &str, code: &str) {
        let diagnostics = match check_source(source) {
            Ok(program) => {
                let diagnostics = check(&program);
                panic!(
                    "borrow checker accepted invalid program (expected {}): {}",
                    code,
                    messages(&diagnostics)
                );
            }
            Err(diagnostics) => diagnostics,
        };
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

    // -- programs the borrow checker must accept -------------------------

    #[test]
    fn shared_borrows_can_coexist() {
        expect_accepted(
            "fn main() {
                 let x = 1;
                 let a = &x;
                 let b = &x;
                 let y = *a + *b;
             }",
        );
    }

    #[test]
    fn mutable_borrow_can_be_written_through() {
        expect_accepted(
            "fn main() {
                 let mut x = 1;
                 let r = &mut x;
                 *r = *r + 1;
             }",
        );
    }

    #[test]
    fn sequential_mutable_borrows_after_last_use() {
        expect_accepted(
            "fn main() {
                 let mut x = 1;
                 let a = &mut x;
                 *a = 2;
                 let b = &mut x;
                 *b = 3;
             }",
        );
    }

    #[test]
    fn shared_borrow_ends_before_mutable_borrow() {
        expect_accepted(
            "fn main() {
                 let mut x = 1;
                 let a = &x;
                 let y = *a;
                 let m = &mut x;
                 *m = y;
             }",
        );
    }

    #[test]
    fn reborrow_shared_from_mutable_parameter() {
        expect_accepted(
            "fn peek(r: &mut i64) -> i64 {
                 let s = &*r;
                 *s
             }
             fn main() {
                 let mut x = 1;
                 let y = peek(&mut x);
             }",
        );
    }

    #[test]
    fn reborrow_mut_from_mut_parameter() {
        expect_accepted(
            "fn bump(r: &mut i64) {
                 let a = &mut *r;
                 *a = 1;
             }
             fn main() {
                 let mut x = 1;
                 bump(&mut x);
             }",
        );
    }

    #[test]
    fn disjoint_fields_can_be_borrowed_mutably() {
        expect_accepted(
            "struct Pair { first: i64, second: i64 }
             fn main() {
                 let mut pair = Pair { first: 1, second: 2 };
                 let a = &mut pair.first;
                 let b = &mut pair.second;
                 *a = 3;
                 *b = 4;
             }",
        );
    }

    #[test]
    fn borrow_field_then_read_other_field() {
        expect_accepted(
            "struct Pair { first: i64, second: i64 }
             fn main() {
                 let pair = Pair { first: 1, second: 2 };
                 let a = &pair.first;
                 let b = pair.second;
                 let c = *a + b;
             }",
        );
    }

    #[test]
    fn borrow_whole_struct_and_read_field() {
        expect_accepted(
            "struct Pair { first: i64, second: i64 }
             fn main() {
                 let pair = Pair { first: 1, second: 2 };
                 let whole = &pair;
                 let b = pair.second;
                 let c = (*whole).first + b;
             }",
        );
    }

    #[test]
    fn return_reference_to_single_parameter() {
        expect_accepted(
            "fn identity(r: &i64) -> &i64 {
                 r
             }
             fn main() {
                 let x = 1;
                 let r = identity(&x);
                 let y = *r;
             }",
        );
    }

    #[test]
    fn return_mutable_reference_to_single_parameter() {
        expect_accepted(
            "fn identity(r: &mut i64) -> &mut i64 {
                 r
             }
             fn main() {
                 let mut x = 1;
                 let r = identity(&mut x);
                 *r = 2;
             }",
        );
    }

    #[test]
    fn method_returns_reference_to_own_field() {
        expect_accepted(
            "struct Counter { value: i64 }
             impl Counter {
                 fn value(&self) -> &i64 {
                     &self.value
                 }
             }
             fn main() {
                 let counter = Counter { value: 1 };
                 let r = counter.value();
                 let y = *r;
             }",
        );
    }

    #[test]
    fn copy_of_reference_is_a_shared_borrow() {
        expect_accepted(
            "fn main() {
                 let x = 1;
                 let a = &x;
                 let b = a;
                 let y = *b;
             }",
        );
    }

    #[test]
    fn borrow_living_across_loop_iterations_is_allowed() {
        expect_accepted(
            "fn main() {
                 let mut x = 1;
                 let r = &mut x;
                 let mut i = 0;
                 while i < 3 {
                     *r = *r + 1;
                     i = i + 1;
                 }
             }",
        );
    }

    #[test]
    fn read_through_shared_parameter() {
        expect_accepted(
            "fn twice(r: &i64) -> i64 {
                 *r + *r
             }
             fn main() {
                 let x = 1;
                 let y = twice(&x);
             }",
        );
    }

    #[test]
    fn write_through_mutable_parameter() {
        expect_accepted(
            "fn set(r: &mut i64) {
                 *r = 9;
             }
             fn main() {
                 let mut x = 1;
                 set(&mut x);
                 let y = x;
             }",
        );
    }

    #[test]
    fn block_returns_borrow_of_outer_variable() {
        expect_accepted(
            "fn main() {
                 let mut x = 1;
                 let r = { &mut x };
                 *r = 5;
             }",
        );
    }

    // -- programs the borrow checker must reject -------------------------

    #[test]
    fn two_mutable_borrows_are_rejected() {
        expect_rejected(
            "fn main() {
                 let mut x = 1;
                 let a = &mut x;
                 let b = &mut x;
                 *a = 2;
                 *b = 3;
             }",
            "E0499",
        );
    }

    #[test]
    fn mutable_borrow_blocks_shared_borrow() {
        expect_rejected(
            "fn main() {
                 let mut x = 1;
                 let m = &mut x;
                 let s = &x;
                 *m = 2;
                 let y = *s;
             }",
            "E0502",
        );
    }

    #[test]
    fn shared_borrow_blocks_mutable_borrow() {
        expect_rejected(
            "fn main() {
                 let mut x = 1;
                 let s = &x;
                 let m = &mut x;
                 let y = *s;
                 *m = 2;
             }",
            "E0502",
        );
    }

    #[test]
    fn assignment_while_shared_borrowed() {
        expect_rejected(
            "fn main() {
                 let mut x = 1;
                 let s = &x;
                 x = 2;
                 let y = *s;
             }",
            "E0506",
        );
    }

    #[test]
    fn assignment_while_mutable_borrowed() {
        expect_rejected(
            "fn main() {
                 let mut x = 1;
                 let m = &mut x;
                 x = 2;
                 *m = 3;
             }",
            "E0506",
        );
    }

    #[test]
    fn field_assignment_while_struct_borrowed() {
        expect_rejected(
            "struct Pair { first: i64, second: i64 }
             fn main() {
                 let mut pair = Pair { first: 1, second: 2 };
                 let s = &pair;
                 pair.second = 3;
                 let y = (*s).first;
             }",
            "E0506",
        );
    }

    #[test]
    fn read_while_mutable_borrowed() {
        expect_rejected(
            "fn main() {
                 let mut x = 1;
                 let m = &mut x;
                 let y = x;
                 *m = 2;
             }",
            "E0502",
        );
    }

    #[test]
    fn cannot_return_reference_to_local() {
        expect_rejected(
            "fn bad() -> &i64 {
                 let x = 1;
                 &x
             }
             fn main() {}",
            "E0515",
        );
    }

    #[test]
    fn cannot_return_reference_to_local_through_else_branch() {
        expect_rejected(
            "fn choose(flag: bool, other: &i64) -> &i64 {
                 if flag {
                     other
                 } else {
                     let x = 1;
                     &x
                 }
             }
             fn main() {}",
            "E0515",
        );
    }

    #[test]
    fn cannot_return_reference_to_local_through_match_arm() {
        expect_rejected(
            "fn choose(flag: bool, other: &i64) -> &i64 {
                 match flag {
                     true => other,
                     false => {
                         let x = 1;
                         &x
                     }
                 }
             }
             fn main() {}",
            "E0515",
        );
    }

    #[test]
    fn cannot_return_reference_to_local_from_method_branch() {
        expect_rejected(
            "struct Point { x: i64 }
             impl Point {
                 fn choose(&self, flag: bool) -> &i64 {
                     if flag {
                         &self.x
                     } else {
                         let local = 1;
                         &local
                     }
                 }
             }
             fn main() {}",
            "E0515",
        );
    }

    #[test]
    fn cannot_escape_reference_to_local_through_call_argument() {
        expect_rejected(
            "fn id(value: &i64) -> &i64 {
                 value
             }
             fn choose(flag: bool, other: &i64) -> &i64 {
                 id(if flag {
                     other
                 } else {
                     let x = 1;
                     &x
                 })
             }
             fn main() {}",
            "E0515",
        );
    }

    #[test]
    fn cannot_escape_reference_to_local_bound_from_branch() {
        expect_rejected(
            "fn choose(flag: bool, other: &i64) -> &i64 {
                 let r = if flag {
                     other
                 } else {
                     let x = 1;
                     &x
                 };
                 r
             }
             fn main() {}",
            "E0515",
        );
    }

    #[test]
    fn cannot_escape_reference_to_local_bound_from_call_argument() {
        expect_rejected(
            "fn id(value: &i64) -> &i64 {
                 value
             }
             fn choose(flag: bool, other: &i64) -> &i64 {
                 let r = id(if flag {
                     other
                 } else {
                     let x = 1;
                     &x
                 });
                 r
             }
             fn main() {}",
            "E0515",
        );
    }

    #[test]
    fn cannot_return_reference_to_by_value_parameter() {
        expect_rejected(
            "fn bad(x: i64, y: &i64) -> &i64 {
                 &x
             }
             fn main() {}",
            "E0515",
        );
    }

    #[test]
    fn cannot_return_reference_derived_from_wrong_parameter() {
        expect_rejected(
            "struct Counter { value: i64 }
             impl Counter {
                 fn pick(&self, other: &i64) -> &i64 {
                     other
                 }
             }
             fn main() {}",
            "E0515",
        );
    }

    #[test]
    fn borrow_of_local_does_not_live_long_enough() {
        expect_rejected(
            "fn main() {
                 let r = {
                     let x = 1;
                     &x
                 };
                 let y = *r;
             }",
            "E0597",
        );
    }

    #[test]
    fn two_mutable_reborrows_are_rejected() {
        expect_rejected(
            "fn bump(r: &mut i64) {
                 let a = &mut *r;
                 let b = &mut *r;
                 *a = 1;
                 *b = 2;
             }
             fn main() {
                 let mut x = 1;
                 bump(&mut x);
             }",
            "E0499",
        );
    }

    #[test]
    fn shared_reborrow_blocks_mutable_reborrow() {
        expect_rejected(
            "fn bump(r: &mut i64) {
                 let s = &*r;
                 let m = &mut *r;
                 let a = *s;
                 *m = 1;
             }
             fn main() {
                 let mut x = 1;
                 bump(&mut x);
             }",
            "E0502",
        );
    }
}
