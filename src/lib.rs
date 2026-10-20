//! Quill: a readable, safe programming language for backends and AI-powered apps.
//!
//! Pipeline: lexer → parser → checker → bytecode compiler → VM.

pub mod ai;
pub mod bring;
pub mod checker;
pub mod compiler;
pub mod decimal;
pub mod diagnostic;
pub mod explain;
pub mod json;
pub mod lexer;
pub mod native;
pub mod parser;
pub mod value;
pub mod vm;
pub mod web;

use std::io::Write;

use compiler::Chunk;
use diagnostic::Diagnostic;

/// Runs every stage up to bytecode, stopping at the first stage with problems.
/// Files it brings are found next to the folder Quill runs in.
pub fn compile_source(source: &str) -> Result<Chunk, Vec<Diagnostic>> {
    compile_file("example.ql", source).0
}

/// Compiles the program in `path`, with every file it brings. The files come back
/// either way, so problems can be shown with the file each one is in.
pub fn compile_file(
    path: &str,
    source: &str,
) -> (Result<Chunk, Vec<Diagnostic>>, Vec<bring::File>) {
    let (program, files) = bring::load(path, source);
    let result = program.and_then(|program| {
        checker::check(&program)?;
        let mut chunk = compiler::compile(&program).map_err(|problem| vec![problem])?;
        // Paths the program names are counted from its own folder.
        chunk.folder = std::path::Path::new(path)
            .parent()
            .map(|folder| folder.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(chunk)
    });
    (result, files)
}

/// Compiles and runs `source`, writing what it says to `out`.
pub fn run_source(source: &str, out: &mut impl Write) -> Result<(), Vec<Diagnostic>> {
    run_file("example.ql", source, out).0
}

/// Compiles and runs the program in `path`, with every file it brings.
pub fn run_file(
    path: &str,
    source: &str,
    out: &mut impl Write,
) -> (Result<(), Vec<Diagnostic>>, Vec<bring::File>) {
    let (chunk, files) = compile_file(path, source);
    let result = chunk.and_then(|chunk| vm::run(&chunk, out).map_err(|problem| vec![problem]));
    (result, files)
}
