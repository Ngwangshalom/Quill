# Money

A price is not just a number. `38.25` could be dollars or euros. Money names the currency, and Quill refuses to mix two currencies.

```quill
say 100 USD + 50 USD
```

```output
150 USD
```

Write the amount, a space, then three capital letters. `USD` is dollars. `EUR` is euros. `XAF` is the Central African franc. The amount stays exact, the same way other numbers do.

You can subtract two amounts in the same currency, and you can multiply or divide an amount by a plain number:

```quill
say 100 USD * 0.90
say 100 USD / 8
say 7.5 USD
```

```output
90 USD
12.50 USD
7.50 USD
```

An amount is shown in whole units (`90 USD`), or with every digit of its cents (`7.50 USD`, never `7.5 USD`). If a calculation leaves a part of a cent, Quill shows it, so nothing is hidden. The next sections show how to decide what happens to it.

## Currencies without cents

Some currencies have no cents. The Central African franc (`XAF`), the West African franc (`XOF`) and the yen (`JPY`) count in whole units. A few, like the Kuwaiti dinar (`KWD`), have three digits. Quill knows which is which:

```quill
say 5000 XAF + 250 XAF
say 1.5 KWD
```

```output
5250 XAF
1.500 KWD
```

In this chapter, "cents" means the currency's smallest unit, whatever it is called.

## Rounding: round

`round` decides what happens to the digits you don't want. `round total to cents` keeps whole cents:

```quill
say round 10.125 USD to cents
say round 10.135 USD to cents
```

```output
10.12 USD
10.14 USD
```

When a value is exactly halfway, Quill rounds to the even digit. This is how banks round, and it means that over many amounts, rounding doesn't push the total up or down.

Say which way when you need it. `round up` goes to the next larger value, and `round down` to the next smaller one:

```quill
say round up 10.121 USD to cents
say round down 10.129 USD to cents
```

```output
10.13 USD
10.12 USD
```

Numbers round to a count of places, or to a whole number:

```quill
say round 2.71828 to 2 places
say round 2.5
say round 3.5
```

```output
2.72
2
4
```

Quill never rounds money by itself. You write `round` where the rule matters, and a reader can see exactly what happens.

## Sharing an amount: split

Dividing 100 USD between 3 people gives 33.333... USD each, which no one can pay. Rounding each share loses a cent. `split` gives whole cents, and the parts always add up to the amount:

```quill
say split 100 USD into 3
```

```output
33.34 USD, 33.33 USD, 33.33 USD
```

The cent that is left over goes to the first part. Check it yourself:

```quill
parts = split 100 USD into 3
first = parts at 1
say first
```

```output
33.34 USD
```

To share by percentages or weights, split by a list of shares. Here an artist keeps 70 and the agency 30:

```quill
say split 1000 USD by list 70, 30
say split 100 USD by list 1, 1, 1
```

```output
700 USD, 300 USD
33.34 USD, 33.33 USD, 33.33 USD
```

## Part of an amount: %

`20% of fee` is a fifth of the fee. It works on money and on plain numbers:

```quill
fee = 500 USD
say 20% of fee
say 15% of 80
say 12.5% of 1000 USD
```

```output
100 USD
12
125 USD
```

A part of a percent can leave a part of a cent. Round it when it does: `round 7.5% of fee to cents`.

## Changing currency: convert

Quill never guesses an exchange rate, because rates change every minute. `convert` changes an amount to another currency at the rate you write:

```quill
say convert 100 USD to EUR at 0.92
say convert 100 USD to XAF at 605.5
```

```output
92 EUR
60550 XAF
```

`at 0.92` means one dollar is worth 0.92 euros. The rate usually comes from a bank or a service, so in a real program it is a value you were given: `convert price to EUR at today_rate`.

## Money in functions

A function names money by the currency:

```quill
fn sale_total(loaves: number, price: USD) -> USD
  return loaves * price

say sale_total(3, 2.50 USD)
```

```output
7.50 USD
```

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
Quill won't mix currencies. Change one with convert, at the rate you were given: convert 20 EUR to USD at 0.92.
```

A currency is written in capitals. `100 usd` is a mistake. Write `100 USD`.

`split` works in whole cents. An amount with part of a cent has to be rounded first:

```quill,error
say split 10.005 USD into 2
```

```output
Problem on line 1 of example.ql:

    say split 10.005 USD into 2
        ^^^^^^^^^^^^^^^^^^^^^^^
split shares out whole cents, but 10.005 USD has a part smaller than that.
Round it first: round it to cents, then split it.
```

`convert` needs the rate:

```quill,error
say convert 100 USD to EUR
```

```output
Problem on line 1 of example.ql:

    say convert 100 USD to EUR
                           ^^^
convert needs the rate, after at.
Quill never guesses a rate: at 0.92 means one unit is worth 0.92 EUR.

Did you mean:  convert 100 USD to EUR at 0.92
```

`round`, `split` and `convert` are Quill's own words, so they can't name a value or a function. Call a function `split_bill`, not `split`.

## Try it

1. Add `20 USD` and `5 USD`, and say the result.
2. Add `20 EUR` and `5 USD`, and read what Quill says.
3. Split a 250 USD dinner between 3 friends. Check that the parts add up to 250 USD.
4. A booking fee is 80000 XAF, and the agency keeps 15%. Say the agency's part, and what the artist keeps.
5. Convert 100 USD to EUR at a rate of 0.92, then round the answer to cents.
