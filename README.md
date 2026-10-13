<p align="center">
  <img src="assets/logo/quill-256.png" alt="Quill logo" width="128" height="128">
</p>

<h1 align="center">Quill</h1>

<p align="center">
  <strong>Write what you mean.</strong><br>
  A readable, safe programming language for backends and AI-powered apps.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/status-pre--alpha-orange" alt="Status: pre-alpha">
  <img src="https://img.shields.io/badge/built%20with-Rust-b7410e" alt="Built with Rust">
  <img src="https://img.shields.io/badge/license-Apache%202.0-blue" alt="License: Apache 2.0">
  <img src="https://img.shields.io/badge/PRs-welcome-brightgreen" alt="PRs welcome">
</p>

---

> ⚠️ **Quill is in early development.** The design is being written down, and the interpreter runs only the first few features so far (`say`, values, `check` and doc examples). Nothing here is ready for production. If you care about how a language should feel, this is the best moment to help shape it.

## Why Quill?

Quill is not trying to invent a new way of programming. It's trying to make the programming people already do **dramatically less painful**.

Today, building a backend means stitching together a framework, an ORM, a validation library, a test runner and a pile of dependencies. Then production goes down anyway, because of a `null` nobody checked, a failure nobody handled, or a price that rounded wrong.

Quill's answer is a language where:

- **Code reads like what it means.** Indentation-based, no semicolons, no braces.
- **Nothing is missing by surprise.** There is no `null`. Values that might not exist say so, and the checker makes you handle them.
- **Failures are part of the signature.** A function declares how it can fail, and callers must deal with it.
- **Money is exact.** `Money<AED>` and `Decimal` are built in. `0.1 + 0.2` is always `0.3`, and you can't add dirhams to dollars by accident.
- **Programs only touch what they're allowed to.** Network, file and system access must be granted with `permit`.
- **Errors teach.** Every error explains what went wrong, why, and how to fix it. `quill explain` shows exactly how the compiler read your code.
- **Tests and docs live with the code.** `check` lines are tests, and examples in doc comments run as tests too.
- **Batteries are included.** HTTP, database access, JSON and validation ship with the language.

None of these ideas is new on its own. Quill's goal is to make them all work together, consistently, in one language that a beginner can read and a team can trust.

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

Building from source needs [Rust](https://rustup.rs): `cargo install --git https://github.com/Ngwangshalom/Quill`

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
fn fizzbuzz(n: Int) -> Text
  when
    n % 15 == 0 -> "FizzBuzz"
    n % 3 == 0  -> "Fizz"
    n % 5 == 0  -> "Buzz"
    else        -> text(n)

for i in 1..15
  say fizzbuzz(i)

check fizzbuzz(9)  == "Fizz"
check fizzbuzz(15) == "FizzBuzz"
```

`when` follows one simple rule: matching on a fixed set of cases (like `maybe` or failures) must cover every case, and matching on numbers or text must end with `else`.

### No null: `maybe`

```quill
fn find_user(id: Int) -> maybe User

when find_user(42)
  found user -> say "Hello, {user.name}"
  missing    -> say "No such user"
```

Forget the `missing` branch and Quill refuses to run.

### Failures you can't ignore

```quill
fn charge(card: Card, amount: Money<AED>) -> Receipt
  fails Declined, NetworkDown

receipt = charge(card, 250 AED)
  on Declined    -> respond 402, "Card declined"
  on NetworkDown -> respond 503, "Payments are unavailable, try again"
```

Inside a function that declares the same failures, `try` passes them upward explicitly:

```quill
fn checkout(cart: Cart) -> Receipt
  fails Declined, NetworkDown
  return try charge(cart.card, cart.total)
```

### Constrained types and exact money

```quill
type Booking
  venue: Text
  guests: Int where 1..500
  deposit: Money<AED>

total = 100 AED + 50 AED     -- 150 AED
mixed = 100 AED + 20 USD     -- error: can't add AED and USD without converting
```

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
| `quill test` | Run every `check` and doc example in the project |
| `quill explain file.ql:LINE` | Show how the compiler read a line |
| `quill new NAME` | Start a new project |

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
- [ ] Language specification (v0.1 draft)
- [ ] Lexer and parser
- [ ] `say`, values, `var` / `set`, expressions
- [ ] `when`, loops and functions

**Safety**
- [ ] Type checker with friendly errors
- [ ] `maybe` and exhaustive `when`
- [ ] `fails`, `on` and `try`
- [ ] `quill explain`

**Running**
- [ ] Bytecode compiler and virtual machine
- [ ] `check` tests and executable doc examples
- [ ] `Decimal` and `Money<CURRENCY>`
- [ ] Constrained types (`where`)
- [ ] `require` / `ensure` contracts (runtime)

**Ecosystem**
- [ ] Permissions (`permit`)
- [ ] `web` module: HTTP server and routing
- [ ] `db` module: PostgreSQL and SQLite
- [ ] AI module: type-checked model calls
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
