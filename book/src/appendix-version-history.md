# Appendix C: Version History

What changed in each version of Quill. The newest version is at the top.

## Unreleased

Changes that are finished but not yet in a release.

- *Nothing yet.*

## v0.3.1 (14 October 2026)

- **Indentation is exact.** A block moves in by 2 spaces. A tab, or any other width, is a mistake.
- A range is two dots. `18...25` is a mistake, and Quill tells you to write `18..25`. `->` belongs inside a when, so `say` takes the text directly.
- **Money.** `100 AED + 50 AED` is `150 AED`. Two currencies cannot be added. A function writes the currency as the kind: `price: AED`.
- **Maybe.** `-> maybe number` means the answer might be missing. `return found 2.50` or `return missing`. `when` opens it with `found` and `missing`, and both are required. `maybe`, `found`, and `missing` are peach.
- **Failures.** `fails Declined` names how a function can fail. `fail Declined` leaves with that failure. The caller writes `on Declined` under the call, or `try` inside a function that fails the same way. `fails`, `fail`, `on`, and `try` are red.
- **Functions.** `fn double(n: number) -> number` names a calculation, and `return` gives the value back. A call is `double(21)`. A function with no `->` does work and gives nothing back. `fn` and `return` are pink.
- `list`, `record`, `of`, and `at` each have their own editor color: clay, sage, gold, and violet.
- **Lists and records.** `list "coding", "reading"` keeps values in order, and `hobbies at 1` is the first. `record` keeps named fields, and `address.city` reads one. `for hobby in hobbies` walks a list. `at` picks one item, and `of` is required on an empty list (`list of text`). The book says where each word is used, and search finds `of` and `at`.
- **`for` loops.** `for i in 1..3` repeats the lines under it for 1, then 2, then 3. Both ends count. The name exists only inside the loop, and the loop moves it to the next number itself. Search in the book finds `for`, `when`, `say`, and the other Quill words that ordinary search skips.
- The book has a **Quill** theme: warm paper, ink, and a brass mark for the chapter you are reading. Pick it from the paintbrush menu. When your system is set to light, the book opens in Quill.
- The book uses the Quill feather as its icon, in the browser tab and beside the title.
- Quill has its own editor colors, one for each kind of word (`say`, `when`, `true` and `false`, `var` and `set`, `check`, and the logic words), the feather on `.ql` files (pale on a dark editor, deep blue on a light one), and underlines for mistakes. Colors for types and the other planned kinds are reserved until those words exist. `maybe` and failures have their colors now. Cursor and VS Code get them the first time Quill runs. Vim, Emacs, and Sublime detect `.ql` files from the `editors` folder. `quill lsp` reports the same mistakes to any editor. Set `QUILL_NO_EDITOR_SETUP=1` to leave Cursor and VS Code alone.

## v0.3.0 (13 October 2026)

- **`quill update`** shows every version of Quill in a menu: choose one with ↑ ↓ and Enter. `quill update latest` or `quill update 0.2.0` installs a version directly, and `quill upgrade` is another name for the same command.
- Quill checks for a new version **at most once a day**, in the background, and says when one is out. It never updates by itself. Set `QUILL_NO_UPDATE_CHECK=1` to turn the check off.

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
