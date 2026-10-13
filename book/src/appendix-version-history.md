# Appendix C: Version History

What changed in each version of Quill. The newest version is at the top.

## Unreleased

Changes that are finished but not yet in a release.

- **`quill update`** installs the latest version of Quill.
- Quill checks for a new version **at most once a day**, in the background, and says when one is out. It never updates by itself. Set `QUILL_NO_UPDATE_CHECK=1` to turn the check off.
- The installers now also install the updater, `quill-update`.

## v0.2.0 (13 October 2026)

- **Arithmetic** with `+`, `-`, `*`, `/` and `%`, brackets, and the usual order of operations. See [Numbers and Calculations](numbers.md).
- **Exact decimal numbers**: `0.1 + 0.2` is exactly `0.3`.
- **Comparisons** `==`, `!=`, `<`, `<=`, `>`, `>=`, and conditions with `and`, `or` and `not`.
- **Values inside text**: `"Hi {name}"`. See [Text](text.md).
- **`var` and `set`** for values that change. See [Values That Change](changing-values.md).
- **`when`** to choose what to do, with `->` and `else`. See [Making Decisions with when](decisions.md).
- **Indentation** marks the cases of a `when`.
- The checker knows the **kind** of every value and catches mistakes like `price + "5"` before running.
- `check` accepts any comparison, not only `==`.
- `quill explain` describes calculations, `var`, `set` and `when`.
- New reserved words: `var`, `set`, `when`, `else`, `true`, `false`, `and`, `or`, `not`.

## v0.1.0 (13 October 2026)

The first release.

- `say` to show a value, and `name = value` to name one.
- Text in double quotes, with escapes and three-quote text; numbers written exactly as typed.
- Comments (`--`) and doc comments (`---`).
- Tests: `check a == b` and `--- example:`.
- Errors in the README style: where, what, why, and "Did you mean".
- The `quill` tool: `run`, `check`, `test`, `explain`, `new`, `tokens`, `help`, `version`.
- Installers for macOS, Linux and Windows.
