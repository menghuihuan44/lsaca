//! The C code generator: HIR to a self contained C project.
//!
//! The generated project consists of:
//!
//! * main.c: the monomorphized user program plus generated type helpers;
//! * lsaca_rt.h and lsaca_rt.c: the runtime (reference counting, String, Vec,
//!   formatting, checked arithmetic, panics);
//! * CMakeLists.txt: a portable build description for the project.
//!
//! The generated code is C17 and has no dependencies beyond the C standard
//! library. See README.md for the generated project layout and the runtime ABI
//! overview.

use crate::hir;

mod builtins;
mod c_writer;
mod cmake;
mod control;
mod emit;
mod expr;
mod helper_defs;
mod helpers;
mod items;
mod ops;
mod types;
mod util;

#[cfg(test)]
mod ref_tests;

#[cfg(test)]
mod tests;

/// Options that influence generated code.
#[derive(Debug, Clone)]
pub struct CodegenOptions {
    /// Name of the generated program, also the CMake project and executable name.
    pub program_name: String,
    /// When true, integer overflow checks are emitted. Release builds wrap.
    pub debug_checks: bool,
    /// Include source file and line information in runtime checks.
    pub emit_location_info: bool,
    /// Emit extra comments in the generated C (useful when debugging codegen).
    pub verbose: bool,
    /// Source file used to resolve HIR spans into panic locations. When None,
    /// diagnostics fall back to the program name with line 0, column 0.
    pub source: Option<crate::span::SourceFile>,
}

impl Default for CodegenOptions {
    fn default() -> CodegenOptions {
        CodegenOptions {
            program_name: "program".to_string(),
            debug_checks: true,
            emit_location_info: true,
            verbose: false,
            source: None,
        }
    }
}

/// The files of a generated C project (the runtime files are separate consts
/// published by the runtime module).
#[derive(Debug, Clone)]
pub struct Generated {
    pub main_c: String,
    pub cmake: String,
    /// Extra C translation units next to main.c. Empty for the usual
    /// single-translation-unit project; large programs are split into main.c
    /// plus up to 8 units so the C compiler can compile them in parallel.
    /// main.c is always [Generated::main_c] and never repeated here.
    pub units: Vec<GeneratedUnit>,
}

/// One extra C translation unit of a split project.
///
/// The file name is relative to the generated project directory, for example
/// lsaca_unit_1.c.
#[derive(Debug, Clone)]
pub struct GeneratedUnit {
    pub file_name: String,
    pub contents: String,
}

/// Generate the C project for a checked program.
pub fn generate(program: &hir::Program, options: &CodegenOptions) -> Generated {
    let registry = types::TypeRegistry::collect(program);
    let (main_c, units) = items::generate_project(program, &registry, options);
    let unit_file_names: Vec<String> = units.iter().map(|unit| unit.file_name.clone()).collect();
    let cmake = cmake::render(&options.program_name, &program.name, &unit_file_names);
    Generated {
        main_c,
        cmake,
        units,
    }
}
