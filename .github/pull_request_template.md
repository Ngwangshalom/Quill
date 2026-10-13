## What does this change?

<!-- One or two sentences. Link the issue: "Fixes #12". -->

## Example

```quill
-- Quill code this makes work (or the error it improves)
```

## Checklist

- [ ] Tests added for the new behaviour **and** its errors
- [ ] Error messages say what went wrong, why, and suggest a fix
- [ ] No `unwrap()` / `panic!` on anything from user code
- [ ] The book teaches it (`book/src/`), the spec is updated (`docs/spec.md`), and "Unreleased" in the version history has a line for it
- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass
