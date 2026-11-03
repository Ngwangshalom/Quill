# {{name}}

A library: reusable functions in their own file, each tested by the examples above it, and a program that brings them in.

| File | What it holds |
|---|---|
| `money.quill` | the library: functions and their examples |
| `main.quill` | a program that uses it with `bring money` |

## Run it

```bash
quill run main.quill
```

## Test it

```bash
quill test
```

Every `--- example:` line in `money.quill` runs as a test. The examples are the documentation too: a reader sees exactly what each function gives back.

## Use it in another project

Copy `money.quill` next to your program and write `bring money` at the top, or bring it by its path: `bring "../shared/money.quill"`. Its functions are then used by name, like `with_tip(40 USD, 15)`.

A library holds functions and types only. Lines that do something, like `say`, belong in the program that brings it.

## Try next

1. Add `fn discount(price: USD, percent: number) -> USD` with two examples.
2. Make one example wrong on purpose and run `quill test` to see how Quill reports it.
3. Bring the library from a second program in another folder, by its path.

The book's chapters *Bringing in Other Files* and *Testing Your Code* explain each part. `QUILL.md` in this folder is a short tour of the language.
