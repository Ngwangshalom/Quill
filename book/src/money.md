# Money

A price is not just a number. `38.25` could be dollars or euros. Money names the currency, and Quill refuses to mix two currencies.

```quill
say 100 USD + 50 USD
```

```output
150 USD
```

Write the amount, a space, then three capital letters. `USD` is dollars. `EUR` is euros. The amount stays exact, the same way other numbers do.

You can subtract two amounts in the same currency, and you can multiply or divide an amount by a plain number:

```quill
say 100 USD * 0.90
say 100 USD / 2
```

```output
90.00 USD
50 USD
```

`0.90` has two digits after the point, so the answer keeps those two digits.

## Common mistakes

Two currencies cannot be added. Quill will not guess an exchange rate:

```quill,error
say 100 USD + 20 EUR
```

```output
Problem on line 1 of example.ql:

    say 100 USD + 20 EUR
        ^^^^^^^^^^^^^^^^
You're adding money in USD (100 USD) and money in EUR (20 EUR).
Quill won't mix currencies. USD and EUR stay apart until one of them is converted.
```

A currency is written in capitals. `100 usd` is a mistake. Write `100 USD`.

A function names money by the currency:

```quill
fn sale_total(loaves: number, price: USD) -> USD
  return loaves * price

say sale_total(3, 2.50 USD)
```

```output
7.50 USD
```

## Try it

1. Add `20 USD` and `5 USD`, and say the result.
2. Add `20 EUR` and `5 USD`, and read what Quill says.
3. Write a function that takes a `USD` amount and gives back half of it.
