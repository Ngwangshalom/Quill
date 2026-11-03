# Text

Text is how programs talk to people. In this chapter you'll learn how to write text, put values inside it, include special characters, and work with the text you have: tidy it, measure it, look inside it, and change parts of it.

## Writing text

Text goes between double quotes, on one line:

```quill
say "Hello, World!"
```

```output
Hello, World!
```

Single quotes don't work in Quill. Text always uses double quotes, so there's only one way to write it.

## Values inside text

Put a value between `{` and `}` inside text, and Quill shows the value there:

```quill
name = "Amina"
qty = 3
price = 12.50

say "Hi {name}!"
say "{qty} items at {price} each"
say "That's {qty * price} in total"
```

```output
Hi Amina!
3 items at 12.50 each
That's 37.50 in total
```

Anything can go inside the braces: a name, a number, a calculation, or a comparison. Each pair of braces holds one value.

This is how you build text from values in Quill. `+` only adds numbers, so `"Hi " + name` is a mistake, and Quill suggests the braces instead:

```quill,error
name = "Amina"
say "Hi " + name
```

```output
Problem on line 2 of example.quill:

    say "Hi " + name
        ^^^^^^^^^^^^
You're adding Text ("Hi ") to Text (name).
Quill won't guess which one you meant.

Did you mean:  say "Hi {name}"
```

## Special characters

Some characters can't be typed directly inside text. Write them with a backslash `\` in front:

| Write | To get |
|---|---|
| `\n` | a new line |
| `\t` | a tab |
| `\"` | a double quote |
| `\\` | a backslash |
| `\{` and `\}` | a brace, instead of a value |

```quill
say "Line one\nLine two"
say "She said \"hi\""
say "Use \{name\} to show a name"
```

```output
Line one
Line two
She said "hi"
Use {name} to show a name
```

## Text with quotes inside

When text has many quotes in it, the backslashes get hard to read. Wrap the text in **three** quotes instead, and quotes inside need no backslash:

```quill
name = "Amina"
say """She said "hi" to {name}."""
```

```output
She said "hi" to Amina.
```

## Tidying text

Text that people type often has spaces at the ends, or capital letters where you don't want them. `trim`, `uppercase`, and `lowercase` give back a tidy copy:

```quill
typed = "  Amina Okafor  "
name = trim typed

say "[{name}]"
say uppercase name
say lowercase "AMINA@SCHOOL.ORG"
```

```output
[Amina Okafor]
AMINA OKAFOR
amina@school.org
```

`trim` takes away spaces, tabs, and new lines at the start and the end. The spaces between words stay. The text you started with doesn't change: `typed` still has its spaces, and `name` is the tidy copy.

A tool works inside braces too: `say "Hello, {trim typed}!"`.

## How long: length of

`length of` says how many letters a text has. Spaces and punctuation count, and so does each letter with an accent:

```quill
code = "INV-2026"
say length of code
say length of "café"
```

```output
8
4
```

Read it as: "the length of the code." It is a number, so it works in a `when`:

```quill
password = "quill"
when
  length of password < 8 -> say "Use at least 8 letters."
  else -> say "That's long enough."
```

```output
Use at least 8 letters.
```

## Looking inside text: contains, starts with, ends with

These three answer `true` or `false`:

```quill
email = "amina@school.org"
code = "INV-2026-001"

say email contains "@"
say code starts with "INV"
say code ends with ".pdf"
```

```output
true
true
false
```

They are conditions, so they go straight into a `when` and join with `and`, `or`, and `not`:

```quill
email = "amina@school.org"
when
  email contains "@" and email ends with ".org" -> say "A school address."
  not email contains "@" -> say "That isn't an email address."
  else -> say "Some other address."
```

```output
A school address.
```

Capital and small letters are different: `"Amina" contains "amina"` is `false`. To ignore them, compare the `lowercase` of both.

## Changing part of the text: replace

`replace OLD with NEW in TEXT` changes every `OLD` in the text to `NEW`:

```quill
code = "INV-2026-001"
say replace "-" with " " in code
say replace "Mr" with "Dr" in "Mr Okafor"
```

```output
INV 2026 001
Dr Okafor
```

Like the other tools, `replace` gives back a new text. `code` is still `"INV-2026-001"`.

`trim`, `uppercase`, `lowercase`, `length`, `contains`, `starts`, `ends`, and `replace` are Quill words only when they are used this way. A value of your own can still be called `length` or `trim`.

## Common mistakes

Forgetting the closing quote:

```quill,error
say "Hello
```

```output
Problem on line 1 of example.quill:

    say "Hello
        ^^^^^^
This text starts with a quote but never ends.
Text must open and close with " on the same line.

Did you mean:  "Hello"
```

Forgetting the closing brace:

```quill,error
name = "Amina"
say "Hi {name"
```

```output
Problem on line 2 of example.quill:

    say "Hi {name"
            ^
This { inside text is never closed.
A value inside text goes between { and }, and text can't go inside the braces.

Did you mean:  {name}
```

The text tools work on text. A number has to become text first, and braces do that:

```quill,error
year = 2026
say length of year
```

```output
Problem on line 2 of example.quill:

    say length of year
                  ^^^^
length of works on text, but year is a Number.
Put it inside text first, like "{year}".
```

`say length of "{year}"` is `4`.

## Try it

1. Create values for a city and its temperature, then say `"It's 31 degrees in Dubai"` using braces.
2. Show a receipt line with the price of 3 items at 4.25 each, calculated inside the braces.
3. Show the text `{curly braces}` with the braces visible.
4. `trim` the text `"  hello  "`, then say it in `uppercase`.
5. Make an email address and use a `when` to say whether it `contains "@"`.
6. Change every space in `"New York City"` to `-` with `replace`.
