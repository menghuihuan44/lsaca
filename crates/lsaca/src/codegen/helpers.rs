//! Emission of generated helper functions: reference counted environment
//! support for closures, the per-type copy / release / debug / equality
//! helpers and the cycle collector tracing helpers derived from the
//! monomorphized type registry.

use crate::codegen::c_writer::CWriter;
use crate::codegen::ops::is_managed;
use crate::codegen::types::TypeRegistry;
use crate::hir;
use crate::types::Ty;

/// Compound types whose layout is emitted by codegen or comes from the frozen
/// runtime header. These need generated debug, equality and tracing helpers.
pub fn compound_types(program: &hir::Program, registry: &TypeRegistry) -> Vec<Ty> {
    let _ = program;
    registry
        .order()
        .iter()
        .filter(|ty| registry.needs_definition(ty) || registry.is_frozen(ty))
        .cloned()
        .collect()
}

/// The subset of compound types that own heap resources.
pub fn managed_types(program: &hir::Program, registry: &TypeRegistry) -> Vec<Ty> {
    compound_types(program, registry)
        .into_iter()
        .filter(is_managed)
        .collect()
}

/// Retain/release pair shared by function values to keep closure environments
/// alive. Both forward to the runtime reference count, which accepts NULL.
pub fn emit_env_types(w: &mut CWriter) {
    w.line("void* lsaca_env_retain(void* env);");
    w.line("void lsaca_env_release(void* env);");
    w.blank();
}

/// Environment support is a thin wrapper around the runtime: a managed
/// allocation carries its own drop function, so releasing the environment runs
/// the generated capture releases and then frees the allocation.
pub fn emit_env_support(w: &mut CWriter) {
    w.line("void* lsaca_env_retain(void* env) {");
    w.indent();
    w.line("return ls_retain(env);");
    w.dedent();
    w.line("}");
    w.blank();

    w.line("void lsaca_env_release(void* env) {");
    w.indent();
    w.line("ls_release(env);");
    w.dedent();
    w.line("}");
    w.blank();
}

/// Declarations of the Rc cell helpers for one shared heap cell type.
fn emit_rc_decls(w: &mut CWriter, registry: &TypeRegistry, inner: &Ty) {
    let names = registry.names();
    let suffix = names.mangle(inner);
    let cell = names.tag(&Ty::rc(inner.clone()));
    let cell_ptr = format!("{}*", cell);
    let inner_ty = names.ctype(inner);
    w.line(format!(
        "{} lsaca_rc_new_{}({} value);",
        cell_ptr, suffix, inner_ty
    ));
    w.line(format!(
        "{} lsaca_rc_get_{}({} self);",
        inner_ty, suffix, cell_ptr
    ));
    w.line(format!(
        "void lsaca_rc_set_{}({} self, {} value);",
        suffix, cell_ptr, inner_ty
    ));
    w.line(format!("void lsaca_rc_drop_{}(void* payload);", suffix));
    w.line(format!(
        "void lsaca_rc_trace_{}(const void* payload, LsTraceVisit visit);",
        suffix
    ));
}

fn helper_decls(w: &mut CWriter, program: &hir::Program, registry: &TypeRegistry) {
    let names = registry.names();
    for ty in compound_types(program, registry) {
        let c_ty = names.ctype(&ty);
        let suffix = names.mangle(&ty);
        if is_managed(&ty) {
            w.line(format!("{} lsaca_copy_{}({} v);", c_ty, suffix, c_ty));
            w.line(format!("void lsaca_release_{}({} v);", suffix, c_ty));
        }
        w.line(format!(
            "LsString* lsaca_debug_{}(LsString* acc, {} v);",
            suffix, c_ty
        ));
        w.line(format!("bool lsaca_eq_{}({} a, {} b);", suffix, c_ty, c_ty));
        if let Ty::Rc(inner) = &ty {
            emit_rc_decls(w, registry, inner);
        } else {
            w.line(format!(
                "void lsaca_trace_{}(const {}* value, LsTraceVisit visit);",
                suffix, c_ty
            ));
        }
    }
}

/// Emit prototypes for every generated helper. Must come after the type
/// definitions (helpers take compound types by value).
pub fn emit_helper_prototypes(w: &mut CWriter, program: &hir::Program, registry: &TypeRegistry) {
    helper_decls(w, program, registry);
    w.blank();
}

/// Emit the definitions of every generated helper.
pub fn emit_helper_definitions(w: &mut CWriter, program: &hir::Program, registry: &TypeRegistry) {
    crate::codegen::helper_defs::emit_all(w, program, registry);
}
