//! Emitters for the frozen Phase C operating-system intrinsics.
//!
//! The standard library declares these extern functions and every call site is
//! expanded here into a call of an existing runtime function. No new runtime
//! entry point is introduced: file, socket, time, environment and process
//! operations all map onto the frozen ABI in lsaca_rt.h.
//!
//! Ownership follows the runtime contract. Borrowed String arguments are read
//! through their place and keep their owner; returned String handles are fresh
//! owned values assigned straight into the target. Functions that return a
//! managed LsVec_String* block transfer the by-value vector out of the block
//! and release the block with its fields cleared, so the moved vector keeps
//! ownership of the elements. Tuple results copy the runtime out parameters
//! into an ordinary tuple value.

use super::emit::Gen;
use super::types::{OPTION_SOME_MEMBER, TAG_FIELD, UNION_FIELD, positional_field};
use super::util;
use crate::hir::HExpr;
use crate::span::Span;
use crate::types::{IntTy, Ty};

/// Every intrinsic of the frozen Phase C ABI, in Appendix C declaration order.
///
/// The dispatch below covers exactly this list. The unit tests expand one call
/// per entry, so a missing emitter fails the test suite instead of surfacing as
/// "no codegen emitter" in the first real program.
pub(crate) const PHASE_C_INTRINSICS: &[&str] = &[
    // Files, paths and error reporting.
    "__io_stdin",
    "__io_stdout",
    "__io_stderr",
    "__io_open",
    "__io_create",
    "__io_close",
    "__io_read",
    "__io_write",
    "__io_seek",
    "__io_flush",
    "__io_size",
    "__io_set_nonblocking",
    "__io_last_error_kind",
    "__io_last_error_message",
    "__io_read_dir",
    "__io_metadata",
    "__io_remove_file",
    "__io_remove_dir",
    "__io_remove_dir_all",
    "__io_create_dir",
    "__io_create_dir_all",
    "__io_rename",
    "__io_canonicalize",
    "__io_current_dir",
    "__io_set_current_dir",
    "__io_temp_dir",
    // Sockets.
    "__net_listen",
    "__net_local_port",
    "__net_accept",
    "__net_connect",
    "__net_set_timeout",
    "__net_shutdown",
    // Clock.
    "__time_now_ms",
    "__time_monotonic_ns",
    "__time_sleep_ms",
    "__time_local_parts",
    "__time_utc_parts",
    // Environment and process.
    "__env_var",
    "__env_vars",
    "__proc_args",
    "__proc_exit",
];

/// True when the name belongs to the frozen Phase C ABI.
pub(crate) fn is_phase_c_intrinsic(name: &str) -> bool {
    PHASE_C_INTRINSICS.contains(&name)
}

/// The argument at index, or an internal error when the call is malformed.
fn phase_c_argument<'e>(name: &str, args: &'e [HExpr], index: usize) -> &'e HExpr {
    match args.get(index) {
        Some(argument) => argument,
        None => util::internal_error(format!(
            "intrinsic {} is called without argument {}",
            name, index
        )),
    }
}

/// True when the type is a borrowed byte slice (a frozen buffer view).
fn is_byte_slice_reference(ty: &Ty) -> bool {
    match ty {
        Ty::Ref { inner, .. } => match &**inner {
            Ty::Slice(elem) => matches!(**elem, Ty::Int(IntTy::U8)),
            _ => false,
        },
        _ => false,
    }
}

/// The payload type of an Option result, or an internal error.
fn option_payload<'t>(name: &str, ty: &'t Ty) -> &'t Ty {
    match ty {
        Ty::Option(payload) => payload,
        _ => util::internal_error(format!("intrinsic {} must produce an Option value", name)),
    }
}

impl<'a> Gen<'a> {
    /// Expand one Phase C intrinsic call into target.
    ///
    /// Returns false when the name does not belong to the frozen Phase C ABI so
    /// the legacy tables below still get a chance to see it.
    pub(crate) fn try_gen_phase_c_intrinsic(
        &mut self,
        name: &str,
        args: &[HExpr],
        ty: &Ty,
        target: &str,
        span: Span,
    ) -> bool {
        if !is_phase_c_intrinsic(name) {
            return false;
        }
        let bare = name.strip_prefix("__").unwrap_or(name);
        match bare {
            // ----- standard streams and handle based file operations -----
            "io_stdin" => {
                self.line(format!("{} = ls_io_stdin();", target));
                true
            }
            "io_stdout" => {
                self.line(format!("{} = ls_io_stdout();", target));
                true
            }
            "io_stderr" => {
                self.line(format!("{} = ls_io_stderr();", target));
                true
            }
            "io_open" => {
                let path = self.phase_c_string(name, args, 0);
                self.line(format!("{} = ls_io_open({});", target, path));
                true
            }
            "io_create" => {
                let path = self.phase_c_string(name, args, 0);
                self.line(format!("{} = ls_io_create({});", target, path));
                true
            }
            "io_close" => {
                let handle = self.phase_c_int(name, args, 0);
                self.line(format!("{} = ls_io_close({});", target, handle));
                true
            }
            "io_read" => {
                let handle = self.phase_c_int(name, args, 0);
                let (data, length) = self.phase_c_buffer(name, args, 1);
                self.line(format!(
                    "{} = ls_io_read({}, (void*)({}), {});",
                    target, handle, data, length
                ));
                true
            }
            "io_write" => {
                let handle = self.phase_c_int(name, args, 0);
                let (data, length) = self.phase_c_buffer(name, args, 1);
                self.line(format!(
                    "{} = ls_io_write({}, (const void*)({}), {});",
                    target, handle, data, length
                ));
                true
            }
            "io_seek" => {
                let handle = self.phase_c_int(name, args, 0);
                let offset = self.phase_c_int(name, args, 1);
                let whence = self.phase_c_int(name, args, 2);
                self.line(format!(
                    "{} = ls_io_seek({}, {}, (int32_t)({}));",
                    target, handle, offset, whence
                ));
                true
            }
            "io_flush" => {
                let handle = self.phase_c_int(name, args, 0);
                self.line(format!("{} = ls_io_flush({});", target, handle));
                true
            }
            "io_size" => {
                let handle = self.phase_c_int(name, args, 0);
                self.line(format!("{} = ls_io_size({});", target, handle));
                true
            }
            "io_set_nonblocking" => {
                let handle = self.phase_c_int(name, args, 0);
                let enabled = self.phase_c_bool(name, args, 1);
                self.line(format!(
                    "{} = ls_io_set_nonblocking({}, {});",
                    target, handle, enabled
                ));
                true
            }
            "io_last_error_kind" => {
                self.line(format!("{} = ls_io_last_error_kind();", target));
                true
            }
            "io_last_error_message" => {
                self.line(format!("{} = ls_io_last_error_message();", target));
                true
            }
            "io_read_dir" => {
                let path = self.phase_c_string(name, args, 0);
                let payload = option_payload(name, ty).clone();
                let payload_ctype = self.ctype(&payload);
                let block = self.new_plain_of(&format!("{}*", payload_ctype), "read_dir_block");
                self.line(format!("{} = ls_io_read_dir({});", block, path));
                let zero = self.zero_expr(ty);
                self.line(format!("{} = {};", target, zero));
                self.line(format!("if ({} != NULL) {{", block));
                self.w.indent();
                let entries = self.new_plain(&payload, "read_dir_entries");
                self.steal_managed_vec(&payload, &block, &entries);
                self.line(format!("{}.{} = 1;", target, TAG_FIELD));
                self.line(format!(
                    "{}.{}.{} = {};",
                    target, UNION_FIELD, OPTION_SOME_MEMBER, entries
                ));
                self.w.dedent();
                self.line("}");
                true
            }
            "io_metadata" => {
                let path = self.phase_c_string(name, args, 0);
                let size = self.new_plain_of("int64_t", "meta_size");
                let modified = self.new_plain_of("int64_t", "meta_modified");
                let kind = self.new_plain_of("int64_t", "meta_kind");
                let status = self.new_plain_of("int64_t", "meta_status");
                self.line(format!(
                    "{} = ls_io_metadata({}, &{}, &{}, &{});",
                    status, path, size, modified, kind
                ));
                let fields = vec![size, modified, kind];
                self.write_tuple_status(ty, target, &status, &fields);
                true
            }
            "io_remove_file" => self.emit_io_path_call(name, args, target, "ls_io_remove_file"),
            "io_remove_dir" => self.emit_io_path_call(name, args, target, "ls_io_remove_dir"),
            "io_remove_dir_all" => {
                self.emit_io_path_call(name, args, target, "ls_io_remove_dir_all")
            }
            "io_create_dir" => self.emit_io_path_call(name, args, target, "ls_io_create_dir"),
            "io_create_dir_all" => {
                self.emit_io_path_call(name, args, target, "ls_io_create_dir_all")
            }
            "io_set_current_dir" => {
                self.emit_io_path_call(name, args, target, "ls_io_set_current_dir")
            }
            "io_rename" => {
                let from = self.phase_c_string(name, args, 0);
                let to = self.phase_c_string(name, args, 1);
                self.line(format!("{} = ls_io_rename({}, {});", target, from, to));
                true
            }
            "io_canonicalize" => {
                let path = self.phase_c_string(name, args, 0);
                let call = format!("ls_io_canonicalize({})", path);
                self.emit_option_string(name, ty, target, &call);
                true
            }
            "io_current_dir" => {
                self.emit_option_string(name, ty, target, "ls_io_current_dir()");
                true
            }
            "io_temp_dir" => {
                let value = self.new_plain_of("LsString*", "temp_dir");
                self.line(format!("{} = ls_io_temp_dir();", value));
                self.line(format!("if ({} == NULL) {{", value));
                self.w.indent();
                self.line(format!("{} = ls_str_from_parts(\"\", (size_t)0);", value));
                self.w.dedent();
                self.line("}");
                self.line(format!("{} = {};", target, value));
                true
            }
            // ----- sockets -------------------------------------------------
            "net_listen" => {
                let address = self.phase_c_string(name, args, 0);
                self.line(format!("{} = ls_net_listen({});", target, address));
                true
            }
            "net_local_port" => {
                let handle = self.phase_c_int(name, args, 0);
                self.line(format!("{} = ls_net_local_port({});", target, handle));
                true
            }
            "net_accept" => {
                let listener = self.phase_c_int(name, args, 0);
                self.line(format!("{} = ls_net_accept({});", target, listener));
                true
            }
            "net_connect" => {
                let address = self.phase_c_string(name, args, 0);
                self.line(format!("{} = ls_net_connect({});", target, address));
                true
            }
            "net_set_timeout" => {
                let handle = self.phase_c_int(name, args, 0);
                let milliseconds = self.phase_c_int(name, args, 1);
                self.line(format!(
                    "{} = ls_net_set_timeout({}, {});",
                    target, handle, milliseconds
                ));
                true
            }
            "net_shutdown" => {
                let handle = self.phase_c_int(name, args, 0);
                let how = self.phase_c_int(name, args, 1);
                self.line(format!(
                    "{} = ls_net_shutdown({}, (int32_t)({}));",
                    target, handle, how
                ));
                true
            }
            // ----- clock ---------------------------------------------------
            "time_now_ms" => {
                self.line(format!("{} = ls_time_now_ms();", target));
                true
            }
            "time_monotonic_ns" => {
                self.line(format!("{} = ls_time_monotonic_ns();", target));
                true
            }
            "time_sleep_ms" => {
                let milliseconds = self.phase_c_int(name, args, 0);
                self.line(format!("ls_time_sleep_ms({});", milliseconds));
                let zero = self.zero_expr(&Ty::unit());
                self.line(format!("{} = {};", target, zero));
                true
            }
            "time_local_parts" => {
                self.emit_time_parts(name, args, ty, target, "ls_time_local_parts");
                true
            }
            "time_utc_parts" => {
                self.emit_time_parts(name, args, ty, target, "ls_time_utc_parts");
                true
            }
            // ----- environment and process ---------------------------------
            "env_var" => {
                let path = self.phase_c_string(name, args, 0);
                let call = format!("ls_env_var({})", path);
                self.emit_option_string(name, ty, target, &call);
                true
            }
            "env_vars" => self.emit_managed_vec(name, ty, target, "ls_env_vars"),
            "proc_args" => {
                if !matches!(ty, Ty::Vec(_)) {
                    util::internal_error(format!("intrinsic {} must produce a Vec value", name));
                }
                // The located runtime entry point keeps the panic message for
                // arguments that are not valid UTF-8 pointing at the call site.
                let location = self.loc(span);
                self.line(format!("{} = ls_args_at({});", target, location));
                true
            }
            "proc_exit" => {
                let code = self.phase_c_int(name, args, 0);
                let unit = self.zero_expr(&Ty::unit());
                self.line(format!("{} = {};", target, unit));
                self.line(format!("ls_exit((int32_t)({}));", code));
                self.block_terminated = true;
                self.fn_terminated = true;
                true
            }
            other => util::internal_error(format!(
                "intrinsic __{} belongs to the Phase C ABI but has no emitter",
                other
            )),
        }
    }

    /// Emit a call whose runtime function takes one owned String path argument.
    fn emit_io_path_call(
        &mut self,
        name: &str,
        args: &[HExpr],
        target: &str,
        function: &str,
    ) -> bool {
        let path = self.phase_c_string(name, args, 0);
        self.line(format!("{} = {}({});", target, function, path));
        true
    }

    /// Emit a call producing an Option of a borrowed runtime String handle.
    ///
    /// The runtime returns a fresh owned handle or NULL. A non-NULL handle is
    /// moved into the Some payload without copying.
    fn emit_option_string(&mut self, name: &str, ty: &Ty, target: &str, call: &str) {
        let payload = option_payload(name, ty);
        let payload_ctype = self.ctype(payload);
        let value = self.new_plain_of(&payload_ctype, "option_string");
        self.line(format!("{} = {};", value, call));
        let zero = self.zero_expr(ty);
        self.line(format!("{} = {};", target, zero));
        self.line(format!("if ({} != NULL) {{", value));
        self.w.indent();
        self.line(format!("{}.{} = 1;", target, TAG_FIELD));
        self.line(format!(
            "{}.{}.{} = {};",
            target, UNION_FIELD, OPTION_SOME_MEMBER, value
        ));
        self.w.dedent();
        self.line("}");
    }

    /// Emit a call producing a by-value Vec from a managed runtime block.
    ///
    /// A NULL block yields the empty vector owned by the target.
    fn emit_managed_vec(&mut self, name: &str, ty: &Ty, target: &str, function: &str) -> bool {
        if !matches!(ty, Ty::Vec(_)) {
            util::internal_error(format!("intrinsic {} must produce a Vec value", name));
        }
        let ctype = self.ctype(ty);
        let block = self.new_plain_of(&format!("{}*", ctype), "vec_block");
        self.line(format!("{} = {}();", block, function));
        let value = self.new_plain(ty, "vec_value");
        self.line(format!("if ({} != NULL) {{", block));
        self.w.indent();
        self.steal_managed_vec(ty, &block, &value);
        self.w.dedent();
        self.line("}");
        self.line(format!("{} = {};", target, value));
        true
    }

    /// Move a by-value vector out of a managed runtime block.
    ///
    /// The block owns its payload; after the struct copy its fields are cleared
    /// so releasing the block frees only the block itself. The copy now owns
    /// the elements.
    fn steal_managed_vec(&mut self, vec_ty: &Ty, block: &str, value: &str) {
        let ctype = self.ctype(vec_ty);
        self.line(format!("{} = *((const {}*)({}));", value, ctype, block));
        self.line(format!("({})->len = 0;", block));
        self.line(format!("({})->cap = 0;", block));
        self.line(format!("({})->data = NULL;", block));
        self.line(format!("ls_release((void*)({}));", block));
    }

    /// Emit the nine element formatted time tuple.
    fn emit_time_parts(
        &mut self,
        name: &str,
        args: &[HExpr],
        ty: &Ty,
        target: &str,
        function: &str,
    ) {
        let unix_ms = self.phase_c_int(name, args, 0);
        let year = self.new_plain_of("int32_t", "part_year");
        let month = self.new_plain_of("int32_t", "part_month");
        let day = self.new_plain_of("int32_t", "part_day");
        let hour = self.new_plain_of("int32_t", "part_hour");
        let minute = self.new_plain_of("int32_t", "part_minute");
        let second = self.new_plain_of("int32_t", "part_second");
        let weekday = self.new_plain_of("int32_t", "part_weekday");
        let yearday = self.new_plain_of("int32_t", "part_yearday");
        let status = self.new_plain_of("int64_t", "parts_status");
        self.line(format!(
            "{} = {}({}, &{}, &{}, &{}, &{}, &{}, &{}, &{}, &{});",
            status, function, unix_ms, year, month, day, hour, minute, second, weekday, yearday
        ));
        let fields = vec![year, month, day, hour, minute, second, weekday, yearday]
            .into_iter()
            .map(|field| format!("(int64_t)({})", field))
            .collect::<Vec<String>>();
        self.write_tuple_status(ty, target, &status, &fields);
    }

    /// Write a tuple whose first field is a status code and whose remaining
    /// fields are int64_t values of the runtime out parameters.
    fn write_tuple_status(&mut self, ty: &Ty, target: &str, status: &str, fields: &[String]) {
        let zero = self.zero_expr(ty);
        self.line(format!("{} = {};", target, zero));
        self.line(format!("{}.{} = {};", target, positional_field(0), status));
        for (index, field) in fields.iter().enumerate() {
            self.line(format!(
                "{}.{} = {};",
                target,
                positional_field(index + 1),
                field
            ));
        }
    }

    /// Borrow a text argument: the runtime call does not take ownership.
    ///
    /// The standard library declares these parameters as `&str`, but the
    /// front end may still hand over an owned String (e.g. for a `&String`
    /// argument), so both shapes are accepted.
    fn phase_c_string(&mut self, name: &str, args: &[HExpr], index: usize) -> String {
        let argument = phase_c_argument(name, args, index);
        let context = format!("intrinsic {}", name);
        self.string_text_argument(argument, &context)
    }

    /// Evaluate an int64_t argument.
    fn phase_c_int(&mut self, name: &str, args: &[HExpr], index: usize) -> String {
        let argument = phase_c_argument(name, args, index);
        let value = self.gen_borrowed(argument);
        format!("(int64_t)({})", value)
    }

    /// Evaluate a bool argument.
    fn phase_c_bool(&mut self, name: &str, args: &[HExpr], index: usize) -> String {
        let argument = phase_c_argument(name, args, index);
        self.gen_borrowed(argument)
    }

    /// The data pointer and length of a borrowed byte slice argument.
    fn phase_c_buffer(&mut self, name: &str, args: &[HExpr], index: usize) -> (String, String) {
        let argument = phase_c_argument(name, args, index);
        if !is_byte_slice_reference(&argument.ty) {
            util::internal_error(format!(
                "intrinsic {} expects a borrowed byte slice argument",
                name
            ));
        }
        let fat = self.gen_borrowed(argument);
        (
            format!("({}).data", fat),
            format!("(int64_t)(({}).len)", fat),
        )
    }
}
