# Money

A price is not just a number. `38.25` could be dirhams or dollars. Money names the currency, and Quill refuses to mix two currencies.

```quill
say 100 AED + 50 AED
```

```output
150 AED
```

Write the amount, a space, then three capital letters. `AED` is dirhams. `USD` is dollars. The amount stays exact, the same way other numbers do.

You can subtract two amounts in the same currency, and you can multiply or divide an amount by a plain number:

```quill
say 100 AED * 0.90
say 100 AED / 2
```

```output
90.00 AED
50 AED
```

`0.90` has two digits after the point, so the answer keeps those two digits.

## Common mistakes

Two currencies cannot be added. Quill will not guess an exchange rate:

```quill,error
say 100 AED + 20 USD
```

```output
Problem on line 1 of example.ql:

    say 100 AED + 20 USD
        ^^^^^^^^^^^^^^^^
You're adding money in AED (100 AED) and money in USD (20 USD).
Quill won't mix currencies. AED and USD stay apart until one of them is converted.
```

A currency is written in capitals. `100 aed` is a mistake. Write `100 AED`.

A function names money by the currency:

```quill
fn sale_total(loaves: number, price: AED) -> AED
  return loaves * price

say sale_total(3, 2.50 AED)
```

```output
7.50 AED
```

## Try it

1. Add `20 USD` and `5 USD`, and say the result.
2. Add `20 USD` and `5 AED`, and read what Quill says.
3. Write a function that takes an `AED` amount and gives back half of it.
