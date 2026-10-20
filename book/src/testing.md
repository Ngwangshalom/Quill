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

An example runs where it's written, so it can only use values created above it, like any other line. Inside a function, an example runs each time that function is called.

### Examples for a function

The best place to document a function is right above it. Examples written directly above a `fn` run just after that function is written, so they can call it:

```quill
--- Adds 5% VAT to a price.
--- example: with_vat(100 USD) == 105 USD
--- example: with_vat(0 USD) == 0 USD
fn with_vat(price: USD) -> USD
  return price * 1.05

say with_vat(20 USD)
```

```output
21.00 USD
```

Anyone reading `with_vat` sees what it does and what it gives back before reading a line of its code, and `quill test` proves the examples are still true.

"Directly above" means nothing but comments and blank lines between the examples and the `fn`. If another line comes between them, the example runs where it is, before the function exists, and Quill tells you to move it:

```quill,error
--- example: with_vat(100 USD) == 105 USD
say "Prices"
fn with_vat(price: USD) -> USD
  return price * 1.05
```

```output
Problem on line 1 of example.ql:

    --- example: with_vat(100 USD) == 105 USD
                 ^^^^^^^^
with_vat is used here, but the function isn't written until line 3.
An example runs after the fn right below it. Move this example directly above fn with_vat, with no other lines between them.
```

## Running every test: quill test

`quill test` looks through the current folder and its subfolders, runs every `.ql` file that contains a `check` or `--- example:`, and reports the results:

```text
$ quill test
shop/hello.ql: 1 check passed
shop/prices.ql: 4 checks passed

5 checks passed.
```

To run just one file, use `quill test shop/prices.ql`. The file runs even if it contains no checks; in that case, the report says `0 checks passed`. A missing file or a path without `.ql` reports a problem.

The count is the number of successful checks actually executed. Checks inside called functions count, and a check inside a loop counts each time it passes. Skipped branches and functions that are never called do not count. Doc examples count the same way. A failed check stops that file; earlier passes still count in the summary, and project testing continues with the other files.

Run it often, especially before sharing your code.

## Try it

1. Write a program that calculates the area of a 4 by 2.5 rectangle, with a `check` that the answer is 10.
2. Make the check fail on purpose and read the message.
3. Add a `--- example:` line above a value, then run `quill test`.
4. Run only that file with `quill test your-file.ql`.
