//! Whole file assembly: type definitions, generated helpers, function bodies
//! and the C entry point.
//!
//! The generated main.c is laid out in a fixed, deterministic order:
//!
//! 1. includes and the runtime header;
//! 2. the unit type and float to integer cast helpers;
//! 3. forward typedefs and topologically ordered type definitions;
//! 4. closure environment structs;
//! 5. prototypes for helpers, functions, closures and environment drop / trace functions;
//! 6. helper definitions (copy, release, debug, equality, tracing);
//! 7. environment support and per closure environment drop and trace functions;
//! 8. function value thunks, closure bodies, user function bodies;
//! 9. the C main wrapper that drives the runtime lifecycle.
//!
//! Large programs are split into main.c plus extra translation units so a C
//! compiler can compile them in parallel. In that case main.c keeps items 1, 2,
//! 4, 5, 6, the interned literal table, thunks and closures, the user entry
//! point and the C wrapper; the remaining user functions are distributed over
//! deterministic unit files. Every unit repeats the includes, the unit type,
//! the type definitions, the environment structs and the declarations, and
//! additionally declares the cast helpers, interned literal handles and thunks
//! that main.c defines.

use crate::hir::{HBlock, HFn, HParam, Program};
use crate::types::{IntTy, SelfKind, Ty};

use super::CodegenOptions;
use super::GeneratedUnit;
use super::c_writer::CWriter;
use super::emit::{BoundVar, EnvInfo, Gen};
use super::helper_defs;
use super::helpers;
use super::ops;
use super::types::{
    EMPTY_STRUCT_FIELD, FN_ENV_FIELD, FN_PTR_FIELD, OPTION_SOME_MEMBER, RESULT_ERR_MEMBER,
    RESULT_OK_MEMBER, TAG_FIELD, TypeDef, TypeRegistry, UNION_FIELD, field_cname, positional_field,
    variant_member,
};
use super::util;

/// Above this many bytes of generated user function bodies a program is split
/// into several C translation units so that a C compiler can compile them in
/// parallel. Smaller programs stay in a single main.c, byte for byte as before.
pub(crate) const SPLIT_THRESHOLD_BYTES: usize = 256 * 1024;

/// Maximum number of extra translation units in a split project.
pub(crate) const MAX_SPLIT_UNITS: usize = 8;

/// Generate the C translation units for one checked program.
///
/// The first element of the result is always main.c. The second element lists
/// the extra translation units of a split build; it is empty for the usual
/// single-translation-unit project.
pub(crate) fn generate_project(
    program: &Program,
    registry: &TypeRegistry,
    options: &CodegenOptions,
) -> (String, Vec<GeneratedUnit>) {
    let mut generator = Gen::new(program, registry, options, CWriter::new());
    generator.emit_program()
}

/// Concatenate writer sections into one C file without copying their text.
fn stitch(prefix: CWriter, sections: impl IntoIterator<Item = CWriter>) -> String {
    let mut writer = prefix;
    for section in sections {
        writer.append(section);
    }
    writer.finish()
}

impl<'a> Gen<'a> {
    /// Emit the whole project and return main.c plus the extra translation
    /// units of a split build.
    ///
    /// The file layout is described in the module documentation. Sections are
    /// generated in the same order as the single-translation-unit layout but
    /// captured into separate writers, so the unsplit output stays byte
    /// identical while a split build can distribute the function bodies.
    fn emit_program(&mut self) -> (String, Vec<GeneratedUnit>) {
        self.emit_prelude();
        self.emit_unit_type();
        let includes = self.take_w();

        self.emit_cast_helpers();
        let cast_definitions = self.take_w();

        let ordered = self.registry.topological_order(self.program);
        self.emit_forward_typedefs(&ordered);
        self.emit_type_definitions(&ordered);
        helpers::emit_env_types(&mut self.w);
        self.emit_env_structs();
        helpers::emit_helper_prototypes(&mut self.w, self.program, self.registry);
        self.emit_function_prototypes();
        let declarations = self.take_w();

        helpers::emit_helper_definitions(&mut self.w, self.program, self.registry);
        helpers::emit_env_support(&mut self.w);
        self.emit_env_drops();
        self.emit_env_traces();
        let helper_definitions = self.take_w();

        self.emit_thunks();
        self.emit_closure_fns();
        let closure_support = self.take_w();

        let function_texts = self.emit_user_fns();
        self.emit_main_wrapper();
        let wrapper = self.take_w();

        // The interned string literal table must be declared before every
        // function body that uses it, but the set of literals is only known
        // after all bodies have been generated. The table is therefore
        // emitted last and stitched in front of the bodies, which keeps the
        // unsplit output byte identical.
        let function_bytes: usize = function_texts.iter().map(String::len).sum();
        let split = function_bytes >= SPLIT_THRESHOLD_BYTES;
        self.emit_interned_literals(split);
        let literals = self.take_w();

        if !split {
            let mut out = stitch(
                includes,
                [
                    cast_definitions,
                    declarations,
                    helper_definitions,
                    literals,
                    closure_support,
                ],
            );
            out.push_str(&function_texts.concat());
            out.push_str(&wrapper.finish());
            return (out, Vec::new());
        }

        // Split build: main.c keeps the shared declarations, the helper
        // definitions, the literal table, thunks and closures, the user entry
        // point and the C wrapper. The remaining user functions are spread
        // byte balanced over a bounded number of deterministic units, in the
        // existing emission order.
        let main_fn_index = self.program.main_fn;
        let mut main_fn_text = String::new();
        let mut distributable: Vec<String> = Vec::with_capacity(function_texts.len());
        for (index, text) in function_texts.into_iter().enumerate() {
            if index == main_fn_index {
                main_fn_text = text;
            } else {
                distributable.push(text);
            }
        }
        let distributable_bytes: usize = distributable.iter().map(String::len).sum();
        let unit_count = distributable_bytes
            .div_ceil(SPLIT_THRESHOLD_BYTES)
            .clamp(1, MAX_SPLIT_UNITS);
        let mut groups: Vec<Vec<String>> = vec![Vec::new(); unit_count];
        let mut group_bytes = vec![0usize; unit_count];
        for text in distributable {
            let mut target = 0;
            for index in 1..unit_count {
                if group_bytes[index] < group_bytes[target] {
                    target = index;
                }
            }
            group_bytes[target] += text.len();
            groups[target].push(text);
        }

        // Every unit repeats the shared declarations and additionally needs
        // declarations of everything defined in main.c that it may reference:
        // the saturating cast helpers, the interned literal handles and the
        // function value thunks. The helper, closure and closure environment
        // prototypes are already part of the shared declarations.
        let mut unit_prefix = includes.clone();
        Self::emit_cast_helper_prototypes(&mut unit_prefix);
        unit_prefix.append(declarations.clone());
        self.emit_interned_literal_externs(&mut unit_prefix);
        self.emit_thunk_prototypes(&mut unit_prefix);
        let unit_prefix = unit_prefix.finish();
        let units: Vec<GeneratedUnit> = groups
            .into_iter()
            .enumerate()
            .map(|(index, texts)| GeneratedUnit {
                file_name: format!("lsaca_unit_{}.c", index + 1),
                contents: format!("{}{}", unit_prefix, texts.concat()),
            })
            .collect();

        let mut main_c = stitch(
            includes,
            [
                cast_definitions,
                declarations,
                helper_definitions,
                literals,
                closure_support,
            ],
        );
        main_c.push_str(&main_fn_text);
        main_c.push_str(&wrapper.finish());
        (main_c, units)
    }

    /// Finish the current writer and start an empty one.
    fn take_w(&mut self) -> CWriter {
        std::mem::take(&mut self.w)
    }

    fn emit_prelude(&mut self) {
        self.w
            .line("/* Generated by the lsaca compiler. Do not edit. */");
        self.w.blank();
        self.w.line("#include <stdbool.h>");
        self.w.line("#include <stddef.h>");
        self.w.line("#include <stdint.h>");
        self.w.line("#include <stdio.h>");
        self.w.line("#include <stdlib.h>");
        self.w.line("#include <string.h>");
        self.w.line("#include <math.h>");
        self.w.blank();
        self.w.line("#include \"lsaca_rt.h\"");
        self.w.blank();
    }

    fn emit_unit_type(&mut self) {
        self.w.line("typedef struct LsUnit {");
        self.w.indent();
        self.w.line("char _unit;");
        self.w.dedent();
        self.w.line("} LsUnit;");
        self.w.blank();
    }

    /// Saturating float to integer conversions used by as casts.
    ///
    /// C leaves out of range float to integer conversions undefined, while the
    /// language requires saturating conversions (NaN converts to zero).
    fn emit_cast_helpers(&mut self) {
        self.w
            .line("/* Saturating float to integer conversions. */");
        for int_ty in IntTy::ALL {
            let c_type = int_ty.c_type();
            let name = cast_helper_name(int_ty);
            self.w.line(format!("{} {}(double value) {{", c_type, name));
            self.w.indent();
            self.w
                .line(format!("if (value != value) {{ return ({})0; }}", c_type));
            if int_ty.is_signed() {
                let max = signed_max_macro(int_ty);
                let min = signed_min_macro(int_ty);
                self.w.line(format!(
                    "if (value >= (double){}) {{ return ({}){}; }}",
                    max, c_type, max
                ));
                self.w.line(format!(
                    "if (value <= (double){}) {{ return ({}){}; }}",
                    min, c_type, min
                ));
            } else {
                let max = unsigned_max_macro(int_ty);
                self.w
                    .line(format!("if (value <= 0.0) {{ return ({})0; }}", c_type));
                self.w.line(format!(
                    "if (value >= (double){}) {{ return ({}){}; }}",
                    max, c_type, max
                ));
            }
            self.w.line(format!("return ({})value;", c_type));
            self.w.dedent();
            self.w.line("}");
        }
        self.w.blank();
    }

    fn emit_forward_typedefs(&mut self, ordered: &[Ty]) {
        for ty in ordered {
            if self.registry.is_frozen(ty) {
                continue;
            }
            let c_name = self.registry.tag(ty);
            self.w
                .line(format!("typedef struct {} {};", c_name, c_name));
        }
        if !ordered.is_empty() {
            self.w.blank();
        }
    }

    fn emit_type_definitions(&mut self, ordered: &[Ty]) {
        for ty in ordered {
            self.emit_type_definition(ty);
        }
    }

    fn emit_type_definition(&mut self, ty: &Ty) {
        if self.registry.is_frozen(ty) {
            return;
        }
        let definition = match self.registry.get(ty) {
            Some(definition) => definition.clone(),
            None => return,
        };
        match definition {
            TypeDef::Scalar | TypeDef::Frozen => {}
            TypeDef::Struct(index) => self.emit_struct_definition(ty, index),
            TypeDef::Enum(index) => self.emit_enum_definition(ty, index),
            TypeDef::Vec(elem) => {
                let c_name = self.registry.tag(ty);
                let element = self.registry.ctype(&elem);
                self.w.line(format!("struct {} {{", c_name));
                self.w.indent();
                self.w.line("size_t len;");
                self.w.line("size_t cap;");
                self.w.line(format!("{}* data;", element));
                self.w.dedent();
                self.w.line("};");
                self.w.blank();
            }
            TypeDef::Option(elem) => {
                let c_name = self.registry.tag(ty);
                let payload = self.registry.ctype(&elem);
                self.w.line(format!("struct {} {{", c_name));
                self.w.indent();
                self.w.line(format!("int32_t {};", TAG_FIELD));
                self.w.line("union {");
                self.w.indent();
                self.w.line(format!("{} {};", payload, OPTION_SOME_MEMBER));
                self.w.dedent();
                self.w.line(format!("}} {};", UNION_FIELD));
                self.w.dedent();
                self.w.line("};");
                self.w.blank();
            }
            TypeDef::Result(ok, err) => {
                let c_name = self.registry.tag(ty);
                let ok_type = self.registry.ctype(&ok);
                let err_type = self.registry.ctype(&err);
                self.w.line(format!("struct {} {{", c_name));
                self.w.indent();
                self.w.line(format!("int32_t {};", TAG_FIELD));
                self.w.line("union {");
                self.w.indent();
                self.w.line(format!("{} {};", ok_type, RESULT_OK_MEMBER));
                self.w.line(format!("{} {};", err_type, RESULT_ERR_MEMBER));
                self.w.dedent();
                self.w.line(format!("}} {};", UNION_FIELD));
                self.w.dedent();
                self.w.line("};");
                self.w.blank();
            }
            TypeDef::Tuple(elems) => {
                let c_name = self.registry.tag(ty);
                self.w.line(format!("struct {} {{", c_name));
                self.w.indent();
                if elems.is_empty() {
                    self.w.line(format!("char {};", EMPTY_STRUCT_FIELD));
                } else {
                    for (index, elem) in elems.iter().enumerate() {
                        let field_type = self.registry.ctype(elem);
                        self.w
                            .line(format!("{} {};", field_type, positional_field(index)));
                    }
                }
                self.w.dedent();
                self.w.line("};");
                self.w.blank();
            }
            TypeDef::Fn(params, ret) => {
                let c_name = self.registry.tag(ty);
                let return_type = self.registry.ctype(&ret);
                let mut parameter_types = Vec::with_capacity(params.len());
                for param in &params {
                    parameter_types.push(self.registry.ctype(param));
                }
                self.w.line(format!("struct {} {{", c_name));
                self.w.indent();
                self.w.line(format!("void* {};", FN_ENV_FIELD));
                if parameter_types.is_empty() {
                    self.w
                        .line(format!("{} (*{})(void*);", return_type, FN_PTR_FIELD));
                } else {
                    self.w.line(format!(
                        "{} (*{})(void*, {});",
                        return_type,
                        FN_PTR_FIELD,
                        parameter_types.join(", ")
                    ));
                }
                self.w.dedent();
                self.w.line("};");
                self.w.blank();
            }
            TypeDef::HashMap(_, _) => {
                // The runtime owns the byte table; the generated wrapper is a
                // managed allocation so that the cycle collector can trace
                // keys and values held through the map.
                let c_name = self.registry.tag(ty);
                self.w.line(format!("struct {} {{", c_name));
                self.w.indent();
                self.w.line("LsMapTable table;");
                self.w.dedent();
                self.w.line("};");
                self.w.blank();
            }
            TypeDef::Rc(elem) | TypeDef::Box(elem) => {
                let c_name = self.registry.tag(ty);
                let payload = self.registry.ctype(&elem);
                self.w.line(format!("struct {} {{", c_name));
                self.w.indent();
                self.w.line(format!("{} payload;", payload));
                self.w.dedent();
                self.w.line("};");
                self.w.blank();
            }
        }
    }

    fn emit_struct_definition(&mut self, ty: &Ty, struct_id: usize) {
        let item = match self.program.structs.get(struct_id) {
            Some(item) => item.clone(),
            None => return,
        };
        let c_name = self.registry.tag(ty);
        self.w.line(format!("struct {} {{", c_name));
        self.w.indent();
        if item.fields.is_empty() {
            self.w.line(format!("char {};", EMPTY_STRUCT_FIELD));
        } else {
            for field in &item.fields {
                let field_type = self.registry.ctype(&field.ty);
                let field_name = field_cname(&field.name);
                self.w.line(format!("{} {};", field_type, field_name));
            }
        }
        self.w.dedent();
        self.w.line("};");
        self.w.blank();
    }

    fn emit_enum_definition(&mut self, ty: &Ty, enum_id: usize) {
        let item = match self.program.enums.get(enum_id) {
            Some(item) => item.clone(),
            None => return,
        };
        let c_name = self.registry.tag(ty);
        self.w.line(format!("struct {} {{", c_name));
        self.w.indent();
        self.w.line(format!("int32_t {};", TAG_FIELD));
        let payload_variants: Vec<_> = item
            .variants
            .iter()
            .filter(|variant| !variant.fields.is_empty())
            .cloned()
            .collect();
        if !payload_variants.is_empty() {
            self.w.line("union {");
            self.w.indent();
            for variant in &payload_variants {
                self.w.line("struct {");
                self.w.indent();
                for (index, field_type) in variant.fields.iter().enumerate() {
                    let field_type = self.registry.ctype(field_type);
                    self.w
                        .line(format!("{} {};", field_type, positional_field(index)));
                }
                self.w.dedent();
                self.w
                    .line(format!("}} {};", variant_member(&variant.name)));
            }
            self.w.dedent();
            self.w.line(format!("}} {};", UNION_FIELD));
        }
        self.w.dedent();
        self.w.line("};");
        self.w.blank();
    }

    fn emit_env_structs(&mut self) {
        let closures = self.program.closures.clone();
        for closure in &closures {
            self.w
                .line(format!("typedef struct LsEnv_{} {{", closure.id));
            self.w.indent();
            for (index, capture) in closure.captures.iter().enumerate() {
                let capture_type = self.registry.ctype(&capture.ty);
                self.w.line(format!("{} c{};", capture_type, index));
            }
            if closure.captures.is_empty() {
                // ISO C requires a struct to have at least one member; an
                // environment holding no captures still needs a stable size.
                self.w.line("unsigned char _pad;");
            }
            self.w.dedent();
            self.w.line(format!("}} LsEnv_{};", closure.id));
            self.w.blank();
        }
    }

    fn emit_env_drops(&mut self) {
        let closures = self.program.closures.clone();
        for closure in &closures {
            let c_type = format!("LsEnv_{}", closure.id);
            self.w
                .line(format!("void lsaca_env_drop_{}(void* self) {{", closure.id));
            self.w.indent();
            let env_name = self.fresh("env");
            self.w
                .line(format!("{}* {} = ({}*)self;", c_type, env_name, c_type));
            let mut releases = Vec::new();
            for (index, capture) in closure.captures.iter().enumerate() {
                let field = format!("{}->c{}", env_name, index);
                if let Some(statement) = ops::release_stmt(self.names, &capture.ty, &field) {
                    releases.push(statement);
                }
            }
            if releases.is_empty() {
                self.w.line(format!("(void){};", env_name));
            } else {
                for statement in releases {
                    self.w.line(statement);
                }
            }
            self.w.dedent();
            self.w.line("}");
            self.w.blank();
        }
    }

    fn emit_env_traces(&mut self) {
        let closures = self.program.closures.clone();
        for closure in &closures {
            let c_type = format!("LsEnv_{}", closure.id);
            self.w.line(format!(
                "void lsaca_trace_env_{}(const void* payload, LsTraceVisit visit) {{",
                closure.id
            ));
            self.w.indent();
            let env_name = self.fresh("env");
            self.w.line(format!(
                "const {}* {} = (const {}*)payload;",
                c_type, env_name, c_type
            ));
            self.w.line(format!("(void){};", env_name));
            for (index, capture) in closure.captures.iter().enumerate() {
                let field = format!("{}->c{}", env_name, index);
                helper_defs::emit_trace_slot(&mut self.w, self.names, &capture.ty, &field);
            }
            self.w.line("(void)visit;");
            self.w.dedent();
            self.w.line("}");
            self.w.blank();
        }
    }

    fn emit_function_prototypes(&mut self) {
        let fns = self.program.fns.clone();
        for function in &fns {
            let return_type = self.registry.ctype(&function.ret);
            let parameters = self.fn_param_declarations(&function.params, function.self_kind);
            self.w.line(format!(
                "{} {}({});",
                return_type,
                function.c_name,
                parameters.join(", ")
            ));
        }
        let closures = self.program.closures.clone();
        for closure in &closures {
            let return_type = self.registry.ctype(&closure.ret);
            let parameters = self.fn_param_declarations(&closure.params, None);
            self.w.line(format!(
                "{} lsaca_closure_{}(void* env{});",
                return_type,
                closure.id,
                comma_prefixed(&parameters)
            ));
            self.w
                .line(format!("void lsaca_env_drop_{}(void* self);", closure.id));
            self.w.line(format!(
                "void lsaca_trace_env_{}(const void* payload, LsTraceVisit visit);",
                closure.id
            ));
        }
        self.w.blank();
    }

    fn fn_param_declarations(&self, params: &[HParam], self_kind: Option<SelfKind>) -> Vec<String> {
        let mut declarations = Vec::with_capacity(params.len());
        for (index, param) in params.iter().enumerate() {
            let mut parameter_type = self.registry.ctype(&param.ty);
            if index == 0 {
                match self_kind {
                    Some(SelfKind::Ref) | Some(SelfKind::RefMut) => {
                        // The HIR signature stores the receiver as a shared or
                        // mutable reference, but the C calling convention
                        // passes a plain pointer to the impl type.
                        parameter_type = match param.ty.ref_inner() {
                            Some(inner) => format!("{}*", self.registry.ctype(inner)),
                            None => format!("{}*", parameter_type),
                        };
                    }
                    _ => {}
                }
            }
            declarations.push(format!("{} {}", parameter_type, param.c_name));
        }
        declarations
    }

    fn emit_thunks(&mut self) {
        let fns = self.program.fns.clone();
        for (fn_id, function) in fns.iter().enumerate() {
            if function.self_kind.is_some() {
                continue;
            }
            let return_type = self.registry.ctype(&function.ret);
            let parameters = self.fn_param_declarations(&function.params, None);
            self.w.line(format!(
                "{} lsaca_thunk_{}(void* env{}) {{",
                return_type,
                fn_id,
                comma_prefixed(&parameters)
            ));
            self.w.indent();
            self.w.line("(void)env;");
            let arguments = function
                .params
                .iter()
                .map(|param| param.c_name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            self.w
                .line(format!("return {}({});", function.c_name, arguments));
            self.w.dedent();
            self.w.line("}");
            self.w.blank();
        }
    }

    fn emit_closure_fns(&mut self) {
        let closures = self.program.closures.clone();
        for closure in &closures {
            let return_type = self.registry.ctype(&closure.ret);
            let parameters = self.fn_param_declarations(&closure.params, None);
            self.w.line(format!(
                "{} lsaca_closure_{}(void* env{})",
                return_type,
                closure.id,
                comma_prefixed(&parameters)
            ));
            let env = EnvInfo {
                c_type: format!("LsEnv_{}", closure.id),
                var: String::from("env"),
            };
            self.emit_function_body(
                &closure.params,
                None,
                &closure.ret,
                &closure.body,
                Some(env),
            );
            self.w.blank();
        }
    }

    /// Emit every user function definition, capturing each into its own
    /// buffer so a split build can distribute them without regenerating.
    fn emit_user_fns(&mut self) -> Vec<String> {
        let fns = self.program.fns.clone();
        let mut texts = Vec::with_capacity(fns.len());
        for function in &fns {
            // The buffer must be empty between functions; each function body is
            // generated into a fresh writer and captured afterwards.
            self.w = CWriter::new();
            self.emit_user_fn(function);
            texts.push(self.take_w().finish());
        }
        texts
    }

    fn emit_user_fn(&mut self, function: &HFn) {
        match function.self_kind {
            Some(SelfKind::Value) => {
                self.w
                    .line("/* Receiver is passed by value: the caller transfers one owned");
                self.w
                    .line("   reference and this function releases it on every exit path. */");
            }
            Some(SelfKind::Ref) | Some(SelfKind::RefMut) => {
                self.w
                    .line("/* Receiver is passed by pointer and stays owned by the caller. */");
            }
            None => {}
        }
        let return_type = self.registry.ctype(&function.ret);
        let parameters = self.fn_param_declarations(&function.params, function.self_kind);
        self.w.line(format!(
            "{} {}({})",
            return_type,
            function.c_name,
            parameters.join(", ")
        ));
        self.emit_function_body(
            &function.params,
            function.self_kind,
            &function.ret,
            &function.body,
            None,
        );
        self.w.blank();
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_function_body(
        &mut self,
        params: &[HParam],
        self_kind: Option<SelfKind>,
        return_type: &Ty,
        body: &HBlock,
        env: Option<EnvInfo>,
    ) {
        self.frames.clear();
        self.loops.clear();
        self.block_terminated = false;
        self.fn_terminated = false;
        self.fn_ret = return_type.clone();

        self.env = env;
        self.w.line("{");
        self.w.indent();
        self.push_frame();
        if let Some(env) = &self.env {
            let var = env.var.clone();
            self.w.line(format!("(void){};", var));
        }
        let receiver_is_borrowed =
            matches!(self_kind, Some(SelfKind::Ref) | Some(SelfKind::RefMut));
        for (index, param) in params.iter().enumerate() {
            if !(receiver_is_borrowed && index == 0) {
                let live = if self.is_trivial_copy(&param.ty) {
                    None
                } else {
                    let flag = format!("{}_live", param.c_name);
                    self.w.line(format!("bool {} = true;", flag));
                    Some(flag)
                };
                if let Some(frame) = self.frames.last_mut() {
                    frame.vars.push(BoundVar {
                        c_name: param.c_name.clone(),
                        ty: param.ty.clone(),
                        owned: true,
                        live,
                    });
                }
            }
            self.w.line(format!("(void){};", param.c_name));
        }
        self.predeclare_block(body);
        self.gen_stmts(&body.stmts);
        if self.fn_terminated {
            self.frames.clear();
            self.w.dedent();
            self.w.line("}");
            return;
        }
        if self.block_terminated {
            let zero = self.raw_zero_expr(return_type);
            self.w.line(format!("return {};", zero));
            self.frames.clear();
            self.w.dedent();
            self.w.line("}");
            return;
        }
        let result = self.new_plain(return_type, "result");
        match &body.tail {
            Some(tail) => self.gen_move_into(tail, &result),
            None => {
                let zero = self.zero_expr(return_type);
                self.w.line(format!("{} = {};", result, zero));
            }
        }
        if self.fn_terminated {
            self.frames.clear();
            self.w.dedent();
            self.w.line("}");
            return;
        }
        self.frame_cleanup(0);
        self.pop_frame();
        self.w.line(format!("return {};", result));
        self.w.dedent();
        self.w.line("}");
    }

    /// Declarations and startup initialisation of the immortal interned string
    /// literals, in first-use order.
    ///
    /// Each distinct literal becomes one immortal allocation created during
    /// startup; evaluating the literal later only retains the shared handle.
    /// The shared bytes are therefore never mutated by copy-on-write, and the
    /// per-evaluation managed allocation disappears.
    ///
    /// In a split build the handles are plain (non-static) definitions in
    /// main.c and the units refer to them through the declarations emitted by
    /// [Gen::emit_interned_literal_externs].
    fn emit_interned_literals(&mut self, split: bool) {
        if self.interned.is_empty() {
            return;
        }
        self.w
            .line("/* Immortal interned string literals, in first-use order. */");
        for (index, value) in self.interned.iter().enumerate() {
            self.w.line(format!(
                "static const char lsaca_lit_data_{}[] = {};",
                index,
                util::c_string_literal(value)
            ));
            if split {
                self.w
                    .line(format!("LsString* lsaca_lit_{} = NULL;", index));
            } else {
                self.w
                    .line(format!("static LsString* lsaca_lit_{} = NULL;", index));
            }
        }
        self.w.blank();
        self.w
            .line("static void lsaca_init_interned_literals(void) {");
        self.w.indent();
        for (index, value) in self.interned.iter().enumerate() {
            self.w.line(format!(
                "lsaca_lit_{} = ls_str_static(lsaca_lit_data_{}, (size_t){});",
                index,
                index,
                value.len()
            ));
        }
        self.w.dedent();
        self.w.line("}");
        self.w.blank();
    }

    /// Declarations of the saturating cast helpers defined in main.c.
    ///
    /// Split translation units call those helpers without seeing their
    /// definitions.
    fn emit_cast_helper_prototypes(w: &mut CWriter) {
        w.line("/* Saturating float to integer conversions (defined in main.c). */");
        for int_ty in IntTy::ALL {
            w.line(format!(
                "{} {}(double value);",
                int_ty.c_type(),
                cast_helper_name(int_ty)
            ));
        }
        w.blank();
    }

    /// Declarations of the interned literal handles that main.c owns.
    fn emit_interned_literal_externs(&self, w: &mut CWriter) {
        if self.interned.is_empty() {
            return;
        }
        let count = self.interned.len();
        w.line("/* Immortal interned string literals defined in main.c. */");
        for index in 0..count {
            w.line(format!("extern LsString* lsaca_lit_{};", index));
        }
        w.blank();
    }

    /// Declarations of the function value thunks defined in main.c.
    fn emit_thunk_prototypes(&self, w: &mut CWriter) {
        let mut emitted = false;
        for (fn_id, function) in self.program.fns.iter().enumerate() {
            if function.self_kind.is_some() {
                continue;
            }
            let return_type = self.registry.ctype(&function.ret);
            let parameters = self.fn_param_declarations(&function.params, None);
            w.line(format!(
                "{} lsaca_thunk_{}(void* env{});",
                return_type,
                fn_id,
                comma_prefixed(&parameters)
            ));
            emitted = true;
        }
        if emitted {
            w.blank();
        }
    }

    fn emit_main_wrapper(&mut self) {
        self.w.line("int main(int argc, char** argv) {");
        self.w.indent();
        self.w.line("ls_runtime_init(argc, argv);");
        if !self.interned.is_empty() {
            self.w.line("lsaca_init_interned_literals();");
        }
        let main_fn = self.program.fns.get(self.program.main_fn).cloned();
        if let Some(function) = main_fn {
            let mut arguments = Vec::with_capacity(function.params.len());
            for param in &function.params {
                arguments.push(self.raw_zero_expr(&param.ty));
            }
            self.w
                .line(format!("{}({});", function.c_name, arguments.join(", ")));
        }
        self.w.line("ls_runtime_shutdown();");
        self.w.line("return 0;");
        self.w.dedent();
        self.w.line("}");
    }
}

/// C name of the saturating float to integer cast helper for one integer
/// type.
fn cast_helper_name(int_ty: IntTy) -> String {
    format!("lsaca_cast_f2i_{}", int_ty.name())
}

/// Build the ", a, b" suffix used after a leading parameter.
fn comma_prefixed(parameters: &[String]) -> String {
    if parameters.is_empty() {
        String::new()
    } else {
        format!(", {}", parameters.join(", "))
    }
}

fn signed_max_macro(int_ty: IntTy) -> &'static str {
    match int_ty {
        IntTy::I8 => "INT8_MAX",
        IntTy::I16 => "INT16_MAX",
        IntTy::I32 => "INT32_MAX",
        IntTy::I64 => "INT64_MAX",
        _ => "INT64_MAX",
    }
}

fn signed_min_macro(int_ty: IntTy) -> &'static str {
    match int_ty {
        IntTy::I8 => "INT8_MIN",
        IntTy::I16 => "INT16_MIN",
        IntTy::I32 => "INT32_MIN",
        IntTy::I64 => "INT64_MIN",
        _ => "INT64_MIN",
    }
}

fn unsigned_max_macro(int_ty: IntTy) -> &'static str {
    match int_ty {
        IntTy::U8 => "UINT8_MAX",
        IntTy::U16 => "UINT16_MAX",
        IntTy::U32 => "UINT32_MAX",
        IntTy::U64 => "UINT64_MAX",
        _ => "UINT64_MAX",
    }
}
