# Contributing to Quill

Thanks for wanting to help build Quill! This guide gets you from zero to your first pull request.

Every kind of help counts:

- **Code:** new language features, better errors, faster runtime.
- **Language design:** opinions on syntax and behaviour while the design is still flexible. Open a [Discussion](../../discussions).
- **Examples and docs:** small `.ql` programs, clearer explanations, fixed typos.
- **Bug reports:** especially confusing error messages. If an error didn't help you, that's a bug.

## Getting started

You need Rust (stable). Install it from [rustup.rs](https://rustup.rs), then:

```bash
git clone https://github.com/<your-username>/quill.git
cd quill
cargo test                   # everything should pass
cargo run -- run hello.ql    # runs the example program
```

Useful commands while you work:

| Command | What it does |
|---|---|
| `cargo run -- run FILE.ql` | Run a Quill program |
| `cargo run -- check FILE.ql` | Find problems without running |
| `cargo run -- test` | Run every `check` and doc example in the folder |
| `cargo run -- tokens FILE.ql` | See how the lexer read a file |
| `cargo run -- explain FILE.ql:N` | See how Quill understood line N |

## How Quill is built

The [README](README.md) is the language spec. The implementation is a pipeline, one module per stage:

```
source (.ql) → lexer → parser → checker → compiler → VM
```

| File | Job |
|---|---|
| `src/lexer.rs` | Turns text into tokens (`say`, `"text"`, `42`, `=`, ...), each with a line and column |
| `src/parser.rs` | Turns tokens into statements (`say x`, `x = 5`, `check a == b`) |
| `src/checker.rs` | Finds mistakes before anything runs: unknown names, values created twice, ... |
| `src/compiler.rs` | Turns checked statements into bytecode instructions |
| `src/vm.rs` | Runs the bytecode |
| `src/value.rs` | The values programs work with at runtime |
| `src/diagnostic.rs` | The shared error type and the README-style error format |
| `src/explain.rs` | `quill explain` |
| `src/lib.rs` | Connects the stages together |
| `src/main.rs` | The `quill` command line tool |
| `tests/cli.rs` | End-to-end tests that run the real `quill` binary |
| `tests/programs/` | Small `.ql` programs used by those tests |

### Adding a feature: the usual path

Most features touch several stages, in pipeline order. Say you are adding `+`:

1. **Lexer:** add a `Plus` variant to `TokenKind`, give it a `Display` arm, and recognise `'+'` in `Lexer::run`.
2. **Parser:** add an expression kind for it (e.g. `ExprKind::Add`) and parse `a + b`.
3. **Checker:** reject what doesn't make sense (adding text to a number), with a helpful error.
4. **Compiler:** emit a new `Op::Add`.
5. **VM:** execute `Op::Add`.
6. **Tests:** at every step (see below).

You don't have to do it all in one pull request. A lexer-only PR with tests is a fine first step.

## The rules

These are what reviewers look for.

### 1. Errors must teach

Every error message says **what** went wrong, **why**, and, when possible, **how to fix it**. Build errors with `Diagnostic`:

```rust
Diagnostic::new(
    token.span(),
    "say needs something to say.\n\
     Put text, a number or the name of a value after it.",
)
.suggest("say \"Hello, World!\"")
```

which the user sees as:

```
Problem on line 3 of shop.ql:

    say
    ^^^
say needs something to say.
Put text, a number or the name of a value after it.

Did you mean:  say "Hello, World!"
```

Write messages for a beginner: plain words, no compiler jargon ("token", "AST", "lvalue").

### 2. Never panic on user code

Bad code must produce a `Diagnostic`, never a crash. No `unwrap()`, `expect()` or unchecked indexing on anything that comes from user input. If something "can't happen", return an error anyway. The VM's `internal(...)` helper shows how. A panic in Quill is always a bug worth reporting.

### 3. Every feature gets tests

- **Unit tests** go at the bottom of the module you changed, in its `mod tests`. Test both what works *and* each error: its line, column and suggestion.
- **End-to-end tests** go in `tests/cli.rs`. They run the real `quill` binary on a file in `tests/programs/` and compare the exact output.

### 4. Small, working steps

Each pull request should do one thing, and the code must pass all checks after it:

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

GitHub runs these automatically on every pull request.

## Making a pull request

1. Fork the repository and create a branch: `git checkout -b add-plus-operator`.
2. Make your change with tests.
3. Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test`.
4. Open a pull request that says what it does and why. Fill in the checklist in the template.
5. A maintainer reviews it. Expect friendly questions: they're about the code, never about you.

**Changing the language itself** (new syntax, changed behaviour)? Open a Discussion or an issue first, so we agree on the design before you write code. That saves you from rewriting work.

## Good first issues

Look for the [`good first issue`](../../labels/good%20first%20issue) label. Some ideas that fit well:

- **`var` and `set`:** mutable values, as described in the README. A good tour of every stage.
- **Arithmetic:** `+`, `-`, `*`, `/` on numbers, with errors for text + number.
- **Text interpolation:** `say "Hello, {name}"`.
- **Number separators:** let `1_000_000` be written for readability.
- **Better suggestions:** find an error message that confused you and improve it.
- **Example programs:** add small, well-commented `.ql` files.
- **Editor support:** syntax highlighting for VS Code.

The [Roadmap](README.md#roadmap) in the README lists the bigger milestones.

## Code of Conduct

Everyone taking part in Quill must follow our [Code of Conduct](CODE_OF_CONDUCT.md). Be kind, assume good intent, and help newcomers.

## License

By contributing, you agree that your contributions are licensed under the [Apache License 2.0](LICENSE), the same license as Quill.
