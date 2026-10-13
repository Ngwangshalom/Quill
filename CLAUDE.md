# Quill

Quill is a readable, safe programming language for backends and AI-powered apps.
The full design is in README.md. docs/spec.md defines the language as implemented;
keep it in sync when behaviour changes.

## Implementation
- Written in Rust. Pipeline: lexer → parser → checker → bytecode compiler → VM.
- Source files use the `.ql` extension.
- Keep each stage in its own module (src/lexer.rs, src/parser.rs, ...).

## Rules
- Error messages must follow the README style: say what went wrong,
  why, and suggest a fix. Never panic on bad user code.
- Every new feature gets tests.
- Build in small, working steps. `cargo test` must pass after each one.
