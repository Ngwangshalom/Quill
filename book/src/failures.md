# Failures

A payment can be declined. The function says so, and the caller has to decide what happens. Quill will not let the failure disappear.

```quill
fn charge(amount: USD) -> text
  fails Declined
  when
    amount <= 0 USD -> fail Declined
    else -> return "paid"

var receipt = charge(250 USD)
  on Declined -> say "Card declined"

say receipt
```

```output
paid
```

`fails Declined` goes at the top of the function, right under `fn`. A failure's name starts with a capital letter. `fail Declined` leaves the function with that failure. It does not give the text back.

Under the call, `on Declined` says what to do. Every failure named in `fails` needs its own `on`. When the call succeeds, Quill skips the `on` lines and continues. `receipt` holds `"paid"`, and the next line says it.

When the call fails, the `on` line runs and the program stops. The lines after the call do not run, so they cannot use a receipt that was never created:

```quill
fn charge(amount: USD) -> text
  fails Declined
  when
    amount <= 0 USD -> fail Declined
    else -> return "paid"

var receipt = charge(0 USD)
  on Declined -> say "Card declined"

say receipt
```

```output
Card declined
```

There is no `paid` line. The charge was declined, the `on` line said so, and the program stopped.

## Carrying on with a value

Stopping is right when nothing sensible can happen next. Often something can: a declined card still deserves a receipt that says so. Put a value after `->` instead of a statement. That value takes the place of the call's answer, and the program carries on:

```quill
fn charge(amount: USD) -> text
  fails Declined
  when
    amount <= 0 USD -> fail Declined
    else -> return "paid"

receipt = charge(0 USD)
  on Declined -> "declined"

say "Receipt: {receipt}"
say "Thank you for shopping"
```

```output
Receipt: declined
Thank you for shopping
```

Read it out loud: "receipt is the charge, or on Declined, `declined`." The value has to be the same kind the call gives back. `charge` gives back text, so `on Declined -> 0` is a mistake:

```quill,error
fn charge(amount: USD) -> text
  fails Declined
  when
    amount <= 0 USD -> fail Declined
    else -> return "paid"

receipt = charge(0 USD)
  on Declined -> 0
```

```output
Problem on line 8 of example.ql:

      on Declined -> 0
                     ^
charge gives back Text, but 0 is a Number.
The value after -> takes the place of the call's answer, so it has to be text too.
```

Each `on` chooses for itself. One failure can give a value while another stops:

```quill
fn charge(amount: USD) -> text
  fails Declined, NetworkDown
  when
    amount <= 0 USD -> fail Declined
    amount > 1000 USD -> fail NetworkDown
  return "paid"

receipt = charge(5000 USD)
  on Declined -> "declined"
  on NetworkDown -> say "The bank is offline. Try again later."

say "Receipt: {receipt}"
```

```output
The bank is offline. Try again later.
```

So the rule after `->` is: a statement like `say` runs and then stops, and a value is used and the program carries on. A call on its own line gives nothing back, so under it `on` always runs a statement.

A function can pass a failure upward instead of handling it. `try` is allowed only inside a function that fails the same way:

```quill
fn charge(amount: USD) -> text
  fails Declined
  when
    amount <= 0 USD -> fail Declined
    else -> return "paid"

fn checkout(amount: USD) -> text
  fails Declined
  return try charge(amount)

var receipt = checkout(250 USD)
  on Declined -> say "Card declined"

say receipt
```

```output
paid
```

`checkout` declares `Declined` too, so `try charge(amount)` can pass that failure on. The caller of `checkout` still handles it.

Calling `charge` with no `on` and no `try` is a mistake. Quill names the failure you forgot.

`try` goes where the call's answer is used: after `return`, `=` or `say`. A call that gives nothing back, like `write_file`, stands on its own line, and `try` can't stand there. Pass its failure upward with `on` instead:

```quill,error
permit write "out"

fn save(note: text)
  fails CantWrite
  try write_file("out/note.txt", note)
```

```quill
permit write "out"

fn save(note: text)
  fails CantWrite
  write_file("out/note.txt", note)
    on CantWrite -> fail CantWrite
```

`on CantWrite -> fail CantWrite` does what `try` would do: `save` leaves with the same failure, and its caller handles it.

## Try it

1. Charge `0 USD` and handle `Declined`.
2. Add `fails NetworkDown` and a second `on`. Forget one of them and read what Quill says.
3. Move the charge into a second function with `try`, and handle `Declined` at the call.
4. Change `on Declined -> say "Card declined"` to `on Declined -> "declined"`, and add a `say` line after the call. Which version reaches that line?
