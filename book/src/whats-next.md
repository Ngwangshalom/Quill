# What's Coming Next

Quill is young, and these features are designed but not built yet. The examples on this page show how they're planned to work. They don't run yet.

## Loops

Repeat work for each number in a range:

```quill,ignore
for i in 1..15
  say i
```

## Functions

Name a piece of code and reuse it:

```quill,ignore
fn fizzbuzz(n: Int) -> Text
  when
    n % 15 == 0 -> "FizzBuzz"
    n % 3 == 0  -> "Fizz"
    n % 5 == 0  -> "Buzz"
    else        -> text(n)
```

## No null: maybe

A value that might not exist says so, and Quill makes you handle the missing case:

```quill,ignore
when find_user(42)
  found user -> say "Hello, {user.name}"
  missing    -> say "No such user"
```

## Failures you can't ignore

A function declares how it can fail, and callers must handle each failure:

```quill,ignore
receipt = charge(card, 250 AED)
  on Declined    -> respond 402, "Card declined"
  on NetworkDown -> respond 503, "Payments are unavailable, try again"
```

## Exact money

Money with its currency, so dirhams and dollars can't be mixed by accident:

```quill,ignore
total = 100 AED + 50 AED     -- 150 AED
```

## And more

Types with rules (`guests: Int where 1..500`), contracts (`require` and `ensure`), permissions (`permit net "api.stripe.com"`), and built-in modules for web servers and databases. See the [README](https://github.com/Ngwangshalom/Quill#readme) for the full design.

Each of these will get its own chapter in this book as it's built. Want to help build one? Read the [contributor guide](https://github.com/Ngwangshalom/Quill/blob/main/CONTRIBUTING.md).
