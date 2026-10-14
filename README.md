<p align="center">
  <img src="assets/logo/quill-256.png" alt="Quill logo" width="128" height="128">
</p>

<h1 align="center">Quill</h1>

<p align="center">
  <strong>Write what you mean.</strong><br>
  A readable, safe language for backends and AI development.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/status-pre--alpha-orange" alt="Status: pre-alpha">
  <img src="https://img.shields.io/badge/built%20with-Rust-b7410e" alt="Built with Rust">
  <img src="https://img.shields.io/badge/license-Apache%202.0-blue" alt="License: Apache 2.0">
  <img src="https://img.shields.io/badge/PRs-welcome-brightgreen" alt="PRs welcome">
</p>

---

> ⚠️ **Quill is in early development.** The design is being written down, and the interpreter runs the first features so far: `say`, values, `var` / `set`, exact arithmetic, `{values}` inside text, `when`, `for` loops, lists, records, `check` and doc examples, functions (`fn`, `return`), money (`100 AED`), `maybe`, and failures (`fails`, `on`, `try`). Nothing here is ready for production. If you care about how a language should feel, this is the best moment to help shape it.

## Why Quill?

Quill is for two kinds of work. One is backends: the services behind a product, such as an account lookup, an order, a charge, or a job that can fail. The other is AI development: calling a model, handling a missing answer, and handling a call that failed. The code should read like those rules, and a beginner should be able to learn it. Banks and payments are one place the rules matter. A shop, a school, a clinic, and an AI product are others.

Today Quill can run those rules on their own. It cannot yet be the server around them, and it cannot yet call a model. There is no web server, no database, and no network. `permit`, HTTP, database access, and the AI module are designed and not built. That is the path to a backend you can deploy, and to AI calls the checker can read.

What works now:

- **Code reads like what it means.** Indentation-based, no semicolons, no braces.
- **Money is exact.** `100 AED + 50 AED` is `150 AED`. `0.1 + 0.2` is `0.3`. Dirhams and dollars cannot be added.
- **Nothing is missing by surprise.** There is no `null`. A `maybe` says the value might be missing, and both cases have to be handled.
- **Failures are part of the function.** `fails Declined` names the failure, `fail Declined` leaves with it, and the caller handles it with `on` or passes it on with `try`.
- **Errors teach.** Every error explains what went wrong, why, and how to fix it. `quill explain` shows how Quill read the code.
- **Tests live with the code.** `check` lines are tests, and examples in doc comments run as tests too.

None of these ideas is new on its own. Quill's goal is to make them work together, in one language that a beginner can read and a backend or AI team can trust.

## Install

**macOS and Linux:**

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Ngwangshalom/Quill/releases/latest/download/quill-installer.sh | sh
```

**Windows (PowerShell):**

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/Ngwangshalom/Quill/releases/latest/download/quill-installer.ps1 | iex"
```

Or download a build for your system from the [Releases page](https://github.com/Ngwangshalom/Quill/releases). Then try it:

```bash
quill new hello
quill run hello/hello.ql
```

To update later, run `quill update` and choose a version with the arrow keys. Quill also tells you when a new version is out (set `QUILL_NO_UPDATE_CHECK=1` to turn that off).

Building from source needs [Rust](https://rustup.rs): `cargo install --git https://github.com/Ngwangshalom/Quill`

## Learn Quill

**[The Quill Programming Language](book/src/SUMMARY.md)** is a book that teaches Quill from the very beginning, for new and experienced programmers. It grows with the language, and every example in it is tested.

The first time you run Quill, Cursor and VS Code get Quill's own colors, the feather on every `.ql` file, and underlines for mistakes. The feather is pale on a dark editor and deep blue on a light one. Each kind of word has its own color: `say` is nib orange `#F0A35E`, `when` is feather blue `#7EAEF5`, and `true` is green `#5ECA8C`. Vim, Emacs, and Sublime detect `.ql` files from [`editors`](editors). `quill lsp` sends the same mistakes to any editor. Set `QUILL_NO_EDITOR_SETUP=1` to leave Cursor and VS Code alone.

## A tour of Quill

### Hello, World

```quill
say "Hello, World!"
```

### Values and changes

`=` declares a value. Values can't change unless they're marked `var`, and changing one takes `set`, so every mutation is visible when you read the code.

```quill
name = "Amina"
var visits = 0

set visits = visits + 1
set name = "Omar"        -- error: name isn't marked var
set vistis = 2           -- error: no value called vistis (did you mean visits?)
```

Text that contains a quote is wrapped in three quotes, so the quote inside needs no backslash:

```quill
quoted = """She said "hi"."""
```

### Functions, `when` and tests

```quill
fn double(n: number) -> number
  return n * 2

say double(21)

fn greet(name: text)
  say "Hello, {name}"

greet("Amina")
```

A parameter's kind is `text`, `number`, `true/false`, or `list of` one of those. Kind names such as `Int` and `Text`, and a `when` that gives a value back, come later.

`when` follows one simple rule: matching on a fixed set of cases (like `maybe` or failures) must cover every case, and matching on numbers or text must end with `else`.

### No null: `maybe`

```quill
fn find_price(name: text) -> maybe number
  when
    name == "bread" -> return found 2.50
    else -> return missing

when find_price("bread")
  found price -> say price
  missing -> say "none"
```

Forget the `missing` branch and Quill refuses to run. Named types such as `User` arrive later.

### Failures you can't ignore

```quill
fn charge(amount: AED) -> text
  fails Declined, NetworkDown

var receipt = charge(250 AED)
  on Declined -> say "Card declined"
  on NetworkDown -> say "Try again"
```

Inside a function that declares the same failures, `try` passes them upward explicitly:

```quill
fn checkout(amount: AED) -> text
  fails Declined, NetworkDown
  return try charge(amount)
```

`respond` arrives with the web module. Today the `on` line is a statement, such as `say`.

### Constrained types and exact money

```quill
total = 100 AED + 50 AED
```

That is `150 AED`. `100 AED + 20 USD` is a mistake: Quill will not mix currencies. Named types such as `Money<AED>` and `where` arrive later. Today the currency itself is the kind: `amount: AED`.

### Contracts

Functions can state what they require and what they guarantee. Quill checks these at runtime today and proves them before running wherever it can.

```quill
fn withdraw(account: Account, amount: Money<AED>) -> Account
  require amount > 0 AED
  ensure result.balance >= 0 AED
  fails InsufficientFunds
```

### Permissions

A program can only reach the network, files or system resources it has been granted.

```quill
permit net "api.stripe.com"
permit read "./config"
```

Anything else is refused with a clear error, so a compromised dependency can't quietly send your data somewhere.

### Executable documentation

Examples in doc comments are real tests, so documentation can't drift out of date.

```quill
--- Adds 5% VAT to a price.
--- example: with_vat(100 AED) == 105 AED
fn with_vat(price: Money<AED>) -> Money<AED>
  return price * 1.05
```

### A small web API

```quill
use web
use db

permit net "localhost:5432"

type Booking
  id: Int
  venue: Text
  guests: Int where 1..500

route GET "/bookings/{id}"
  when db.find(Booking, id)
    found b -> respond 200, b
    missing -> respond 404, "Booking not found"

route POST "/bookings"
  booking = request.body as Booking
    on Invalid err -> respond 400, err.message
  respond 201, db.save(booking)
```

## Errors that teach

```
Problem on line 4 of shop.ql:

    total = price + "5"
                    ^^^
You're adding Money<AED> (price) to Text ("5").
Quill won't guess which one you meant.

Did you mean:  total = price + 5 AED
```

And when you want to see how Quill understood your code:

```
$ quill explain shop.ql:4

Line 4:  total = price + "5"

  total   a new value (can't change; not marked var)
  price   Money<AED>, declared on line 2
  "5"     Text
  +       Money<AED> + Text is not defined

Quill stopped here before running anything.
```

## The `quill` tool

| Command | What it does |
|---|---|
| `quill run file.ql` | Run a program |
| `quill check file.ql` | Find errors without running anything |
| `quill lsp` | Underline those errors in an editor |
| `quill test` | Run every `check` and doc example in the project |
| `quill explain file.ql:LINE` | Show how the compiler read a line |
| `quill new NAME` | Start a new project |
| `quill update` | Choose a version of Quill to install (also `quill upgrade`) |

## Language specification

[docs/spec.md](docs/spec.md) defines exactly what Quill programs mean today: values and their kinds, exact numbers, operators, statements, `when`, tests and errors. This README shows the full design, including features that are still planned.

## How it works

Quill is implemented in **Rust**, which gives it a single-binary install, a fast virtual machine, memory safety, and a WebAssembly build for an in-browser playground.

```
source (.ql) → lexer → parser → checker → bytecode compiler → virtual machine
```

The checker is where Quill earns its keep: type mismatches, unhandled cases, missing values, undeclared failures and broken contracts are caught before anything runs.

## What Quill is not

Being clear about scope is part of the design. Quill is **not** aimed at operating systems, device drivers, 3D games or training machine-learning models. Rust, C++ and Python serve those well. Quill is for backends, APIs, scripts, automation and AI-powered applications, where clarity and correctness matter more than squeezing out the last nanosecond.

## Roadmap

**Foundation**
- [x] [Language specification](docs/spec.md) (v0.1 draft)
- [x] Lexer and parser
- [x] `say`, values, `var` / `set`, expressions
- [x] `when` with conditions
- [x] Loops (`for i in 1..3`, and `for hobby in hobbies`)
- [x] Lists and records
- [x] Functions (`fn double(n: number) -> number`, `return`, and calls)

**Safety**
- [ ] Type checker with friendly errors
- [x] `maybe`, `found`, `missing`, and exhaustive `when`
- [x] `fails`, `fail`, `on` and `try`
- [x] `quill explain`

**Running**
- [x] Bytecode compiler and virtual machine
- [x] `check` tests and executable doc examples
- [x] Exact `Decimal` numbers
- [x] Money (`100 AED + 50 AED`; mixed currencies are a mistake)
- [ ] Constrained types (`where`)
- [ ] `require` / `ensure` contracts (runtime)

**Ecosystem**
- [ ] Permissions (`permit`)
- [ ] `web` module: HTTP server and routing
- [ ] `db` module: PostgreSQL and SQLite
- [ ] AI module: type-checked model calls
- [x] Syntax highlighting for VS Code and Cursor ([editors/vscode](editors/vscode))
- [ ] Browser playground (WebAssembly)
- [ ] Package manager

**Later**
- [ ] Safe concurrency
- [ ] Static verification of contracts

## Contributing

Quill is built in the open, and contributions of every size are welcome: code, documentation, examples, bug reports, and especially opinions on the language design while it's still flexible.

1. Read [CONTRIBUTING.md](CONTRIBUTING.md)
2. Look for issues labeled [`good first issue`](../../labels/good%20first%20issue)
3. Join the conversation in [Discussions](../../discussions)

Please follow our [Code of Conduct](CODE_OF_CONDUCT.md) in all project spaces.

## License

Quill is licensed under the [Apache License 2.0](LICENSE).
