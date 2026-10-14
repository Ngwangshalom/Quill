# Failures

A payment can be declined. The function says so, and the caller has to decide what happens. Quill will not let the failure disappear.

```quill
fn charge(amount: AED) -> text
  fails Declined
  when
    amount <= 0 AED -> fail Declined
    else -> return "paid"

var receipt = charge(250 AED)
  on Declined -> say "Card declined"

say receipt
```

```output
paid
```

`fails Declined` is the first line under `fn`. A failure's name starts with a capital letter. `fail Declined` leaves the function with that failure. It does not give the text back.

Under the call, `on Declined` says what to do. Every failure named in `fails` needs its own `on`. When the call succeeds, Quill skips the `on` lines and continues. `receipt` holds `"paid"`, and the next line says it.

When the call fails, the `on` line runs and the program stops. The lines after the call do not run, so they cannot use a receipt that was never created:

```quill
fn charge(amount: AED) -> text
  fails Declined
  when
    amount <= 0 AED -> fail Declined
    else -> return "paid"

var receipt = charge(0 AED)
  on Declined -> say "Card declined"

say receipt
```

```output
Card declined
```

There is no `paid` line. The charge was declined, the `on` line said so, and the program stopped.

A function can pass a failure upward instead of handling it. `try` is allowed only inside a function that fails the same way:

```quill
fn charge(amount: AED) -> text
  fails Declined
  when
    amount <= 0 AED -> fail Declined
    else -> return "paid"

fn checkout(amount: AED) -> text
  fails Declined
  return try charge(amount)

var receipt = checkout(250 AED)
  on Declined -> say "Card declined"

say receipt
```

```output
paid
```

`checkout` declares `Declined` too, so `try charge(amount)` can pass that failure on. The caller of `checkout` still handles it.

Calling `charge` with no `on` and no `try` is a mistake. Quill names the failure you forgot.

## Try it

1. Charge `0 AED` and handle `Declined`.
2. Add `fails NetworkDown` and a second `on`. Forget one of them and read what Quill says.
3. Move the charge into a second function with `try`, and handle `Declined` at the call.
