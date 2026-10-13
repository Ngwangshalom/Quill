# Testing Your Code

A test is a line of code that checks that your program does what you think it does. In Quill, tests are part of the language and live right next to the code they test.

## check

`check` compares two values. If the comparison is true, nothing happens. If it's false, the program stops and tells you what each side was:

```quill
price = 12.50
qty = 3
total = price * qty

check total == 37.50
check total > 0
say "All good"
```

```output
All good
```

When a check fails:

```quill,error
price = 12.50
qty = 3
total = price + qty   -- oops: should be *

check total == 37.50
```

```output
Problem on line 5 of example.ql:

    check total == 37.50
    ^^^^^^^^^^^^^^^^^^^^
This check failed.
One side is 15.50 and the other is 37.50.
A check passes only when both sides are the same value, so change one of them to match.
```

`check` works with any comparison: `==`, `!=`, `<`, `<=`, `>` and `>=`.

## Examples in documentation

A doc comment starts with `---` and describes the code below it. A doc comment that starts with `--- example:` is also a test:

```quill
vat = 1.05

--- Prices include VAT, so 100 becomes 105.
--- example: 100 * vat == 105
price = 250 * vat

say price
```

```output
262.50
```

Because the example is run like a `check`, the documentation can't quietly become wrong: if someone changes `vat`, the example fails.

An example runs where it's written, so it can only use values created above it, like any other line. When functions arrive in Quill, examples will be the natural way to document them.

## Running every test: quill test

`quill test` looks through the current folder and its subfolders, runs every `.ql` file that contains a `check` or `--- example:`, and reports the results:

```text
$ quill test
shop/hello.ql: 1 check passed
shop/prices.ql: 4 checks passed

5 checks passed.
```

Run it often, especially before sharing your code.

## Try it

1. Write a program that calculates the area of a 4 by 2.5 rectangle, with a `check` that the answer is 10.
2. Make the check fail on purpose and read the message.
3. Add a `--- example:` line above a value, then run `quill test`.
