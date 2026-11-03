//! Trait object support: vtable shapes, static tables, thunks and the two
//! dynamic expressions.
//!
//! A trait object value is a fat struct holding the erased data pointer and a
//! pointer to the vtable of the concrete (trait instantiation, self type)
//! pair. One vtable exists per registered pair. Its slots are, in order,
//! drop, clone, trace and debug, followed by one method thunk per object safe
//! method in declaration order.

use super::c_writer::CWriter;
use super::emit::Gen;
use super::ops;
use super::types::{
    DYN_DATA_FIELD, DYN_VTABLE_FIELD, VTABLE_CLONE_FIELD, VTABLE_DEBUG_FIELD, VTABLE_DROP_FIELD,
    VTABLE_TRACE_FIELD, vtable_method_field, vtable_table_name, vtable_thunk_name,
};
use super::util;
use crate::hir::{HExpr, HTraitObject, HVtable};
use crate::span::Span;
use crate::types::Ty;

impl Gen<'_> {
    /// The trait object and concrete vtable of one registered vtable id.
    fn vtable_spec(&self, vtable_id: usize) -> (HTraitObject, HVtable) {
        let vtable = match self.program.vtables.get(vtable_id) {
            Some(vtable) => vtable.clone(),
            None => {
                util::internal_error(format!("vtable {} is missing from the program", vtable_id))
            }
        };
        let object = match self.program.trait_objects.get(vtable.trait_object) {
            Some(object) => object.clone(),
            None => util::internal_error(format!(
                "trait object {} of vtable {} is missing from the program",
                vtable.trait_object, vtable_id
            )),
        };
        if vtable.methods.len() != object.methods.len() {
            util::internal_error(format!(
                "vtable {} for {} has {} method thunks but the trait declares {}",
                vtable_id,
                object.name,
                vtable.methods.len(),
                object.methods.len()
            ));
        }
        if vtable.self_ty.is_unsized() {
            util::internal_error(format!(
                "vtable {} has the unsized self type {}",
                vtable_id, vtable.self_ty
            ));
        }
        (object, vtable)
    }

    fn vtable_ctype(&self, vtable_id: usize) -> String {
        match self.program.vtables.get(vtable_id) {
            Some(vtable) => match self.program.trait_objects.get(vtable.trait_object) {
                Some(object) => object.c_name.clone(),
                None => util::internal_error(format!(
                    "trait object {} of vtable {} is missing from the program",
                    vtable.trait_object, vtable_id
                )),
            },
            None => {
                util::internal_error(format!("vtable {} is missing from the program", vtable_id))
            }
        }
    }

    /// The trait object id behind a reference, box, rc or plain trait object.
    fn dyn_trait_id(&self, ty: &Ty) -> Option<usize> {
        match ty {
            Ty::Dyn(id) => Some(*id),
            Ty::Ref { inner, .. } => match &**inner {
                Ty::Dyn(id) => Some(*id),
                _ => None,
            },
            Ty::Box(inner) | Ty::Rc(inner) => match &**inner {
                Ty::Dyn(id) => Some(*id),
                _ => None,
            },
            _ => None,
        }
    }

    fn dyn_ctype(&self, trait_object: usize) -> String {
        self.ctype(&Ty::Dyn(trait_object))
    }

    /// Build a fat trait object value expression from the erased data pointer.
    fn dyn_fat_expr(&self, trait_object: usize, vtable_id: usize, data: &str) -> String {
        format!(
            "({}){{ .{} = (void*)({}), .{} = &{} }}",
            self.dyn_ctype(trait_object),
            DYN_DATA_FIELD,
            data,
            DYN_VTABLE_FIELD,
            vtable_table_name(vtable_id)
        )
    }

    // ----- emitted vtable data ---------------------------------------------

    /// Definitions of the vtable struct types, one per registered trait
    /// object. Every concrete self type of the same trait object shares the
    /// same vtable struct, so the type is emitted once.
    pub(crate) fn emit_vtable_types(&mut self) {
        if self.program.trait_objects.is_empty() {
            return;
        }
        for object in self.program.trait_objects.iter() {
            let c_name = object.c_name.clone();
            let methods: Vec<(String, Vec<String>)> = object
                .methods
                .iter()
                .map(|method| {
                    let ret = self.ctype(&method.ret);
                    let params = method
                        .params
                        .iter()
                        .map(|param| self.ctype(param))
                        .collect();
                    (ret, params)
                })
                .collect();
            self.w.line(format!("struct {} {{", c_name));
            self.w.indent();
            self.w
                .line(format!("void (*{})(void* data);", VTABLE_DROP_FIELD));
            self.w
                .line(format!("void* (*{})(void* data);", VTABLE_CLONE_FIELD));
            self.w.line(format!(
                "void (*{})(void* data, LsTraceVisit visit);",
                VTABLE_TRACE_FIELD
            ));
            self.w.line(format!(
                "LsString* (*{})(LsString* acc, void* data);",
                VTABLE_DEBUG_FIELD
            ));
            for (slot, (ret, params)) in methods.iter().enumerate() {
                let mut declarators = vec![String::from("void* data")];
                declarators.extend(params.iter().cloned());
                self.w.line(format!(
                    "{} (*{})({});",
                    ret,
                    vtable_method_field(slot),
                    declarators.join(", ")
                ));
            }
            self.w.dedent();
            self.w.line("};");
        }
        self.w.blank();
    }

    /// Prototypes of the thunk functions of every registered vtable.
    pub(crate) fn emit_vtable_thunk_prototypes(&mut self) {
        if self.program.vtables.is_empty() {
            return;
        }
        for vtable_id in 0..self.program.vtables.len() {
            let (object, _) = self.vtable_spec(vtable_id);
            self.w.line(format!(
                "void {}(void* data);",
                vtable_thunk_name(vtable_id, VTABLE_DROP_FIELD)
            ));
            self.w.line(format!(
                "void* {}(void* data);",
                vtable_thunk_name(vtable_id, VTABLE_CLONE_FIELD)
            ));
            self.w.line(format!(
                "void {}(void* data, LsTraceVisit visit);",
                vtable_thunk_name(vtable_id, VTABLE_TRACE_FIELD)
            ));
            self.w.line(format!(
                "LsString* {}(LsString* acc, void* data);",
                vtable_thunk_name(vtable_id, VTABLE_DEBUG_FIELD)
            ));
            for (slot, method) in object.methods.iter().enumerate() {
                let ret = self.ctype(&method.ret);
                let mut declarators = vec![String::from("void* data")];
                for (index, param) in method.params.iter().enumerate() {
                    declarators.push(format!("{} a{}", self.ctype(param), index));
                }
                self.w.line(format!(
                    "{} {}({});",
                    ret,
                    vtable_thunk_name(vtable_id, &vtable_method_field(slot)),
                    declarators.join(", ")
                ));
            }
        }
        self.w.blank();
    }

    /// Definitions of every thunk function of every registered vtable.
    pub(crate) fn emit_vtable_thunks(&mut self) {
        if self.program.vtables.is_empty() {
            return;
        }
        for vtable_id in 0..self.program.vtables.len() {
            let (object, vtable) = self.vtable_spec(vtable_id);
            self.emit_vtable_lifecycle_thunks(vtable_id, &vtable);
            for (slot, method) in object.methods.iter().enumerate() {
                self.emit_vtable_method_thunk(vtable_id, slot, method, vtable.methods[slot]);
            }
        }
    }

    fn emit_vtable_lifecycle_thunks(&mut self, vtable_id: usize, vtable: &HVtable) {
        let self_ty = vtable.self_ty.clone();
        let self_ctype = self.ctype(&self_ty);
        let box_ty = Ty::box_(self_ty.clone());
        let box_cell = self.names.tag(&box_ty);
        let box_ptr = format!("{}*", box_cell);
        let suffix = self.names.mangle(&self_ty);
        let payload = format!("(*((const {}*)data))", self_ctype);
        let copy = ops::copy_expr(self.names, &self_ty, &payload);

        // Slot 0: drop releases the managed concrete value behind the data
        // pointer. Borrowed coercions never call it.
        self.w.line(format!(
            "void {}(void* data) {{",
            vtable_thunk_name(vtable_id, VTABLE_DROP_FIELD)
        ));
        self.w.indent();
        self.w.line("if (data != NULL) {");
        self.w.indent();
        self.w.line("ls_release(data);");
        self.w.dedent();
        self.w.line("}");
        self.w.dedent();
        self.w.line("}");
        self.w.blank();

        // Slot 1: clone allocates a fresh managed Box cell holding a deep copy
        // of the concrete value.
        self.w.line(format!(
            "void* {}(void* data) {{",
            vtable_thunk_name(vtable_id, VTABLE_CLONE_FIELD)
        ));
        self.w.indent();
        self.w.line(format!(
            "{} clone = ({})ls_alloc_managed(sizeof({}), lsaca_box_drop_{}, lsaca_box_trace_{});",
            box_ptr, box_ptr, box_cell, suffix, suffix
        ));
        self.w.line(format!("clone->payload = {};", copy));
        self.w.line("return (void*)clone;");
        self.w.dedent();
        self.w.line("}");
        self.w.blank();

        // Slot 2: trace visits the managed concrete value as a node of the
        // cycle graph.
        self.w.line(format!(
            "void {}(void* data, LsTraceVisit visit) {{",
            vtable_thunk_name(vtable_id, VTABLE_TRACE_FIELD)
        ));
        self.w.indent();
        self.w.line("if (data != NULL) {");
        self.w.indent();
        self.w.line("visit(data);");
        self.w.dedent();
        self.w.line("}");
        self.w.line("(void)visit;");
        self.w.dedent();
        self.w.line("}");
        self.w.blank();

        // Slot 3: debug renders the concrete value into the accumulator.
        self.w.line(format!(
            "LsString* {}(LsString* acc, void* data) {{",
            vtable_thunk_name(vtable_id, VTABLE_DEBUG_FIELD)
        ));
        self.w.indent();
        self.w.line(format!(
            "const {}* value = (const {}*)data;",
            self_ctype, self_ctype
        ));
        match ops::debug_append_statement(self.names, &self_ty, "acc", "(*value)") {
            Some(statement) => self.w.line(statement),
            None => self.w.line("(void)value;"),
        }
        self.w.line("return acc;");
        self.w.dedent();
        self.w.line("}");
        self.w.blank();
    }

    fn emit_vtable_method_thunk(
        &mut self,
        vtable_id: usize,
        slot: usize,
        method: &crate::hir::HTraitMethodSig,
        fn_id: usize,
    ) {
        let function = match self.program.fns.get(fn_id) {
            Some(function) => function.clone(),
            None => util::internal_error(format!(
                "vtable {} slot {} refers to missing function {}",
                vtable_id, slot, fn_id
            )),
        };
        if function.self_kind.is_none() {
            util::internal_error(format!(
                "vtable {} slot {} uses {} which has no self receiver",
                vtable_id, slot, function.name
            ));
        }
        let ret = self.ctype(&method.ret);
        let mut declarations = vec![String::from("void* data")];
        let mut arguments = vec![format!(
            "(({}*)data)",
            self.vtable_receiver_ctype(vtable_id)
        )];
        for (index, param) in method.params.iter().enumerate() {
            declarations.push(format!("{} a{}", self.ctype(param), index));
            arguments.push(format!("a{}", index));
        }
        self.w.line(format!(
            "{} {}({}) {{",
            ret,
            vtable_thunk_name(vtable_id, &vtable_method_field(slot)),
            declarations.join(", ")
        ));
        self.w.indent();
        self.w.line(format!(
            "return {}({});",
            function.c_name,
            arguments.join(", ")
        ));
        self.w.dedent();
        self.w.line("}");
        self.w.blank();
    }

    fn vtable_receiver_ctype(&self, vtable_id: usize) -> String {
        match self.program.vtables.get(vtable_id) {
            Some(vtable) => self.ctype(&vtable.self_ty),
            None => {
                util::internal_error(format!("vtable {} is missing from the program", vtable_id))
            }
        }
    }

    /// Definitions of the static vtable tables.
    pub(crate) fn emit_vtable_tables(&mut self) {
        if self.program.vtables.is_empty() {
            return;
        }
        for vtable_id in 0..self.program.vtables.len() {
            let (object, _) = self.vtable_spec(vtable_id);
            self.w.line(format!(
                "const {} {} = {{",
                object.c_name,
                vtable_table_name(vtable_id)
            ));
            self.w.indent();
            self.w.line(format!(
                ".{} = {},",
                VTABLE_DROP_FIELD,
                vtable_thunk_name(vtable_id, VTABLE_DROP_FIELD)
            ));
            self.w.line(format!(
                ".{} = {},",
                VTABLE_CLONE_FIELD,
                vtable_thunk_name(vtable_id, VTABLE_CLONE_FIELD)
            ));
            self.w.line(format!(
                ".{} = {},",
                VTABLE_TRACE_FIELD,
                vtable_thunk_name(vtable_id, VTABLE_TRACE_FIELD)
            ));
            self.w.line(format!(
                ".{} = {},",
                VTABLE_DEBUG_FIELD,
                vtable_thunk_name(vtable_id, VTABLE_DEBUG_FIELD)
            ));
            for slot in 0..object.methods.len() {
                let field = vtable_method_field(slot);
                self.w.line(format!(
                    ".{} = {},",
                    field,
                    vtable_thunk_name(vtable_id, &field)
                ));
            }
            self.w.dedent();
            self.w.line("};");
        }
        self.w.blank();
    }

    /// Declarations of the static vtable tables, for translation units that do
    /// not contain the definitions (split builds).
    pub(crate) fn emit_vtable_table_externs(&self, w: &mut CWriter) {
        if self.program.vtables.is_empty() {
            return;
        }
        w.line("/* Static vtables defined in main.c. */");
        for vtable_id in 0..self.program.vtables.len() {
            w.line(format!(
                "extern const {} {};",
                self.vtable_ctype(vtable_id),
                vtable_table_name(vtable_id)
            ));
        }
        w.blank();
    }

    // ----- dynamic expressions ---------------------------------------------

    /// Lower a coercion to a trait object.
    ///
    /// The HIR value is already the source form: a reference expression for
    /// borrowed trait objects, a Box/Rc expression for owned ones.
    pub(crate) fn gen_dyn_coerce(
        &mut self,
        value: &HExpr,
        vtable_id: usize,
        ty: &Ty,
        target: &str,
    ) {
        let Some(trait_object) = self.dyn_trait_id(ty) else {
            util::internal_error(format!(
                "dynamic coercion of a value whose type {} is not a trait object",
                ty
            ));
        };
        let vtable = match self.program.vtables.get(vtable_id) {
            Some(vtable) => vtable.clone(),
            None => {
                util::internal_error(format!("vtable {} is missing from the program", vtable_id))
            }
        };
        if vtable.trait_object != trait_object {
            util::internal_error(format!(
                "vtable {} belongs to trait object {} but the coercion targets trait object {}",
                vtable_id, vtable.trait_object, trait_object
            ));
        }
        match ty {
            Ty::Ref { .. } => {
                let pointer = self.gen_owned(value);
                let fat = self.dyn_fat_expr(trait_object, vtable_id, &pointer);
                self.w.line(format!("{} = {};", target, fat));
            }
            Ty::Box(_) => {
                let cell = self.gen_owned_move(value);
                let fat = self.dyn_fat_expr(trait_object, vtable_id, &cell);
                let suffix = self.names.mangle(&Ty::Dyn(trait_object));
                self.w
                    .line(format!("{} = lsaca_box_new_{}({});", target, suffix, fat));
                self.mark_moved(&cell);
            }
            Ty::Rc(_) => {
                let cell = self.gen_owned_move(value);
                let fat = self.dyn_fat_expr(trait_object, vtable_id, &cell);
                let suffix = self.names.mangle(&Ty::Dyn(trait_object));
                self.w
                    .line(format!("{} = lsaca_rc_new_{}({});", target, suffix, fat));
                self.mark_moved(&cell);
            }
            Ty::Dyn(_) => {
                let source = self.gen_owned(value);
                self.w.line(format!("{} = {};", target, source));
            }
            _ => util::internal_error(format!(
                "dynamic coercion target {} is not a reference, Box, Rc or trait object",
                ty
            )),
        }
    }

    /// Lower a virtual method call through a trait object.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn gen_dyn_call(
        &mut self,
        object: &HExpr,
        trait_object: usize,
        slot: usize,
        args: &[HExpr],
        ty: &Ty,
        target: &str,
        _span: Span,
    ) {
        let object_spec = match self.program.trait_objects.get(trait_object) {
            Some(object) => object.clone(),
            None => util::internal_error(format!(
                "trait object {} is missing from the program",
                trait_object
            )),
        };
        let method = match object_spec.methods.get(slot) {
            Some(method) => method.clone(),
            None => util::internal_error(format!(
                "trait object {} has no method slot {}",
                object_spec.name, slot
            )),
        };
        let fat = match &object.ty {
            Ty::Ref { inner, .. } if inner.is_dyn() => self.gen_borrowed(object),
            Ty::Box(inner) | Ty::Rc(inner) if inner.is_dyn() => {
                let cell = self.gen_borrowed(object);
                format!("({})->payload", cell)
            }
            Ty::Dyn(_) => self.gen_borrowed(object),
            other => util::internal_error(format!(
                "dynamic call on a value whose type {} is not a trait object",
                other
            )),
        };
        let mut arguments: Vec<String> = Vec::new();
        let mut moved: Vec<String> = Vec::new();
        for (index, argument) in args.iter().enumerate() {
            let parameter_ty = match method.params.get(index) {
                Some(parameter_ty) => parameter_ty.clone(),
                None => util::internal_error(format!(
                    "method {} of trait object {} takes {} parameters but a call passes {}",
                    method.name,
                    object_spec.name,
                    method.params.len(),
                    args.len()
                )),
            };
            if parameter_ty.is_ref() {
                arguments.push(self.gen_borrowed(argument));
            } else if self.is_trivial_copy(&parameter_ty) {
                arguments.push(self.gen_owned(argument));
            } else {
                let temp = self.declare_temp(&parameter_ty);
                self.gen_move_into(argument, &temp);
                arguments.push(temp.clone());
                moved.push(temp);
            }
        }
        let mut call = format!(
            "({}).{}->{}(({}).{}",
            fat,
            DYN_VTABLE_FIELD,
            vtable_method_field(slot),
            fat,
            DYN_DATA_FIELD
        );
        if !arguments.is_empty() {
            call.push_str(", ");
            call.push_str(&arguments.join(", "));
        }
        call.push(')');
        if ty.is_unit() {
            let zero = self.zero_expr(ty);
            self.w.line(format!("{};", call));
            self.w.line(format!("{} = {};", target, zero));
        } else {
            self.w.line(format!("{} = {};", target, call));
        }
        for temp in &moved {
            self.mark_moved(temp);
        }
    }
}
