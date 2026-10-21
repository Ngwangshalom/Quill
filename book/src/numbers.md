# Numbers and Calculations

Quill's numbers are built for real-world work like prices, quantities and money. In this chapter you'll learn how to calculate, and why Quill's answers are always exact.

## Calculating

Quill uses the symbols you know from maths:

| Symbol | Meaning | Example | Result |
|---|---|---|---|
| `+` | add | `7 + 2` | `9` |
| `-` | subtract | `7 - 2` | `5` |
| `*` | multiply | `7 * 2` | `14` |
| `/` | divide | `7 / 2` | `3.5` |
| `%` | remainder after dividing | `7 % 2` | `1` |

```quill
price = 12.50
qty = 3
total = price * qty

say total
say total - 5
```

```output
37.50
32.50
```

## Order of operations

As in maths, `*`, `/` and `%` happen before `+` and `-`. Use brackets to change the order:

```quill
say 1 + 2 * 3
say (1 + 2) * 3
```

```output
7
9
```

A `-` in front of a value makes it negative:

```quill
balance = 20
say -balance
say 5 - 8
```

```output
-20
-3
```

## Numbers are exact

Many programming languages store numbers in a way that can't represent most decimals exactly, so `0.1 + 0.2` comes out as `0.30000000000000004`. That's a real problem when the numbers are money.

Quill's numbers are exact decimals:

```quill
say 0.1 + 0.2
say 100 * 1.05
```

```output
0.3
105.00
```

### Digits after the point

A number keeps the digits after the point that it was written with, so prices keep looking like prices:

```quill
say 0.10
say 0.10 + 0.2
say 1.50 * 2
```

```output
0.10
0.30
3.00
```

The rule is simple: the answer keeps as many digits after the point as the number that had the most.

Comparing ignores those extra zeros, so `0.10` and `0.1` are equal.

### Dividing

Some divisions never end, like 1 ÷ 3 = 0.3333... Quill keeps up to 20 digits after the point:

```quill
say 10 / 4
say 1 / 3
```

```output
2.5
0.33333333333333333333
```

### How big?

A Quill number can hold up to 38 digits in total, before and after the point. That's more than enough for any amount of money. If a calculation grows bigger than that, Quill stops and tells you, rather than quietly giving a wrong answer.

## Dividing by zero

Dividing by zero has no answer. If you write it directly, Quill catches it before running:

```quill,error
say 10 / 0
```

```output
Problem on line 1 of example.quill:

    say 10 / 0
             ^
This divides by zero, which has no answer.
Dividing by zero isn't allowed, so use a number other than 0.
```

If a value only turns out to be zero while the program runs, Quill stops at that line and explains:

```quill,error
var count = 0
say "Starting"
say 100 / count
```

```output
Starting
Problem on line 3 of example.quill:

    say 100 / count
        ^^^^^^^^^^^
This divides by zero, which has no answer.
The number on the right was 0 when this line ran. Use when to handle zero before dividing.
```

You'll learn how to handle this with `when` in [Making Decisions](decisions.md).

## Numbers and text don't mix

Calculations only work on numbers. Adding text to a number is a mistake, and Quill finds it before running anything:

```quill,error,file=shop.quill
price = 100
total = price + "5"
```

```output
Problem on line 2 of shop.quill:

    total = price + "5"
                    ^^^
You're adding Number (price) to Text ("5").
Quill won't guess which one you meant.

Did you mean:  total = price + 5
```

`"5"` in quotes is text, not a number. Quill won't guess whether you wanted to add numbers (105) or join text ("1005"), so it asks you to say which.

## Try it

1. A café sells coffee for 3.50 and cake for 4.25. Calculate the cost of 2 coffees and 1 cake.
2. Add 5% tax to your answer by multiplying by 1.05. How many digits does the answer have after the point? Why?
3. Use `%` to find the remainder when 17 is divided by 5.
4. Write `say 2 * "3"` and read the error.
