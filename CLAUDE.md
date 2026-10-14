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

## Documentation: the Quill book
The book in book/ (mdBook) teaches Quill and will be published. It must
always match the language. Any change that adds, changes or removes something
users can see (syntax, behaviour, error messages, commands) also, in the same
change:
- Teaches it in the book: update the chapter in book/src/ that covers it, or
  add a chapter (and list it in book/src/SUMMARY.md). Write for beginners:
  explain the idea, show examples and the common mistakes, end with "Try it".
- Updates docs/spec.md, the precise reference (Appendix B of the book).
- Adds a line under "Unreleased" in book/src/appendix-version-history.md.
- Updates README.md, CONTRIBUTING.md and `quill help` where they mention it.
- Updates editor support in the same change, so every editor still
  recognizes the new words: the grammar and colors in `editors/vscode`,
  plus the Vim, Emacs, and Sublime files. Each kind of word has its own
  color: `say`, `when`, `for`, `true` and `false`, `var` and `set`, `check`, and
  the logic words. Planned kinds have a reserved color in the book's
  editor chapter. When the word is added, use that color. Color a word
  only once the language actually has it.

Book examples are run by `cargo test` (tests/book.rs): ```quill must run,
```quill,error must stop with a problem, ```quill,ignore is not run (planned
features), and an ```output block after an example must match exactly. After an
intended output change, `QUILL_BLESS=1 cargo test --test book` refreshes the
output blocks; read the diff before committing.

On a release: move the "Unreleased" notes under the new version's heading, and
add the release to the timeline in book/src/story.md.
