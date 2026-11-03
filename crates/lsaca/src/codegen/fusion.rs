//! Fusion of lazy iterator chains into plain C loops.
//!
//! The iterator sugar in the type checker lowers iter(), map(), filter() and
//! friends to calls of the hidden __lsaca_it_* helpers from the standard
//! library. Those helpers are ordinary generic code: every item goes through a
//! boxed closure call and every adapter is a reference counted struct. For the
//! common case of a chain over a Vec whose closures are small and capture
//! nothing, that generic lowering is pure overhead. This module recognises the
//! HIR shape of such a chain and expands it in place into a single C loop with
//! the closure bodies inlined.
//!
//! The transformation is only an optimisation. It is applied only when the
//! whole chain has Copy items and every closure body is a Copy-only expression
//! that reads nothing but its own parameters. In every other case the generic
//! iterator lowering runs, so the recognition rules can never change the
//! meaning of a program.

use crate::hir::{HBlock, HClosure, HExpr, HExprKind, HFormatPart, HFormatPartKind, HPat};
use crate::span::Span;
use crate::types::Ty;

use super::emit::{Gen, LoopCtx};
use super::ops;
use super::reachability;
use super::types;
use super::util;

/// One adapter stage of a recognised iterator chain.
#[derive(Clone)]
enum FusedStage {
    Map { closure: HClosure },
    Filter { closure: HClosure },
    Take { count: HExpr },
    Skip { count: HExpr },
    Enumerate,
}

/// A recognised Vec source followed by its adapter stages.
#[derive(Clone)]
struct FusedPipeline {
    source: HExpr,
    stages: Vec<FusedStage>,
}

/// The operation that consumes a fused chain.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
enum FusedConsumer {
    ForBody { pat: HPat, body: HBlock },
    Fold { init: HExpr, closure: HClosure },
    Count,
    SumI64,
    SumF64,
    Any { closure: HClosure },
    All { closure: HClosure },
    Find { closure: HClosure },
    CollectVec,
}

/// C state owned by one adapter stage of a fused loop.
enum FusedSlot {
    Plain,
    Limited { limit: String, counter: String },
    Counting { counter: String },
}

/// C state owned by the consumer of a fused loop.
enum FusedRuntime {
    None,
    Fold { accumulator: String },
    Count { counter: String },
    SumI64 { total: String },
    SumF64 { total: String },
    AnyAll { matched: String },
    Find { result: String },
    Collect { result: String },
}

/// Try to expand a for loop over a recognised iterator chain.
///
/// Returns true when the loop was emitted as a fused C loop, in which case the
/// caller must not generate the generic iterator lowering.
pub(crate) fn try_gen_fused_for(
    generator: &mut Gen,
    iter: &HExpr,
    pat: &HPat,
    body: &HBlock,
    ty: &Ty,
    target: &str,
) -> bool {
    let Some(pipeline) = parse_fused_pipeline(generator, iter) else {
        return false;
    };
    let Some(item_ty) = fused_item_type(generator, &pipeline) else {
        return false;
    };
    if pat.ty != item_ty {
        return false;
    }
    let consumer = FusedConsumer::ForBody {
        pat: pat.clone(),
        body: body.clone(),
    };
    emit_fused_pipeline(generator, &pipeline, &consumer, ty, target, iter.span);
    true
}

/// Try to expand a consuming terminal call over a recognised iterator chain.
///
/// Returns true when the call was emitted as a fused C loop, in which case the
/// caller must not generate the generic stdlib call.
pub(crate) fn try_gen_fused_terminal(
    generator: &mut Gen,
    fn_id: usize,
    args: &[HExpr],
    ty: &Ty,
    target: &str,
    span: Span,
) -> bool {
    let Some(name) = standard_library_name(generator, fn_id) else {
        return false;
    };
    match name.as_str() {
        "__lsaca_it_fold" => {
            if args.len() != 3 {
                return false;
            }
            let Some(pipeline) = parse_fused_pipeline(generator, &args[0]) else {
                return false;
            };
            let Some(item_ty) = fused_item_type(generator, &pipeline) else {
                return false;
            };
            let Some(closure) = fused_closure(generator, &args[2], 2) else {
                return false;
            };
            let init_ty = args[1].ty.clone();
            if closure.ret != init_ty {
                return false;
            }
            if closure.params[0].ty != init_ty || closure.params[1].ty != item_ty {
                return false;
            }
            if ty != &init_ty {
                return false;
            }
            let consumer = FusedConsumer::Fold {
                init: args[1].clone(),
                closure,
            };
            emit_fused_pipeline(generator, &pipeline, &consumer, ty, target, span);
            true
        }
        "__lsaca_it_count" => {
            if args.len() != 1 {
                return false;
            }
            let Some(pipeline) = parse_fused_pipeline(generator, &args[0]) else {
                return false;
            };
            if fused_item_type(generator, &pipeline).is_none() {
                return false;
            }
            if ty != &Ty::i64() {
                return false;
            }
            emit_fused_pipeline(
                generator,
                &pipeline,
                &FusedConsumer::Count,
                ty,
                target,
                span,
            );
            true
        }
        "__lsaca_it_sum_i64" => {
            if args.len() != 1 {
                return false;
            }
            let Some(pipeline) = parse_fused_pipeline(generator, &args[0]) else {
                return false;
            };
            let Some(item_ty) = fused_item_type(generator, &pipeline) else {
                return false;
            };
            if item_ty != Ty::i64() || ty != &Ty::i64() {
                return false;
            }
            emit_fused_pipeline(
                generator,
                &pipeline,
                &FusedConsumer::SumI64,
                ty,
                target,
                span,
            );
            true
        }
        "__lsaca_it_sum_f64" => {
            if args.len() != 1 {
                return false;
            }
            let Some(pipeline) = parse_fused_pipeline(generator, &args[0]) else {
                return false;
            };
            let Some(item_ty) = fused_item_type(generator, &pipeline) else {
                return false;
            };
            if item_ty != Ty::f64() || ty != &Ty::f64() {
                return false;
            }
            emit_fused_pipeline(
                generator,
                &pipeline,
                &FusedConsumer::SumF64,
                ty,
                target,
                span,
            );
            true
        }
        "__lsaca_it_any" | "__lsaca_it_all" => {
            if args.len() != 2 {
                return false;
            }
            let Some(pipeline) = parse_fused_pipeline(generator, &args[0]) else {
                return false;
            };
            let Some(item_ty) = fused_item_type(generator, &pipeline) else {
                return false;
            };
            let Some(closure) = fused_closure(generator, &args[1], 1) else {
                return false;
            };
            let expected = Ty::Ref {
                mutable: false,
                inner: Box::new(item_ty),
            };
            if closure.params[0].ty != expected || closure.ret != Ty::Bool || ty != &Ty::Bool {
                return false;
            }
            let consumer = if name == "__lsaca_it_any" {
                FusedConsumer::Any { closure }
            } else {
                FusedConsumer::All { closure }
            };
            emit_fused_pipeline(generator, &pipeline, &consumer, ty, target, span);
            true
        }
        "__lsaca_it_find" => {
            if args.len() != 2 {
                return false;
            }
            let Some(pipeline) = parse_fused_pipeline(generator, &args[0]) else {
                return false;
            };
            let Some(item_ty) = fused_item_type(generator, &pipeline) else {
                return false;
            };
            let Some(closure) = fused_closure(generator, &args[1], 1) else {
                return false;
            };
            let expected = Ty::Ref {
                mutable: false,
                inner: Box::new(item_ty.clone()),
            };
            let option_ty = Ty::Option(Box::new(item_ty));
            if closure.params[0].ty != expected || closure.ret != Ty::Bool {
                return false;
            }
            if ty != &option_ty {
                return false;
            }
            let consumer = FusedConsumer::Find { closure };
            emit_fused_pipeline(generator, &pipeline, &consumer, ty, target, span);
            true
        }
        "__lsaca_it_collect_vec" => {
            if args.len() != 1 {
                return false;
            }
            let Some(pipeline) = parse_fused_pipeline(generator, &args[0]) else {
                return false;
            };
            let Some(item_ty) = fused_item_type(generator, &pipeline) else {
                return false;
            };
            if ty != &Ty::Vec(Box::new(item_ty)) {
                return false;
            }
            emit_fused_pipeline(
                generator,
                &pipeline,
                &FusedConsumer::CollectVec,
                ty,
                target,
                span,
            );
            true
        }
        _ => false,
    }
}

/// Bare name of a reachable standard-library function, if the call targets one.
fn standard_library_name(generator: &Gen, fn_id: usize) -> Option<String> {
    let function = generator.program.fns.get(fn_id)?;
    if !reachability::is_standard_library(generator.options, function.span) {
        return None;
    }
    // Monomorphised instances carry their type arguments in the HIR name, for
    // example __lsaca_it_fold<i64, i64, ...>; the emitter tables are keyed by
    // the declared name before the first angle bracket.
    let bare = function
        .name
        .split('<')
        .next()
        .unwrap_or(function.name.as_str());
    Some(bare.trim().to_string())
}

/// Recognise a cloneable closure with a trivial body and the given arity.
fn fused_closure(generator: &Gen, expr: &HExpr, arity: usize) -> Option<HClosure> {
    let HExprKind::Closure {
        closure_id,
        captures,
    } = &expr.kind
    else {
        return None;
    };
    if !captures.is_empty() {
        return None;
    }
    let closure = generator.program.closures.get(*closure_id)?.clone();
    if !closure.captures.is_empty() || closure.params.len() != arity {
        return None;
    }
    for parameter in &closure.params {
        if !generator.program.is_copy(&parameter.ty) {
            return None;
        }
    }
    if !generator.program.is_copy(&closure.ret) {
        return None;
    }
    if !closure.body.stmts.is_empty() {
        return None;
    }
    let tail = closure.body.tail.as_deref()?;
    if tail.ty != closure.ret {
        return None;
    }
    let mut parameters = Vec::with_capacity(closure.params.len());
    for parameter in &closure.params {
        parameters.push(parameter.c_name.clone());
    }
    if !fused_expression_safe(generator, tail, &parameters) {
        return None;
    }
    Some(closure)
}

/// Recognise the hidden iterator helper chain of an expression.
fn parse_fused_pipeline(generator: &Gen, expr: &HExpr) -> Option<FusedPipeline> {
    let HExprKind::DirectCall {
        fn_id,
        receiver: None,
        args,
    } = &expr.kind
    else {
        return None;
    };
    let name = standard_library_name(generator, *fn_id)?;
    match name.as_str() {
        "__lsaca_vec_iter" => {
            if args.len() != 1 {
                return None;
            }
            let Ty::Vec(element) = &args[0].ty else {
                return None;
            };
            if !generator.program.is_copy(element) {
                return None;
            }
            Some(FusedPipeline {
                source: args[0].clone(),
                stages: Vec::new(),
            })
        }
        "__lsaca_it_map" => {
            if args.len() != 2 {
                return None;
            }
            let mut pipeline = parse_fused_pipeline(generator, &args[0])?;
            let closure = fused_closure(generator, &args[1], 1)?;
            pipeline.stages.push(FusedStage::Map { closure });
            Some(pipeline)
        }
        "__lsaca_it_filter" => {
            if args.len() != 2 {
                return None;
            }
            let mut pipeline = parse_fused_pipeline(generator, &args[0])?;
            let closure = fused_closure(generator, &args[1], 1)?;
            pipeline.stages.push(FusedStage::Filter { closure });
            Some(pipeline)
        }
        "__lsaca_it_take" => {
            if args.len() != 2 || args[1].ty != Ty::i64() {
                return None;
            }
            let mut pipeline = parse_fused_pipeline(generator, &args[0])?;
            pipeline.stages.push(FusedStage::Take {
                count: args[1].clone(),
            });
            Some(pipeline)
        }
        "__lsaca_it_skip" => {
            if args.len() != 2 || args[1].ty != Ty::i64() {
                return None;
            }
            let mut pipeline = parse_fused_pipeline(generator, &args[0])?;
            pipeline.stages.push(FusedStage::Skip {
                count: args[1].clone(),
            });
            Some(pipeline)
        }
        "__lsaca_it_enumerate" => {
            if args.len() != 1 {
                return None;
            }
            let mut pipeline = parse_fused_pipeline(generator, &args[0])?;
            pipeline.stages.push(FusedStage::Enumerate);
            Some(pipeline)
        }
        _ => None,
    }
}

/// Type of one item after every adapter stage, if the chain is well formed.
fn fused_item_type(generator: &Gen, pipeline: &FusedPipeline) -> Option<Ty> {
    let Ty::Vec(element) = &pipeline.source.ty else {
        return None;
    };
    let mut item = (**element).clone();
    for stage in &pipeline.stages {
        match stage {
            FusedStage::Map { closure } => {
                if closure.params[0].ty != item {
                    return None;
                }
                item = closure.ret.clone();
            }
            FusedStage::Filter { closure } => {
                let expected = Ty::Ref {
                    mutable: false,
                    inner: Box::new(item.clone()),
                };
                if closure.params[0].ty != expected || closure.ret != Ty::Bool {
                    return None;
                }
            }
            FusedStage::Take { .. } | FusedStage::Skip { .. } => {}
            FusedStage::Enumerate => {
                item = Ty::tuple(vec![Ty::i64(), item]);
            }
        }
    }
    if !generator.program.is_copy(&item) {
        return None;
    }
    Some(item)
}

/// True when an expression is Copy-only and reads nothing but its parameters.
fn fused_expression_safe(generator: &Gen, expr: &HExpr, parameters: &[String]) -> bool {
    if !generator.program.is_copy(&expr.ty) {
        return false;
    }
    match &expr.kind {
        HExprKind::Lit(_) | HExprKind::ConstRef { .. } | HExprKind::OptionNone => true,
        HExprKind::Local { c_name, .. } => parameters.iter().any(|parameter| parameter == c_name),
        HExprKind::Capture { .. }
        | HExprKind::FnRef { .. }
        | HExprKind::Closure { .. }
        | HExprKind::Call { .. }
        | HExprKind::Break
        | HExprKind::Continue
        | HExprKind::Return(_)
        | HExprKind::Loop { .. }
        | HExprKind::While { .. }
        | HExprKind::ForRange { .. }
        | HExprKind::ForVec { .. }
        | HExprKind::ForIter { .. }
        | HExprKind::VecRepeat { .. }
        | HExprKind::DynCoerce { .. }
        | HExprKind::DynCall { .. }
        | HExprKind::Slice { .. } => false,
        HExprKind::DirectCall { receiver, args, .. } => {
            let receiver_safe = match receiver.as_deref() {
                Some(value) => fused_expression_safe(generator, value, parameters),
                None => true,
            };
            receiver_safe
                && args
                    .iter()
                    .all(|argument| fused_expression_safe(generator, argument, parameters))
        }
        HExprKind::BuiltinMethod { receiver, args, .. } => {
            fused_expression_safe(generator, receiver, parameters)
                && args
                    .iter()
                    .all(|argument| fused_expression_safe(generator, argument, parameters))
        }
        HExprKind::BuiltinAssoc { args, .. } | HExprKind::BuiltinCall { args, .. } => args
            .iter()
            .all(|argument| fused_expression_safe(generator, argument, parameters)),
        HExprKind::Field { base, .. }
        | HExprKind::Cast { expr: base }
        | HExprKind::Unary { expr: base, .. }
        | HExprKind::Ref { expr: base, .. }
        | HExprKind::Deref { expr: base } => fused_expression_safe(generator, base, parameters),
        HExprKind::Index { base, index } => {
            fused_expression_safe(generator, base, parameters)
                && fused_expression_safe(generator, index, parameters)
        }
        HExprKind::Binary { lhs, rhs, .. } => {
            fused_expression_safe(generator, lhs, parameters)
                && fused_expression_safe(generator, rhs, parameters)
        }
        HExprKind::Assign { target, value, .. } => {
            fused_expression_safe(generator, target, parameters)
                && fused_expression_safe(generator, value, parameters)
        }
        HExprKind::Block(block) => fused_block_safe(generator, block, parameters),
        HExprKind::If {
            cond,
            then_block,
            else_expr,
        } => {
            let else_safe = match else_expr.as_deref() {
                Some(value) => fused_expression_safe(generator, value, parameters),
                None => true,
            };
            fused_expression_safe(generator, cond, parameters)
                && fused_block_safe(generator, then_block, parameters)
                && else_safe
        }
        HExprKind::Match { scrutinee, arms } => {
            fused_expression_safe(generator, scrutinee, parameters)
                && arms.iter().all(|arm| {
                    let guard_safe = match &arm.guard {
                        Some(guard) => fused_expression_safe(generator, guard, parameters),
                        None => true,
                    };
                    guard_safe && fused_expression_safe(generator, &arm.body, parameters)
                })
        }
        HExprKind::StructLit { fields, .. } => fields
            .iter()
            .all(|(_, value)| fused_expression_safe(generator, value, parameters)),
        HExprKind::Tuple(elems) | HExprKind::VecLit { elems } => elems
            .iter()
            .all(|element| fused_expression_safe(generator, element, parameters)),
        HExprKind::EnumLit { args, .. } => args
            .iter()
            .all(|argument| fused_expression_safe(generator, argument, parameters)),
        HExprKind::OptionSome(inner) | HExprKind::ResultOk(inner) | HExprKind::ResultErr(inner) => {
            fused_expression_safe(generator, inner, parameters)
        }
        HExprKind::Format { parts } => parts
            .iter()
            .all(|part| fused_format_part_safe(generator, part, parameters)),
        HExprKind::Macro(macro_call) => {
            macro_call
                .parts
                .iter()
                .all(|part| fused_format_part_safe(generator, part, parameters))
                && macro_call
                    .args
                    .iter()
                    .all(|argument| fused_expression_safe(generator, argument, parameters))
        }
    }
}

fn fused_block_safe(generator: &Gen, block: &HBlock, parameters: &[String]) -> bool {
    if !block.stmts.is_empty() {
        return false;
    }
    match block.tail.as_deref() {
        Some(tail) => fused_expression_safe(generator, tail, parameters),
        None => true,
    }
}

fn fused_format_part_safe(generator: &Gen, part: &HFormatPart, parameters: &[String]) -> bool {
    match &part.kind {
        HFormatPartKind::Literal(_) => true,
        HFormatPartKind::Arg { expr, .. } => fused_expression_safe(generator, expr, parameters),
    }
}

/// Emit one fused loop for the recognised chain.
fn emit_fused_pipeline(
    generator: &mut Gen,
    pipeline: &FusedPipeline,
    consumer: &FusedConsumer,
    ty: &Ty,
    target: &str,
    span: Span,
) {
    let break_label = generator.fresh_label("for_end");
    let continue_label = generator.fresh_label("for_next");
    if matches!(consumer, FusedConsumer::ForBody { .. }) && !ty.is_unit() {
        let zero = generator.zero_expr(ty);
        generator.w.line(format!("{} = {};", target, zero));
    }
    generator.push_frame();
    let frame = generator.frames.len() - 1;
    let source_ty = pipeline.source.ty.clone();
    let vector = generator.declare_temp(&source_ty);
    generator.gen_move_into(&pipeline.source, &vector);
    let mut slots = Vec::with_capacity(pipeline.stages.len());
    for stage in &pipeline.stages {
        let slot = match stage {
            FusedStage::Take { count } | FusedStage::Skip { count } => {
                let limit = generator.gen_owned(count);
                let counter = generator.new_plain_of("int64_t", "taken");
                generator.w.line(format!("{} = 0;", counter));
                FusedSlot::Limited { limit, counter }
            }
            FusedStage::Enumerate => {
                let counter = generator.new_plain_of("int64_t", "position");
                generator.w.line(format!("{} = 0;", counter));
                FusedSlot::Counting { counter }
            }
            FusedStage::Map { .. } | FusedStage::Filter { .. } => FusedSlot::Plain,
        };
        slots.push(slot);
    }
    let runtime = fused_runtime(generator, consumer, ty);
    let index = generator.new_plain_of("size_t", "i");
    generator.w.line(format!("{} = 0;", index));
    generator.loops.push(LoopCtx {
        break_label: break_label.clone(),
        continue_label: continue_label.clone(),
        body_frame: generator.frames.len(),
        used_break: false,
        used_continue: false,
    });
    generator.w.line("for (;;) {");
    generator.w.indent();
    generator
        .w
        .line(format!("if ({} >= ({}).len) break;", index, vector));
    let current = format!("({}).data[{}]", vector, index);
    if fused_item_type(generator, pipeline).is_none() {
        util::internal_error("fused iterator pipeline has no item type");
    }
    // Each stage receives the type of the item it consumes. The loop body
    // starts from the element type of the source vector; adapters derive the
    // type they hand to the next stage (map uses its return type, enumerate
    // wraps the incoming type in a tuple). Seeding the chain with the final
    // item type would wrap enumerate pairs in a second tuple.
    let Ty::Vec(element) = &pipeline.source.ty else {
        util::internal_error("fused iterator pipeline source is not a vector");
    };
    let element_ty = (**element).clone();
    emit_fused_stages(
        generator,
        pipeline,
        &slots,
        &runtime,
        0,
        current,
        &element_ty,
        consumer,
        &continue_label,
        span,
    );
    generator.w.line(format!("{} += 1;", index));
    generator.w.dedent();
    generator.w.line("}");
    let used_break = generator
        .loops
        .last()
        .map(|context| context.used_break)
        .unwrap_or(false);
    generator.loops.pop();
    if used_break {
        generator.w.line(format!("{}:;", break_label));
    }
    fused_finish(generator, &runtime, target);
    generator.frame_cleanup(frame);
    generator.pop_frame();
    generator.block_terminated = false;
    generator.fn_terminated = false;
}

/// Allocate the C state of a consumer before the fused loop runs.
fn fused_runtime(generator: &mut Gen, consumer: &FusedConsumer, ty: &Ty) -> FusedRuntime {
    match consumer {
        FusedConsumer::ForBody { .. } => FusedRuntime::None,
        FusedConsumer::Fold { init, .. } => {
            let init_ty = init.ty.clone();
            let accumulator = generator.new_plain(&init_ty, "acc");
            generator.gen_move_into(init, &accumulator);
            FusedRuntime::Fold { accumulator }
        }
        FusedConsumer::Count => {
            let counter = generator.new_plain_of("int64_t", "count");
            generator.w.line(format!("{} = 0;", counter));
            FusedRuntime::Count { counter }
        }
        FusedConsumer::SumI64 => {
            let total = generator.new_plain_of("int64_t", "total");
            generator.w.line(format!("{} = 0;", total));
            FusedRuntime::SumI64 { total }
        }
        FusedConsumer::SumF64 => {
            let double = generator.ctype(&Ty::f64());
            let total = generator.new_plain_of(&double, "total");
            generator.w.line(format!("{} = 0.0;", total));
            FusedRuntime::SumF64 { total }
        }
        FusedConsumer::Any { .. } => {
            let matched = generator.new_plain_of("bool", "matched");
            generator.w.line(format!("{} = false;", matched));
            FusedRuntime::AnyAll { matched }
        }
        FusedConsumer::All { .. } => {
            let matched = generator.new_plain_of("bool", "matched");
            generator.w.line(format!("{} = true;", matched));
            FusedRuntime::AnyAll { matched }
        }
        FusedConsumer::Find { .. } => {
            let result = generator.declare_temp(ty);
            FusedRuntime::Find { result }
        }
        FusedConsumer::CollectVec => {
            let result = generator.declare_temp(ty);
            FusedRuntime::Collect { result }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_fused_stages(
    generator: &mut Gen,
    pipeline: &FusedPipeline,
    slots: &[FusedSlot],
    runtime: &FusedRuntime,
    index: usize,
    current: String,
    current_ty: &Ty,
    consumer: &FusedConsumer,
    continue_label: &str,
    span: Span,
) {
    let Some(stage) = pipeline.stages.get(index) else {
        emit_fused_leaf(
            generator,
            current,
            current_ty,
            runtime,
            consumer,
            continue_label,
            span,
        );
        return;
    };
    match stage {
        FusedStage::Map { closure } => {
            let Some(tail) = closure.body.tail.as_deref() else {
                return;
            };
            let parameter = &closure.params[0];
            if parameter.name != "_" {
                let ctype = generator.ctype(&parameter.ty);
                generator
                    .w
                    .line(format!("{} {} = {};", ctype, parameter.c_name, current));
            }
            let value = generator.gen_owned(tail);
            let next_ty = closure.ret.clone();
            emit_fused_stages(
                generator,
                pipeline,
                slots,
                runtime,
                index + 1,
                value,
                &next_ty,
                consumer,
                continue_label,
                span,
            );
        }
        FusedStage::Filter { closure } => {
            let Some(tail) = closure.body.tail.as_deref() else {
                return;
            };
            let parameter = &closure.params[0];
            if parameter.name != "_" {
                let ctype = generator.ctype(&parameter.ty);
                generator
                    .w
                    .line(format!("{} {} = &({});", ctype, parameter.c_name, current));
            }
            let predicate = generator.gen_owned(tail);
            generator.w.line(format!("if ({}) {{", predicate));
            generator.w.indent();
            emit_fused_stages(
                generator,
                pipeline,
                slots,
                runtime,
                index + 1,
                current,
                current_ty,
                consumer,
                continue_label,
                span,
            );
            generator.w.dedent();
            generator.w.line("}");
        }
        FusedStage::Take { .. } => {
            let FusedSlot::Limited { limit, counter } = &slots[index] else {
                return;
            };
            generator.w.line(format!("if ({} < {}) {{", counter, limit));
            generator.w.indent();
            generator.w.line(format!("{} += 1;", counter));
            emit_fused_stages(
                generator,
                pipeline,
                slots,
                runtime,
                index + 1,
                current,
                current_ty,
                consumer,
                continue_label,
                span,
            );
            generator.w.dedent();
            generator.w.line("} else {");
            generator.w.indent();
            generator.w.line("break;");
            generator.w.dedent();
            generator.w.line("}");
        }
        FusedStage::Skip { .. } => {
            let FusedSlot::Limited { limit, counter } = &slots[index] else {
                return;
            };
            generator.w.line(format!("if ({} < {}) {{", counter, limit));
            generator.w.indent();
            generator.w.line(format!("{} += 1;", counter));
            generator.w.dedent();
            generator.w.line("} else {");
            generator.w.indent();
            emit_fused_stages(
                generator,
                pipeline,
                slots,
                runtime,
                index + 1,
                current,
                current_ty,
                consumer,
                continue_label,
                span,
            );
            generator.w.dedent();
            generator.w.line("}");
        }
        FusedStage::Enumerate => {
            let FusedSlot::Counting { counter } = &slots[index] else {
                return;
            };
            let tuple_ty = Ty::tuple(vec![Ty::i64(), current_ty.clone()]);
            let pair = generator.new_plain(&tuple_ty, "pair");
            generator.w.line(format!(
                "({}).{} = {};",
                pair,
                types::positional_field(0),
                counter
            ));
            generator.w.line(format!(
                "({}).{} = {};",
                pair,
                types::positional_field(1),
                current
            ));
            generator.w.line(format!("{} += 1;", counter));
            emit_fused_stages(
                generator,
                pipeline,
                slots,
                runtime,
                index + 1,
                pair,
                &tuple_ty,
                consumer,
                continue_label,
                span,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_fused_leaf(
    generator: &mut Gen,
    current: String,
    current_ty: &Ty,
    runtime: &FusedRuntime,
    consumer: &FusedConsumer,
    continue_label: &str,
    span: Span,
) {
    match runtime {
        FusedRuntime::None => {
            if let FusedConsumer::ForBody { pat, body } = consumer {
                generator.gen_loop_body(
                    body,
                    Some((pat, current.as_str(), false)),
                    continue_label,
                    None,
                );
            }
        }
        FusedRuntime::Fold { accumulator } => {
            let FusedConsumer::Fold { closure, .. } = consumer else {
                return;
            };
            let Some(tail) = closure.body.tail.as_deref() else {
                return;
            };
            if let Some(parameter) = closure.params.first()
                && parameter.name != "_"
            {
                let ctype = generator.ctype(&parameter.ty);
                generator
                    .w
                    .line(format!("{} {} = {};", ctype, parameter.c_name, accumulator));
            }
            if let Some(parameter) = closure.params.get(1)
                && parameter.name != "_"
            {
                let ctype = generator.ctype(&parameter.ty);
                generator
                    .w
                    .line(format!("{} {} = {};", ctype, parameter.c_name, current));
            }
            let value = generator.gen_owned(tail);
            generator.w.line(format!("{} = {};", accumulator, value));
        }
        FusedRuntime::Count { counter } => {
            generator.w.line(format!("{} += 1;", counter));
        }
        FusedRuntime::SumI64 { total } => {
            let expression = if generator.debug_checks {
                format!(
                    "ls_add_i64_checked({}, {}, {})",
                    total,
                    current,
                    generator.loc(span)
                )
            } else {
                format!("(int64_t)((uint64_t)({}) + (uint64_t)({}))", total, current)
            };
            generator.w.line(format!("{} = {};", total, expression));
        }
        FusedRuntime::SumF64 { total } => {
            let double = generator.ctype(&Ty::f64());
            generator.w.line(format!(
                "{} = ({})(({}) + ({}));",
                total, double, total, current
            ));
        }
        FusedRuntime::AnyAll { matched } => {
            let closure = match consumer {
                FusedConsumer::Any { closure } | FusedConsumer::All { closure } => closure,
                _ => return,
            };
            let Some(tail) = closure.body.tail.as_deref() else {
                return;
            };
            if let Some(parameter) = closure.params.first()
                && parameter.name != "_"
            {
                let ctype = generator.ctype(&parameter.ty);
                generator
                    .w
                    .line(format!("{} {} = &({});", ctype, parameter.c_name, current));
            }
            let predicate = generator.gen_owned(tail);
            let is_all = matches!(consumer, FusedConsumer::All { .. });
            let condition = if is_all {
                format!("!({})", predicate)
            } else {
                predicate
            };
            generator.w.line(format!("if ({}) {{", condition));
            generator.w.indent();
            let value = if is_all { "false" } else { "true" };
            generator.w.line(format!("{} = {};", matched, value));
            generator.w.line("break;");
            generator.w.dedent();
            generator.w.line("}");
        }
        FusedRuntime::Find { result } => {
            let FusedConsumer::Find { closure } = consumer else {
                return;
            };
            let Some(tail) = closure.body.tail.as_deref() else {
                return;
            };
            if let Some(parameter) = closure.params.first()
                && parameter.name != "_"
            {
                let ctype = generator.ctype(&parameter.ty);
                generator
                    .w
                    .line(format!("{} {} = &({});", ctype, parameter.c_name, current));
            }
            let predicate = generator.gen_owned(tail);
            generator.w.line(format!("if ({}) {{", predicate));
            generator.w.indent();
            generator
                .w
                .line(format!("({}).{} = 1;", result, types::TAG_FIELD));
            generator.w.line(format!(
                "({}).{}.{} = {};",
                result,
                types::UNION_FIELD,
                types::OPTION_SOME_MEMBER,
                current
            ));
            generator.w.line("break;");
            generator.w.dedent();
            generator.w.line("}");
        }
        FusedRuntime::Collect { result } => {
            let length = generator.fresh("len");
            generator
                .w
                .line(format!("size_t {} = ({}).len;", length, result));
            generator
                .w
                .line(format!("if ({} < ({}).cap) {{", length, result));
            generator.w.indent();
            generator
                .w
                .line(format!("({}).data[{}] = {};", result, length, current));
            generator
                .w
                .line(format!("({}).len = {} + 1;", result, length));
            generator.w.dedent();
            generator.w.line("} else {");
            generator.w.indent();
            let element_ty = Ty::Vec(Box::new(current_ty.clone()));
            let slow = ops::push_slow_fn_name(generator.names, &element_ty);
            generator.w.line(format!(
                "({}) = {}(({}), {});",
                result, slow, result, current
            ));
            generator.w.dedent();
            generator.w.line("}");
        }
    }
}

/// Store the finished consumer value into the call target.
fn fused_finish(generator: &mut Gen, runtime: &FusedRuntime, target: &str) {
    match runtime {
        FusedRuntime::None => {}
        FusedRuntime::Fold { accumulator } => {
            generator.w.line(format!("{} = {};", target, accumulator));
        }
        FusedRuntime::Count { counter } => {
            generator.w.line(format!("{} = {};", target, counter));
        }
        FusedRuntime::SumI64 { total } | FusedRuntime::SumF64 { total } => {
            generator.w.line(format!("{} = {};", target, total));
        }
        FusedRuntime::AnyAll { matched } => {
            generator.w.line(format!("{} = {};", target, matched));
        }
        FusedRuntime::Find { result } => {
            generator.w.line(format!("{} = {};", target, result));
        }
        FusedRuntime::Collect { result } => {
            generator.w.line(format!("{} = {};", target, result));
            generator.mark_moved(result);
        }
    }
}
