//! Code generation tests for the v1.3 reference ABI (`&T` / `&mut T`).
//!
//! These tests build HIR programs directly instead of going through the front
//! end, so they can pin the C shapes of the reference representation while the
//! parser and type checker for references are still settling. They only
//! inspect the emitted C; compiling it is covered by the end to end suite.

use super::{CodegenOptions, generate};
use crate::hir::{
    ExternAbi, FormatSpec, HArm, HBinaryOp, HBuiltinAssoc, HBuiltinMethod, HClosure, HConst, HEnum,
    HExpr, HExprKind, HField, HFieldRef, HFn, HFormatPart, HFormatPartKind, HMacroCall, HMacroKind,
    HParam, HPat, HPatKind, HStmt, HStruct, HUnion, HVariant, Program,
};
use crate::span::Span;
use crate::types::{IntTy, SelfKind, Ty};

fn sp() -> Span {
    Span { start: 0, end: 0 }
}

fn int_ty() -> Ty {
    Ty::i64()
}

fn int_lit(value: i128) -> HExpr {
    HExpr::new(
        int_ty(),
        HExprKind::Lit(crate::hir::HLit::Int {
            value,
            int_ty: IntTy::I64,
        }),
        sp(),
    )
}

fn local(name: &str, c_name: &str, ty: Ty) -> HExpr {
    HExpr::new(
        ty,
        HExprKind::Local {
            name: name.to_string(),
            c_name: c_name.to_string(),
        },
        sp(),
    )
}

fn borrow(expr: HExpr, mutable: bool) -> HExpr {
    let inner = expr.ty.clone();
    HExpr::new(
        Ty::Ref {
            mutable,
            inner: Box::new(inner),
        },
        HExprKind::Ref {
            mutable,
            expr: Box::new(expr),
        },
        sp(),
    )
}

fn deref(expr: HExpr) -> HExpr {
    let ty = expr.ty.ref_inner().cloned().unwrap_or_else(int_ty);
    HExpr::new(
        ty,
        HExprKind::Deref {
            expr: Box::new(expr),
        },
        sp(),
    )
}

fn bind_pat(ty: Ty, c_name: &str) -> HPat {
    HPat {
        ty,
        kind: HPatKind::Bind {
            c_name: c_name.to_string(),
            name: c_name.to_string(),
        },
        span: sp(),
    }
}

fn display_arg(expr: HExpr) -> HFormatPart {
    HFormatPart {
        kind: HFormatPartKind::Arg {
            expr,
            spec: Box::new(FormatSpec::plain()),
        },
    }
}

fn println_of(parts: Vec<HFormatPart>) -> HExpr {
    HExpr::new(
        Ty::unit(),
        HExprKind::Macro(HMacroCall {
            kind: HMacroKind::Println,
            parts,
            args: Vec::new(),
        }),
        sp(),
    )
}

fn let_stmt(pat: HPat, init: HExpr) -> HStmt {
    HStmt::Let {
        pat,
        init: Some(init),
        span: sp(),
    }
}

fn block(stmts: Vec<HStmt>, tail: Option<HExpr>) -> crate::hir::HBlock {
    crate::hir::HBlock {
        stmts,
        tail: tail.map(Box::new),
        span: sp(),
    }
}

fn main_fn(body: crate::hir::HBlock) -> HFn {
    HFn {
        name: "main".to_string(),
        c_name: "lsaca_fn_main".to_string(),
        params: Vec::new(),
        ret: Ty::unit(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body,
        span: sp(),
    }
}

fn program(name: &str, structs: Vec<HStruct>, fns: Vec<HFn>, main_fn: usize) -> Program {
    program_with(name, structs, fns, Vec::new(), main_fn)
}

/// Like [program], with closure definitions available to expressions.
fn program_with(
    name: &str,
    structs: Vec<HStruct>,
    fns: Vec<HFn>,
    closures: Vec<HClosure>,
    main_fn: usize,
) -> Program {
    let mut program = Program {
        name: name.to_string(),
        structs,
        enums: Vec::<HEnum>::new(),
        unions: Vec::new(),
        fns,
        consts: Vec::<HConst>::new(),
        closures,
        main_fn,
        copy_structs: Vec::new(),
        copy_enums: Vec::new(),
        drop_structs: Vec::new(),
        drop_enums: Vec::new(),
        trait_objects: Vec::new(),
        vtables: Vec::new(),
    };
    program.classify_types(Vec::new(), Vec::new());
    program
}

fn point() -> HStruct {
    HStruct {
        name: "P".to_string(),
        c_name: "P".to_string(),
        fields: vec![HField {
            name: "x".to_string(),
            ty: int_ty(),
        }],
        span: sp(),
    }
}

fn struct_ty() -> Ty {
    Ty::Struct(0)
}

fn struct_ref() -> Ty {
    Ty::Ref {
        mutable: false,
        inner: Box::new(struct_ty()),
    }
}

fn struct_mut_ref() -> Ty {
    Ty::Ref {
        mutable: true,
        inner: Box::new(struct_ty()),
    }
}

fn field_x(base: HExpr) -> HExpr {
    HExpr::new(
        int_ty(),
        HExprKind::Field {
            base: Box::new(base),
            field: HFieldRef::Struct {
                struct_id: 0,
                index: 0,
            },
        },
        sp(),
    )
}

fn generate_program(name: &str, program: &Program) -> String {
    let options = CodegenOptions {
        program_name: name.to_string(),
        ..CodegenOptions::default()
    };
    generate(program, &options).main_c
}

/// `fn inc(x: &i64) -> i64 { *x + 1 }` called with a borrow of a local.
#[test]
fn emits_shared_reference_parameters_and_borrows() {
    let shared_i64 = Ty::Ref {
        mutable: false,
        inner: Box::new(int_ty()),
    };
    let inc = HFn {
        name: "inc".to_string(),
        c_name: "lsaca_fn_inc".to_string(),
        params: vec![HParam {
            name: "x".to_string(),
            c_name: "p0".to_string(),
            ty: shared_i64.clone(),
        }],
        ret: int_ty(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            Vec::new(),
            Some(HExpr::new(
                int_ty(),
                HExprKind::Binary {
                    op: HBinaryOp::Add,
                    lhs: Box::new(deref(local("x", "p0", shared_i64))),
                    rhs: Box::new(int_lit(1)),
                },
                sp(),
            )),
        ),
        span: sp(),
    };
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(int_ty(), "v0"), int_lit(41)),
            let_stmt(
                bind_pat(int_ty(), "v1"),
                HExpr::new(
                    int_ty(),
                    HExprKind::DirectCall {
                        fn_id: 0,
                        receiver: None,
                        args: vec![borrow(local("n", "v0", int_ty()), false)],
                    },
                    sp(),
                ),
            ),
            HStmt::Expr(println_of(vec![display_arg(local("r", "v1", int_ty()))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "ref_shared",
        &program("ref_shared", Vec::new(), vec![inc, main], 1),
    );

    // A shared reference parameter is a plain const pointer.
    assert!(
        main_c.contains("int64_t lsaca_fn_inc(const int64_t* p0);"),
        "the shared reference parameter is not emitted as a const pointer"
    );
    assert!(
        main_c.contains("(const int64_t*)&(v0);"),
        "the borrow of a local is not emitted as the address of its lvalue"
    );
    // Reading through the reference dereferences the pointer in place; the
    // reference value itself is never copied and carries no retain traffic.
    assert!(
        main_c.contains("(*(p0))"),
        "the shared reference is not dereferenced in place:\n{main_c}"
    );
    // References never receive copy/release helpers.
    assert!(!main_c.contains("lsaca_copy_Ref"));
    assert!(!main_c.contains("lsaca_release_Ref"));
    assert!(!main_c.contains("lsaca_debug_Ref"));
}

/// `impl P { fn get(&self) -> i64; fn bump(&mut self) }`.
#[test]
fn emits_reference_self_receivers() {
    let get = HFn {
        name: "P::get".to_string(),
        c_name: "lsaca_fn_P_get".to_string(),
        params: vec![HParam {
            name: "self".to_string(),
            c_name: "p0".to_string(),
            ty: struct_ref(),
        }],
        ret: int_ty(),
        self_kind: Some(SelfKind::Ref),
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        // `self` is a reference in the HIR, so field access auto-derefs it.
        body: block(Vec::new(), Some(field_x(local("self", "p0", struct_ref())))),
        span: sp(),
    };
    let bump = HFn {
        name: "P::bump".to_string(),
        c_name: "lsaca_fn_P_bump".to_string(),
        params: vec![HParam {
            name: "self".to_string(),
            c_name: "p0".to_string(),
            ty: struct_mut_ref(),
        }],
        ret: Ty::unit(),
        self_kind: Some(SelfKind::RefMut),
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            vec![HStmt::Expr(HExpr::new(
                Ty::unit(),
                HExprKind::Assign {
                    target: Box::new(field_x(local("self", "p0", struct_mut_ref()))),
                    op: None,
                    value: Box::new(int_lit(5)),
                },
                sp(),
            ))],
            None,
        ),
        span: sp(),
    };
    let call_get = || {
        HExpr::new(
            int_ty(),
            HExprKind::DirectCall {
                fn_id: 0,
                receiver: Some(Box::new(local("p", "v0", struct_ty()))),
                args: Vec::new(),
            },
            sp(),
        )
    };
    let main = main_fn(block(
        vec![
            let_stmt(
                bind_pat(struct_ty(), "v0"),
                HExpr::new(
                    struct_ty(),
                    HExprKind::StructLit {
                        struct_id: 0,
                        fields: vec![(0, int_lit(4))],
                    },
                    sp(),
                ),
            ),
            HStmt::Expr(HExpr::new(
                Ty::unit(),
                HExprKind::DirectCall {
                    fn_id: 1,
                    receiver: Some(Box::new(local("p", "v0", struct_ty()))),
                    args: Vec::new(),
                },
                sp(),
            )),
            HStmt::Expr(println_of(vec![display_arg(call_get())])),
        ],
        None,
    ));
    let main_c = generate_program(
        "ref_methods",
        &program("ref_methods", vec![point()], vec![get, bump, main], 2),
    );

    // Both reference receivers are plain pointers; the caller passes the
    // address of the receiver place and keeps ownership.
    assert!(main_c.contains("int64_t lsaca_fn_P_get(LsP* p0);"));
    assert!(main_c.contains("LsUnit lsaca_fn_P_bump(LsP* p0);"));
    assert!(
        main_c.contains("((*(p0))).f_x"),
        "unexpected receiver emission:\n{}",
        main_c
    );
    // By-value receiver convention is documented in the emitted comment block.
    assert!(main_c.contains("Receiver is passed by pointer"));
    // Calls materialise the receiver as a pointer to the place.
    assert!(main_c.contains("lsaca_fn_P_get((LsP*)&(v0))"));
    assert!(main_c.contains("lsaca_fn_P_bump((LsP*)&(v0))"));
    // Mutating through `&mut self` writes through the pointer.
    assert!(main_c.contains("((*(p0))).f_x = "));
    // The receiver is borrowed, never copied or released by the callee.
    assert!(!main_c.contains("lsaca_copy_P((*(p0)))"));
    assert!(!main_c.contains("lsaca_release_P((*(p0)))"));
}

/// A mutable scalar reference: `fn bump(v: &mut i64) { *v = *v + 1; }`.
#[test]
fn emits_mutable_reference_parameters() {
    let mut_i64 = Ty::Ref {
        mutable: true,
        inner: Box::new(int_ty()),
    };
    let bump = HFn {
        name: "bump".to_string(),
        c_name: "lsaca_fn_bump".to_string(),
        params: vec![HParam {
            name: "v".to_string(),
            c_name: "p0".to_string(),
            ty: mut_i64.clone(),
        }],
        ret: Ty::unit(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            vec![HStmt::Expr(HExpr::new(
                Ty::unit(),
                HExprKind::Assign {
                    target: Box::new(deref(local("v", "p0", mut_i64.clone()))),
                    op: None,
                    value: Box::new(HExpr::new(
                        int_ty(),
                        HExprKind::Binary {
                            op: HBinaryOp::Add,
                            lhs: Box::new(deref(local("v", "p0", mut_i64))),
                            rhs: Box::new(int_lit(1)),
                        },
                        sp(),
                    )),
                },
                sp(),
            ))],
            None,
        ),
        span: sp(),
    };
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(int_ty(), "v0"), int_lit(1)),
            HStmt::Expr(HExpr::new(
                Ty::unit(),
                HExprKind::DirectCall {
                    fn_id: 0,
                    receiver: None,
                    args: vec![borrow(local("n", "v0", int_ty()), true)],
                },
                sp(),
            )),
            HStmt::Expr(println_of(vec![display_arg(local("n", "v0", int_ty()))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "ref_mut",
        &program("ref_mut", Vec::new(), vec![bump, main], 1),
    );

    assert!(main_c.contains("LsUnit lsaca_fn_bump(int64_t* p0);"));
    // A mutable borrow keeps a mutable pointer type.
    assert!(main_c.contains("= (int64_t*)&(v0);"));
    // The write goes through the dereferenced pointer and releases no values.
    assert!(
        main_c.contains("(*(p0)) = "),
        "the write does not go through the in-place pointer:\n{main_c}"
    );
    assert!(!main_c.contains("lsaca_copy_Ref"));
    assert!(!main_c.contains("lsaca_release_Ref"));
}

/// Match ergonomics: matching a `&Option<i64>` binds references to the fields.
#[test]
fn emits_match_ergonomics_reference_bindings() {
    let optional = Ty::option(int_ty());
    let shared_opt = Ty::Ref {
        mutable: false,
        inner: Box::new(optional.clone()),
    };
    let shared_i64 = Ty::Ref {
        mutable: false,
        inner: Box::new(int_ty()),
    };
    let some_arm = HArm {
        pat: HPat {
            ty: shared_opt.clone(),
            kind: HPatKind::OptionSome(Box::new(bind_pat(shared_i64.clone(), "v1"))),
            span: sp(),
        },
        guard: None,
        body: deref(local("x", "v1", shared_i64.clone())),
    };
    let none_arm = HArm {
        pat: HPat {
            ty: shared_opt.clone(),
            kind: HPatKind::OptionNone,
            span: sp(),
        },
        guard: None,
        body: int_lit(0),
    };
    let describe = HFn {
        name: "describe".to_string(),
        c_name: "lsaca_fn_describe".to_string(),
        params: vec![HParam {
            name: "o".to_string(),
            c_name: "p0".to_string(),
            ty: shared_opt.clone(),
        }],
        ret: int_ty(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            Vec::new(),
            Some(HExpr::new(
                int_ty(),
                HExprKind::Match {
                    scrutinee: Box::new(local("o", "p0", shared_opt.clone())),
                    arms: vec![some_arm, none_arm],
                },
                sp(),
            )),
        ),
        span: sp(),
    };
    let main = main_fn(block(
        vec![
            let_stmt(
                bind_pat(optional.clone(), "v0"),
                HExpr::new(
                    optional.clone(),
                    HExprKind::OptionSome(Box::new(int_lit(7))),
                    sp(),
                ),
            ),
            HStmt::Expr(println_of(vec![display_arg(HExpr::new(
                int_ty(),
                HExprKind::DirectCall {
                    fn_id: 0,
                    receiver: None,
                    args: vec![borrow(local("v", "v0", optional), false)],
                },
                sp(),
            ))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "ref_match",
        &program("ref_match", Vec::new(), vec![describe, main], 1),
    );

    // The binding is a reference to the matched place, not a copy of it.
    assert!(main_c.contains("= (const int64_t*)(&("));
    // Tests read the tag and payload through the scrutinee pointer.
    assert!(main_c.contains(")).tag != (int32_t)1"));
    assert!(main_c.contains(")).tag != (int32_t)0"));
    assert!(main_c.contains("= (const LsOpt_i64*)&(v0);"));
    assert!(!main_c.contains("lsaca_copy_Opt_i64(LsOpt_i64"));
}
/// A reference returned from a function is a plain borrowed pointer: no
/// retain, no release, and the pointee is addressed through the argument.
#[test]
fn emits_reference_returns() {
    let vec_ty = Ty::vec(int_ty());
    let shared_vec = Ty::Ref {
        mutable: false,
        inner: Box::new(vec_ty.clone()),
    };
    let shared_i64 = Ty::Ref {
        mutable: false,
        inner: Box::new(int_ty()),
    };
    let element = HExpr::new(
        int_ty(),
        HExprKind::Index {
            base: Box::new(local("v", "p0", shared_vec.clone())),
            index: Box::new(int_lit(0)),
        },
        sp(),
    );
    let first = HFn {
        name: "first".to_string(),
        c_name: "lsaca_fn_first".to_string(),
        params: vec![HParam {
            name: "v".to_string(),
            c_name: "p0".to_string(),
            ty: shared_vec.clone(),
        }],
        ret: shared_i64.clone(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            Vec::new(),
            Some(HExpr::new(
                shared_i64.clone(),
                HExprKind::Ref {
                    mutable: false,
                    expr: Box::new(element),
                },
                sp(),
            )),
        ),
        span: sp(),
    };
    let main = main_fn(block(
        vec![
            let_stmt(
                bind_pat(vec_ty.clone(), "v0"),
                HExpr::new(
                    vec_ty.clone(),
                    HExprKind::VecLit {
                        elems: vec![int_lit(3), int_lit(4), int_lit(5)],
                    },
                    sp(),
                ),
            ),
            HStmt::Expr(println_of(vec![display_arg(deref(HExpr::new(
                shared_i64.clone(),
                HExprKind::DirectCall {
                    fn_id: 0,
                    receiver: None,
                    args: vec![borrow(local("v", "v0", vec_ty.clone()), false)],
                },
                sp(),
            )))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "ref_return",
        &program("ref_return", Vec::new(), vec![first, main], 1),
    );

    assert!(main_c.contains("const int64_t* lsaca_fn_first(const LsVec_i64* p0);"));
    // The pointee is addressed through the borrowed vector, not copied.
    assert!(main_c.contains("= (const int64_t*)&(((*(p0))).data[(size_t)("));
    assert!(!main_c.contains("lsaca_copy_Ref"));
}

/// Builtin methods read through a reference receiver instead of copying it.
#[test]
fn emits_builtin_methods_through_references() {
    let shared_string = Ty::Ref {
        mutable: false,
        inner: Box::new(Ty::string()),
    };
    let slen = HFn {
        name: "slen".to_string(),
        c_name: "lsaca_fn_slen".to_string(),
        params: vec![HParam {
            name: "s".to_string(),
            c_name: "p0".to_string(),
            ty: shared_string.clone(),
        }],
        ret: int_ty(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            Vec::new(),
            Some(HExpr::new(
                int_ty(),
                HExprKind::BuiltinMethod {
                    receiver: Box::new(local("s", "p0", shared_string.clone())),
                    method: crate::hir::HBuiltinMethod::StrLen,
                    args: Vec::new(),
                },
                sp(),
            )),
        ),
        span: sp(),
    };
    let main = main_fn(block(
        vec![
            let_stmt(
                bind_pat(Ty::string(), "v0"),
                HExpr::new(
                    Ty::string(),
                    HExprKind::Lit(crate::hir::HLit::Str("abcd".to_string())),
                    sp(),
                ),
            ),
            HStmt::Expr(println_of(vec![display_arg(HExpr::new(
                int_ty(),
                HExprKind::DirectCall {
                    fn_id: 0,
                    receiver: None,
                    args: vec![borrow(local("s", "v0", Ty::string()), false)],
                },
                sp(),
            ))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "ref_string",
        &program("ref_string", Vec::new(), vec![slen, main], 1),
    );

    // `&String` points at the string handle, so the parameter type is
    // `const LsString**` and the method reads through it without copying.
    assert!(main_c.contains("int64_t lsaca_fn_slen(const LsString** p0);"));
    assert!(main_c.contains("ls_str_len((*(p0)))"));
    assert!(!main_c.contains("lsaca_copy_Ref"));
}

// ----- helpers for the extended reference tests -----

fn str_lit(text: &str) -> HExpr {
    HExpr::new(
        Ty::string(),
        HExprKind::Lit(crate::hir::HLit::Str(text.to_string())),
        sp(),
    )
}

fn unit_expr() -> HExpr {
    HExpr::new(Ty::unit(), HExprKind::Lit(crate::hir::HLit::Unit), sp())
}

fn ref_ty(mutable: bool, inner: Ty) -> Ty {
    Ty::Ref {
        mutable,
        inner: Box::new(inner),
    }
}

fn vec_ty() -> Ty {
    Ty::vec(Ty::i64())
}

fn opt_ty() -> Ty {
    Ty::option(Ty::i64())
}

fn rc_ty() -> Ty {
    Ty::rc(Ty::i64())
}

fn direct_call(fn_id: usize, args: Vec<HExpr>, ret: Ty) -> HExpr {
    HExpr::new(
        ret,
        HExprKind::DirectCall {
            fn_id,
            receiver: None,
            args,
        },
        sp(),
    )
}

fn literal_part(text: &str) -> HFormatPart {
    HFormatPart {
        kind: HFormatPartKind::Literal(text.to_string()),
    }
}

fn index_of(base: HExpr, index: HExpr) -> HExpr {
    let ty = base.ty.payload().cloned().unwrap_or_else(int_ty);
    HExpr::new(
        ty,
        HExprKind::Index {
            base: Box::new(base),
            index: Box::new(index),
        },
        sp(),
    )
}

fn arm(pat: HPat, body: HExpr) -> HArm {
    HArm {
        pat,
        guard: None,
        body,
    }
}

/// Extracts the definition of one generated C function (signature plus body).
///
/// User functions put the opening brace on its own line while the generated
/// helpers put it on the signature line, so both layouts are accepted. The
/// search skips the forward declaration (which ends in a semicolon).
fn function_body(main_c: &str, signature: &str) -> String {
    let mut search_from = 0;
    let (definition_start, body_start) = loop {
        let at = match main_c[search_from..].find(signature) {
            Some(found) => search_from + found,
            None => panic!("missing emitted definition for {signature}"),
        };
        let after = at + signature.len();
        let tail = &main_c[after..];
        let brace = tail.find('{');
        let semicolon = tail.find(';');
        let is_definition = match (brace, semicolon) {
            (Some(brace), Some(semicolon)) => brace < semicolon,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if is_definition {
            let opening = after + brace.expect("checked above");
            break (at, opening + 1);
        }
        search_from = after;
    };
    let rest = &main_c[body_start..];
    let end = rest
        .find("\n}")
        .unwrap_or_else(|| panic!("unterminated emitted definition for {signature}"));
    main_c[definition_start..body_start + end + 2].to_string()
}

/// `fn read(pp: &&i64) -> i64 { **pp }` receives a pointer to a pointer.
#[test]
fn emits_nested_shared_references() {
    let shared_i64 = ref_ty(false, int_ty());
    let shared_shared = ref_ty(false, shared_i64.clone());
    let read = HFn {
        name: "read".to_string(),
        c_name: "lsaca_fn_read".to_string(),
        params: vec![HParam {
            name: "pp".to_string(),
            c_name: "p0".to_string(),
            ty: shared_shared.clone(),
        }],
        ret: int_ty(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            Vec::new(),
            Some(deref(deref(local("pp", "p0", shared_shared.clone())))),
        ),
        span: sp(),
    };
    let call_read = direct_call(
        0,
        vec![borrow(local("r", "v1", shared_i64.clone()), false)],
        int_ty(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(int_ty(), "v0"), int_lit(7)),
            let_stmt(
                bind_pat(shared_i64.clone(), "v1"),
                borrow(local("a", "v0", int_ty()), false),
            ),
            HStmt::Expr(println_of(vec![display_arg(call_read)])),
        ],
        None,
    ));
    let main_c = generate_program(
        "ref_nested",
        &program("ref_nested", Vec::new(), vec![read, main], 1),
    );

    // `&&i64` is `const int64_t**`: the outer pointer is taken over a reference
    // local and the function dereferences it twice, never copying the referent
    // eagerly.
    assert!(main_c.contains("int64_t lsaca_fn_read(const int64_t** p0);"));
    assert!(main_c.contains("(const int64_t**)&("));
    let body = function_body(&main_c, "int64_t lsaca_fn_read(const int64_t** p0)");
    assert!(
        body.matches("(*(").count() >= 2,
        "the nested reference is not dereferenced twice in place: {body}"
    );
    assert!(!main_c.contains("lsaca_copy_Ref"));
}

/// Borrows of struct fields and vector elements are passed as plain `&T`
/// arguments, and the place is evaluated exactly once.
#[test]
fn emits_reference_arguments_to_struct_and_vec_places() {
    let shared_i64 = ref_ty(false, int_ty());
    let take = HFn {
        name: "take".to_string(),
        c_name: "lsaca_fn_take".to_string(),
        params: vec![HParam {
            name: "x".to_string(),
            c_name: "p0".to_string(),
            ty: shared_i64.clone(),
        }],
        ret: int_ty(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            Vec::new(),
            Some(deref(local("x", "p0", shared_i64.clone()))),
        ),
        span: sp(),
    };
    let field_borrow = borrow(field_x(local("p", "v0", struct_ty())), false);
    let element_borrow = borrow(index_of(local("v", "v1", vec_ty()), int_lit(0)), false);
    let call_field = direct_call(0, vec![field_borrow], int_ty());
    let call_element = direct_call(0, vec![element_borrow], int_ty());
    let main = main_fn(block(
        vec![
            let_stmt(
                bind_pat(struct_ty(), "v0"),
                HExpr::new(
                    struct_ty(),
                    HExprKind::StructLit {
                        struct_id: 0,
                        fields: vec![(0, int_lit(3))],
                    },
                    sp(),
                ),
            ),
            let_stmt(
                bind_pat(vec_ty(), "v1"),
                HExpr::new(
                    vec_ty(),
                    HExprKind::VecLit {
                        elems: vec![int_lit(4), int_lit(5)],
                    },
                    sp(),
                ),
            ),
            HStmt::Expr(println_of(vec![
                display_arg(call_field),
                literal_part(" "),
                display_arg(call_element),
            ])),
        ],
        None,
    ));
    let main_c = generate_program(
        "ref_places",
        &program("ref_places", vec![point()], vec![take, main], 1),
    );

    assert!(main_c.contains("int64_t lsaca_fn_take(const int64_t* p0);"));
    assert!(main_c.contains("= (const int64_t*)&((v0).f_x);"));
    assert!(main_c.contains("= (const int64_t*)&((v1).data[(size_t)("));
    // The place is evaluated once: one bounds check, not one per use of the
    // temporary pointer (regression test for the double evaluation bug).
    assert_eq!(
        main_c.matches("ls_panic_bounds(").count(),
        1,
        "the vector element place is evaluated more than once"
    );
    assert_eq!(main_c.matches("= lsaca_fn_take(t").count(), 2);
    assert!(!main_c.contains("lsaca_copy_Ref"));
}

/// Comparing two references compares the referenced values, not the addresses.
#[test]
fn emits_reference_equality_comparing_referents() {
    let shared_i64 = ref_ty(false, int_ty());
    let same = HFn {
        name: "same".to_string(),
        c_name: "lsaca_fn_same".to_string(),
        params: vec![
            HParam {
                name: "r1".to_string(),
                c_name: "p0".to_string(),
                ty: shared_i64.clone(),
            },
            HParam {
                name: "r2".to_string(),
                c_name: "p1".to_string(),
                ty: shared_i64.clone(),
            },
        ],
        ret: Ty::bool_(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            Vec::new(),
            Some(HExpr::new(
                Ty::bool_(),
                HExprKind::Binary {
                    op: HBinaryOp::Eq,
                    lhs: Box::new(local("r1", "p0", shared_i64.clone())),
                    rhs: Box::new(local("r2", "p1", shared_i64.clone())),
                },
                sp(),
            )),
        ),
        span: sp(),
    };
    let first = direct_call(
        0,
        vec![
            borrow(local("a", "v0", int_ty()), false),
            borrow(local("b", "v1", int_ty()), false),
        ],
        Ty::bool_(),
    );
    let second = direct_call(
        0,
        vec![
            borrow(local("a", "v0", int_ty()), false),
            borrow(local("a", "v0", int_ty()), false),
        ],
        Ty::bool_(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(int_ty(), "v0"), int_lit(3)),
            let_stmt(bind_pat(int_ty(), "v1"), int_lit(4)),
            HStmt::Expr(println_of(vec![
                display_arg(first),
                literal_part(" "),
                display_arg(second),
            ])),
        ],
        None,
    ));
    let main_c = generate_program(
        "ref_eq",
        &program("ref_eq", Vec::new(), vec![same, main], 1),
    );

    assert!(main_c.contains("bool lsaca_fn_same(const int64_t* p0, const int64_t* p1);"));
    let body = function_body(
        &main_c,
        "bool lsaca_fn_same(const int64_t* p0, const int64_t* p1)",
    );
    // Both operands are borrowed places read through their parameter lvalues,
    // so the comparison dereferences the parameters directly instead of
    // materialising owned temporaries; pointers are never compared.
    assert!(
        body.contains("result1 = (((*(p0))) == ((*(p1))));"),
        "unexpected comparison: {body}"
    );
    assert_eq!(
        body.matches(" == ").count(),
        1,
        "unexpected comparison: {body}"
    );
    assert!(!body.contains("(*(t"), "no temporaries: {body}");
    assert!(!main_c.contains("p0 == p1"));
    assert!(!main_c.contains("lsaca_copy_Ref"));
}

/// Matching a `&mut Option<T>` binds `&mut T` and writes reach the original.
#[test]
fn emits_match_ergonomics_writes_through_mutable_bindings() {
    let mut_opt = ref_ty(true, opt_ty());
    let mut_i64 = ref_ty(true, int_ty());
    let set = HFn {
        name: "set".to_string(),
        c_name: "lsaca_fn_set_value".to_string(),
        params: vec![HParam {
            name: "o".to_string(),
            c_name: "p0".to_string(),
            ty: mut_opt.clone(),
        }],
        ret: Ty::unit(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            Vec::new(),
            Some(HExpr::new(
                Ty::unit(),
                HExprKind::Match {
                    scrutinee: Box::new(local("o", "p0", mut_opt.clone())),
                    arms: vec![
                        arm(
                            HPat {
                                ty: opt_ty(),
                                kind: HPatKind::OptionSome(Box::new(bind_pat(
                                    mut_i64.clone(),
                                    "v1",
                                ))),
                                span: sp(),
                            },
                            HExpr::new(
                                Ty::unit(),
                                HExprKind::Assign {
                                    target: Box::new(deref(local("x", "v1", mut_i64.clone()))),
                                    op: None,
                                    value: Box::new(int_lit(9)),
                                },
                                sp(),
                            ),
                        ),
                        arm(
                            HPat {
                                ty: opt_ty(),
                                kind: HPatKind::OptionNone,
                                span: sp(),
                            },
                            unit_expr(),
                        ),
                    ],
                },
                sp(),
            )),
        ),
        span: sp(),
    };
    let call_set = direct_call(
        0,
        vec![borrow(local("o", "v0", opt_ty()), true)],
        Ty::unit(),
    );
    let unwrap = HExpr::new(
        int_ty(),
        HExprKind::BuiltinMethod {
            receiver: Box::new(local("o", "v0", opt_ty())),
            method: HBuiltinMethod::OptUnwrap,
            args: Vec::new(),
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(
                bind_pat(opt_ty(), "v0"),
                HExpr::new(opt_ty(), HExprKind::OptionSome(Box::new(int_lit(1))), sp()),
            ),
            HStmt::Expr(call_set),
            HStmt::Expr(println_of(vec![display_arg(unwrap)])),
        ],
        None,
    ));
    let main_c = generate_program(
        "ref_match_mut",
        &program("ref_match_mut", Vec::new(), vec![set, main], 1),
    );

    assert!(main_c.contains("LsUnit lsaca_fn_set_value(LsOpt_i64* p0);"));
    let body = function_body(&main_c, "LsUnit lsaca_fn_set_value(LsOpt_i64* p0)");
    // The scrutinee is a pointer to the option and the tag tests read through it.
    assert_eq!(
        body.matches("tag != (int32_t)").count(),
        2,
        "unexpected match: {body}"
    );
    // The ergonomic binding is the address of the payload, not a copy.
    assert!(
        body.contains("(int64_t*)(&(((*(t"),
        "missing payload address binding: {body}"
    );
    // Writing through the binding stores through the in-place pointer.
    assert!(
        body.contains("(*(v1)) = "),
        "the mutable binding is not written through: {body}"
    );
    assert!(!main_c.contains("lsaca_copy_Opt_i64(LsOpt_i64"));
    assert!(main_c.contains("(LsOpt_i64*)&(v0);"));
}

/// `&Rc<T>` coerces to `&T` by addressing the payload cell, without refcount traffic.
#[test]
fn emits_reference_to_rc_payload_without_refcount_traffic() {
    let shared_i64 = ref_ty(false, int_ty());
    let rc_i64 = rc_ty();
    let shared_rc = ref_ty(false, rc_i64.clone());
    let take = HFn {
        name: "take".to_string(),
        c_name: "lsaca_fn_take".to_string(),
        params: vec![HParam {
            name: "x".to_string(),
            c_name: "p0".to_string(),
            ty: shared_i64.clone(),
        }],
        ret: int_ty(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            Vec::new(),
            Some(deref(local("x", "p0", shared_i64.clone()))),
        ),
        span: sp(),
    };
    // The type checker coerces `&*rc` to `&i64`: a Ref node typed `&i64` whose
    // place expression has type `&Rc<i64>`.
    let payload_borrow = HExpr::new(
        shared_i64.clone(),
        HExprKind::Ref {
            mutable: false,
            expr: Box::new(local("rc", "p0", shared_rc.clone())),
        },
        sp(),
    );
    let value = HFn {
        name: "value".to_string(),
        c_name: "lsaca_fn_value".to_string(),
        params: vec![HParam {
            name: "rc".to_string(),
            c_name: "p0".to_string(),
            ty: shared_rc.clone(),
        }],
        ret: int_ty(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            Vec::new(),
            Some(direct_call(0, vec![payload_borrow], int_ty())),
        ),
        span: sp(),
    };
    let rc_new = HExpr::new(
        rc_i64.clone(),
        HExprKind::BuiltinAssoc {
            func: HBuiltinAssoc::RcNew,
            args: vec![int_lit(3)],
            type_args: vec![int_ty()],
        },
        sp(),
    );
    let call_value = direct_call(
        1,
        vec![borrow(local("rc", "v0", rc_i64.clone()), false)],
        int_ty(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(rc_i64.clone(), "v0"), rc_new),
            HStmt::Expr(println_of(vec![display_arg(call_value)])),
        ],
        None,
    ));
    let main_c = generate_program(
        "ref_rc",
        &program("ref_rc", Vec::new(), vec![take, value, main], 2),
    );

    assert!(main_c.contains("int64_t lsaca_fn_value(const LsRc_i64** p0);"));
    let body = function_body(&main_c, "int64_t lsaca_fn_value(const LsRc_i64** p0)");
    // The borrow points at the payload cell of the shared cell.
    assert!(
        body.contains("((((*(p0)))->payload))"),
        "the Rc payload is not addressed through the reference: {body}"
    );
    // Addressing the payload is a pure pointer coercion: no retain or release.
    assert!(!body.contains("ls_retain("), "unexpected retain: {body}");
    assert!(!body.contains("ls_release("), "unexpected release: {body}");
    assert!(!body.contains("lsaca_copy_Rc"));
    assert!(main_c.contains("(const LsRc_i64**)&(v0);"));
}

/// A `&String` parameter is `const LsString**` and is dereferenced exactly once.
#[test]
fn emits_shared_string_reference_dereferencing_once() {
    let shared_string = ref_ty(false, Ty::string());
    let slen = HFn {
        name: "slen".to_string(),
        c_name: "lsaca_fn_slen".to_string(),
        params: vec![HParam {
            name: "s".to_string(),
            c_name: "p0".to_string(),
            ty: shared_string.clone(),
        }],
        ret: int_ty(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            Vec::new(),
            Some(HExpr::new(
                int_ty(),
                HExprKind::BuiltinMethod {
                    receiver: Box::new(local("s", "p0", shared_string.clone())),
                    method: HBuiltinMethod::StrLen,
                    args: Vec::new(),
                },
                sp(),
            )),
        ),
        span: sp(),
    };
    let call_slen = direct_call(
        0,
        vec![borrow(local("s", "v0", Ty::string()), false)],
        int_ty(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(Ty::string(), "v0"), str_lit("abcd")),
            HStmt::Expr(println_of(vec![display_arg(call_slen)])),
        ],
        None,
    ));
    let main_c = generate_program(
        "ref_str_once",
        &program("ref_str_once", Vec::new(), vec![slen, main], 1),
    );

    assert!(main_c.contains("int64_t lsaca_fn_slen(const LsString** p0);"));
    let body = function_body(&main_c, "int64_t lsaca_fn_slen(const LsString** p0)");
    // The handle is read through the single dereference of the borrowed pointer.
    assert_eq!(
        body.matches("(*(p0))").count(),
        1,
        "unexpected derefs: {body}"
    );
    assert!(body.contains("ls_str_len((*(p0)))"));
    assert!(!body.contains("ls_retain("));
    assert!(!main_c.contains("lsaca_copy_Ref"));
}

/// Copying a reference value binds the pointer directly instead of routing it
/// through a temporary that would then be set but never used.
#[test]
fn emits_reference_copies_without_temporaries() {
    let shared_i64 = ref_ty(false, int_ty());
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(int_ty(), "v0"), int_lit(10)),
            let_stmt(
                bind_pat(shared_i64.clone(), "v1"),
                borrow(local("value", "v0", int_ty()), false),
            ),
            let_stmt(
                bind_pat(shared_i64.clone(), "v2"),
                local("a", "v1", shared_i64.clone()),
            ),
            HStmt::Expr(println_of(vec![display_arg(deref(local(
                "b",
                "v2",
                shared_i64.clone(),
            )))])),
        ],
        None,
    ));
    let main_c = generate_program("ref_copy", &program("ref_copy", Vec::new(), vec![main], 0));
    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");

    assert!(
        body.contains("v2 = v1;"),
        "the reference copy is not direct: {body}"
    );
    assert_eq!(
        body.matches("= v1;").count(),
        1,
        "the reference copy routes through a temporary: {body}"
    );
}

/// Borrowed Rc and Box cells: fn read(cell: &Rc<i64>) -> i64 { cell.get() },
/// the Box counterpart, and fn bump(cell: &Rc<i64>) { cell.set(9); }.
///
/// A cell reached through a shared reference is a const-qualified pointer in C,
/// so the read-only payload accessors must accept const cell pointers and the
/// mutating accessor must cast the qualifier away. A discarded-qualifiers
/// regression made generated projects fail to build with -Werror.
#[test]
fn emits_const_correct_payload_accessors_for_borrowed_cells() {
    let rc = rc_ty();
    let rc_borrow = ref_ty(false, rc.clone());
    let read_rc = HFn {
        name: "read_rc".to_string(),
        c_name: "lsaca_fn_read_rc".to_string(),
        params: vec![HParam {
            name: "cell".to_string(),
            c_name: "p0".to_string(),
            ty: rc_borrow.clone(),
        }],
        ret: int_ty(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            Vec::new(),
            Some(HExpr::new(
                int_ty(),
                HExprKind::BuiltinMethod {
                    receiver: Box::new(deref(local("cell", "p0", rc_borrow.clone()))),
                    method: HBuiltinMethod::RcGet,
                    args: Vec::new(),
                },
                sp(),
            )),
        ),
        span: sp(),
    };
    let boxed = Ty::box_(int_ty());
    let box_borrow = ref_ty(false, boxed.clone());
    let read_box = HFn {
        name: "read_box".to_string(),
        c_name: "lsaca_fn_read_box".to_string(),
        params: vec![HParam {
            name: "value".to_string(),
            c_name: "p0".to_string(),
            ty: box_borrow.clone(),
        }],
        ret: int_ty(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            Vec::new(),
            Some(HExpr::new(
                int_ty(),
                HExprKind::BuiltinMethod {
                    receiver: Box::new(deref(local("value", "p0", box_borrow.clone()))),
                    method: HBuiltinMethod::BoxGet,
                    args: Vec::new(),
                },
                sp(),
            )),
        ),
        span: sp(),
    };
    let bump = HFn {
        name: "bump".to_string(),
        c_name: "lsaca_fn_bump".to_string(),
        params: vec![HParam {
            name: "cell".to_string(),
            c_name: "p0".to_string(),
            ty: rc_borrow.clone(),
        }],
        ret: Ty::unit(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            vec![HStmt::Expr(HExpr::new(
                Ty::unit(),
                HExprKind::BuiltinMethod {
                    receiver: Box::new(deref(local("cell", "p0", rc_borrow.clone()))),
                    method: HBuiltinMethod::RcSet,
                    args: vec![int_lit(9)],
                },
                sp(),
            ))],
            None,
        ),
        span: sp(),
    };
    // The entry point calls the helpers so reachability pruning keeps them.
    let rc_new = HExpr::new(
        rc_ty(),
        HExprKind::BuiltinAssoc {
            func: HBuiltinAssoc::RcNew,
            args: vec![int_lit(1)],
            type_args: Vec::new(),
        },
        sp(),
    );
    let box_new = HExpr::new(
        boxed.clone(),
        HExprKind::BuiltinAssoc {
            func: HBuiltinAssoc::BoxNew,
            args: vec![int_lit(2)],
            type_args: Vec::new(),
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(rc_ty(), "v0"), rc_new),
            let_stmt(bind_pat(boxed.clone(), "v1"), box_new),
            HStmt::Expr(direct_call(
                0,
                vec![borrow(local("cell", "v0", rc_borrow.clone()), false)],
                int_ty(),
            )),
            HStmt::Expr(direct_call(
                1,
                vec![borrow(local("value", "v1", box_borrow.clone()), false)],
                int_ty(),
            )),
            HStmt::Expr(direct_call(
                2,
                vec![borrow(local("cell", "v0", rc_borrow.clone()), false)],
                Ty::unit(),
            )),
        ],
        None,
    ));
    let main_c = generate_program(
        "ref_cells",
        &program(
            "ref_cells",
            Vec::new(),
            vec![read_rc, read_box, bump, main],
            3,
        ),
    );

    let read_rc_body = function_body(&main_c, "int64_t lsaca_fn_read_rc(const LsRc_i64** p0)");
    assert!(
        main_c.contains("int64_t lsaca_rc_get_i64(const LsRc_i64* self);"),
        "the Rc payload accessor does not accept a const cell pointer"
    );
    assert!(
        main_c.contains("int64_t lsaca_rc_get_i64(const LsRc_i64* self) {"),
        "the Rc payload accessor definition does not accept a const cell pointer"
    );
    assert!(
        read_rc_body.contains("lsaca_rc_get_i64((*("),
        "the borrowed Rc payload is not read through a dereference: {read_rc_body}"
    );
    assert!(
        main_c.contains("int64_t lsaca_box_get_i64(const LsBox_i64* self) {"),
        "the Box payload accessor definition does not accept a const cell pointer"
    );
    let bump_body = function_body(&main_c, "LsUnit lsaca_fn_bump(const LsRc_i64** p0)");
    assert!(
        bump_body.contains("lsaca_rc_set_i64(((LsRc_i64*)((*("),
        "the mutating accessor does not cast the const qualifier away: {bump_body}"
    );
}

// ----- v1.5: maps, lazy iterators, function values, recursive layout -----

fn field_of(base: HExpr, struct_id: usize, index: usize, ty: Ty) -> HExpr {
    HExpr::new(
        ty,
        HExprKind::Field {
            base: Box::new(base),
            field: HFieldRef::Struct { struct_id, index },
        },
        sp(),
    )
}

fn struct_lit(struct_id: usize, ty: Ty, fields: Vec<(usize, HExpr)>) -> HExpr {
    HExpr::new(ty, HExprKind::StructLit { struct_id, fields }, sp())
}

fn builtin_method_expr(receiver: HExpr, method: HBuiltinMethod, args: Vec<HExpr>, ty: Ty) -> HExpr {
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

fn builtin_assoc_expr(func: HBuiltinAssoc, args: Vec<HExpr>, ty: Ty) -> HExpr {
    HExpr::new(
        ty,
        HExprKind::BuiltinAssoc {
            func,
            args,
            type_args: Vec::new(),
        },
        sp(),
    )
}

fn option_some(inner: HExpr, option_ty: Ty) -> HExpr {
    HExpr::new(option_ty, HExprKind::OptionSome(Box::new(inner)), sp())
}

fn tuple_value(elems: Vec<HExpr>, ty: Ty) -> HExpr {
    HExpr::new(ty, HExprKind::Tuple(elems), sp())
}

fn call_expr(callee: HExpr, args: Vec<HExpr>, ret: Ty) -> HExpr {
    HExpr::new(
        ret,
        HExprKind::Call {
            callee: Box::new(callee),
            args,
        },
        sp(),
    )
}

fn add(lhs: HExpr, rhs: HExpr) -> HExpr {
    HExpr::new(
        int_ty(),
        HExprKind::Binary {
            op: HBinaryOp::Add,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        sp(),
    )
}

fn debug_arg(expr: HExpr) -> HFormatPart {
    HFormatPart {
        kind: HFormatPartKind::Arg {
            expr,
            spec: Box::new(FormatSpec {
                debug: true,
                ..FormatSpec::plain()
            }),
        },
    }
}

/// A fn(&String) -> i64 closure reading the length of the borrowed string.
fn read_length_closure() -> (HClosure, HExpr) {
    let string = Ty::string();
    let shared_string = ref_ty(false, string.clone());
    let closure = HClosure {
        id: 0,
        name: "closure#1".to_string(),
        c_name: "lsaca_closure_0".to_string(),
        params: vec![HParam {
            name: "value".to_string(),
            c_name: "p0".to_string(),
            ty: shared_string.clone(),
        }],
        ret: int_ty(),
        captures: Vec::new(),
        body: block(
            Vec::new(),
            Some(HExpr::new(
                int_ty(),
                HExprKind::BuiltinMethod {
                    receiver: Box::new(deref(local("value", "p0", shared_string.clone()))),
                    method: HBuiltinMethod::StrLen,
                    args: Vec::new(),
                },
                sp(),
            )),
        ),
        span: sp(),
    };
    let value_ty = Ty::Fn(vec![shared_string], Box::new(int_ty()));
    let value = HExpr::new(
        value_ty,
        HExprKind::Closure {
            closure_id: 0,
            captures: Vec::new(),
        },
        sp(),
    );
    (closure, value)
}

/// Every map operation is a thin call into the generated lsaca_map_* family,
/// key arguments are copied, moved values are cleared, and the shell returned by
/// keys/values is released after the Vec header is taken out of it.
#[test]
fn emits_hash_map_helpers_and_builtin_operations() {
    let string = Ty::string();
    let map_ty = Ty::hash_map(int_ty(), string.clone());
    let option_string = Ty::option(string.clone());
    let map_local = local("m", "v0", map_ty.clone());
    let (closure, closure_value) = read_length_closure();
    let main = main_fn(block(
        vec![
            let_stmt(
                bind_pat(map_ty.clone(), "v0"),
                builtin_assoc_expr(HBuiltinAssoc::HashMapNew, Vec::new(), map_ty.clone()),
            ),
            let_stmt(
                bind_pat(option_string.clone(), "v1"),
                builtin_method_expr(
                    map_local.clone(),
                    HBuiltinMethod::MapInsert,
                    vec![int_lit(1), str_lit("a")],
                    option_string.clone(),
                ),
            ),
            let_stmt(
                bind_pat(option_string.clone(), "v2"),
                builtin_method_expr(
                    map_local.clone(),
                    HBuiltinMethod::MapGet,
                    vec![int_lit(1)],
                    option_string.clone(),
                ),
            ),
            let_stmt(
                bind_pat(Ty::Bool, "v3"),
                builtin_method_expr(
                    map_local.clone(),
                    HBuiltinMethod::MapContainsKey,
                    vec![int_lit(2)],
                    Ty::Bool,
                ),
            ),
            let_stmt(
                bind_pat(Ty::i64(), "v4"),
                builtin_method_expr(
                    map_local.clone(),
                    HBuiltinMethod::MapLen,
                    Vec::new(),
                    int_ty(),
                ),
            ),
            let_stmt(
                bind_pat(Ty::vec(int_ty()), "v5"),
                builtin_method_expr(
                    map_local.clone(),
                    HBuiltinMethod::MapKeys,
                    Vec::new(),
                    Ty::vec(int_ty()),
                ),
            ),
            let_stmt(
                bind_pat(Ty::vec(string.clone()), "v6"),
                builtin_method_expr(
                    map_local.clone(),
                    HBuiltinMethod::MapValues,
                    Vec::new(),
                    Ty::vec(string.clone()),
                ),
            ),
            let_stmt(
                bind_pat(Ty::option(int_ty()), "v7"),
                builtin_method_expr(
                    map_local.clone(),
                    HBuiltinMethod::MapGetWith,
                    vec![int_lit(1), closure_value],
                    Ty::option(int_ty()),
                ),
            ),
            let_stmt(
                bind_pat(map_ty.clone(), "v8"),
                builtin_assoc_expr(HBuiltinAssoc::HashMapNew, Vec::new(), map_ty.clone()),
            ),
            let_stmt(
                bind_pat(Ty::Bool, "v9"),
                HExpr::new(
                    Ty::Bool,
                    HExprKind::Binary {
                        op: HBinaryOp::Eq,
                        lhs: Box::new(local("m", "v0", map_ty.clone())),
                        rhs: Box::new(local("other", "v8", map_ty.clone())),
                    },
                    sp(),
                ),
            ),
            HStmt::Expr(builtin_method_expr(
                map_local.clone(),
                HBuiltinMethod::MapClear,
                Vec::new(),
                Ty::unit(),
            )),
            HStmt::Expr(println_of(vec![
                display_arg(local("n", "v4", int_ty())),
                debug_arg(local("m", "v0", map_ty.clone())),
            ])),
        ],
        None,
    ));
    let main_c = generate_program(
        "map_basic",
        &program_with("map_basic", Vec::new(), vec![main], vec![closure], 0),
    );

    assert!(
        main_c.contains("struct LsMap_i64_String {"),
        "the map cell type is not defined"
    );
    assert!(main_c.contains("LsMapTable table;"));
    for declaration in [
        "LsMap_i64_String* lsaca_map_new_i64_String(void);",
        "void lsaca_map_drop_i64_String(void* payload);",
        "void lsaca_map_trace_i64_String(const void* payload, LsTraceVisit visit);",
        "LsMap_i64_String* lsaca_copy_Map_i64_String(const LsMap_i64_String* other);",
        "bool lsaca_map_insert_i64_String(",
        "bool lsaca_map_get_i64_String(",
        "bool lsaca_map_get_ptr_i64_String(",
        "bool lsaca_map_contains_i64_String(",
        "bool lsaca_map_remove_i64_String(",
        "int64_t lsaca_map_len_i64_String(",
        "bool lsaca_map_is_empty_i64_String(",
        "void lsaca_map_clear_i64_String(",
        "LsVec_i64* lsaca_map_keys_i64_String(",
        "LsVec_String* lsaca_map_values_i64_String(",
        "bool lsaca_map_iter_next_index_i64_String(",
        "const void* lsaca_map_key_ptr_i64_String(",
        "const void* lsaca_map_value_ptr_i64_String(",
        "void lsaca_map_debug_i64_String(const LsMap_i64_String* self, LsString** out);",
        "bool lsaca_map_eq_i64_String(const LsMap_i64_String* a, const LsMap_i64_String* b);",
    ] {
        assert!(
            main_c.contains(declaration),
            "missing map helper declaration {declaration}"
        );
    }

    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    for call in [
        "lsaca_map_new_i64_String(",
        "lsaca_map_insert_i64_String(",
        "lsaca_map_get_i64_String(",
        "lsaca_map_get_ptr_i64_String(",
        "lsaca_map_contains_i64_String(",
        "lsaca_map_len_i64_String(",
        "lsaca_map_keys_i64_String(",
        "lsaca_map_values_i64_String(",
        "lsaca_map_eq_i64_String(",
        "lsaca_map_clear_i64_String(",
        "lsaca_map_debug_i64_String(",
    ] {
        assert!(body.contains(call), "missing map operation {call}: {body}");
    }
    assert!(
        body.contains("ls_release((void*)(keys"),
        "the Vec shell returned by keys is not released: {body}"
    );
    assert!(
        body.contains("ls_release((void*)(values"),
        "the Vec shell returned by values is not released: {body}"
    );
    assert!(
        body.contains("ls_release((void*)(v0))"),
        "the owned map local is not released: {body}"
    );
}

fn iterator_struct() -> HStruct {
    // A lazy iterator owns state, so it is a managed (non-Copy) value that is
    // released exactly once when the loop finishes.
    HStruct {
        name: "Iter".to_string(),
        c_name: "Iter".to_string(),
        fields: vec![HField {
            name: "inner".to_string(),
            ty: Ty::string(),
        }],
        span: sp(),
    }
}

fn iterator_ty() -> Ty {
    Ty::Struct(0)
}

fn iterator_next_fn() -> HFn {
    let option_ty = Ty::option(int_ty());
    HFn {
        name: "next".to_string(),
        c_name: "lsaca_fn_Iter_next".to_string(),
        params: Vec::new(),
        ret: option_ty.clone(),
        self_kind: Some(SelfKind::RefMut),
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(Vec::new(), Some(option_some(int_lit(7), option_ty))),
        span: sp(),
    }
}

fn for_iter_stmt(body: crate::hir::HBlock) -> HStmt {
    HStmt::Expr(HExpr::new(
        Ty::unit(),
        HExprKind::ForIter {
            label: None,
            iter: Box::new(local("it", "v0", iterator_ty())),
            next_fn: 0,
            pat: bind_pat(int_ty(), "v1"),
            body,
        },
        sp(),
    ))
}

fn print_iteration() -> HStmt {
    HStmt::Expr(println_of(vec![display_arg(local(
        "value",
        "v1",
        int_ty(),
    ))]))
}

/// A lazy loop polls next_fn(&iterator), transfers Some payloads into the
/// pattern, clears the polled discriminant, and releases the iterator exactly
/// once after the loop.
#[test]
fn emits_for_iter_loop_and_releases_the_iterator_once() {
    let main = main_fn(block(
        vec![
            let_stmt(
                bind_pat(iterator_ty(), "v0"),
                struct_lit(0, iterator_ty(), vec![(0, str_lit("seed"))]),
            ),
            for_iter_stmt(block(vec![print_iteration()], None)),
        ],
        None,
    ));
    let main_c = generate_program(
        "for_iter",
        &program(
            "for_iter",
            vec![iterator_struct()],
            vec![iterator_next_fn(), main],
            1,
        ),
    );

    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    assert!(body.contains("for (;;) {"), "not a lazy loop: {body}");
    assert!(
        body.contains("= lsaca_fn_Iter_next(("),
        "the iterator is not polled through next_fn: {body}"
    );
    assert!(
        body.contains(").tag == 0) break;"),
        "the loop does not stop on None: {body}"
    );
    assert!(
        body.contains(").u.some"),
        "the Some payload is not bound: {body}"
    );
    assert!(
        body.contains(").tag = 0;"),
        "the transferred payload discriminant is not cleared: {body}"
    );
    assert_eq!(
        body.matches("lsaca_release_Iter(t").count(),
        1,
        "the iterator must be released exactly once on the normal path: {body}"
    );
}

/// Breaking out of a lazy loop releases the iterator once, after the loop label,
/// and never twice.
#[test]
fn releases_the_iterator_once_when_the_loop_breaks() {
    let main = main_fn(block(
        vec![
            let_stmt(
                bind_pat(iterator_ty(), "v0"),
                struct_lit(0, iterator_ty(), vec![(0, str_lit("seed"))]),
            ),
            for_iter_stmt(block(
                vec![
                    print_iteration(),
                    HStmt::Expr(HExpr::new(
                        Ty::unit(),
                        HExprKind::Break {
                            label: None,
                            value: None,
                        },
                        sp(),
                    )),
                ],
                None,
            )),
        ],
        None,
    ));
    let main_c = generate_program(
        "for_iter_break",
        &program(
            "for_iter_break",
            vec![iterator_struct()],
            vec![iterator_next_fn(), main],
            1,
        ),
    );

    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    assert!(
        body.contains("goto lsaca_for_end_"),
        "the break does not jump to the loop exit label: {body}"
    );
    assert_eq!(
        body.matches("lsaca_release_Iter(t").count(),
        1,
        "the iterator must be released exactly once after a break: {body}"
    );
}

/// An early return from the loop body releases the iterator as part of the
/// function frame cleanup; the normal exit path keeps its own cleanup, so every
/// runtime path releases exactly one handle.
#[test]
fn releases_the_iterator_once_when_the_body_returns() {
    let run = HFn {
        name: "run".to_string(),
        c_name: "lsaca_fn_run".to_string(),
        params: Vec::new(),
        ret: int_ty(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            vec![
                let_stmt(
                    bind_pat(iterator_ty(), "v0"),
                    struct_lit(0, iterator_ty(), vec![(0, str_lit("seed"))]),
                ),
                for_iter_stmt(block(
                    vec![HStmt::Expr(HExpr::new(
                        Ty::unit(),
                        HExprKind::Return(Some(Box::new(int_lit(1)))),
                        sp(),
                    ))],
                    None,
                )),
            ],
            Some(int_lit(2)),
        ),
        span: sp(),
    };
    let main = main_fn(block(
        vec![
            let_stmt(
                bind_pat(int_ty(), "v1"),
                direct_call(1, Vec::new(), int_ty()),
            ),
            HStmt::Expr(println_of(vec![display_arg(local("r", "v1", int_ty()))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "for_iter_return",
        &program(
            "for_iter_return",
            vec![iterator_struct()],
            vec![iterator_next_fn(), run, main],
            2,
        ),
    );

    let body = function_body(&main_c, "int64_t lsaca_fn_run()");
    assert!(
        body.contains("return r"),
        "the early return is missing: {body}"
    );
    // One release on the return path and one on the normal loop exit path: the
    // paths are mutually exclusive at runtime.
    assert_eq!(
        body.matches("lsaca_release_Iter(t").count(),
        2,
        "each exit path must release the iterator exactly once: {body}"
    );
}

fn callback_ty() -> Ty {
    Ty::Fn(vec![int_ty()], Box::new(int_ty()))
}

fn callback_struct() -> HStruct {
    HStruct {
        name: "Callback".to_string(),
        c_name: "Callback".to_string(),
        fields: vec![HField {
            name: "f".to_string(),
            ty: callback_ty(),
        }],
        span: sp(),
    }
}

/// A function value stored in a struct field is a managed handle: copying the
/// struct retains the environment, releasing it releases the environment, and a
/// call through the field goes directly through the field place without copying
/// or consuming the handle.
#[test]
fn emits_function_values_stored_in_struct_fields() {
    let holder_ty = Ty::Struct(0);
    let field_ty = callback_ty();
    let inc = HFn {
        name: "inc".to_string(),
        c_name: "lsaca_fn_inc".to_string(),
        params: vec![HParam {
            name: "x".to_string(),
            c_name: "p0".to_string(),
            ty: int_ty(),
        }],
        ret: int_ty(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(Vec::new(), Some(local("x", "p0", int_ty()))),
        span: sp(),
    };
    let holder = local("cb", "v0", holder_ty.clone());
    let main = main_fn(block(
        vec![
            let_stmt(
                bind_pat(holder_ty.clone(), "v0"),
                struct_lit(
                    0,
                    holder_ty.clone(),
                    vec![(
                        0,
                        HExpr::new(field_ty.clone(), HExprKind::FnRef { fn_id: 0 }, sp()),
                    )],
                ),
            ),
            let_stmt(
                bind_pat(int_ty(), "v1"),
                call_expr(
                    field_of(holder.clone(), 0, 0, field_ty.clone()),
                    vec![int_lit(2)],
                    int_ty(),
                ),
            ),
            let_stmt(
                bind_pat(int_ty(), "v2"),
                call_expr(
                    field_of(holder.clone(), 0, 0, field_ty.clone()),
                    vec![int_lit(3)],
                    int_ty(),
                ),
            ),
            HStmt::Expr(println_of(vec![display_arg(add(
                local("a", "v1", int_ty()),
                local("b", "v2", int_ty()),
            ))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "callback_field",
        &program(
            "callback_field",
            vec![callback_struct()],
            vec![inc, main],
            1,
        ),
    );

    assert!(main_c.contains("struct LsCallback {"));
    assert!(
        main_c.contains("LsFn1_i64_to_i64 f_f;"),
        "the function field does not use the function type: {main_c}"
    );
    let copy_body = function_body(&main_c, "LsCallback lsaca_copy_Callback(LsCallback v)");
    assert!(
        copy_body.contains("lsaca_copy_Fn1_i64_to_i64("),
        "copying the struct does not copy the function handle: {copy_body}"
    );
    let release_body = function_body(&main_c, "void lsaca_release_Callback(LsCallback v)");
    assert!(
        release_body.contains("lsaca_release_Fn1_i64_to_i64("),
        "releasing the struct does not release the function handle: {release_body}"
    );
    let function_copy = function_body(
        &main_c,
        "LsFn1_i64_to_i64 lsaca_copy_Fn1_i64_to_i64(LsFn1_i64_to_i64 v)",
    );
    assert!(
        function_copy.contains("lsaca_env_retain("),
        "copying a function value does not retain its environment: {function_copy}"
    );
    let function_release = function_body(
        &main_c,
        "void lsaca_release_Fn1_i64_to_i64(LsFn1_i64_to_i64 v)",
    );
    assert!(
        function_release.contains("lsaca_env_release("),
        "releasing a function value does not release its environment: {function_release}"
    );
    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    assert_eq!(
        body.matches(".fn_ptr(").count(),
        2,
        "the field is not called twice: {body}"
    );
    assert_eq!(
        body.matches("lsaca_copy_Fn1_i64_to_i64(").count(),
        0,
        "a call through the borrowed function field must not copy the handle: {body}"
    );
}

/// Aggregate equality goes through the generated lsaca_eq_* helpers, and
/// assert_eq! routes Box payloads through the same equality code.
#[test]
fn compares_aggregates_through_generated_equality() {
    let named = HStruct {
        name: "Named".to_string(),
        c_name: "Named".to_string(),
        fields: vec![HField {
            name: "label".to_string(),
            ty: Ty::string(),
        }],
        span: sp(),
    };
    let named_ty = Ty::Struct(0);
    let boxed = Ty::box_(int_ty());
    let box_a = builtin_assoc_expr(HBuiltinAssoc::BoxNew, vec![int_lit(1)], boxed.clone());
    let box_b = builtin_assoc_expr(HBuiltinAssoc::BoxNew, vec![int_lit(1)], boxed.clone());
    let main = main_fn(block(
        vec![
            let_stmt(
                bind_pat(named_ty.clone(), "v0"),
                struct_lit(0, named_ty.clone(), vec![(0, str_lit("same"))]),
            ),
            let_stmt(
                bind_pat(named_ty.clone(), "v1"),
                struct_lit(0, named_ty.clone(), vec![(0, str_lit("same"))]),
            ),
            let_stmt(
                bind_pat(Ty::Bool, "v2"),
                HExpr::new(
                    Ty::Bool,
                    HExprKind::Binary {
                        op: HBinaryOp::Eq,
                        lhs: Box::new(local("a", "v0", named_ty.clone())),
                        rhs: Box::new(local("b", "v1", named_ty.clone())),
                    },
                    sp(),
                ),
            ),
            HStmt::Expr(HExpr::new(
                Ty::unit(),
                HExprKind::Macro(HMacroCall {
                    kind: HMacroKind::AssertEq,
                    parts: Vec::new(),
                    args: vec![box_a, box_b],
                }),
                sp(),
            )),
            HStmt::Expr(println_of(vec![display_arg(local(
                "equal",
                "v2",
                Ty::Bool,
            ))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "aggregate_eq",
        &program("aggregate_eq", vec![named], vec![main], 0),
    );

    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    assert!(
        body.contains("lsaca_eq_Named("),
        "struct equality does not use the generated helper: {body}"
    );
    assert!(
        body.contains("->payload"),
        "assert_eq on a Box does not compare the payload: {body}"
    );
    let eq_body = function_body(&main_c, "bool lsaca_eq_Named(LsNamed a, LsNamed b)");
    assert!(
        eq_body.contains("ls_str_eq("),
        "the generated struct equality ignores the String field: {eq_body}"
    );
}

/// Aggregate layout and helper instantiation are recursive: nested tuples,
/// Option<scalar> fields and Box<Option<scalar>> fields are all complete
/// types, emitted in dependency order.
#[test]
fn lays_out_recursive_aggregates_in_dependency_order() {
    let scalar_pair = Ty::Tuple(vec![int_ty(), int_ty()]);
    let nested_pair = Ty::Tuple(vec![scalar_pair.clone(), int_ty()]);
    let option_ty = Ty::option(int_ty());
    let boxed_option = Ty::box_(option_ty.clone());
    let rec = HStruct {
        name: "Rec".to_string(),
        c_name: "Rec".to_string(),
        fields: vec![
            HField {
                name: "a".to_string(),
                ty: option_ty.clone(),
            },
            HField {
                name: "b".to_string(),
                ty: scalar_pair.clone(),
            },
            HField {
                name: "c".to_string(),
                ty: boxed_option.clone(),
            },
        ],
        span: sp(),
    };
    let rec_ty = Ty::Struct(0);
    let match_pattern = HPat {
        ty: nested_pair.clone(),
        kind: HPatKind::Tuple(vec![
            HPat {
                ty: scalar_pair.clone(),
                kind: HPatKind::Tuple(vec![bind_pat(int_ty(), "va"), bind_pat(int_ty(), "vb")]),
                span: sp(),
            },
            bind_pat(int_ty(), "vc"),
        ]),
        span: sp(),
    };
    let nested_match = HExpr::new(
        int_ty(),
        HExprKind::Match {
            scrutinee: Box::new(local("t", "v1", nested_pair.clone())),
            arms: vec![arm(
                match_pattern,
                add(
                    add(local("va", "va", int_ty()), local("vb", "vb", int_ty())),
                    local("vc", "vc", int_ty()),
                ),
            )],
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(
                bind_pat(rec_ty.clone(), "v0"),
                struct_lit(
                    0,
                    rec_ty.clone(),
                    vec![
                        (0, option_some(int_lit(11), option_ty.clone())),
                        (
                            1,
                            tuple_value(vec![int_lit(22), int_lit(33)], scalar_pair.clone()),
                        ),
                        (
                            2,
                            builtin_assoc_expr(
                                HBuiltinAssoc::BoxNew,
                                vec![option_some(int_lit(44), option_ty.clone())],
                                boxed_option.clone(),
                            ),
                        ),
                    ],
                ),
            ),
            let_stmt(
                bind_pat(nested_pair.clone(), "v1"),
                tuple_value(
                    vec![
                        tuple_value(vec![int_lit(1), int_lit(2)], scalar_pair.clone()),
                        int_lit(3),
                    ],
                    nested_pair.clone(),
                ),
            ),
            let_stmt(bind_pat(int_ty(), "v2"), nested_match),
            HStmt::Expr(println_of(vec![display_arg(local("sum", "v2", int_ty()))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "recursive_layout",
        &program("recursive_layout", vec![rec], vec![main], 0),
    );
    let option_at = main_c
        .find("struct LsOpt_i64 {")
        .expect("the Option<i64> type is missing");
    let pair_at = main_c
        .find("struct LsTup2_i64_i64 {")
        .expect("the tuple type is missing");
    let rec_at = main_c
        .find("struct LsRec {")
        .expect("the struct type is missing");
    assert!(
        option_at < rec_at,
        "the Option<scalar> field type is not defined before its user"
    );
    assert!(
        pair_at < rec_at,
        "the tuple field type is not defined before its user"
    );
    assert!(main_c.contains("LsOpt_i64 f_a;"));
    assert!(main_c.contains("LsTup2_i64_i64 f_b;"));
    assert!(main_c.contains("LsBox_Opt_i64* f_c;"));
    assert!(
        main_c.contains("struct LsTup2_Tup2_i64_i64_i64 {"),
        "the nested tuple type is not defined"
    );
    assert!(
        main_c.contains("LsTup2_i64_i64 f0;"),
        "the nested tuple element is not stored in its own field"
    );

    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    assert!(
        body.contains("f_a.tag = (int32_t)1;"),
        "the Option field is not written into its tagged layout: {body}"
    );
    assert!(
        body.contains(").f0).f0"),
        "the nested tuple pattern does not read through both levels: {body}"
    );
    assert!(
        body.contains("lsaca_box_new_Opt_i64("),
        "the Box<Option<scalar>> field does not use the Box helpers: {body}"
    );
}

/// Capture-less closures still need a non-empty environment struct: ISO C
/// forbids empty structures and pedantic compilers reject them (-Wpedantic).
#[test]
fn emits_non_empty_environment_structs_for_capture_less_closures() {
    let (closure, closure_value) = read_length_closure();
    let fn_ty = closure_value.ty.clone();
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(fn_ty.clone(), "v0"), closure_value),
            HStmt::Expr(println_of(vec![display_arg(local("f", "v0", fn_ty))])),
        ],
        None,
    ));
    let generated = generate_program(
        "captureless",
        &program_with("captureless", Vec::new(), vec![main], vec![closure], 0),
    );

    assert!(
        generated.contains("typedef struct LsEnv_0 {"),
        "the closure environment struct is missing: {generated}"
    );
    assert!(
        generated.contains("unsigned char _pad;"),
        "a capture-less environment struct has no members, which is invalid C"
    );
    assert!(
        !generated.contains("typedef struct LsEnv_0 {\n}"),
        "a capture-less environment struct was emitted empty"
    );
}

/// A diverging loop has unit type: it never completes with a value, so no unit
/// completion store may be written into the surrounding return slot (defect E:
/// the unit store produced a gcc type error).
#[test]
fn does_not_store_unit_completion_values_for_diverging_loops() {
    let loop_body = block(
        vec![HStmt::Expr(HExpr::new(
            Ty::unit(),
            HExprKind::Return(Some(Box::new(int_lit(5)))),
            sp(),
        ))],
        None,
    );
    let f = HFn {
        name: "f".to_string(),
        c_name: "lsaca_fn_f".to_string(),
        params: Vec::new(),
        ret: int_ty(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(
            Vec::new(),
            Some(HExpr::new(
                Ty::unit(),
                HExprKind::Loop {
                    label: None,
                    body: loop_body,
                },
                sp(),
            )),
        ),
        span: sp(),
    };
    // The entry point must call `f`, otherwise reachability pruning removes the
    // function under test.
    let main = main_fn(block(
        vec![HStmt::Expr(direct_call(0, Vec::new(), Ty::unit()))],
        None,
    ));
    let generated = generate_program(
        "diverging_loop",
        &program("diverging_loop", Vec::new(), vec![f, main], 1),
    );

    let body = function_body(&generated, "int64_t lsaca_fn_f()");
    let stores_unit_into_result = body
        .lines()
        .any(|line| line.contains("result") && line.contains("((LsUnit){0})"));
    assert!(
        !stores_unit_into_result,
        "a diverging loop must not store a unit completion value: {body}"
    );
    assert!(
        body.contains("int64_t result"),
        "the return slot must still be zero-initialised: {body}"
    );
}

/// HashMap helper signatures mention Vec<K> and Vec<V> even when the program
/// never uses those vectors, so the vector definitions must always be
/// registered (defect F: gcc reported unknown type name LsVec_i64).
#[test]
fn registers_vec_definitions_for_hash_map_helpers() {
    let map_ty = Ty::hash_map(Ty::string(), Ty::i64());
    let init = builtin_assoc_expr(HBuiltinAssoc::HashMapNew, Vec::new(), map_ty.clone());
    let main = main_fn(block(
        vec![let_stmt(bind_pat(map_ty.clone(), "v0"), init)],
        None,
    ));
    let generated = generate_program(
        "map_without_vec",
        &program("map_without_vec", Vec::new(), vec![main], 0),
    );

    assert!(
        generated.contains("struct LsVec_i64 {"),
        "LsVec_i64 must be defined for the map helpers: {generated}"
    );
    assert!(
        generated.contains("LsVec_String* lsaca_map_keys_String_i64(LsMap_String_i64* self);"),
        "the map keys helper declaration is missing"
    );
    assert!(
        generated.contains("LsVec_i64* lsaca_map_values_String_i64(LsMap_String_i64* self);"),
        "the map values helper declaration is missing"
    );
}

/// A borrowed reference never owns its referent, so cycle tracing must not
/// follow it (an emitted lsaca_trace_Ref_String call would be undefined).
#[test]
fn does_not_trace_borrowed_reference_slots() {
    let borrowed = Ty::Ref {
        mutable: false,
        inner: Box::new(Ty::string()),
    };
    let holder = HStruct {
        name: "Holder".to_string(),
        c_name: "Holder".to_string(),
        fields: vec![HField {
            name: "opt".to_string(),
            ty: Ty::option(borrowed),
        }],
        span: sp(),
    };
    let generated = generate_program(
        "trace_ref",
        &program(
            "trace_ref",
            vec![holder],
            vec![main_fn(block(Vec::new(), None))],
            0,
        ),
    );

    assert!(
        !generated.contains("lsaca_trace_Ref_String"),
        "a borrowed reference must not be traced: {generated}"
    );
}

/// A struct holding a reference field never owns the referent: the generated
/// helpers copy the pointer, skip it when releasing, read through it for Debug
/// and compare referents, so no reference counting traffic is emitted for the
/// field.
#[test]
fn emits_reference_fields_without_refcount_traffic() {
    let shared_i64 = ref_ty(false, int_ty());
    let holder = HStruct {
        name: "Holder".to_string(),
        c_name: "Holder".to_string(),
        fields: vec![
            HField {
                name: "name".to_string(),
                ty: Ty::string(),
            },
            HField {
                name: "shared".to_string(),
                ty: shared_i64,
            },
        ],
        span: sp(),
    };
    let holder_ty = Ty::Struct(0);
    let holder_lit = |name: &str| {
        struct_lit(
            0,
            holder_ty.clone(),
            vec![
                (0, str_lit(name)),
                (1, borrow(local("number", "v0", int_ty()), false)),
            ],
        )
    };
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(int_ty(), "v0"), int_lit(7)),
            let_stmt(bind_pat(holder_ty.clone(), "v1"), holder_lit("hi")),
            let_stmt(bind_pat(holder_ty.clone(), "v2"), holder_lit("hi")),
            HStmt::Expr(println_of(vec![debug_arg(local(
                "first",
                "v1",
                holder_ty.clone(),
            ))])),
            HStmt::Expr(println_of(vec![display_arg(HExpr::new(
                Ty::Bool,
                HExprKind::Binary {
                    op: HBinaryOp::Eq,
                    lhs: Box::new(local("first", "v1", holder_ty.clone())),
                    rhs: Box::new(local("second", "v2", holder_ty.clone())),
                },
                sp(),
            ))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "ref_fields",
        &program("ref_fields", vec![holder], vec![main], 0),
    );

    let copy = function_body(&main_c, "lsaca_copy_Holder");
    assert!(
        copy.contains("out.f_shared = v.f_shared;"),
        "a reference field is copied as a bare pointer: {copy}"
    );
    assert_eq!(
        copy.matches("ls_retain").count(),
        1,
        "only the owned String field is retained: {copy}"
    );

    let release = function_body(&main_c, "lsaca_release_Holder");
    assert!(
        !release.contains("f_shared"),
        "a reference field is never released: {release}"
    );
    assert!(
        release.contains("ls_release((void*)(v.f_name));"),
        "the owned String field is still released: {release}"
    );

    let debug = function_body(&main_c, "lsaca_debug_Holder");
    assert!(
        debug.contains("(*(v.f_shared))"),
        "Debug reads through the reference: {debug}"
    );
    assert!(
        !debug.contains("ls_retain"),
        "Debug does not retain a reference field: {debug}"
    );

    let eq = function_body(&main_c, "lsaca_eq_Holder");
    assert!(
        eq.contains("(*(a.f_shared))") && eq.contains("(*(b.f_shared))"),
        "equality compares the referents of reference fields: {eq}"
    );
}

/// Push_str over a borrowed format temporary appends the formatted pieces
/// straight to the string: the HIR arrives as a slice of the format expression
/// and the emitter must not build an intermediate accumulator string.
#[test]
fn fuses_a_borrowed_format_temporary_into_push_str() {
    let format_expr = HExpr::new(
        Ty::string(),
        HExprKind::Format {
            parts: vec![literal_part("value="), display_arg(int_lit(7))],
        },
        sp(),
    );
    let borrowed_str = ref_ty(false, Ty::Str);
    let argument = HExpr::new(
        borrowed_str,
        HExprKind::Slice {
            base: Box::new(format_expr),
            start: None,
            end: None,
            inclusive: false,
        },
        sp(),
    );
    let push = HExpr::new(
        Ty::unit(),
        HExprKind::BuiltinMethod {
            receiver: Box::new(local("text", "v0", Ty::string())),
            method: HBuiltinMethod::StrPushStr,
            args: vec![argument],
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(Ty::string(), "v0"), str_lit("")),
            HStmt::Expr(push),
            HStmt::Expr(println_of(vec![display_arg(local(
                "text",
                "v0",
                Ty::string(),
            ))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "format_push",
        &program("format_push", Vec::new(), vec![main], 0),
    );
    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");

    assert!(
        body.contains("ls_format_push_cstr(v0, \"value=\")"),
        "the literal piece is appended to the receiver: {body}"
    );
    assert!(
        !body.contains("ls_str_push_str("),
        "the fused path must not call push_str: {body}"
    );
    assert_eq!(
        body.matches("ls_format_begin").count(),
        1,
        "only the println accumulator is built, the pushed text is not: {body}"
    );
}

// ----- v2.2: borrowed references stored inside aggregates -----

/// Builds a program whose registry also contains user enums. The plain
/// [program] helper always emits an empty enum list, but a borrowed reference
/// may also appear as an enum payload.
fn program_with_enums(
    name: &str,
    structs: Vec<HStruct>,
    enums: Vec<HEnum>,
    fns: Vec<HFn>,
    main_fn: usize,
) -> Program {
    let mut program = Program {
        name: name.to_string(),
        structs,
        enums,
        unions: Vec::new(),
        fns,
        consts: Vec::<HConst>::new(),
        closures: Vec::new(),
        main_fn,
        copy_structs: Vec::new(),
        copy_enums: Vec::new(),
        drop_structs: Vec::new(),
        drop_enums: Vec::new(),
        trait_objects: Vec::new(),
        vtables: Vec::new(),
    };
    program.classify_types(Vec::new(), Vec::new());
    program
}

fn usize_lit(value: i128) -> HExpr {
    HExpr::new(
        Ty::Int(IntTy::Usize),
        HExprKind::Lit(crate::hir::HLit::Int {
            value,
            int_ty: IntTy::Usize,
        }),
        sp(),
    )
}

/// A string literal already specialised to `&str`: it is emitted as a static
/// fat view, not as an interned `String` handle.
fn shared_str_lit(text: &str) -> HExpr {
    HExpr::new(
        ref_ty(false, Ty::Str),
        HExprKind::Lit(crate::hir::HLit::Str(text.to_string())),
        sp(),
    )
}

/// A reference to an unsized pointee is a fat view stored by value in C, so
/// the view struct must be completely defined before any aggregate that
/// embeds it; gcc rejects an incomplete by-value field. This covers struct
/// fields, enum payloads, Option payloads and arrays of references.
#[test]
fn orders_fat_reference_views_before_aggregates_that_embed_them() {
    let shared_str = ref_ty(false, Ty::Str);
    let shared_slice = ref_ty(false, Ty::slice(int_ty()));
    let array_of_refs = Ty::array(shared_str.clone(), 2);

    let view = HStruct {
        name: "View".to_string(),
        c_name: "View".to_string(),
        fields: vec![HField {
            name: "text".to_string(),
            ty: shared_str.clone(),
        }],
        span: sp(),
    };
    let slice_view = HStruct {
        name: "SliceView".to_string(),
        c_name: "SliceView".to_string(),
        fields: vec![
            HField {
                name: "data".to_string(),
                ty: shared_slice.clone(),
            },
            HField {
                name: "len".to_string(),
                ty: Ty::Int(IntTy::Usize),
            },
        ],
        span: sp(),
    };
    let maybe = HStruct {
        name: "Maybe".to_string(),
        c_name: "Maybe".to_string(),
        fields: vec![HField {
            name: "value".to_string(),
            ty: Ty::option(shared_str.clone()),
        }],
        span: sp(),
    };
    let named = HStruct {
        name: "Named".to_string(),
        c_name: "Named".to_string(),
        fields: vec![HField {
            name: "names".to_string(),
            ty: array_of_refs.clone(),
        }],
        span: sp(),
    };
    let slot = HEnum {
        name: "Slot".to_string(),
        c_name: "Slot".to_string(),
        variants: vec![
            HVariant {
                name: "Empty".to_string(),
                tag: 0,
                fields: Vec::new(),
            },
            HVariant {
                name: "Text".to_string(),
                tag: 1,
                fields: vec![shared_str.clone()],
            },
            HVariant {
                name: "Data".to_string(),
                tag: 2,
                fields: vec![shared_slice.clone()],
            },
        ],
        span: sp(),
    };

    let array_ty = Ty::array(int_ty(), 2);
    let view_ty = Ty::Struct(0);
    let slice_view_ty = Ty::Struct(1);
    let maybe_ty = Ty::Struct(2);
    let named_ty = Ty::Struct(3);
    let slot_ty = Ty::Enum(0);

    let numbers = HExpr::new(
        array_ty.clone(),
        HExprKind::Array {
            elems: vec![int_lit(1), int_lit(2)],
        },
        sp(),
    );
    let slice_expr = HExpr::new(
        shared_slice.clone(),
        HExprKind::Slice {
            base: Box::new(local("numbers", "v0", array_ty.clone())),
            start: None,
            end: None,
            inclusive: false,
        },
        sp(),
    );
    let slot_text = HExpr::new(
        slot_ty.clone(),
        HExprKind::EnumLit {
            enum_id: 0,
            variant: 1,
            args: vec![local("text", "v1", shared_str.clone())],
        },
        sp(),
    );
    let maybe_some = struct_lit(
        2,
        maybe_ty.clone(),
        vec![(
            0,
            HExpr::new(
                Ty::option(shared_str.clone()),
                HExprKind::OptionSome(Box::new(local("text", "v1", shared_str.clone()))),
                sp(),
            ),
        )],
    );
    let named_lit = struct_lit(
        3,
        named_ty.clone(),
        vec![(
            0,
            HExpr::new(
                array_of_refs.clone(),
                HExprKind::Array {
                    elems: vec![shared_str_lit("a"), shared_str_lit("b")],
                },
                sp(),
            ),
        )],
    );
    let body = block(
        vec![
            let_stmt(bind_pat(array_ty, "v0"), numbers),
            let_stmt(bind_pat(shared_str.clone(), "v1"), shared_str_lit("hi")),
            let_stmt(
                bind_pat(view_ty.clone(), "v2"),
                struct_lit(
                    0,
                    view_ty.clone(),
                    vec![(0, local("text", "v1", shared_str.clone()))],
                ),
            ),
            let_stmt(
                bind_pat(slice_view_ty.clone(), "v3"),
                struct_lit(
                    1,
                    slice_view_ty.clone(),
                    vec![(0, slice_expr), (1, usize_lit(2))],
                ),
            ),
            let_stmt(bind_pat(slot_ty.clone(), "v4"), slot_text),
            let_stmt(bind_pat(maybe_ty.clone(), "v5"), maybe_some),
            let_stmt(bind_pat(named_ty.clone(), "v6"), named_lit),
            HStmt::Expr(println_of(vec![debug_arg(local("view", "v2", view_ty))])),
            HStmt::Expr(println_of(vec![debug_arg(local(
                "slice_view",
                "v3",
                slice_view_ty,
            ))])),
            HStmt::Expr(println_of(vec![debug_arg(local("slot", "v4", slot_ty))])),
            HStmt::Expr(println_of(vec![debug_arg(local("maybe", "v5", maybe_ty))])),
            HStmt::Expr(println_of(vec![debug_arg(local("named", "v6", named_ty))])),
        ],
        None,
    );
    let main = main_fn(body);
    let program = program_with_enums(
        "ref_aggregates",
        vec![view, slice_view, maybe, named],
        vec![slot],
        vec![main],
        0,
    );
    let main_c = generate_program("ref_aggregates", &program);

    let position = |needle: &str| {
        main_c
            .find(needle)
            .unwrap_or_else(|| panic!("missing {needle} in generated C"))
    };
    let str_view = position("struct LsStr {");
    assert!(
        str_view < position("struct LsView {"),
        "the &str view must be defined before the struct embedding it: {main_c}"
    );
    assert!(
        str_view < position("struct LsSlot {"),
        "the &str view must be defined before the enum payload using it: {main_c}"
    );
    assert!(
        str_view < position("struct LsMaybe {"),
        "the &str view must be defined before the Option payload using it: {main_c}"
    );
    assert!(
        str_view < position("struct LsNamed {"),
        "the &str view must be defined before the array of references: {main_c}"
    );
    assert!(
        position("struct LsSlice_i64 {") < position("struct LsSliceView {"),
        "the &[i64] view must be defined before the struct embedding it: {main_c}"
    );
    assert!(
        position("struct LsSlice_i64 {") < position("struct LsSlot {"),
        "the &[i64] view must be defined before the enum payload using it: {main_c}"
    );

    // The helpers read the borrowed views instead of copying them through a
    // retain, so the operand is used and no reference count traffic appears.
    let copy = function_body(&main_c, "LsView lsaca_copy_View(LsView v)");
    assert!(
        copy.contains("out.f_text = v.f_text;"),
        "the by-value view is copied bit for bit: {copy}"
    );
    assert!(
        !copy.contains("ls_retain"),
        "a borrowed view never bumps a reference count: {copy}"
    );
    let slice_copy = function_body(&main_c, "LsSliceView lsaca_copy_SliceView(LsSliceView v)");
    assert!(
        slice_copy.contains("out.f_data = v.f_data;")
            && slice_copy.contains("out.f_len = v.f_len;"),
        "the slice view and its length are copied by value: {slice_copy}"
    );
    assert!(
        main_c.contains("(v.f_text)"),
        "the debug helper reads the borrowed field: {main_c}"
    );
}

/// A value-carrying break transfers its payload into the loop slot that the
/// surrounding expression has already zeroed and declared.
#[test]
fn break_values_are_written_to_the_loop_result_slot() {
    let break_expr = HExpr::new(
        Ty::unit(),
        HExprKind::Break {
            label: None,
            value: Some(Box::new(int_lit(7))),
        },
        sp(),
    );
    let loop_expr = HExpr::new(
        int_ty(),
        HExprKind::Loop {
            label: None,
            body: block(vec![HStmt::Expr(break_expr)], None),
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(int_ty(), "v0"), loop_expr),
            HStmt::Expr(println_of(vec![display_arg(local("r", "v0", int_ty()))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "break_value",
        &program("break_value", Vec::new(), vec![main], 0),
    );
    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    let loop_at = body
        .find("for (;;)")
        .unwrap_or_else(|| panic!("missing loop in generated C: {body}"));
    let goto_at = body[loop_at..]
        .find("goto ")
        .map(|offset| loop_at + offset)
        .unwrap_or_else(|| panic!("missing break jump in generated C: {body}"));
    let segment = &body[loop_at..goto_at];
    assert!(
        segment.contains("v0 = "),
        "the break payload is not transferred into the loop slot: {segment}"
    );
    assert!(
        segment.contains('7'),
        "the break payload is not evaluated before the jump: {segment}"
    );
    assert!(
        main_c.contains("int64_t v0 ="),
        "the loop result local is not declared before the loop: {main_c}"
    );
}

/// A labelled break value must target the labelled loop's slot even when it is
/// nested inside one or more inner loops.
#[test]
fn labelled_break_values_leave_nested_loops() {
    let break_outer = HExpr::new(
        Ty::unit(),
        HExprKind::Break {
            label: Some("outer".to_string()),
            value: Some(Box::new(int_lit(9))),
        },
        sp(),
    );
    let inner_loop = HExpr::new(
        Ty::unit(),
        HExprKind::Loop {
            label: None,
            body: block(vec![HStmt::Expr(break_outer)], None),
        },
        sp(),
    );
    let outer_loop = HExpr::new(
        int_ty(),
        HExprKind::Loop {
            label: Some("outer".to_string()),
            body: block(vec![HStmt::Expr(inner_loop)], None),
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(int_ty(), "v0"), outer_loop),
            HStmt::Expr(println_of(vec![display_arg(local("r", "v0", int_ty()))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "labelled_break_value",
        &program("labelled_break_value", Vec::new(), vec![main], 0),
    );
    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    assert!(
        body.contains("((int64_t)9)")
            && body
                .lines()
                .any(|line| line.trim_start().starts_with("v0 = t")),
        "the labelled break payload is not stored in the outer loop slot: {body}"
    );
    assert_eq!(
        body.matches("for (;;)").count(),
        2,
        "a labelled break value test needs both the outer and inner loops: {body}"
    );
}

/// The value produced by an inner loop must flow through its own result slot
/// before the outer loop's break value consumes it.
#[test]
fn nested_loop_values_flow_through_inner_results() {
    let inner_break = HExpr::new(
        Ty::unit(),
        HExprKind::Break {
            label: None,
            value: Some(Box::new(int_lit(7))),
        },
        sp(),
    );
    let inner_loop = HExpr::new(
        int_ty(),
        HExprKind::Loop {
            label: None,
            body: block(vec![HStmt::Expr(inner_break)], None),
        },
        sp(),
    );
    let outer_break = HExpr::new(
        Ty::unit(),
        HExprKind::Break {
            label: None,
            value: Some(Box::new(add(local("next", "v1", int_ty()), int_lit(1)))),
        },
        sp(),
    );
    let outer_loop = HExpr::new(
        int_ty(),
        HExprKind::Loop {
            label: None,
            body: block(
                vec![
                    let_stmt(bind_pat(int_ty(), "v1"), inner_loop),
                    HStmt::Expr(outer_break),
                ],
                None,
            ),
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(int_ty(), "v0"), outer_loop),
            HStmt::Expr(println_of(vec![display_arg(local("r", "v0", int_ty()))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "nested_loop_value",
        &program("nested_loop_value", Vec::new(), vec![main], 0),
    );
    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    assert!(
        body.contains("((int64_t)7)")
            && body
                .lines()
                .any(|line| line.trim_start().starts_with("v1 = t")),
        "the inner break payload is not stored in the inner loop slot: {body}"
    );
    assert!(
        body.lines()
            .any(|line| line.trim_start().starts_with("v0 = t")),
        "the outer break does not store the combined value: {body}"
    );
    assert!(
        body.contains("ls_add_i64_checked(v1,"),
        "the outer break value does not read the inner result: {body}"
    );
}

/// An array pattern binds each element from the inline data array and never
/// emits a runtime length check: the array type already fixes the length.
#[test]
fn array_patterns_bind_elements_in_place() {
    let array_ty = Ty::array(int_ty(), 2);
    let pat = HPat {
        ty: array_ty.clone(),
        kind: HPatKind::Array(vec![bind_pat(int_ty(), "v1"), bind_pat(int_ty(), "v2")]),
        span: sp(),
    };
    let init = HExpr::new(
        array_ty.clone(),
        HExprKind::Array {
            elems: vec![int_lit(1), int_lit(2)],
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(pat, init),
            HStmt::Expr(println_of(vec![
                display_arg(local("a", "v1", int_ty())),
                display_arg(local("b", "v2", int_ty())),
            ])),
        ],
        None,
    ));
    let main_c = generate_program(
        "array_pattern",
        &program("array_pattern", Vec::new(), vec![main], 0),
    );
    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    assert!(
        body.contains(".data[0]"),
        "the first array element is not bound from data[0]: {body}"
    );
    assert!(
        body.contains(".data[1]"),
        "the second array element is not bound from data[1]: {body}"
    );
    assert!(
        !body.contains(".len"),
        "an array pattern must not emit a slice length check: {body}"
    );
}

/// An At pattern binds the whole value first and then binds the selected
/// parts from the same source.
#[test]
fn at_patterns_bind_the_whole_value_and_its_parts() {
    let array_ty = Ty::array(int_ty(), 2);
    let pat = HPat {
        ty: array_ty.clone(),
        kind: HPatKind::At {
            binding: Box::new(bind_pat(array_ty.clone(), "v1")),
            pattern: Box::new(HPat {
                ty: array_ty.clone(),
                kind: HPatKind::Array(vec![bind_pat(int_ty(), "v2"), bind_pat(int_ty(), "v3")]),
                span: sp(),
            }),
        },
        span: sp(),
    };
    let init = HExpr::new(
        array_ty.clone(),
        HExprKind::Array {
            elems: vec![int_lit(4), int_lit(5)],
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(pat, init),
            HStmt::Expr(println_of(vec![
                display_arg(local("whole", "v1", array_ty)),
                display_arg(local("a", "v2", int_ty())),
                display_arg(local("b", "v3", int_ty())),
            ])),
        ],
        None,
    ));
    let main_c = generate_program(
        "at_pattern",
        &program("at_pattern", Vec::new(), vec![main], 0),
    );
    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    assert!(
        body.contains("v1 = "),
        "the whole-value binding is not assigned before the sub-pattern: {body}"
    );
    assert!(
        body.contains(".data[0]"),
        "the sub-pattern does not read the first element: {body}"
    );
    assert!(
        body.contains(".data[1]"),
        "the sub-pattern does not read the second element: {body}"
    );
}

/// Slice patterns check the length exactly for [a, b] or as a minimum for
/// [a, .., b] and bind the suffix from the end of the view.
#[test]
fn slice_patterns_check_length_and_bind_prefix_and_suffix() {
    let array_ty = Ty::array(int_ty(), 3);
    let slice_ty = Ty::slice(int_ty());
    let shared_slice = ref_ty(false, slice_ty.clone());
    let shared_i64 = ref_ty(false, int_ty());
    let pattern = |rest: bool| HPat {
        ty: slice_ty.clone(),
        kind: HPatKind::Slice {
            prefix: vec![bind_pat(shared_i64.clone(), "v1")],
            suffix: vec![bind_pat(shared_i64.clone(), "v2")],
            rest,
        },
        span: sp(),
    };
    let wildcard = HPat {
        ty: slice_ty.clone(),
        kind: HPatKind::Wild,
        span: sp(),
    };
    let match_on = |rest: bool| {
        HExpr::new(
            int_ty(),
            HExprKind::Match {
                scrutinee: Box::new(local("s", "v3", shared_slice.clone())),
                arms: vec![
                    arm(pattern(rest), int_lit(1)),
                    arm(wildcard.clone(), int_lit(0)),
                ],
            },
            sp(),
        )
    };
    let init = HExpr::new(
        array_ty.clone(),
        HExprKind::Array {
            elems: vec![int_lit(1), int_lit(2), int_lit(3)],
        },
        sp(),
    );
    let slice_expr = HExpr::new(
        shared_slice.clone(),
        HExprKind::Slice {
            base: Box::new(local("numbers", "v0", array_ty.clone())),
            start: None,
            end: None,
            inclusive: false,
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(array_ty, "v0"), init),
            let_stmt(bind_pat(shared_slice.clone(), "v3"), slice_expr),
            HStmt::Expr(match_on(true)),
            HStmt::Expr(match_on(false)),
        ],
        None,
    ));
    let main_c = generate_program(
        "slice_pattern",
        &program("slice_pattern", Vec::new(), vec![main], 0),
    );
    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    assert!(
        body.contains(".len < (size_t)2"),
        "a rest slice pattern must check the minimum length: {body}"
    );
    assert!(
        body.contains(".len != (size_t)2"),
        "an exact slice pattern must check the exact length: {body}"
    );
    assert!(
        body.contains(".len - 1 + 0"),
        "the suffix binding must index from the end of the view: {body}"
    );
}

/// Consuming an At pattern moves the whole value into the binding exactly
/// once; the wildcard sub-pattern must not drop it a second time.
#[test]
fn at_patterns_with_a_wildcard_release_the_whole_value_once() {
    let array_ty = Ty::array(Ty::string(), 2);
    let pat = HPat {
        ty: array_ty.clone(),
        kind: HPatKind::At {
            binding: Box::new(bind_pat(array_ty.clone(), "v1")),
            pattern: Box::new(HPat {
                ty: array_ty.clone(),
                kind: HPatKind::Wild,
                span: sp(),
            }),
        },
        span: sp(),
    };
    let init = HExpr::new(
        array_ty.clone(),
        HExprKind::Array {
            elems: vec![str_lit("a"), str_lit("b")],
        },
        sp(),
    );
    let main = main_fn(block(vec![let_stmt(pat, init)], None));
    let main_c = generate_program(
        "at_pattern_consume",
        &program("at_pattern_consume", Vec::new(), vec![main], 0),
    );
    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    assert!(
        body.contains("v1 = "),
        "the whole owning array is not moved into the at-binding: {body}"
    );
    assert_eq!(
        body.matches("lsaca_release_Array_String_2(").count(),
        2,
        "the whole owning array must only be released through the binding: {body}"
    );
    assert_eq!(
        body.matches("if (v1_live) {").count(),
        2,
        "both array releases must be guarded by the binding live flag: {body}"
    );
}

/// A bracket pattern over an array reads the elements directly: arrays carry
/// no runtime length, so both the length check and the suffix index must be
/// static.
#[test]
fn slice_patterns_on_arrays_use_the_static_length() {
    let array_ty = Ty::array(int_ty(), 3);
    let pat = HPat {
        ty: array_ty.clone(),
        kind: HPatKind::Slice {
            prefix: vec![bind_pat(int_ty(), "v1")],
            suffix: vec![bind_pat(int_ty(), "v2")],
            rest: true,
        },
        span: sp(),
    };
    let init = HExpr::new(
        array_ty.clone(),
        HExprKind::Array {
            elems: vec![int_lit(1), int_lit(2), int_lit(3)],
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(pat, init),
            HStmt::Expr(println_of(vec![
                display_arg(local("a", "v1", int_ty())),
                display_arg(local("b", "v2", int_ty())),
            ])),
        ],
        None,
    ));
    let main_c = generate_program(
        "array_slice_pattern",
        &program("array_slice_pattern", Vec::new(), vec![main], 0),
    );
    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    assert!(
        body.lines()
            .any(|line| line.trim_start().starts_with("v1 = ") && line.contains(".data[0];")),
        "the prefix element is not read from data[0]: {body}"
    );
    assert!(
        body.lines()
            .any(|line| line.trim_start().starts_with("v2 = ") && line.contains(".data[2];")),
        "the suffix element is not read from the static last index: {body}"
    );
    assert!(
        !body.contains(".len"),
        "an array-slice pattern must not read a runtime length: {body}"
    );
}

/// Matching an array with a bracket pattern must only emit the element tests;
/// the static array length makes a runtime length test unnecessary.
#[test]
fn slice_patterns_on_arrays_emit_no_runtime_length_test() {
    let array_ty = Ty::array(int_ty(), 3);
    let pattern = HPat {
        ty: array_ty.clone(),
        kind: HPatKind::Slice {
            prefix: vec![bind_pat(int_ty(), "v1")],
            suffix: vec![bind_pat(int_ty(), "v2")],
            rest: false,
        },
        span: sp(),
    };
    let wildcard = HPat {
        ty: array_ty.clone(),
        kind: HPatKind::Wild,
        span: sp(),
    };
    let match_expr = HExpr::new(
        int_ty(),
        HExprKind::Match {
            scrutinee: Box::new(local("values", "v0", array_ty.clone())),
            arms: vec![arm(pattern, int_lit(1)), arm(wildcard, int_lit(0))],
        },
        sp(),
    );
    let init = HExpr::new(
        array_ty.clone(),
        HExprKind::Array {
            elems: vec![int_lit(1), int_lit(2), int_lit(3)],
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(array_ty, "v0"), init),
            let_stmt(bind_pat(int_ty(), "v3"), match_expr),
            HStmt::Expr(println_of(vec![display_arg(local("r", "v3", int_ty()))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "array_slice_match",
        &program("array_slice_match", Vec::new(), vec![main], 0),
    );
    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    assert!(
        !body.contains(".len"),
        "an array-slice match must not read a runtime length: {body}"
    );
}

/// Destructuring an owned String array must move the selected element and
/// drop every unbound element exactly once, without releasing the moved-out
/// array shell again.
#[test]
fn slice_patterns_on_arrays_move_unbound_elements_to_temporaries() {
    let array_ty = Ty::array(Ty::string(), 3);
    let pat = HPat {
        ty: array_ty.clone(),
        kind: HPatKind::Slice {
            prefix: vec![bind_pat(Ty::string(), "v1")],
            suffix: vec![bind_pat(Ty::string(), "v2")],
            rest: true,
        },
        span: sp(),
    };
    let init = HExpr::new(
        array_ty.clone(),
        HExprKind::Array {
            elems: vec![str_lit("a"), str_lit("b"), str_lit("c")],
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(pat, init),
            HStmt::Expr(println_of(vec![
                display_arg(local("head", "v1", Ty::string())),
                display_arg(local("tail", "v2", Ty::string())),
            ])),
        ],
        None,
    ));
    let main_c = generate_program(
        "array_slice_consume",
        &program("array_slice_consume", Vec::new(), vec![main], 0),
    );
    let body = function_body(&main_c, "LsUnit lsaca_fn_main()");
    assert!(
        body.lines()
            .any(|line| line.trim_start().starts_with("v1 = ") && line.contains(".data[0];")),
        "the head element is not moved into its binding: {body}"
    );
    assert!(
        body.lines()
            .any(|line| line.trim_start().starts_with("v2 = ") && line.contains(".data[2];")),
        "the tail element is not moved from the static last index: {body}"
    );
    assert!(
        body.contains(".data[1]"),
        "the unbound middle element is not moved into a temporary: {body}"
    );
    assert!(
        !body.contains("lsaca_release_Array_String_3("),
        "the moved-out array shell must not be released again: {body}"
    );
}

fn uintptr_ty() -> Ty {
    Ty::Int(IntTy::Usize)
}

fn ptr_ty(mutable: bool, inner: Ty) -> Ty {
    Ty::Ptr {
        mutable,
        inner: Box::new(inner),
    }
}

fn unit_lit() -> HExpr {
    HExpr::new(Ty::unit(), HExprKind::Lit(crate::hir::HLit::Unit), sp())
}

/// A union declaration is emitted as a plain C union with a matching forward
/// typedef, the literal is a designated initializer, and field access uses the
/// member name. Unions get no generated managed helpers.
#[test]
fn union_definitions_and_fields_use_plain_c_unions() {
    let union_ty = Ty::Union(0);
    let lit = HExpr::new(
        union_ty.clone(),
        HExprKind::UnionLit {
            union_id: 0,
            field: 0,
            value: Box::new(int_lit(7)),
        },
        sp(),
    );
    let read = HExpr::new(
        int_ty(),
        HExprKind::Field {
            base: Box::new(local("value", "v0", union_ty.clone())),
            field: HFieldRef::Union {
                union_id: 0,
                index: 0,
            },
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(union_ty.clone(), "v0"), lit),
            HStmt::Expr(println_of(vec![display_arg(read)])),
        ],
        None,
    ));
    let mut program = program("union_c", Vec::new(), vec![main], 0);
    program.unions.push(HUnion {
        name: "Value".to_string(),
        c_name: "Value".to_string(),
        fields: vec![
            HField {
                name: "int_value".to_string(),
                ty: int_ty(),
            },
            HField {
                name: "float_value".to_string(),
                ty: Ty::f64(),
            },
        ],
        span: sp(),
    });
    let main_c = generate_program("union_c", &program);

    assert!(
        main_c.contains("typedef union Value Value;"),
        "the union forward typedef is not a union typedef:\n{main_c}"
    );
    assert!(
        main_c.contains("union Value {") && main_c.contains("int64_t f_int_value;"),
        "the union definition does not declare its fields in order:\n{main_c}"
    );
    assert!(
        main_c.contains("double f_float_value;"),
        "the union float field is missing:\n{main_c}"
    );
    assert!(
        main_c.contains(".f_int_value = t1"),
        "the union literal is not a designated initializer:\n{main_c}"
    );
    assert!(
        main_c.contains("(v0).f_int_value"),
        "a union field read does not use the member name:\n{main_c}"
    );
    // A union is a Copy byte image, so it gets no managed copy/release helpers.
    for helper in ["copy", "release"] {
        assert!(
            !main_c.contains(&format!("lsaca_{helper}_Value")),
            "a union must not get a {helper} helper:\n{main_c}"
        );
    }
    // Debug, equality and tracing fallbacks exist for aggregates that contain
    // the union, even though typeck rejects using them on a bare union value.
    for helper in ["debug", "eq", "trace"] {
        assert!(
            main_c.contains(&format!("lsaca_{helper}_Value")),
            "a union needs a {helper} fallback for containing aggregates:\n{main_c}"
        );
    }
}

/// Raw pointers use const / mutable C pointer types, unit pointers use void
/// pointers, and MaybeUninit<T> has exactly the representation of T.
#[test]
fn raw_pointers_and_maybe_uninit_use_their_c_representations() {
    let const_i64 = ptr_ty(false, int_ty());
    let mut_i64 = ptr_ty(true, int_ty());
    let const_unit = ptr_ty(false, Ty::unit());
    let maybe_i64 = Ty::MaybeUninit(Box::new(int_ty()));
    let check = HFn {
        name: "check".to_string(),
        c_name: "lsaca_fn_check".to_string(),
        params: vec![
            HParam {
                name: "p0".to_string(),
                c_name: "p0".to_string(),
                ty: const_i64.clone(),
            },
            HParam {
                name: "p1".to_string(),
                c_name: "p1".to_string(),
                ty: mut_i64.clone(),
            },
            HParam {
                name: "p2".to_string(),
                c_name: "p2".to_string(),
                ty: const_unit.clone(),
            },
            HParam {
                name: "p3".to_string(),
                c_name: "p3".to_string(),
                ty: maybe_i64.clone(),
            },
        ],
        ret: Ty::unit(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(Vec::new(), Some(unit_lit())),
        span: sp(),
    };
    let cast_to_int = HExpr::new(
        uintptr_ty(),
        HExprKind::Cast {
            expr: Box::new(local("p1", "p1", mut_i64.clone())),
        },
        sp(),
    );
    let cast_to_const = HExpr::new(
        const_i64.clone(),
        HExprKind::Cast {
            expr: Box::new(local("p1", "p1", mut_i64.clone())),
        },
        sp(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(uintptr_ty(), "v0"), cast_to_int),
            let_stmt(bind_pat(const_i64.clone(), "v1"), cast_to_const),
            HStmt::Expr(direct_call(
                0,
                vec![
                    local("p0", "p0", const_i64.clone()),
                    local("p1", "p1", mut_i64.clone()),
                    local("p2", "p2", const_unit.clone()),
                    local("p3", "p3", maybe_i64.clone()),
                ],
                Ty::unit(),
            )),
            HStmt::Expr(println_of(vec![
                display_arg(local("n", "v0", uintptr_ty())),
                display_arg(local("p", "v1", const_i64.clone())),
            ])),
        ],
        None,
    ));
    let main_c = generate_program("ptr_c", &program("ptr_c", Vec::new(), vec![check, main], 1));

    assert!(
        main_c.contains(
            "LsUnit lsaca_fn_check(const int64_t* p0, int64_t* p1, const void* p2, int64_t p3);"
        ),
        "pointer and MaybeUninit parameters do not use their C representations:\n{main_c}"
    );
    assert!(
        main_c.contains("(uintptr_t)(uintptr_t)("),
        "a pointer to integer cast does not go through uintptr_t:\n{main_c}"
    );
    assert!(
        main_c.contains("(const int64_t*)(uintptr_t)("),
        "a pointer to pointer cast does not use the target pointer type:\n{main_c}"
    );
    assert!(
        !main_c.contains("struct LsMaybeUninit"),
        "MaybeUninit must not introduce a wrapper type:\n{main_c}"
    );
}

/// Extern C functions get one private prototype whose assembler label is the
/// declared symbol, and call sites use the private name. No body is emitted.
#[test]
fn extern_functions_use_private_ffi_symbols() {
    let const_u8 = ptr_ty(false, Ty::Int(IntTy::U8));
    let strlen = HFn {
        name: "strlen".to_string(),
        c_name: "strlen".to_string(),
        params: vec![HParam {
            name: "s".to_string(),
            c_name: "p0".to_string(),
            ty: const_u8.clone(),
        }],
        ret: uintptr_ty(),
        self_kind: None,
        intrinsic: false,
        extern_abi: Some(ExternAbi::C),
        is_unsafe: true,
        body: block(Vec::new(), None),
        span: sp(),
    };
    let call = direct_call(0, vec![local("s", "p0", const_u8.clone())], uintptr_ty());
    let main = main_fn(block(
        vec![
            let_stmt(bind_pat(uintptr_ty(), "v0"), call),
            HStmt::Expr(println_of(vec![display_arg(local(
                "n",
                "v0",
                uintptr_ty(),
            ))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "ffi_c",
        &program("ffi_c", Vec::new(), vec![strlen, main], 1),
    );

    assert!(
        main_c.contains("#define LSACA_FFI_ASM(name) __asm__(#name)"),
        "the FFI assembler-label macro is missing:\n{main_c}"
    );
    assert!(
        main_c.contains("extern uintptr_t lsaca_ffi_0(const uint8_t* p0) LSACA_FFI_ASM(strlen);"),
        "the extern prototype does not bind the declared symbol:\n{main_c}"
    );
    assert!(
        main_c.contains("lsaca_ffi_0("),
        "the call site does not use the private FFI name:\n{main_c}"
    );
    assert!(
        !main_c.contains("lsaca_fn_strlen"),
        "an extern function must not get a generated definition:\n{main_c}"
    );
}

/// MaybeUninit is emitted as storage of T: uninit zeroes the storage, write
/// assigns through the slot pointer, as_mut_ptr yields the storage address, and
/// assume_init copies the value out without releasing the storage.
#[test]
fn maybe_uninit_intrinsics_use_the_payload_storage() {
    let maybe_i64 = Ty::MaybeUninit(Box::new(int_ty()));
    let slot_ref = Ty::Ref {
        mutable: true,
        inner: Box::new(maybe_i64.clone()),
    };
    let mut_i64 = ptr_ty(true, int_ty());
    let intrinsic = |name: &str, params: Vec<HParam>, ret: Ty| HFn {
        name: name.to_string(),
        c_name: name.to_string(),
        params,
        ret,
        self_kind: None,
        intrinsic: true,
        extern_abi: None,
        is_unsafe: false,
        body: block(Vec::new(), None),
        span: sp(),
    };
    let slot_param = || HParam {
        name: "slot".to_string(),
        c_name: "p0".to_string(),
        ty: slot_ref.clone(),
    };
    let uninit = intrinsic("__maybe_uninit_uninit<i64>", Vec::new(), maybe_i64.clone());
    let write = intrinsic(
        "__maybe_uninit_write<i64>",
        vec![
            slot_param(),
            HParam {
                name: "value".to_string(),
                c_name: "p1".to_string(),
                ty: int_ty(),
            },
        ],
        Ty::unit(),
    );
    let as_mut_ptr = intrinsic(
        "__maybe_uninit_as_mut_ptr<i64>",
        vec![slot_param()],
        mut_i64.clone(),
    );
    let assume_init = intrinsic(
        "__maybe_uninit_assume_init<i64>",
        vec![slot_param()],
        int_ty(),
    );
    let main = main_fn(block(
        vec![
            let_stmt(
                bind_pat(maybe_i64.clone(), "v0"),
                direct_call(0, Vec::new(), maybe_i64.clone()),
            ),
            HStmt::Expr(direct_call(
                1,
                vec![
                    borrow(local("storage", "v0", maybe_i64.clone()), true),
                    int_lit(42),
                ],
                Ty::unit(),
            )),
            let_stmt(
                bind_pat(mut_i64.clone(), "v1"),
                direct_call(
                    2,
                    vec![borrow(local("storage", "v0", maybe_i64.clone()), true)],
                    mut_i64.clone(),
                ),
            ),
            let_stmt(
                bind_pat(int_ty(), "v2"),
                direct_call(
                    3,
                    vec![borrow(local("storage", "v0", maybe_i64.clone()), true)],
                    int_ty(),
                ),
            ),
            HStmt::Expr(println_of(vec![display_arg(local(
                "value",
                "v2",
                int_ty(),
            ))])),
        ],
        None,
    ));
    let main_c = generate_program(
        "maybe_uninit_c",
        &program(
            "maybe_uninit_c",
            Vec::new(),
            vec![uninit, write, as_mut_ptr, assume_init, main],
            4,
        ),
    );

    assert!(
        main_c.contains("((int64_t){0})"),
        "uninit does not zero the payload storage:\n{main_c}"
    );
    assert!(
        main_c.contains("*(t2) = t3;"),
        "write does not assign through the slot pointer:\n{main_c}"
    );
    assert!(
        main_c.contains("v1 = (int64_t*)t4;"),
        "as_mut_ptr does not return the storage address:\n{main_c}"
    );
    assert!(
        main_c.contains("v2 = *(t5);"),
        "assume_init does not copy the payload out:\n{main_c}"
    );
    assert!(
        !main_c.contains("lsaca_release_MaybeUninit"),
        "MaybeUninit storage must not be released:\n{main_c}"
    );
}

/// Constness of a pointer to a pointer qualifies the inner pointer, not the
/// scalar: *const *mut i64 is emitted as int64_t* const*.
#[test]
fn const_pointers_to_pointers_qualify_the_inner_pointer() {
    let inner = ptr_ty(true, int_ty());
    let nested = ptr_ty(false, inner.clone());
    let check = HFn {
        name: "nested".to_string(),
        c_name: "lsaca_fn_nested".to_string(),
        params: vec![HParam {
            name: "p0".to_string(),
            c_name: "p0".to_string(),
            ty: nested,
        }],
        ret: Ty::unit(),
        self_kind: None,
        intrinsic: false,
        extern_abi: None,
        is_unsafe: false,
        body: block(Vec::new(), Some(unit_lit())),
        span: sp(),
    };
    let main_c = generate_program(
        "nested_ptr_c",
        &program("nested_ptr_c", Vec::new(), vec![check], 0),
    );
    assert!(
        main_c.contains("LsUnit lsaca_fn_nested(int64_t* const* p0);"),
        "a const pointer to a mutable pointer is not qualified on the inner pointer:\n{main_c}"
    );
}
