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
                    let _ = self.infer.unify(&ty, &ITy::i64());
                    ty = ITy::i64();
                }
                VarKind::FloatLiteral => {
                    let _ = self.infer.unify(&ty, &ITy::f64());
                    ty = ITy::f64();
                }
                VarKind::General => return,
            }
        }
        let mut matrix: Vec<Vec<EPat>> = Vec::new();
        for arm in arms {
            let rows = self.expand_pattern(&arm.pat);
            let mut useful = false;
            for row in &rows {
                if self.is_useful(&matrix, std::slice::from_ref(row), &[ty.clone()]) {
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
        if self.is_useful(&matrix, &[EPat::Wild], &[ty.clone()]) {
            let rendered = match self.find_witness(&matrix, &[ty.clone()]) {
                Some(witness) => match witness.first() {
                    Some(epat) => self.render_witness(&ty, epat),
                    None => "_".to_string(),
                },
                None => "_".to_string(),
            };
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
    }

    /// True when the pattern always matches values of its type.
    pub fn pattern_irrefutable(&self, pat: &TPat) -> bool {
        match &pat.kind {
            TPatKind::Wild | TPatKind::Bind { .. } => true,
            TPatKind::Lit(_) => false,
            TPatKind::Tuple(pats) => pats.iter().all(|pat| self.pattern_irrefutable(pat)),
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
            TPatKind::Tuple(pats) => {
                let children: Vec<Vec<EPat>> =
                    pats.iter().map(|pat| self.expand_pattern(pat)).collect();
                product(&children)
                    .into_iter()
                    .map(|row| EPat::Ctor(0, row))
                    .collect()
            }
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
            ITy::Tuple(_) => Some(vec![0]),
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
