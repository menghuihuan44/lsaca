//! Pattern exhaustiveness, redundancy and irrefutability analysis.

use super::check::Checker;
use super::itype::{ITy, VarKind};
use super::tree::{TLit, TPat, TPatKind};
use crate::span::Span;

/// A literal value that participates in match coverage.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LitKey {
    Int(i128),
    Float(u64),
    Char(char),
    Str(String),
}

/// A simplified pattern used by the usefulness algorithm. Or-patterns are
/// expanded into multiple alternatives before analysis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EPat {
    Wild,
    Ctor(usize, Vec<EPat>),
    Lit(LitKey),
}

/// Cartesian product of pattern alternatives.
fn product(alternatives: &[Vec<EPat>]) -> Vec<Vec<EPat>> {
    let mut acc: Vec<Vec<EPat>> = vec![Vec::new()];
    for slot in alternatives {
        let mut next = Vec::new();
        for prefix in &acc {
            for alternative in slot {
                let mut row = prefix.clone();
                row.push(alternative.clone());
                next.push(row);
            }
        }
        acc = next;
    }
    acc
}

impl<'a> Checker<'a> {
    /// Check a match for exhaustiveness and report unreachable arms.
    pub fn check_match_exhaustive(
        &mut self,
        scrutinee: &ITy,
        arms: &[super::tree::TArm],
        span: Span,
    ) {
        let mut ty = self.infer.resolve(scrutinee);
        if let ITy::Var(id) = ty {
            match self.infer.var_kind(id) {
                VarKind::IntLiteral => {
                    let _ = self.infer.unify(&ty, &ITy::i32());
                    ty = ITy::i32();
                }
                VarKind::FloatLiteral => {
                    let _ = self.infer.unify(&ty, &ITy::f64());
                    ty = ITy::f64();
                }
                VarKind::General => return,
            }
        }
        if let ITy::Slice(elem) = &ty {
            let elem = (**elem).clone();
            let max_total = arms
                .iter()
                .map(|arm| self.slice_pattern_max_total(&arm.pat))
                .max()
                .unwrap_or(0);
            // Slice patterns describe an open family of lengths. Partitioning
            // it into 0..=max_total plus an unbounded tail keeps the analysis
            // exact for real matches without letting a pathological pattern
            // set enumerate an unreasonable number of classes.
            if max_total <= 256 {
                self.check_slice_match_exhaustive(&elem, arms, span, max_total);
                return;
            }
        }
        self.check_match_exhaustive_rows(&ty, arms, span);
    }

    /// Exhaustiveness for matches whose patterns have a fixed arity.
    fn check_match_exhaustive_rows(&mut self, ty: &ITy, arms: &[super::tree::TArm], span: Span) {
        let mut matrix: Vec<Vec<EPat>> = Vec::new();
        for arm in arms {
            let rows = self.expand_pattern(&arm.pat);
            let mut useful = false;
            for row in &rows {
                if self.is_useful(&matrix, std::slice::from_ref(row), std::slice::from_ref(ty)) {
                    useful = true;
                    break;
                }
            }
            // `if let` without an `else` is parsed as a match whose last
            // arm is a synthetic wildcard carrying an empty span. When the
            // user's pattern is irrefutable that arm is dead, but warning
            // about it would blame a pattern the user never wrote.
            let synthetic =
                matches!(arm.pat.kind, TPatKind::Wild) && arm.pat.span.start == arm.pat.span.end;
            if !useful && !synthetic {
                self.warning_code(
                    "unreachable pattern".to_string(),
                    arm.pat.span,
                    "unreachable_patterns",
                );
            }
            if arm.guard.is_none() {
                for row in rows {
                    matrix.push(vec![row]);
                }
            }
        }
        if self.is_useful(&matrix, &[EPat::Wild], std::slice::from_ref(ty)) {
            let rendered = match self.find_witness(&matrix, std::slice::from_ref(ty)) {
                Some(witness) => match witness.first() {
                    Some(epat) => self.render_witness(ty, epat),
                    None => "_".to_string(),
                },
                None => "_".to_string(),
            };
            self.report_non_exhaustive(rendered, span);
        }
    }

    /// Exhaustiveness for slice types, one concrete length class at a time.
    fn check_slice_match_exhaustive(
        &mut self,
        elem: &ITy,
        arms: &[super::tree::TArm],
        span: Span,
        max_total: usize,
    ) {
        // Classes 0..=max_total are concrete lengths. The final class stands
        // for every longer length: no-rest patterns never apply there and
        // every rest pattern does, so the representative max_total + 1 has
        // exactly the same coverage shape as the whole tail.
        let tail_len = max_total + 1;
        let lengths: Vec<usize> = (0..=max_total).chain(std::iter::once(tail_len)).collect();
        let types: Vec<Vec<ITy>> = lengths
            .iter()
            .map(|length| vec![elem.clone(); *length])
            .collect();
        let mut matrices: Vec<Vec<Vec<EPat>>> = vec![Vec::new(); lengths.len()];
        for arm in arms {
            let mut useful = false;
            for (index, length) in lengths.iter().enumerate() {
                let rows = self.expand_slice_rows(&arm.pat, *length);
                if rows
                    .iter()
                    .any(|row| self.is_useful(&matrices[index], row, &types[index]))
                {
                    useful = true;
                    break;
                }
            }
            let synthetic =
                matches!(arm.pat.kind, TPatKind::Wild) && arm.pat.span.start == arm.pat.span.end;
            if !useful && !synthetic {
                self.warning_code(
                    "unreachable pattern".to_string(),
                    arm.pat.span,
                    "unreachable_patterns",
                );
            }
            if arm.guard.is_none() {
                for (index, length) in lengths.iter().enumerate() {
                    for row in self.expand_slice_rows(&arm.pat, *length) {
                        matrices[index].push(row);
                    }
                }
            }
        }
        for (index, length) in lengths.iter().enumerate() {
            let query = vec![EPat::Wild; *length];
            if !self.is_useful(&matrices[index], &query, &types[index]) {
                continue;
            }
            let witness = self
                .find_witness(&matrices[index], &types[index])
                .unwrap_or_else(|| query.clone());
            let rendered = if *length == tail_len {
                self.render_rest_slice_witness(elem, &witness)
            } else {
                self.render_exact_slice_witness(elem, &witness)
            };
            self.report_non_exhaustive(rendered, span);
            return;
        }
    }

    fn report_non_exhaustive(&mut self, rendered: String, span: Span) {
        let diag = crate::diag::Diagnostic::error(
            format!("non-exhaustive patterns: {} not covered", rendered),
            span,
        )
        .with_code("E0004")
        .with_span_label(span)
        .with_note(
            "match must be exhaustive; add a wildcard arm or cover the missing patterns"
                .to_string(),
        );
        self.diagnostics.push(diag);
    }

    /// The largest fixed prefix-plus-suffix width among the top-level slice
    /// patterns of an arm. Or-patterns and At bindings are transparent.
    fn slice_pattern_max_total(&self, pat: &TPat) -> usize {
        match &pat.kind {
            TPatKind::Slice { prefix, suffix, .. } => prefix.len() + suffix.len(),
            TPatKind::Or(alternatives) => alternatives
                .iter()
                .map(|alternative| self.slice_pattern_max_total(alternative))
                .max()
                .unwrap_or(0),
            TPatKind::At { pattern, .. } => self.slice_pattern_max_total(pattern),
            _ => 0,
        }
    }

    /// Expand one arm's slice pattern for a concrete length class. A no-rest
    /// pattern applies only at its exact width; a rest pattern applies at
    /// every width at least as large as its fixed prefix and suffix.
    fn expand_slice_rows(&self, pat: &TPat, length: usize) -> Vec<Vec<EPat>> {
        match &pat.kind {
            TPatKind::Wild | TPatKind::Bind { .. } => {
                vec![vec![EPat::Wild; length]]
            }
            TPatKind::Or(alternatives) => {
                let mut rows = Vec::new();
                for alternative in alternatives {
                    rows.extend(self.expand_slice_rows(alternative, length));
                }
                rows
            }
            TPatKind::At { pattern, .. } => self.expand_slice_rows(pattern, length),
            TPatKind::Slice {
                prefix,
                suffix,
                rest,
            } => {
                let total = prefix.len() + suffix.len();
                let applies = if *rest {
                    length >= total
                } else {
                    length == total
                };
                if !applies {
                    return Vec::new();
                }
                let mut children: Vec<Vec<EPat>> = vec![vec![EPat::Wild]; length];
                for (index, child) in prefix.iter().enumerate() {
                    children[index] = self.expand_pattern(child);
                }
                let start = length - suffix.len();
                for (offset, child) in suffix.iter().enumerate() {
                    children[start + offset] = self.expand_pattern(child);
                }
                product(&children)
            }
            _ => Vec::new(),
        }
    }

    fn render_exact_slice_witness(&self, elem: &ITy, row: &[EPat]) -> String {
        let parts: Vec<String> = row
            .iter()
            .map(|epat| self.render_witness(elem, epat))
            .collect();
        format!("[{}]", parts.join(", "))
    }

    /// A tail-class witness is an open pattern: the middle stands for every
    /// unconstrained length beyond the rendered prefix and suffix.
    fn render_rest_slice_witness(&self, elem: &ITy, row: &[EPat]) -> String {
        let Some(first) = row.first() else {
            return "[..]".to_string();
        };
        let first = self.render_witness(elem, first);
        if row.len() == 1 {
            return format!("[{}, ..]", first);
        }
        match row.last() {
            // A wildcard suffix adds no information: the open middle already
            // covers it. Keep the witness compact.
            Some(EPat::Wild) | None => format!("[{}, ..]", first),
            Some(last) => {
                let last = self.render_witness(elem, last);
                format!("[{}, .., {}]", first, last)
            }
        }
    }

    /// True when the pattern always matches values of its type.
    pub fn pattern_irrefutable(&self, pat: &TPat) -> bool {
        match &pat.kind {
            TPatKind::Wild | TPatKind::Bind { .. } => true,
            TPatKind::Lit(_) => false,
            TPatKind::Tuple(pats) | TPatKind::Array(pats) => {
                pats.iter().all(|pat| self.pattern_irrefutable(pat))
            }
            TPatKind::Slice {
                prefix,
                suffix,
                rest,
            } => {
                let children_irrefutable = prefix
                    .iter()
                    .chain(suffix.iter())
                    .all(|pat| self.pattern_irrefutable(pat));
                if !children_irrefutable {
                    return false;
                }
                let total = prefix.len() + suffix.len();
                match self.infer.resolve(&pat.ty) {
                    ITy::Array(_, len) => total <= len && (*rest || total == len),
                    _ => *rest && prefix.is_empty() && suffix.is_empty(),
                }
            }
            TPatKind::At { pattern, .. } => self.pattern_irrefutable(pattern),
            TPatKind::Struct { fields, .. } => {
                fields.iter().all(|(_, pat)| self.pattern_irrefutable(pat))
            }
            TPatKind::Enum { enum_id, args, .. } => {
                self.variant_count(*enum_id) == 1
                    && args.iter().all(|pat| self.pattern_irrefutable(pat))
            }
            TPatKind::OptionSome(_) | TPatKind::OptionNone => false,
            TPatKind::ResultOk(_) | TPatKind::ResultErr(_) => false,
            TPatKind::Or(alts) => alts.iter().any(|pat| self.pattern_irrefutable(pat)),
        }
    }

    /// Expand a checked pattern into a set of or-free alternatives.
    pub fn expand_pattern(&self, pat: &TPat) -> Vec<EPat> {
        match &pat.kind {
            TPatKind::Wild | TPatKind::Bind { .. } => vec![EPat::Wild],
            TPatKind::Or(alts) => {
                let mut out = Vec::new();
                for alt in alts {
                    out.extend(self.expand_pattern(alt));
                }
                out
            }
            TPatKind::Lit(lit) => vec![bool_or_lit_epat(lit)],
            TPatKind::Tuple(pats) | TPatKind::Array(pats) => {
                let children: Vec<Vec<EPat>> =
                    pats.iter().map(|pat| self.expand_pattern(pat)).collect();
                product(&children)
                    .into_iter()
                    .map(|row| EPat::Ctor(0, row))
                    .collect()
            }
            TPatKind::Slice {
                prefix,
                suffix,
                rest,
            } => {
                if let ITy::Array(_, len) = self.infer.resolve(&pat.ty) {
                    let total = prefix.len() + suffix.len();
                    if total <= len && (*rest || total == len) {
                        let mut children: Vec<Vec<EPat>> = vec![vec![EPat::Wild]; len];
                        for (index, child) in prefix.iter().enumerate() {
                            children[index] = self.expand_pattern(child);
                        }
                        let start = len - suffix.len();
                        for (offset, child) in suffix.iter().enumerate() {
                            children[start + offset] = self.expand_pattern(child);
                        }
                        return product(&children)
                            .into_iter()
                            .map(|row| EPat::Ctor(0, row))
                            .collect();
                    }
                    vec![EPat::Ctor(usize::MAX, Vec::new())]
                } else if *rest && prefix.is_empty() && suffix.is_empty() {
                    vec![EPat::Wild]
                } else {
                    // A slice pattern with a known prefix/suffix only covers
                    // slices of those lengths, so it never contributes to
                    // exhaustiveness over the open domain of slice lengths.
                    // The marker keeps the arm useful for redundancy analysis
                    // without claiming any coverage of the wildcard domain.
                    vec![EPat::Ctor(usize::MAX, Vec::new())]
                }
            }
            TPatKind::At { pattern, .. } => self.expand_pattern(pattern),
            TPatKind::Struct {
                struct_id, fields, ..
            } => {
                let count = self.struct_fields(*struct_id).len();
                let mut children: Vec<Vec<EPat>> = vec![vec![EPat::Wild]; count];
                for (index, pat) in fields {
                    if *index < count {
                        children[*index] = self.expand_pattern(pat);
                    }
                }
                product(&children)
                    .into_iter()
                    .map(|row| EPat::Ctor(0, row))
                    .collect()
            }
            TPatKind::Enum { variant, args, .. } => {
                let children: Vec<Vec<EPat>> =
                    args.iter().map(|pat| self.expand_pattern(pat)).collect();
                product(&children)
                    .into_iter()
                    .map(|row| EPat::Ctor(*variant, row))
                    .collect()
            }
            TPatKind::OptionSome(inner) => self
                .expand_pattern(inner)
                .into_iter()
                .map(|pat| EPat::Ctor(1, vec![pat]))
                .collect(),
            TPatKind::OptionNone => vec![EPat::Ctor(0, Vec::new())],
            TPatKind::ResultOk(inner) => self
                .expand_pattern(inner)
                .into_iter()
                .map(|pat| EPat::Ctor(0, vec![pat]))
                .collect(),
            TPatKind::ResultErr(inner) => self
                .expand_pattern(inner)
                .into_iter()
                .map(|pat| EPat::Ctor(1, vec![pat]))
                .collect(),
        }
    }

    /// Finite constructor indices for a type, or None for open domains.
    pub fn finite_ctors(&self, ty: &ITy) -> Option<Vec<usize>> {
        match ty {
            ITy::Unit => Some(vec![0]),
            ITy::Bool => Some(vec![0, 1]),
            ITy::Enum(id) => Some((0..self.variant_count(*id)).collect()),
            ITy::Option(_) => Some(vec![0, 1]),
            ITy::Result(_, _) => Some(vec![0, 1]),
            ITy::Tuple(_) | ITy::Array(_, _) => Some(vec![0]),
            ITy::Struct(_) => Some(vec![0]),
            _ => None,
        }
    }

    /// Argument types of a constructor of the given type.
    pub fn ctor_arg_types(&self, ty: &ITy, ctor: usize) -> Vec<ITy> {
        match ty {
            ITy::Unit | ITy::Bool => Vec::new(),
            ITy::Enum(id) => self.variant_fields(*id, ctor),
            ITy::Option(elem) => {
                if ctor == 1 {
                    vec![(**elem).clone()]
                } else {
                    Vec::new()
                }
            }
            ITy::Result(ok, err) => {
                if ctor == 0 {
                    vec![(**ok).clone()]
                } else {
                    vec![(**err).clone()]
                }
            }
            ITy::Tuple(elems) => elems.clone(),
            ITy::Array(elem, len) => vec![(**elem).clone(); *len],
            ITy::Struct(id) => self
                .struct_fields(*id)
                .into_iter()
                .map(|(_, _, ty)| ty)
                .collect(),
            _ => Vec::new(),
        }
    }

    fn specialize(&self, matrix: &[Vec<EPat>], ctor: usize, arity: usize) -> Vec<Vec<EPat>> {
        let mut out = Vec::new();
        for row in matrix {
            match &row[0] {
                EPat::Wild => {
                    let mut new_row = vec![EPat::Wild; arity];
                    new_row.extend_from_slice(&row[1..]);
                    out.push(new_row);
                }
                EPat::Ctor(candidate, args) => {
                    if *candidate == ctor {
                        let mut new_row = args.clone();
                        new_row.extend_from_slice(&row[1..]);
                        out.push(new_row);
                    }
                }
                EPat::Lit(_) => {}
            }
        }
        out
    }

    fn default_matrix(&self, matrix: &[Vec<EPat>]) -> Vec<Vec<EPat>> {
        let mut out = Vec::new();
        for row in matrix {
            if matches!(row[0], EPat::Wild) {
                out.push(row[1..].to_vec());
            }
        }
        out
    }

    fn specialize_lit(&self, matrix: &[Vec<EPat>], key: &LitKey) -> Vec<Vec<EPat>> {
        let mut out = Vec::new();
        for row in matrix {
            match &row[0] {
                EPat::Wild => out.push(row[1..].to_vec()),
                EPat::Lit(candidate) if candidate == key => out.push(row[1..].to_vec()),
                _ => {}
            }
        }
        out
    }

    /// Maranget-style usefulness test.
    fn is_useful(&self, matrix: &[Vec<EPat>], row: &[EPat], types: &[ITy]) -> bool {
        if types.is_empty() {
            return matrix.is_empty();
        }
        let ty = self.infer.resolve(&types[0]);
        match self.finite_ctors(&ty) {
            Some(ctors) => match &row[0] {
                EPat::Ctor(ctor, args) => {
                    let arg_types = self.ctor_arg_types(&ty, *ctor);
                    let mut new_types = arg_types.clone();
                    new_types.extend_from_slice(&types[1..]);
                    let mut new_row = args.clone();
                    new_row.extend_from_slice(&row[1..]);
                    let specialized = self.specialize(matrix, *ctor, arg_types.len());
                    self.is_useful(&specialized, &new_row, &new_types)
                }
                EPat::Wild => {
                    let complete = ctors.iter().all(|ctor| {
                        matrix
                            .iter()
                            .any(|candidate| matches!(&candidate[0], EPat::Ctor(found, _) if found == ctor))
                    });
                    if !complete {
                        let default = self.default_matrix(matrix);
                        return self.is_useful(&default, &row[1..], &types[1..]);
                    }
                    for ctor in &ctors {
                        let arg_types = self.ctor_arg_types(&ty, *ctor);
                        let mut new_types = arg_types.clone();
                        new_types.extend_from_slice(&types[1..]);
                        let mut new_row = vec![EPat::Wild; arg_types.len()];
                        new_row.extend_from_slice(&row[1..]);
                        let specialized = self.specialize(matrix, *ctor, arg_types.len());
                        if self.is_useful(&specialized, &new_row, &new_types) {
                            return true;
                        }
                    }
                    false
                }
                EPat::Lit(_) => {
                    let default = self.default_matrix(matrix);
                    self.is_useful(&default, &row[1..], &types[1..])
                }
            },
            None => match &row[0] {
                EPat::Lit(key) => {
                    let specialized = self.specialize_lit(matrix, key);
                    self.is_useful(&specialized, &row[1..], &types[1..])
                }
                _ => {
                    let default = self.default_matrix(matrix);
                    self.is_useful(&default, &row[1..], &types[1..])
                }
            },
        }
    }

    /// Search for an uncovered value, if the matrix is not exhaustive.
    fn find_witness(&self, matrix: &[Vec<EPat>], types: &[ITy]) -> Option<Vec<EPat>> {
        if types.is_empty() {
            if matrix.is_empty() {
                return Some(Vec::new());
            }
            return None;
        }
        let ty = self.infer.resolve(&types[0]);
        match self.finite_ctors(&ty) {
            Some(ctors) => {
                for ctor in ctors {
                    let arg_types = self.ctor_arg_types(&ty, ctor);
                    let mut new_types = arg_types.clone();
                    new_types.extend_from_slice(&types[1..]);
                    let specialized = self.specialize(matrix, ctor, arg_types.len());
                    if let Some(mut witness) = self.find_witness(&specialized, &new_types) {
                        let args: Vec<EPat> = witness.drain(0..arg_types.len()).collect();
                        let mut out = vec![EPat::Ctor(ctor, args)];
                        out.extend(witness);
                        return Some(out);
                    }
                }
                None
            }
            None => {
                let mut used = Vec::new();
                let mut has_wild = false;
                for row in matrix {
                    match &row[0] {
                        EPat::Lit(key) => used.push(key.clone()),
                        EPat::Wild => has_wild = true,
                        EPat::Ctor(_, _) => {}
                    }
                }
                let key = fresh_lit(&ty, &used);
                let mut out = vec![EPat::Lit(key)];
                if has_wild {
                    let default = self.default_matrix(matrix);
                    let mut rest = self.find_witness(&default, &types[1..])?;
                    out.append(&mut rest);
                } else {
                    for _ in &types[1..] {
                        out.push(EPat::Wild);
                    }
                }
                Some(out)
            }
        }
    }

    /// Render a witness pattern for a diagnostic message.
    fn render_witness(&self, ty: &ITy, pat: &EPat) -> String {
        let ty = self.infer.resolve(ty);
        match pat {
            EPat::Wild => "_".to_string(),
            EPat::Lit(key) => match key {
                LitKey::Int(value) => value.to_string(),
                LitKey::Float(bits) => f64::from_bits(*bits).to_string(),
                LitKey::Char(value) => format!("{:?}", value),
                LitKey::Str(text) => format!("{:?}", text),
            },
            EPat::Ctor(ctor, args) => match &ty {
                ITy::Unit => "()".to_string(),
                ITy::Bool => {
                    if *ctor == 0 {
                        "false".to_string()
                    } else {
                        "true".to_string()
                    }
                }
                ITy::Enum(id) => {
                    let name = self.variant_name(*id, *ctor);
                    if args.is_empty() {
                        name
                    } else {
                        let arg_types = self.ctor_arg_types(&ty, *ctor);
                        format!("{}({})", name, self.render_children(&arg_types, args))
                    }
                }
                ITy::Option(_) => {
                    if *ctor == 0 {
                        "None".to_string()
                    } else {
                        format!("Some({})", self.render_child(&ty, *ctor, 0, args))
                    }
                }
                ITy::Result(_, _) => {
                    if *ctor == 0 {
                        format!("Ok({})", self.render_child(&ty, *ctor, 0, args))
                    } else {
                        format!("Err({})", self.render_child(&ty, *ctor, 0, args))
                    }
                }
                ITy::Array(elem, len) => {
                    let elems = vec![(**elem).clone(); *len];
                    format!("[{}]", self.render_children(&elems, args))
                }
                ITy::Tuple(elems) => {
                    if elems.is_empty() {
                        "()".to_string()
                    } else if elems.len() == 1 {
                        format!("({},)", self.render_children(elems, args))
                    } else {
                        format!("({})", self.render_children(elems, args))
                    }
                }
                ITy::Struct(id) => {
                    let fields = self.struct_fields(*id);
                    let mut parts = Vec::new();
                    for (index, (name, _, _)) in fields.iter().enumerate() {
                        let rendered = args
                            .get(index)
                            .map(|arg| self.render_witness(&fields[index].2, arg))
                            .unwrap_or_else(|| "_".to_string());
                        parts.push(format!("{}: {}", name, rendered));
                    }
                    format!("{} {{ {} }}", self.struct_name(*id), parts.join(", "))
                }
                _ => "_".to_string(),
            },
        }
    }

    fn render_children(&self, types: &[ITy], args: &[EPat]) -> String {
        let mut parts = Vec::new();
        for (index, child_ty) in types.iter().enumerate() {
            match args.get(index) {
                Some(arg) => parts.push(self.render_witness(child_ty, arg)),
                None => parts.push("_".to_string()),
            }
        }
        parts.join(", ")
    }

    fn render_child(&self, ty: &ITy, ctor: usize, index: usize, args: &[EPat]) -> String {
        let arg_types = self.ctor_arg_types(ty, ctor);
        match (args.get(index), arg_types.get(index)) {
            (Some(arg), Some(child_ty)) => self.render_witness(child_ty, arg),
            _ => "_".to_string(),
        }
    }
}

fn bool_or_lit_epat(lit: &TLit) -> EPat {
    match lit {
        TLit::Int(value) => EPat::Lit(LitKey::Int(*value)),
        TLit::Float(value) => EPat::Lit(LitKey::Float(value.to_bits())),
        TLit::Bool(value) => EPat::Ctor(if *value { 1 } else { 0 }, Vec::new()),
        TLit::Char(value) => EPat::Lit(LitKey::Char(*value)),
        TLit::Str(text) => EPat::Lit(LitKey::Str(text.clone())),
        TLit::Unit => EPat::Ctor(0, Vec::new()),
    }
}

fn fresh_lit(ty: &ITy, used: &[LitKey]) -> LitKey {
    match ty {
        ITy::Int(_) => {
            let mut value: i128 = 0;
            loop {
                let candidate = LitKey::Int(value);
                if !used.contains(&candidate) {
                    return candidate;
                }
                value += 1;
            }
        }
        ITy::Float(_) => {
            let mut value: f64 = 0.0;
            loop {
                let candidate = LitKey::Float(value.to_bits());
                if !used.contains(&candidate) {
                    return candidate;
                }
                value += 1.0;
            }
        }
        ITy::Char => {
            let mut value: u32 = 0x61;
            loop {
                if let Some(character) = char::from_u32(value) {
                    let candidate = LitKey::Char(character);
                    if !used.contains(&candidate) {
                        return candidate;
                    }
                }
                value = match value.checked_add(1) {
                    Some(next) if next <= 0x10FFFF => next,
                    _ => return LitKey::Char('a'),
                };
            }
        }
        ITy::String => {
            let mut counter = 0usize;
            loop {
                let candidate = LitKey::Str(format!("x{}", counter));
                if !used.contains(&candidate) {
                    return candidate;
                }
                counter += 1;
            }
        }
        _ => LitKey::Int(0),
    }
}
