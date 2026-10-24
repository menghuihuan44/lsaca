//! Emission of generated helper functions: reference counted environment
//! support for closures and the per-type copy / release / debug / equality
//! helpers derived from the monomorphized type registry.

use crate::codegen::c_writer::CWriter;
use crate::codegen::ops::is_managed;
use crate::codegen::types::TypeRegistry;
use crate::hir;
use crate::types::Ty;

/// Compound types whose layout is emitted by codegen or comes from the frozen
/// runtime header. These need generated debug and equality helpers.
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

/// Environment header shared by every closure environment plus the generic
/// retain/release pair used by function values.
pub fn emit_env_types(w: &mut CWriter) {
    w.line("typedef struct LsEnvHeader {");
    w.indent();
    w.line("void (*drop)(void* self);");
    w.dedent();
    w.line("} LsEnvHeader;");
    w.blank();
    w.line("void* lsaca_env_retain(void* env);");
    w.line("void lsaca_env_release(void* env);");
    w.blank();
}

pub fn emit_env_support(w: &mut CWriter) {
    w.line("void* lsaca_env_retain(void* env) {");
    w.indent();
    w.line("if (env == NULL) {");
    w.indent();
    w.line("return NULL;");
    w.dedent();
    w.line("}");
    w.line("ls_retain(env);");
    w.line("return env;");
    w.dedent();
    w.line("}");
    w.blank();

    w.line("void lsaca_env_release(void* env) {");
    w.indent();
    w.line("LsEnvHeader* header;");
    w.line("if (env == NULL) {");
    w.indent();
    w.line("return;");
    w.dedent();
    w.line("}");
    w.line("if (ls_refcount(env) == 1) {");
    w.indent();
    w.line("header = (LsEnvHeader*)env;");
    w.line("if (header->drop != NULL) {");
    w.indent();
    w.line("void (*drop)(void*) = header->drop;");
    w.line("header->drop = NULL;");
    w.line("drop(env);");
    w.dedent();
    w.line("}");
    w.dedent();
    w.line("}");
    w.line("ls_release(env);");
    w.dedent();
    w.line("}");
    w.blank();
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
