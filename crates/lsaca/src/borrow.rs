//! The NLL-lite borrow checker for references.
//!
//! lsaca manages memory with ORC (ownership plus reference counting): every
//! owned value is reference counted, so the borrow checker does *not* have to
//! prove memory safety - the runtime owns every string, vector and aggregate.
//! What it proves instead is the aliasing discipline that makes references
//! sound and predictable:
//!
//! * a shared borrow may coexist with any number of other shared borrows;
//! * a mutable borrow is exclusive: no other borrow and no direct access to
//!   the borrowed place may happen while it is live;
//! * writes through a mutable reference are exclusive: a reference derived
//!   from it may not be live at the same time;
//! * a reference may never outlive the value it points at;
//! * a borrowed value may not be moved while the loan is live;
//! * a function may not return a reference to one of its own locals, and a
//!   reference returned from a call is attributed to the parameter the return
//!   lifetime was elided from.
//!
//! Since v1.4 ownership is Rust-like: Copy values copy implicitly, every other
//! owned value is moved by a consuming use, and a moved value is dead. The move
//! checker (`moveck.rs`) owns most move rules; the rule that needs the loan
//! table is E0505: moving a value while a borrow of it is live is rejected
//! here. The ORC runtime could copy the handle, but the source binding would be
//! dead while the reference still points into it.
//!
//! Version 1.5 refines one control-flow detail for the lazy iterators in
//! `prelude.ls`: the condition of an `if` is a temporary scope in Rust, so a
//! borrow created while evaluating the condition ends before either branch
//! runs. That is what makes the `if (pred)(&item) { return Some(item); }` shape
//! legal - borrow a loop binding, then move it. Only ownerless loans created
//! in the condition are closed there; every other temporary keeps its earlier
//! statement-scoped region, so a reference that outlives the branch still
//! reports E0505. As elsewhere the analysis stays conservative: `while`
//! conditions keep their previous loop-wide region.
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
//! borrow), E0505 (move of a borrowed value), E0506 (assignment to a borrowed
//! value), E0515 (returned reference to a local or to the wrong parameter) and
//! E0597 (borrow outlives its referent).

use std::collections::HashMap;

use crate::diag::{Diagnostic, Diagnostics, sort_and_dedup};
use crate::hir::{
    HArm, HBlock, HBuiltinAssoc, HBuiltinMethod, HClosure, HExpr, HExprKind, HFieldRef, HFn,
    HFormatPartKind, HPat, HPatKind, HStmt, Program,
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

/// One consuming use (move) of a local.
///
/// Only whole-local moves are recorded: moving out of a field, an index, a
/// dereference or a closure capture is E0507, which belongs to the move
/// checker.
#[derive(Clone, Debug)]
struct Move {
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
    /// Consuming uses (moves) of locals, for E0505.
    moves: Vec<Move>,
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
            moves: Vec::new(),
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
        diagnostics.extend(self.move_diagnostics());
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

    /// E0505: a consuming use (move) of a place while a borrow of it is live.
    ///
    /// A move is stronger than a write: the source binding is dead afterwards,
    /// so a live loan on the moved place would point into a value the program
    /// no longer owns. Only whole-local moves are recorded; moves out of a
    /// projection are E0507 and are reported by the move checker.
    fn move_diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for move_event in &self.moves {
            if matches!(move_event.target, Target::Opaque) {
                continue;
            }
            let mut conflict: Option<LoanId> = None;
            for (index, loan) in self.loans.iter().enumerate() {
                if !targets_overlap(&move_event.target, &loan.target) {
                    continue;
                }
                if move_event.idx < loan.start || move_event.idx > loan.end {
                    continue;
                }
                // A borrow created after the move in the same statement shares
                // the event index with the move; source order tells the two
                // apart. That case is a use of a moved value (E0382), not a
                // move while borrowed.
                if move_event.idx == loan.start && loan.span.start > move_event.span.start {
                    continue;
                }
                conflict = Some(index);
                break;
            }
            let Some(loan_id) = conflict else {
                continue;
            };
            let loan = &self.loans[loan_id];
            let name = self.describe_target(&move_event.target);
            let label = if loan.mutable {
                "mutable borrow occurs here"
            } else {
                "shared borrow occurs here"
            };
            diagnostics.push(
                Diagnostic::error(
                    format!("cannot move out of {} because it is borrowed", name),
                    move_event.span,
                )
                .with_code("E0505")
                .with_label(loan.span, label)
                .with_note(
                    "a move would leave the reference pointing at a value the \
                     program no longer owns; clone the value instead of moving it"
                        .to_string(),
                ),
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
            self.walk_consumed(tail);
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
                    // A single identifier binding takes ownership of the
                    // initializer; a compound pattern only reads it (its
                    // bindings copy or retain the value).
                    if matches!(&pat.kind, HPatKind::Bind { .. }) {
                        self.walk_consumed(init);
                    } else {
                        self.walk_expr(init);
                    }
                }
                // Bindings the initializer produces start existing after it is
                // evaluated; without the extra tick a reference copied out of
                // the initializer would appear to be borrowed at the same
                // instant as a mutation performed while evaluating it.
                let _ = self.tick();
                self.declare_let(pat, init.as_ref(), *span);
            }
            // A discarded expression statement moves the value and drops it.
            HStmt::Expr(expr) => self.walk_consumed(expr),
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

    /// Walk a consuming context: a single binding, an argument, a payload, a
    /// field initializer, a return value, a block tail or a discarded
    /// statement.
    ///
    /// Copy values are read with the old copy semantics; a non-Copy place that
    /// is exactly a local is moved and recorded for E0505. Anything else is an
    /// rvalue: constructing it does not move a place, and moves out of a
    /// projection, a dereference or a closure capture are E0507 and belong to
    /// the move checker.
    fn walk_consumed(&mut self, expr: &HExpr) {
        if self.program.is_copy(&expr.ty) {
            self.walk_expr(expr);
            return;
        }
        let target = match &expr.kind {
            HExprKind::Local { c_name, .. } => self.lookup(c_name).map(|id| self.place_root(id)),
            _ => None,
        };
        match target {
            Some(target) => {
                self.walk_place(expr);
                self.moves.push(Move {
                    target,
                    idx: self.idx,
                    span: expr.span,
                });
            }
            None => self.walk_expr(expr),
        }
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
                // Closures capture by value: Copy captures are copied, every
                // other capture moves into the closure environment.
                for capture in captures {
                    self.walk_consumed(capture);
                }
            }
            HExprKind::Call { callee, args } => {
                // Calling a closure copies the function value, but arguments
                // are passed by value and therefore move.
                self.walk_expr(callee);
                for arg in args {
                    self.walk_consumed(arg);
                }
            }
            HExprKind::DirectCall {
                fn_id,
                receiver,
                args,
            } => {
                if let Some(receiver) = receiver {
                    if self.method_takes_value_receiver(*fn_id) {
                        // A by-value self receiver is moved into the callee.
                        self.walk_consumed(receiver);
                    } else {
                        self.walk_place(receiver);
                        if self.method_takes_mut_receiver(*fn_id) {
                            self.record_mutation(receiver);
                        }
                    }
                }
                for arg in args {
                    self.walk_consumed(arg);
                }
            }
            HExprKind::BuiltinMethod {
                receiver,
                method,
                args,
            } => {
                if builtin_method_consumes_receiver(*method) {
                    self.walk_consumed(receiver);
                } else if builtin_method_mutates(*method) {
                    self.walk_place(receiver);
                    self.record_mutation(receiver);
                } else {
                    self.walk_expr(receiver);
                }
                for (index, arg) in args.iter().enumerate() {
                    if builtin_method_arg_moves(*method, index) {
                        self.walk_consumed(arg);
                    } else {
                        self.walk_expr(arg);
                    }
                }
            }
            HExprKind::BuiltinAssoc { func, args, .. } => {
                // Only Rc::new and Box::new take ownership of their
                // payload. String::from clones its argument (documented
                // copy semantics), Vec::with_capacity only reads the
                // capacity and HashMap::new takes no arguments, so those
                // constructors never record a move.
                for (index, arg) in args.iter().enumerate() {
                    if builtin_assoc_arg_moves(*func, index) {
                        self.walk_consumed(arg);
                    } else {
                        self.walk_expr(arg);
                    }
                }
            }
            HExprKind::BuiltinCall { args, .. } => {
                for arg in args {
                    self.walk_consumed(arg);
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
            HExprKind::Assign { target, op, value } => {
                self.walk_place(target);
                if op.is_none() {
                    // A plain assignment moves the value into the place.
                    self.walk_consumed(value);
                } else {
                    self.walk_expr(value);
                }
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
                // The condition is a temporary scope: a borrow taken while
                // evaluating it ends before either branch runs. This is what
                // lets prelude filter/find borrow a loop item for the
                // predicate and then move the item into the result.
                let pending = self.loans.len();
                self.walk_expr(cond);
                let end = self.tick();
                self.finalize_inline(pending, end);
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
                // `for x in <vec>` moves the vector into the loop.
                self.walk_consumed(vec);
                self.walk_for(pat, body);
            }
            HExprKind::ForIter {
                iter, pat, body, ..
            } => {
                // `for x in <iterator>` moves the iterator value into the
                // hidden iteration local. The local itself is synthetic and
                // has no user-visible name, so it stays outside the loan
                // table; the value the user wrote is still consumed here.
                self.walk_consumed(iter);
                self.walk_for(pat, body);
            }
            HExprKind::StructLit { fields, .. } => {
                for (_, value) in fields {
                    self.walk_consumed(value);
                }
            }
            HExprKind::Tuple(elements) | HExprKind::VecLit { elems: elements } => {
                for element in elements {
                    self.walk_consumed(element);
                }
            }
            HExprKind::VecRepeat { value, count } => {
                self.walk_consumed(value);
                self.walk_expr(count);
            }
            HExprKind::EnumLit { args, .. } => {
                for arg in args {
                    self.walk_consumed(arg);
                }
            }
            HExprKind::OptionSome(inner)
            | HExprKind::ResultOk(inner)
            | HExprKind::ResultErr(inner) => {
                // Constructors move their payload into the new value.
                self.walk_consumed(inner);
            }
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
                self.walk_consumed(value);
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
        if self.program.is_copy(&scrutinee.ty) {
            self.walk_expr(scrutinee);
        } else {
            // A non-Copy scrutinee is consumed into the match temporary.
            self.walk_consumed(scrutinee);
        }
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

    fn method_takes_value_receiver(&self, fn_id: usize) -> bool {
        self.program
            .fns
            .get(fn_id)
            .map(|function| function.self_kind == Some(SelfKind::Value))
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
            | HBuiltinMethod::MapInsert
            | HBuiltinMethod::MapRemove
            | HBuiltinMethod::MapClear
    )
}

/// Builtin methods whose receiver is consumed by the call.
fn builtin_method_consumes_receiver(method: HBuiltinMethod) -> bool {
    matches!(
        method,
        HBuiltinMethod::OptUnwrap
            | HBuiltinMethod::OptUnwrapOr
            | HBuiltinMethod::OptExpect
            | HBuiltinMethod::ResUnwrap
            | HBuiltinMethod::ResUnwrapOr
            | HBuiltinMethod::ResExpect
    )
}

/// Builtin method arguments that are moved into the callee by value.
///
/// Mirrors the consuming-context table of the move checker so both
/// analyses agree on which arguments are gone after the call: the single
/// payload of the consuming builtins, both halves of `HashMap::insert`
/// and the by-value closure of `HashMap::get_with`. Map keys are copied
/// by the compiler and the closure handles of the iterator adapters are
/// ordinary by-value arguments.
fn builtin_method_arg_moves(method: HBuiltinMethod, index: usize) -> bool {
    match method {
        HBuiltinMethod::OptUnwrapOr
        | HBuiltinMethod::ResUnwrapOr
        | HBuiltinMethod::VecPush
        | HBuiltinMethod::VecExtend
        | HBuiltinMethod::RcSet
        | HBuiltinMethod::BoxSet => index == 0,
        HBuiltinMethod::VecInsert | HBuiltinMethod::MapGetWith => index == 1,
        HBuiltinMethod::MapInsert => index <= 1,
        _ => false,
    }
}

/// Builtin constructor payload arguments that are moved into the new value.
fn builtin_assoc_arg_moves(func: HBuiltinAssoc, index: usize) -> bool {
    // Rc::new and Box::new own their payload. String::from clones its
    // argument, Vec::with_capacity only reads the capacity and
    // HashMap::new takes no arguments, so they record no move.
    matches!(
        (func, index),
        (HBuiltinAssoc::RcNew | HBuiltinAssoc::BoxNew, 0)
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

    /// Render diagnostics with source context so a failing test shows where
    /// the unexpected error was reported.
    fn rendered(source: &str, diagnostics: &Diagnostics) -> String {
        let file = SourceFile::new("test.ls", source);
        diagnostics.render(&file)
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
            Err(diagnostics) => panic!(
                "valid program was rejected:\n{}",
                rendered(source, &diagnostics)
            ),
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

    /// Parse one source string into an AST, panicking on lexer or parser
    /// errors. The prelude-aware helpers below need the prelude AST too.
    fn parse_ast(source: &str, file: &SourceFile) -> crate::ast::Program {
        let (tokens, lex_diagnostics) = lexer::tokenize(source);
        assert!(
            !lex_diagnostics.has_errors(),
            "lexer errors: {}",
            messages(&lex_diagnostics)
        );
        let (program, parse_diagnostics) = parser::parse(&tokens, file);
        assert!(
            !parse_diagnostics.has_errors(),
            "parser errors: {}",
            messages(&parse_diagnostics)
        );
        program
    }

    /// Front end and type check a program against the v1.5 prelude. Iterator
    /// chains only exist once the prelude is registered, so those tests need
    /// this entry point instead of [check_source].
    fn check_source_with_prelude(source: &str) -> Result<hir::Program, Diagnostics> {
        let file = SourceFile::new("test.ls", source);
        let program = parse_ast(source, &file);
        let prelude_file = SourceFile::new(crate::prelude::FILE_NAME, crate::prelude::SOURCE);
        let prelude = parse_ast(crate::prelude::SOURCE, &prelude_file);
        typeck::check_with_prelude(&program, Some(&prelude), &file)
    }

    fn expect_accepted_with_prelude(source: &str) {
        let program = match check_source_with_prelude(source) {
            Ok(program) => program,
            Err(diagnostics) => panic!("valid program was rejected: {}", messages(&diagnostics)),
        };
        // Pin down that the borrow checker stays a pure read-only pass.
        let diagnostics = check(&program);
        assert!(
            !diagnostics.has_errors(),
            "borrow checker rejected valid program: {}",
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

    // -- moves of borrowed values (v1.4, E0505) --------------------------

    #[test]
    fn shared_borrow_then_move_is_rejected() {
        expect_rejected(
            "fn main() {
                 let s = String::from(\"hi\");
                 let r = &s;
                 let gone = s;
                 let n = r.len();
             }",
            "E0505",
        );
    }

    #[test]
    fn mutable_borrow_then_move_is_rejected() {
        expect_rejected(
            "fn main() {
                 let mut s = String::from(\"hi\");
                 let r = &mut s;
                 let gone = s;
                 let n = r.len();
             }",
            "E0505",
        );
    }

    #[test]
    fn moving_into_call_while_borrowed_is_rejected() {
        expect_rejected(
            "fn take(s: String) {}
             fn main() {
                 let s = String::from(\"hi\");
                 let r = &s;
                 take(s);
                 let n = r.len();
             }",
            "E0505",
        );
    }

    #[test]
    fn assignment_moves_borrowed_value_is_rejected() {
        expect_rejected(
            "fn main() {
                 let s = String::from(\"hi\");
                 let mut t = String::from(\"x\");
                 let r = &s;
                 t = s;
                 let n = r.len();
             }",
            "E0505",
        );
    }

    #[test]
    fn constructor_payload_move_while_borrowed_is_rejected() {
        expect_rejected(
            "fn main() {
                 let s = String::from(\"hi\");
                 let r = &s;
                 let o = Some(s);
                 let n = r.len();
             }",
            "E0505",
        );
    }

    #[test]
    fn for_loop_moves_borrowed_vector_is_rejected() {
        expect_rejected(
            "fn main() {
                 let v: Vec<i64> = Vec::new();
                 let r = &v;
                 for item in v {
                     println!(\"{}\", item);
                 }
                 let n = r.len();
             }",
            "E0505",
        );
    }

    #[test]
    fn closure_capture_moves_borrowed_value_is_rejected() {
        expect_rejected(
            "fn main() {
                 let s = String::from(\"hi\");
                 let r = &s;
                 let f = || {
                     println!(\"{}\", s);
                 };
                 f();
                 let n = r.len();
             }",
            "E0505",
        );
    }

    #[test]
    fn move_is_labelled_at_the_borrow() {
        let source = "fn main() {
                          let s = String::from(\"hi\");
                          let r = &s;
                          let gone = s;
                          let n = r.len();
                      }";
        let diagnostics = match check_source(source) {
            Ok(program) => check(&program),
            Err(diagnostics) => diagnostics,
        };
        let diagnostic = diagnostics
            .as_slice()
            .iter()
            .find(|diagnostic| diagnostic.code == Some("E0505"))
            .expect("expected an E0505 diagnostic");
        assert!(
            diagnostic.labels.iter().any(|label| label
                .message
                .as_deref()
                .is_some_and(|message| message.contains("borrow"))),
            "expected a label at the borrow location",
        );
    }

    #[test]
    fn move_after_borrow_ends_is_accepted() {
        expect_accepted(
            "fn main() {
                 let s = String::from(\"hi\");
                 let r = &s;
                 let n = r.len();
                 let gone = s;
             }",
        );
    }

    #[test]
    fn move_after_mutable_borrow_ends_is_accepted() {
        expect_accepted(
            "fn main() {
                 let mut s = String::from(\"hi\");
                 let r = &mut s;
                 let n = r.len();
                 let gone = s;
             }",
        );
    }

    #[test]
    fn move_then_borrow_is_accepted() {
        expect_accepted(
            "fn main() {
                 let s = String::from(\"hi\");
                 let gone = s;
                 let r = &gone;
                 let n = r.len();
             }",
        );
    }

    #[test]
    fn copy_value_read_while_borrowed_is_accepted() {
        expect_accepted(
            "fn main() {
                 let mut x = 1;
                 let r = &x;
                 let y = x;
                 let z = *r + y;
                 let w = z;
             }",
        );
    }

    #[test]
    fn returning_a_local_moves_it() {
        expect_accepted(
            "fn give() -> String {
                 let s = String::from(\"hi\");
                 let r = &s;
                 let n = r.len();
                 s
             }
             fn main() {
                 let t = give();
             }",
        );
    }

    #[test]
    fn clone_does_not_move_the_receiver() {
        expect_accepted(
            "fn main() {
                 let s = String::from(\"hi\");
                 let r = &s;
                 let c = s.clone();
                 let n = r.len();
                 let m = c.len();
             }",
        );
    }

    #[test]
    fn format_arguments_do_not_move() {
        expect_accepted(
            "fn main() {
                 let s = String::from(\"hi\");
                 let r = &s;
                 println!(\"{}\", s);
                 let n = r.len();
             }",
        );
    }

    #[test]
    fn borrowed_receiver_does_not_move() {
        expect_accepted(
            "fn main() {
                 let s = String::from(\"hi\");
                 let n = s.len();
                 let gone = s;
             }",
        );
    }

    // -- version 1.5: an if condition is a temporary scope ----------------
    //
    // The prelude iterators borrow a loop item for their predicate and then
    // move it into the returned value:
    //
    //     if (self.pred)(&item) { return Some(item); }
    //
    // A borrow created while evaluating the condition ends with the
    // condition, so the move in the then branch does not leave a dangling
    // reference behind.

    #[test]
    fn if_condition_borrow_then_move_is_accepted() {
        expect_accepted(
            "fn is_long(s: &String) -> bool {
                 s.len() > 2
             }
             fn main() {
                 let text = String::from(\"lsaca\");
                 if is_long(&text) {
                     let moved = text;
                     println!(\"{}\", moved);
                 }
             }",
        );
    }

    #[test]
    fn closure_condition_borrow_then_move_is_accepted() {
        expect_accepted(
            "fn main() {
                 let text = String::from(\"lsaca\");
                 let keep = |value: &String| -> bool { value.len() > 2 };
                 if keep(&text) {
                     let moved = text;
                     println!(\"{}\", moved);
                 }
             }",
        );
    }

    #[test]
    fn if_condition_mutable_borrow_then_move_is_accepted() {
        expect_accepted(
            "fn bump(r: &mut String) -> bool {
                 r.push_str(\"!\");
                 r.len() > 2
             }
             fn main() {
                 let mut text = String::from(\"lsaca\");
                 if bump(&mut text) {
                     let moved = text;
                     println!(\"{}\", moved);
                 }
             }",
        );
    }

    #[test]
    fn if_condition_borrow_then_move_inside_loop_is_accepted() {
        expect_accepted(
            "fn is_long(s: &String) -> bool {
                 s.len() > 3
             }
             fn main() {
                 let words = vec![String::from(\"alpha\"), String::from(\"b\")];
                 for word in words {
                     if is_long(&word) {
                         let owned = word;
                         println!(\"{}\", owned);
                     }
                 }
             }",
        );
    }

    #[test]
    fn else_branch_move_after_condition_borrow_is_accepted() {
        expect_accepted(
            "fn is_long(s: &String) -> bool {
                 s.len() > 3
             }
             fn main() {
                 let text = String::from(\"lsaca\");
                 if is_long(&text) {
                     println!(\"long\");
                 } else {
                     let moved = text;
                     println!(\"{}\", moved);
                 }
             }",
        );
    }

    #[test]
    fn borrow_ending_in_condition_then_move_is_accepted() {
        expect_accepted(
            "fn main() {
                 let text = String::from(\"lsaca\");
                 let borrowed = &text;
                 if borrowed.len() > 2 {
                     let moved = text;
                     println!(\"{}\", moved);
                 }
             }",
        );
    }

    #[test]
    fn predicate_borrow_then_move_through_closure_is_accepted() {
        // The exact shape prelude filter/find are written with: borrow the
        // item for the predicate call, then move it into the result.
        expect_accepted(
            "fn find_item(item: String, pred: fn(&String) -> bool) -> Option<String> {
                 if pred(&item) {
                     return Some(item);
                 }
                 None
             }
             fn main() {
                 let keep = |value: &String| -> bool { value.len() < 5 };
                 let found = find_item(String::from(\"abc\"), keep);
                 match found {
                     Some(value) => println!(\"{}\", value),
                     None => println!(\"none\"),
                 }
             }",
        );
    }

    #[test]
    fn move_while_borrow_is_used_in_branch_is_rejected() {
        expect_rejected(
            "fn main() {
                 let text = String::from(\"hi\");
                 let borrowed = &text;
                 if borrowed.len() > 0 {
                     let moved = text;
                     let n = borrowed.len();
                 }
             }",
            "E0505",
        );
    }

    #[test]
    fn match_scrutinee_borrow_then_move_is_rejected() {
        // A match scrutinee temporary lives through the arms, so moving the
        // scrutinee while a pattern binding borrows it is still an error.
        expect_rejected(
            "fn main() {
                 let maybe = Some(String::from(\"hi\"));
                 match &maybe {
                     Some(value) => {
                         let moved = maybe;
                         let n = value.len();
                     }
                     None => {}
                 }
             }",
            "E0505",
        );
    }

    // -- version 1.5: HashMap ownership rules ------------------------------
    //
    // insert takes ownership of both halves, so it moves them; the map
    // receiver itself is only borrowed mutably, and the read-only methods
    // (len in the test below) never move the map.

    #[test]
    fn map_insert_moves_borrowed_key_is_rejected() {
        expect_rejected(
            "fn main() {
                 let key = String::from(\"k\");
                 let borrowed = &key;
                 let mut scores: HashMap<String, i64> = HashMap::new();
                 scores.insert(key, 1);
                 let n = borrowed.len();
             }",
            "E0505",
        );
    }

    #[test]
    fn map_insert_moves_borrowed_value_is_rejected() {
        expect_rejected(
            "fn main() {
                 let value = String::from(\"v\");
                 let borrowed = &value;
                 let mut entries: HashMap<String, String> = HashMap::new();
                 entries.insert(String::from(\"k\"), value);
                 let n = borrowed.len();
             }",
            "E0505",
        );
    }

    #[test]
    fn map_insert_while_map_is_borrowed_is_rejected() {
        expect_rejected(
            "fn main() {
                 let mut scores: HashMap<String, i64> = HashMap::new();
                 let borrowed = &scores;
                 scores.insert(String::from(\"k\"), 1);
                 let n = borrowed.len();
             }",
            "E0506",
        );
    }

    #[test]
    fn map_read_receiver_then_move_is_accepted() {
        expect_accepted(
            "fn main() {
                 let scores: HashMap<String, i64> = HashMap::new();
                 let n = scores.len();
                 let moved = scores;
                 println!(\"{}\", n);
                 let empty = moved.is_empty();
                 println!(\"{}\", empty);
             }",
        );
    }

    // -- version 1.5: NLL edges around HashMap and iterator chains ---------
    //
    // The prelude iterators and the HashMap methods create short-lived
    // borrows. These tests pin down that a loan only lives until its last
    // use, so mutating or moving the owner afterwards is accepted, while
    // mutating it before that last use is still E0506.

    #[test]
    fn map_get_with_then_mutate_is_accepted() {
        expect_accepted(
            "fn main() {
                 let mut counts: HashMap<String, i64> = HashMap::new();
                 counts.insert(String::from(\"a\"), 1);
                 let doubled = counts.get_with(String::from(\"a\"), |value: &i64| -> i64 { *value * 2 });
                 counts.insert(String::from(\"b\"), 2);
                 println!(\"{}\", doubled.unwrap());
             }",
        );
    }

    #[test]
    fn map_get_then_mutate_after_shared_borrow_is_accepted() {
        expect_accepted(
            "fn main() {
                 let mut scores: HashMap<String, i64> = HashMap::new();
                 scores.insert(String::from(\"k\"), 1);
                 let borrowed = &scores;
                 let value = scores.get(String::from(\"k\"));
                 let n = borrowed.len();
                 scores.insert(String::from(\"j\"), 2);
                 println!(\"{}\", value.unwrap() + n);
             }",
        );
    }

    #[test]
    fn map_clear_while_shared_borrowed_is_rejected() {
        expect_rejected(
            "fn main() {
                 let mut scores: HashMap<String, i64> = HashMap::new();
                 let borrowed = &scores;
                 scores.clear();
                 let n = borrowed.len();
                 println!(\"{}\", n);
             }",
            "E0506",
        );
    }

    #[test]
    fn map_move_after_shared_borrow_ends_is_accepted() {
        expect_accepted(
            "fn main() {
                 let mut scores: HashMap<String, i64> = HashMap::new();
                 scores.insert(String::from(\"k\"), 1);
                 let borrowed = &scores;
                 let n = borrowed.len();
                 let moved = scores;
                 println!(\"{}\", n);
                 println!(\"{}\", moved.len());
             }",
        );
    }

    #[test]
    fn iterator_chain_capture_and_body_borrow_of_same_local_is_accepted() {
        expect_accepted_with_prelude(
            "fn main() {
                 let numbers: Vec<i64> = vec![1, 2, 3];
                 let offset = 10;
                 let boosted: Vec<i64> = numbers.iter().map(|item: i64| -> i64 { item + offset }).collect();
                 for value in boosted.iter() {
                     let borrowed = &offset;
                     println!(\"{}\", value + *borrowed);
                 }
             }",
        );
    }

    #[test]
    fn for_iter_loop_body_borrow_then_move_after_loop_is_accepted() {
        expect_accepted_with_prelude(
            "fn main() {
                 let numbers: Vec<i64> = vec![1, 2, 3];
                 let label = String::from(\"n\");
                 for value in numbers.iter() {
                     let borrowed = &label;
                     println!(\"{}{}\", borrowed, value);
                 }
                 let moved = label;
                 println!(\"{}\", moved);
             }",
        );
    }

    #[test]
    fn two_disjoint_borrows_of_different_locals_in_one_call_is_accepted() {
        expect_accepted(
            "fn combine(left: &String, right: &String) -> i64 {
                 left.len() + right.len()
             }
             fn main() {
                 let left = String::from(\"ab\");
                 let right = String::from(\"cde\");
                 let total = combine(&left, &right);
                 println!(\"{}\", total);
             }",
        );
    }
}
