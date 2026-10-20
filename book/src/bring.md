# Bringing in Other Files

A small program fits in one file. A real one doesn't: the billing rules, the booking types and the program that uses them are easier to read, test and change apart. `bring` puts a program together from several files.

Say the billing rules live in their own file, `billing.ql`:

```quill,part,file=billing.ql
-- billing.ql: what a booking costs.

--- example: deposit(1000 USD) == 200 USD
fn deposit(fee: USD) -> USD
  return 20% of fee

type Invoice
  to: text
  amount: USD
```

Another file, in the same folder, brings it in and uses what it holds:

```quill,file=app.ql
bring billing

invoice = Invoice to = "Amina", amount = deposit(500 USD)
say "{invoice.to} pays {invoice.amount}"
```

```output
Amina pays 100 USD
```

`bring billing` reads `billing.ql`, the file with that name next to this one. Its functions and types are then used by name, as if they were written here. Write the name without `.ql`.

## What a brought file holds

A brought file holds **functions and types**: the pieces other files use. It can also bring files of its own.

Its tests stay with it. The `--- example:` above `deposit` runs when you test `billing.ql` with `quill test billing.ql`, not every time `app.ql` runs. That way each file proves itself, and a program that brings it doesn't repeat that work.

A brought file doesn't do anything when it is brought in: a line like `say` would run in every program that brings it, so it is a mistake:

```quill,part,file=noisy.ql
say "Loading billing..."

fn fee() -> USD
  return 10 USD
```

```quill,error,file=app.ql
bring noisy

say fee()
```

```output
Problem on line 1 of noisy.ql:

    say "Loading billing..."
    ^^^
noisy.ql is brought into app.ql, so it holds functions and types, but this line does something.
Move it into the program that brings noisy, or into a function.
```

## Files in other folders

`bring billing` looks next to the file that brings it. For a file in another folder, write its path in quotes, with `.ql`:

```quill,ignore
bring "shared/money.ql"
```

The path is counted from the folder of the file that brings it, with `/` between folders on every system.

## Common mistakes

A misspelled name is found before anything runs, and Quill suggests the file you probably meant:

```quill,error,file=app.ql
bring biling
```

```output
Problem on line 1 of app.ql:

    bring biling
          ^^^^^^
app.ql brings biling.ql, but there's no file biling.ql in this folder.
bring biling looks for biling.ql next to app.ql.

Did you mean:  bring billing
```

`bring` takes the name without `.ql`:

```quill,error,file=app.ql
bring billing.ql
```

```output
Problem on line 1 of app.ql:

    bring billing.ql
                 ^
bring names the file without .ql.
bring billing brings billing.ql. For a file in another folder, write its path in quotes: bring "shared/billing.ql".

Did you mean:  bring billing
```

Two files can't write a function or a type with the same name, because a reader couldn't tell which one a call means:

```quill,error,file=app.ql
bring billing

fn deposit(fee: USD) -> USD
  return fee
```

```output
Problem on line 3 of app.ql:

    fn deposit(fee: USD) -> USD
       ^^^^^^^
deposit is already a function, written in billing.ql on line 4.
Each function needs its own name, across every file the program brings. Rename one of them.
```

Two files can't bring each other. Move what both need into a third file, and bring that one from each.

`web`, `db`, `ai` and `http` are the names of Quill's own modules, so `bring web` brings the web module, not a file called `web.ql`.

## Try it

1. Make a folder with two files: `prices.ql` with `fn with_tax(price: USD) -> USD`, and `shop.ql` that brings `prices` and says `with_tax(100 USD)`.
2. Add an `--- example:` above `with_tax` in `prices.ql`, and run `quill test prices.ql`.
3. Misspell the name after `bring` and read what Quill suggests.
4. Add a `type Product` to `prices.ql`, and make a `Product` in `shop.ql`.
