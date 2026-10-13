# Text

Text is how programs talk to people. In this chapter you'll learn how to write text, put values inside it, and include special characters.

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
Problem on line 2 of example.ql:

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

## Common mistakes

Forgetting the closing quote:

```quill,error
say "Hello
```

```output
Problem on line 1 of example.ql:

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
Problem on line 2 of example.ql:

    say "Hi {name"
            ^
This { inside text is never closed.
A value inside text goes between { and }, and text can't go inside the braces.

Did you mean:  {name}
```

## Try it

1. Create values for a city and its temperature, then say `"It's 31 degrees in Dubai"` using braces.
2. Show a receipt line with the price of 3 items at 4.25 each, calculated inside the braces.
3. Show the text `{curly braces}` with the braces visible.
