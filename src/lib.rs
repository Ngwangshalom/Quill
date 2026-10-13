//! Quill: a readable, safe programming language for backends and AI-powered apps.
//!
//! Pipeline: lexer → parser → checker → bytecode compiler → VM.

pub mod checker;
pub mod compiler;
pub mod diagnostic;
pub mod lexer;
pub mod parser;
pub mod value;
pub mod vm;

use std::io::Write;

use compiler::Chunk;
use diagnostic::Diagnostic;

/// Runs every stage up to bytecode, stopping at the first stage with problems.
pub fn compile_source(source: &str) -> Result<Chunk, Vec<Diagnostic>> {
    let tokens = lexer::lex(source).map_err(|problem| vec![problem])?;
    let program = parser::parse(&tokens).map_err(|problem| vec![problem])?;
    checker::check(&program)?;
    compiler::compile(&program).map_err(|problem| vec![problem])
}

/// Compiles and runs `source`, writing what it says to `out`.
pub fn run_source(source: &str, out: &mut impl Write) -> Result<(), Vec<Diagnostic>> {
    let chunk = compile_source(source)?;
    vm::run(&chunk, out).map_err(|problem| vec![problem])
}
