# Making Decisions with when

Programs often need to do different things in different situations: free delivery for big orders, a warning when stock runs low, a different greeting for new visitors. In this chapter you'll learn how to ask questions and act on the answers.

## Asking questions: comparisons

A comparison asks a yes-or-no question. Its answer is `true` or `false`:

| Symbol | Question | Example | Answer |
|---|---|---|---|
| `==` | equal to? | `3 == 3` | `true` |
| `!=` | not equal to? | `3 != 3` | `false` |
| `<` | less than? | `2 < 3` | `true` |
| `<=` | less than or equal to? | `3 <= 3` | `true` |
| `>` | more than? | `2 > 3` | `false` |
| `>=` | more than or equal to? | `2 >= 3` | `false` |

```quill
total = 120
say total > 100
say total == 50
```

```output
true
false
```

`==` and `!=` work on any two values of the same kind, including text. `<`, `<=`, `>` and `>=` work on numbers.

Notice the difference between `=` and `==`: `=` gives a value a name, while `==` asks whether two values are equal.

## Combining questions: and, or, not

| Word | Answer is true when | Example |
|---|---|---|
| `and` | both sides are true | `age >= 18 and has_ticket` |
| `or` | at least one side is true | `is_member or total > 100` |
| `not` | the value is false | `not sold_out` |

```quill
age = 25
has_ticket = true
sold_out = false

say age >= 18 and has_ticket
say not sold_out
```

```output
true
true
```

To check that a number is between two others, use `and`: write `1 <= qty and qty <= 10`. Quill doesn't allow `1 <= qty <= 10`, because it's easy to misread.

`and` stops as soon as it knows the answer. In `count != 0 and total / count > 10`, if `count` is zero, the division never happens. That makes `and` a safe way to protect a calculation.

## Acting on answers: when

`when` checks a list of cases from the top and runs the first one that's true:

```quill
total = 45

when
  total > 100 -> say "Free delivery!"
  total > 30  -> say "Delivery: 5"
  else        -> say "Delivery: 10"
```

```output
Delivery: 5
```

Here's how to read it:

- The cases go on the lines **below** `when`, **indented** (moved in with spaces) further than `when` itself.
- Each case is a **condition**, then `->`, then **what to do**.
- Read `->` as "then": "when total is more than 100, **then** say Free delivery".
- **Only the first true case runs.** `total` is 45, so the first case is false and the second is true. Quill runs the second case and skips the rest, even though `else` would also match.
- `else` is the case for "none of the above". It's optional, and if you use it, it must come last.

If no case is true and there's no `else`, nothing happens:

```quill
stock = 50

when
  stock < 10 -> say "Running low!"

say "Done"
```

```output
Done
```

## Why indentation matters

Indentation shows which lines belong to the `when`. The block ends when a line moves back to where `when` started:

```quill
temperature = 31

when
  temperature > 30 -> say "It's hot"
  else             -> say "It's pleasant"
say "Have a nice day"   -- not part of the when: this always runs
```

```output
It's hot
Have a nice day
```

A block moves in by **exactly 2 spaces**. Not one space, not four, and not a tab. The cases of a `when` line up with each other, and a line that ends the block comes back to the same column as the `when`.

```quill,error
temperature = 31

when
    temperature > 30 -> say "It's hot"
```

```output
Problem on line 4 of example.ql:

        temperature > 30 -> say "It's hot"
    ^^^^
This line is indented by 4 spaces.
A block moves in by exactly 2 spaces from the line above it, so this line should be indented by 2 spaces.
```

## Changing values in a when

A case can change a `var` with `set`, but it can't create a new value, because that value would only exist when that case runs. Create the value before the `when`, then set it inside:

```quill
total = 120
var delivery = 10

when
  total > 100 -> set delivery = 0
  total > 30  -> set delivery = 5

say "Delivery: {delivery}"
```

```output
Delivery: 0
```

If you try to create a value inside a case, Quill explains what to do instead:

```quill,error
total = 120
when
  total > 100 -> delivery = 0
```

```output
Problem on line 3 of example.ql:

      total > 100 -> delivery = 0
                     ^^^^^^^^
A case of a when can't create a new value.
delivery would only exist when this case runs. Create it before the when with var delivery = ..., then change it here with set.

Did you mean:  set delivery = 0
```

## A when that gives a value

Often a `when` only exists to choose a value. Then you don't need `var` and `set` at all. Put `when` where the value goes, and give a value after each `->`:

```quill
total = 120

delivery = when
  total > 100 -> 0
  total > 30  -> 5
  else        -> 10

say "Delivery: {delivery}"
```

```output
Delivery: 0
```

Read it out loud: "delivery is, when total is over 100, 0; when it's over 30, 5; otherwise 10." Quill picks the first case whose condition is true and uses its value. `delivery` never changes after that, so it doesn't need `var`.

This works anywhere a value fits on its own: after `=`, after `set`, after `say`, and after `return`. A function that chooses between values can return the `when` directly:

```quill
fn grade(score: number) -> text
  return when
    score >= 70 -> "A"
    score >= 50 -> "Pass"
    else        -> "Fail"

say grade(85)
say grade(40)
```

```output
A
Fail
```

Two rules keep the value certain to exist:

- **The last case is `else`.** Without it, no case might be true, and there would be no value:

```quill,error
total = 20
delivery = when
  total > 100 -> 0
```

```output
Problem on line 2 of example.ql:

    delivery = when
               ^^^^
A when that gives a value needs else as its last case.
Without else, no case might be true, and there would be no value.

Did you mean:  else -> "..."
```

- **Every case gives the same kind of value.** `delivery` can't be a number in one case and text in another:

```quill,error
total = 20
delivery = when
  total > 100 -> 0
  else        -> "ten"
```

```output
Problem on line 4 of example.ql:

      else        -> "ten"
                     ^^^^^
Every case of this when has to give the same kind of value.
0 on line 3 is a Number, but "ten" is Text.
```

When each case should *do* something, like `say`, keep `when` on its own line as before. When each case should *be* something, put `when` after `=`.

## A when inside a when

A case can hold another `when`, indented further:

```quill
logged_in = true
is_admin = false

when
  logged_in -> when
    is_admin -> say "Welcome, admin"
    else     -> say "Welcome back"
  else -> say "Please log in"
```

```output
Welcome back
```

## FizzBuzz, almost

FizzBuzz is a classic exercise: for a number, say "Fizz" if it divides by 3, "Buzz" if it divides by 5, "FizzBuzz" if both, and otherwise the number itself. `%` gives the remainder, so `n % 3 == 0` asks "does n divide by 3?":

```quill
n = 15

when
  n % 15 == 0 -> say "FizzBuzz"
  n % 3 == 0  -> say "Fizz"
  n % 5 == 0  -> say "Buzz"
  else        -> say n
```

```output
FizzBuzz
```

The order of the cases matters: `15` also divides by 3, but the `FizzBuzz` case comes first. A `for` loop, in [Repeating with for](loops.md), runs this for every number from 1 to 100.

## Conditions must be true or false

A case needs a yes-or-no question. A number on its own isn't one:

```quill,error
count = 3
when
  count -> say "We have some"
```

```output
Problem on line 3 of example.ql:

      count -> say "We have some"
      ^^^^^
A case of a when needs a condition that is true or false, but count is a Number.
Compare it to something, like count > 0.
```

## Try it

1. Make a value `age` and use `when` to say "child" under 13, "teenager" under 18, and "adult" otherwise. Change `age` and run it again.
2. A shop gives 10% off orders over 200. Make a `var price` and use `when` to reduce it with `set` when it qualifies.
3. Rewrite FizzBuzz for a different number. What happens if you move the `FizzBuzz` case to the bottom?
4. Rewrite exercise 1 as `stage = when ...`, then `say stage`. Remove the `else` case and read what Quill says.
