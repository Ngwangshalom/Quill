# {{name}}

A bank account that can never go below zero. It shows what Quill is built for: the rules are part of the code, and a failure can't be ignored.

| File | What it holds |
|---|---|
| `main.quill` | one account, with money paid in and out |
| `bank.quill` | `type Account` and the rules, with tests |

## Run it

```bash
quill run main.quill
```

## Test it

```bash
quill test
```

## How the rules hold

- `require amount > 0 USD` means no one can deposit or withdraw zero or less. A call that tries stops the program and names the line that broke it.
- `ensure result.balance >= 0 USD` is the promise that a balance never goes below zero, checked each time `withdraw` gives an account back.
- `fails NotEnoughMoney` says `withdraw` can fail. Every caller must handle it with `on`, or Quill won't run the program. In `main.quill`, `on NotEnoughMoney -> account` keeps the account as it was.

## Try next

1. Add `fn transfer(from: Account, to: Account, amount: USD)` that withdraws from one and deposits into the other.
2. Try `deposit(account, 0 USD)` and read what Quill says.
3. Add a daily limit with `require amount <= 2000 USD` on `withdraw`.

The book's chapters *Contracts*, *Failures*, and *Money* explain each part. `QUILL.md` in this folder is a short tour of the language.
