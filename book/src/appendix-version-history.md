# Appendix C: Version History

What changed in each version of Quill. The newest version is at the top.

## Unreleased

Changes that are finished but not yet in a release.

- `quill test shop` tests one folder: it runs the files in `shop` and its subfolders that contain a `check` or `--- example:`. Before, naming a folder was refused as a path without `.ql`.
- `try` on a line by itself, like `try write_file(...)`, explains that `try` goes where a call's answer is used, and shows `on CantWrite -> fail CantWrite` for passing a failure on from a call on its own line. Before, Quill suggested `say try ...`, which can't work for a call that gives nothing back.
- **Web services.** `bring web`, then `route GET "/hello/{name}"` and `respond 200, "Hello, {name}!"`, turn a program into a web service. `{name}` parts of the path are values, `request` holds the method, path, query and body, and a record is sent as JSON. `quill run` serves on `PORT` (8080). Unknown paths get 404, the wrong method 405, and a route that stops 500, with the problem shown and the server still running. `route` and `respond` are mist, like `bring`.
- **Asking an AI model.** `bring ai`, then `ask "Summarize: {text}"` gives back the model's answer, and `ask "..." as Review` gives back a `Review`: the model is sent the type's shape, and the answer is checked against every field and `where` range. `ModelFailed` and `Invalid` are handled like any failure. `permit env "ANTHROPIC_API_KEY"` chooses Claude (`claude-opus-5-5` unless `QUILL_AI_MODEL` says otherwise); `permit env "OPENAI_API_KEY"` chooses any OpenAI-compatible service, including one on your own computer. `ask` is spark.
- **Web requests.** `bring http` adds `fetch(url)`, `post(url, body)` and `send(method, url, body, headers)`. Each needs `permit net` for the host, gives back an answer with `status` and `body`, and fails with `NetworkDown` when the service can't be reached. A `404` or `500` is still an answer, so the program decides.
- **JSON.** `body as Booking` reads JSON text as any kind, checking every field, its kind, and its `where` range; money is read exactly, even from text like `"500.00"`. Text that doesn't fit fails with `Invalid`, handled with `on Invalid` or `try`. `json(value)` writes any value as JSON. `as` is steel, like `type`.
- **Permits, files and settings.** `permit read "config"`, `permit write "reports"`, `permit env "STRIPE_KEY"` and `permit net "api.stripe.com"` say what a program may touch; anything else is refused. `read_file`, `write_file`, `file_exists` and `env` are Quill's own functions, and their failures (`FileMissing`, `CantWrite`) are handled with `on` or `try`. A path or name written in the call is checked before running, and `..` can't leave a permitted folder. A brought file can't give itself permits. `permit` is rust.
- **`bring` puts a program together from files.** `bring billing` reads billing.ql next to the file, and `bring "shared/money.ql"` reads a file by its path. Its functions and types are used by name; its own tests run when that file is tested. A problem in a brought file names that file. Misspelled names suggest the file you meant; files that bring each other in a circle, a brought file that does something, and a name written in two files are explained. `bring` is mist blue, the color saved for modules.
- **Money that never loses a cent.** `round total to cents` rounds half to even, as banks do; `round up` and `round down` choose a direction, and `round x to 2 places` works on numbers. `split 100 USD into 3` gives `33.34 USD, 33.33 USD, 33.33 USD`, and `split fee by list 70, 30` shares by weights; the parts always add up. `20% of fee` takes a percent. `convert 100 USD to EUR at 0.92` changes currency at a rate you write, and adding two currencies now suggests it. Money is shown in whole units or with every cent (`90 USD`, `7.50 USD`), and Quill knows currencies without cents, like XAF and JPY, and with three digits, like KWD. `round`, `split` and `convert` are mint.
- **Types.** `type Booking`, then one field per line, like `guests: number where 1..500`, names what every booking has. `Booking venue = "Town Hall", guests = 120, fee = 500 USD` makes one; every field must be there and be the right kind, and each field's range is checked as it is made. A type works as a kind (`b: Booking`, `-> Booking`, `list of Booking`), and a function that takes one checks the type's ranges. A misspelled field is reported once. `type` is steel, like `where`.

## v0.4.0 (20 October 2026)

- **Contracts.** `require amount > 0 USD` under `fn` says what has to be true when the function is called. `ensure result >= 0 USD` says what is true each time it gives back, and `result` is the value it gives back. A broken one stops the program, names the values it read, and says whose mistake it was; a broken `require` names the line of the call. `require`, `ensure`, and `fails` go at the top of the function in any order. `quill explain` describes contract lines. `require`, `ensure`, and `result` inside an `ensure` are brass (`#C8A06A` on a dark editor, `#7A5A20` on a light one).
- **Ranges with `where`.** `fn book(guests: number where 1..500)` takes 1 through 500 guests, both ends included. Money works too: `amount: USD where 1 USD..10000 USD`. A written value outside the range, like `book(600)`, is caught before the program runs; any other value stops the call and names the value and the line of the call. Both ends are written values of the parameter's kind, smallest first. `quill explain` describes the range. `where` is steel (`#6AA8C8` on a dark editor, `#1A5270` on a light one), the color saved for rules on types.
- **Filtering with `where`.** `students where score >= 50` keeps the records whose condition is true, like Laravel's `where`. Inside the condition a field is read by its name. It works inside `get` and `sum` (`sum fee of roster where paid == false`), after `for ... in`, and again after itself (`where score >= 50 where paid`). No match gives an empty list. A misspelled field names the record's fields.
- **Asking for data.** `sum fee of roster` adds the fee of each record. `get fee of roster` takes that field as a list. `get name, age, fee of roster` keeps those fields together on each record. The same line reads one record: `get name, fees of school` is one record with those fields. `sum fees of school` is that one amount. A dot after `of` reads a field further in: `get place of desk.bookings`, `get gpa of school.acceptances`. Name the record before the dot: `first = desk.bookings at 1`, then `first.place`. `get bookings with acts in roster` is the longer form, for a list and the field embedded in it. A missing amount is left out of a single field. `for` remains when each record needs its own step. `get`, `sum`, and `with` are sea (`#6EC1D6` on a dark editor, `#15556A` on a light one).
- **`when` gives a value.** `delivery = when`, then indented cases like `total > 100 -> 0`, ending with `else -> 10`. The first true case gives the value. It works after `=`, `set`, `say`, and `return`. `else` is required, and every case gives the same kind of value. `quill explain` describes each case.
- **`on` can carry on.** `on Declined -> "declined"` uses that value in place of the call's answer, and the program continues. `on Declined -> say "..."` still runs and then stops. Each `on` line chooses for itself.
- **Examples above a function test it.** `--- example: with_vat(100 USD) == 105 USD` directly above `fn with_vat` runs once the function is written, so a function's documentation and its tests sit together above it. A failed example says "An example", not "A example".
- `quill test FILE.ql` runs one file. Test reports count executed checks, including functions, loops, and passes before a failure.
- Remainder overflow reports a Quill arithmetic problem instead of crashing.
- The README marks the type checker and contracts as implemented.
- The README writes money as `USD`, the way Quill does, instead of `Money<USD>`. Contracts stay `require` and `ensure`. Its error and `quill explain` examples show Quill's real output.
- Money examples use dollars. `100 USD + 50 USD` is `150 USD`.
- Quill's public description is this: intent is clear, failures are explicit, and correctness is part of the code. It is for backend engineering, financial software, automation, reliable business applications, and AI and machine learning. Asking a model and training one are part of that. Operating systems, device drivers, and 3D games stay with Rust and C++.

## v0.3.1 (14 October 2026)

- **Indentation is exact.** A block moves in by 2 spaces. A tab, or any other width, is a mistake.
- A range is two dots. `18...25` is a mistake, and Quill tells you to write `18..25`. `->` belongs inside a when, so `say` takes the text directly.
- **Money.** `100 USD + 50 USD` is `150 USD`. Two currencies cannot be added. A function writes the currency as the kind: `price: USD`.
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
