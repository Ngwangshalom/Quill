# Bringing in Other Files

A small program fits in one file. A real one doesn't: the billing rules, the booking types and the program that uses them are easier to read, test and change apart. `bring` puts a program together from several files.

Say the billing rules live in their own file, `billing.quill`:

```quill,part,file=billing.quill
-- billing.quill: what a booking costs.

--- example: deposit(1000 USD) == 200 USD
fn deposit(fee: USD) -> USD
  return 20% of fee

type Invoice
  to: text
  amount: USD
```

Another file, in the same folder, brings it in and uses what it holds:

```quill,file=app.quill
bring billing

invoice = Invoice to = "Amina", amount = deposit(500 USD)
say "{invoice.to} pays {invoice.amount}"
```

```output
Amina pays 100 USD
```

`bring billing` reads `billing.quill`, the file with that name next to this one. Its functions and types are then used by name, as if they were written here. Write the name without `.quill`.

## What a brought file holds

A brought file holds **functions and types**: the pieces other files use. It can also bring files of its own.

Its tests stay with it. The `--- example:` above `deposit` runs when you test `billing.quill` with `quill test billing.quill`, not every time `app.quill` runs. That way each file proves itself, and a program that brings it doesn't repeat that work.

A brought file doesn't do anything when it is brought in: a line like `say` would run in every program that brings it, so it is a mistake:

```quill,part,file=noisy.quill
say "Loading billing..."

fn fee() -> USD
  return 10 USD
```

```quill,error,file=app.quill
bring noisy

say fee()
```

```output
Problem on line 1 of noisy.quill:

    say "Loading billing..."
    ^^^
noisy.quill is brought into app.quill, so it holds functions and types, but this line does something.
Move it into the program that brings noisy, or into a function.
```

## Files in other folders

`bring billing` looks next to the file that brings it. For a file in another folder, write its path in quotes, with `.quill`:

```quill,ignore
bring "shared/money.quill"
```

The path is counted from the folder of the file that brings it, with `/` between folders on every system.

## Common mistakes

A misspelled name is found before anything runs, and Quill suggests the file you probably meant:

```quill,error,file=app.quill
bring biling
```

```output
Problem on line 1 of app.quill:

    bring biling
          ^^^^^^
app.quill brings biling.quill, but there's no file biling.quill in this folder.
bring biling looks for biling.quill next to app.quill.

Did you mean:  bring billing
```

`bring` takes the name without `.quill`:

```quill,error,file=app.quill
bring billing.quill
```

```output
Problem on line 1 of app.quill:

    bring billing.quill
                 ^
bring names the file without .quill.
bring billing brings billing.quill. For a file in another folder, write its path in quotes: bring "shared/billing.quill".

Did you mean:  bring billing
```

Two files can't write a function or a type with the same name, because a reader couldn't tell which one a call means:

```quill,error,file=app.quill
bring billing

fn deposit(fee: USD) -> USD
  return fee
```

```output
Problem on line 3 of app.quill:

    fn deposit(fee: USD) -> USD
       ^^^^^^^
deposit is already a function, written in billing.quill on line 4.
Each function needs its own name, across every file the program brings. Rename one of them.
```

Two files can't bring each other. Move what both need into a third file, and bring that one from each.

`web`, `db`, `ai` and `http` are the names of Quill's own modules, so `bring web` brings the web module, not a file called `web.quill`.

## Try it

1. Make a folder with two files: `prices.quill` with `fn with_tax(price: USD) -> USD`, and `shop.quill` that brings `prices` and says `with_tax(100 USD)`.
2. Add an `--- example:` above `with_tax` in `prices.quill`, and run `quill test prices.quill`.
3. Misspell the name after `bring` and read what Quill suggests.
4. Add a `type Product` to `prices.quill`, and make a `Product` in `shop.quill`.
