# Contracts

A bank never lets you withdraw a negative amount, and it never leaves you owing money after a withdrawal. Those are rules about the function itself. In Quill, you write them at the top of the function, where every reader sees them first.

```quill
fn withdraw(balance: USD, amount: USD) -> USD
  require amount > 0 USD
  ensure result >= 0 USD
  return balance - amount

say withdraw(100 USD, 30 USD)
```

```output
70 USD
```

`require` says what has to be true **when the function is called**. Here, the amount has to be more than zero.

`ensure` says what is true **when the function gives back**. `result` is the value it gives back. Here, the balance that is left is never below zero.

Together they are the function's contract: what it needs, and what it promises. When both are true, nothing happens and the program carries on. The rules cost one line each, and they don't change what the function does.

## When a require is broken

If the caller passes a value the function doesn't accept, the program stops at the `require` line. Quill says what the value was, and which call passed it:

```quill,error,file=bank.quill
fn withdraw(balance: USD, amount: USD) -> USD
  require amount > 0 USD
  ensure result >= 0 USD
  return balance - amount

say withdraw(100 USD, -5 USD)
```

```output
Problem on line 2 of bank.quill:

      require amount > 0 USD
      ^^^^^^^^^^^^^^^^^^^^^^
withdraw requires amount > 0 USD, but amount is -5 USD.
The call on line 6 gave withdraw a value it doesn't accept. Check the value before calling withdraw.
```

A broken `require` is the caller's mistake. `withdraw` never ran a line of its own work, so nothing was taken from the balance.

## When an ensure is broken

If the function is about to give back a value that breaks its own promise, the program stops at the `ensure` line instead:

```quill,error,file=bank.quill
fn withdraw(balance: USD, amount: USD) -> USD
  require amount > 0 USD
  ensure result >= 0 USD
  return balance - amount

say withdraw(10 USD, 30 USD)
```

```output
Problem on line 3 of bank.quill:

      ensure result >= 0 USD
      ^^^^^^^^^^^^^^^^^^^^^^
withdraw ensures result >= 0 USD, but result is -20 USD.
The mistake is inside withdraw: it was about to give back a value that breaks its own ensure.
```

A broken `ensure` is the function's own mistake. Here `withdraw` let someone take more than they had. One way to fix it is a second `require`, so the caller has to check first:

```quill
fn withdraw(balance: USD, amount: USD) -> USD
  require amount > 0 USD
  require amount <= balance
  ensure result >= 0 USD
  return balance - amount

say withdraw(100 USD, 100 USD)
```

```output
0 USD
```

Another way is a [failure](failures.md), when running out of money is something the caller should handle and carry on from:

```quill
fn withdraw(balance: USD, amount: USD) -> USD
  require amount > 0 USD
  ensure result >= 0 USD
  fails InsufficientFunds
  when
    amount > balance -> fail InsufficientFunds
  return balance - amount

left = withdraw(100 USD, 30 USD)
  on InsufficientFunds -> say "Not enough money"
say left
```

```output
70 USD
```

## A range on a parameter: where

Many rules are a range: a number of guests from 1 to 500, a score from 0 to 100, a payment that is never more than the limit. Write those right on the parameter with `where`:

```quill
fn book(guests: number where 1..500) -> text
  return "Booked for {guests} guests"

say book(120)
```

```output
Booked for 120 guests
```

`where 1..500` means from 1 through 500, including both ends. It works the same way as `require guests >= 1 and guests <= 500`, but it sits next to the parameter, so a reader sees the rule and the kind together.

Money works too. Both ends are amounts in the parameter's currency:

```quill
fn pay(amount: USD where 1 USD..10000 USD) -> USD
  return amount

say pay(250 USD)
```

```output
250 USD
```

When the call passes a value written right there, like `book(600)`, Quill catches it before the program runs:

```quill,error,file=hall.quill
fn book(guests: number where 1..500) -> text
  return "Booked for {guests} guests"

say book(600)
```

```output
Problem on line 4 of hall.quill:

    say book(600)
             ^^^
book takes guests from 1 through 500, but this call gives 600.
Line 1 says guests where 1..500. Give book a value in that range.
```

When the value is only known while the program runs, the call stops at the rule, the same way a broken `require` does:

```quill,error,file=hall.quill
fn book(guests: number where 1..500) -> text
  return "Booked for {guests} guests"

guests = 200 + 400
say book(guests)
```

```output
Problem on line 1 of hall.quill:

    fn book(guests: number where 1..500) -> text
                           ^^^^^^^^^^^^
book takes guests from 1 through 500, but guests is 600.
The call on line 5 gave book a value outside that range. Check the value before calling book.
```

Use `where` for a range, and `require` for every other rule: one parameter compared to another (`require amount <= balance`), text that must not be empty, or a number that only has a lower limit (`require amount > 0 USD`).

## Where contracts help

Add a contract when a wrong value would quietly give a wrong answer instead of an error. Some places where that happens:

- **Money.** A payment above zero, a balance that never goes below zero, a discount that is never more than the price.
- **Dividing.** A count that is more than zero before you divide by it.
- **Ranges.** A score from 0 to 100, a quantity from 1 to 50, a percentage from 0 to 100. Write these with `where`.
- **Promises about the answer.** A grade that is never empty, a total that is never less than its first item.

Here is a school that keeps all three kinds of rule:

```quill
fn average(total: number, count: number) -> number
  require count > 0
  ensure result >= 0
  return total / count

fn pay_fees(owed: USD, paid: USD) -> USD
  require paid > 0 USD
  require paid <= owed
  ensure result >= 0 USD
  return owed - paid

fn grade(score: number where 0..100) -> text
  ensure result != ""
  when
    score >= 70 -> return "A"
    score >= 50 -> return "Pass"
  return "Fail"

say average(240, 3)
say pay_fees(500 USD, 200 USD)
say grade(85)
say grade(40)
```

```output
80
300 USD
A
Fail
```

Without `require count > 0`, `average(240, 0)` would stop with a divide-by-zero deep inside the function. With it, Quill says which call passed the `0`. Without `require paid <= owed`, `pay_fees(500 USD, 900 USD)` would give back `-400 USD`, and the school would owe the student money.

A contract is worth writing when you can say the rule in one short sentence. If you can't, the rule probably belongs in the function's work, or in a [failure](failures.md).

## A contract, a failure, or a check?

They look alike, but each one answers a different question:

| Word | Who is wrong when it happens | What happens |
|---|---|---|
| `require` | the caller passed a value the function doesn't accept | the program stops |
| `where` | the caller passed a value outside the parameter's range | Quill stops before running, or the program stops |
| `ensure` | the function broke its own promise | the program stops |
| `fails` | nobody: it is a normal outcome, like a declined card | the caller handles it with `on` |
| `check` | a test found a wrong answer | `quill test` reports it |

Use `fails` when the caller can do something about it. Use `require` and `ensure` for things that should never happen in a correct program.

## Writing contracts

- `require`, `ensure`, and `fails` go at the top of the function, right under the `fn` line, in any order. A function can have as many `require` and `ensure` lines as it needs.
- Each one holds one condition that is true or false. Join two with `and`, or write two lines.
- `require` reads the parameters. `ensure` reads the parameters and `result`.
- An `ensure` is checked at every `return`, so a function with three `return` lines keeps its promise on all three.
- `result` exists only in `ensure`. Inside the function, `result` is an ordinary name you can use for something else.
- `where` goes after a parameter's kind: `guests: number where 1..500`. It works on a number or a currency, both ends are written values of that kind, and the smaller one comes first. Each `where` is checked when the function is called, before its `require` lines.

## Common mistakes

`ensure` on a function that gives nothing back has no `result` to check:

```quill,error
fn greet(name: text)
  ensure name != ""
  say "Hello, {name}"
```

```output
Problem on line 2 of example.quill:

      ensure name != ""
      ^^^^^^^^^^^^^^^^^
ensure promises something about the value greet gives back, but greet gives nothing back.
Add -> and a kind to the fn line, or use require for what has to be true when greet is called.
```

A `require` can't read `result`, because it is checked before there is one:

```quill,error
fn double(n: number) -> number
  require result > 0
  return n * 2
```

```output
Problem on line 2 of example.quill:

      require result > 0
      ^^^^^^^^^^^^^^^^^^
result is the value double gives back, and it doesn't exist yet when double is called.
Check result with ensure instead of require.
```

A contract goes at the top. After the function has started its work, it is too late:

```quill,error
fn double(n: number) -> number
  say n
  require n > 0
  return n * 2
```

```output
Problem on line 3 of example.quill:

      require n > 0
      ^^^^^^^
require goes at the top of a function, right under the fn line.
require says what has to be true when the function is called, and ensure says what is true when it gives back. Move this line up, above the function's other lines.
```

A `where` range is written smallest first:

```quill,error
fn grade(score: number where 100..0) -> text
  return "A"
```

```output
Problem on line 1 of example.quill:

    fn grade(score: number where 100..0) -> text
                           ^^^^^^^^^^^^
score where 100..0 starts above where it ends, so no value fits it.
Write the smallest value first, then .., then the largest.

Did you mean:  where 0..100
```

Both ends of the range have the same kind as the parameter. For money, that means writing the currency on each end:

```quill,error
fn pay(amount: USD where 0..100 USD) -> USD
  return amount
```

```output
Problem on line 1 of example.quill:

    fn pay(amount: USD where 0..100 USD) -> USD
                             ^
amount is money in USD, so each end of its range is too, but 0 is a number.
A rule can only compare values of the same kind.

Did you mean:  0 USD
```

`quill explain` shows how Quill reads a contract line. On the `ensure` line of `withdraw`, it says `result` is "the value withdraw gives back". On a `fn` line with `where`, it says when the range is checked.

## Try it

1. Write `fn split_bill(total: USD, people: number) -> USD` that divides a bill. Add `require people > 0`, then call it with `0` people and read what Quill says.
2. Add `ensure result <= total` to `split_bill`. Call it with `1` person, then with `4`.
3. Write a function with two `return` lines and one `ensure`. Make one of the returns break the promise, and see which line Quill points to.
4. Change `split_bill` so `people` is `number where 1..20`. Call it with `25`, then with a value worked out while the program runs, like `10 + 15`. Compare the two messages.
