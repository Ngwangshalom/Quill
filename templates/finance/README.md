# {{name}}

An invoice with exact money: lines, a subtotal, VAT rounded the way banks round, and the total shared into three payments that add up to the cent.

| File | What it holds |
|---|---|
| `main.quill` | one invoice, worked out and shown |
| `billing.quill` | the types and rules, each with tests |

## Run it

```bash
quill run main.quill
```

## Test it

```bash
quill test
```

The `--- example:` lines in `billing.quill` and the `check` in `main.quill` are the tests.

## Why the money is right

- `0.1 + 0.2` is exactly `0.3` in Quill, and `100 USD + 20 EUR` is a mistake, caught before the program runs.
- `round ... to cents` rounds half to even, as banks do.
- `split due into 3` gives three amounts in whole cents that always add up to `due`.
- `require` and `ensure` on `vat` say what has to be true. If a change ever breaks them, the program stops and names the values.

## Try next

1. Add a line to the invoice.
2. Give a 10% discount on invoices over 1000 USD, with an example that proves it.
3. Change the VAT rate, and update the examples so the tests pass.

The book's chapters *Money* and *Contracts* explain each part. `QUILL.md` in this folder is a short tour of the language.
