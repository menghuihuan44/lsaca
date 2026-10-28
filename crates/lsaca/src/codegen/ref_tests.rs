//! Code generation tests for the v1.3 reference ABI (`&T` / `&mut T`).
//!
//! These tests build HIR programs directly instead of going through the front
//! end, so they can pin the C shapes of the reference representation while the
//! parser and type checker for references are still settling. They only
//! inspect the emitted C; compiling it is covered by the end to end suite.

use super::{CodegenOptions, generate};
use crate::hir::{
    FormatSpec, HArm, HBinaryOp, HBuiltinAssoc, HBuiltinMethod, HClosure, HConst, HEnum, HExpr,
    HExprKind, HField, HFieldRef, HFn, HFormatPart, HFormatPartKind, HMacroCall, HMacroKind,
    HParam, HPat, HPatKind, HStmt, HStruct, Program,
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
        body,
        span: sp(),
    }
}

fn program(name: &str, structs: Vec<HStruct>, fns: Vec<HFn>, main_fn: usize) -> Program {
    Program {
        name: name.to_string(),
        structs,
        enums: Vec::<HEnum>::new(),
        fns,
        consts: Vec::<HConst>::new(),
        closures: Vec::<HClosure>::new(),
        main_fn,
    }
}

fn point() -> HStruct {
    HStruct {
        name: "P".to_string(),
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
    // Reading through the reference materialises the pointer into a temporary and
    // dereferences it; no retain traffic.
    assert!(main_c.contains("= p0;"));
    assert!(main_c.contains("(*(t"));
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
    assert!(main_c.contains("= p0;"));
    assert!(main_c.contains("(*(t"));
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
fn function_body(main_c: &str, signature: &str) -> String {
    let marker = format!("{}\n{{", signature);
    let start = main_c
        .find(&marker)
        .unwrap_or_else(|| panic!("missing emitted definition for {signature}"));
    let rest = &main_c[start..];
    let end = rest
        .find("\n}")
        .unwrap_or_else(|| panic!("unterminated emitted definition for {signature}"));
    rest[..end + 2].to_string()
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
        body.matches("(*(t").count() >= 2,
        "the nested reference is not dereferenced twice: {body}"
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
    // The referents are compared through temporary dereferences, so the emitted
    // comparison dereferences both references instead of comparing the pointers.
    assert_eq!(
        body.matches("(*(t").count(),
        2,
        "unexpected comparison: {body}"
    );
    assert!(body.contains("result1 = (((*(t"));
    assert!(body.contains(" == "));
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
    // Writing through the binding stores through the pointer.
    assert!(
        body.lines()
            .any(|line| line.contains("(*(t") && line.contains(") = ")),
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
    let main = main_fn(block(Vec::new(), None));
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
