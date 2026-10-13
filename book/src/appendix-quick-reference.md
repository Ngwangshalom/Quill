# Appendix A: Quick Reference

All of Quill on one page.

```quill
-- A comment
--- A doc comment
--- example: 1 + 1 == 2

name = "Amina"                  -- a value that can't change
price = 12.50                   -- numbers are exact decimals
in_stock = true                 -- true or false

var total = 0                   -- a value that can change
set total = total + price * 2   -- change it

say "Hi {name}, total {total}"  -- {} puts a value inside text
say """She said "hi"."""        -- three quotes: no backslash needed

when
  total > 100 -> say "Free delivery"
  total > 20  -> say "Delivery: 5"
  else        -> say "Delivery: 10"

check total == 25.00            -- a test
```

```output
Hi Amina, total 25.00
She said "hi".
Delivery: 5
```

## Kinds of values

| Kind | Examples |
|---|---|
| Text | `"Hello"`, `"Hi {name}"`, `"""He said "hi"."""` |
| Number | `42`, `0.10`, `-3.5` |
| true/false | `true`, `false`, `total > 10` |

## Operators, tightest first

| Operators | Meaning |
|---|---|
| `( )` | brackets |
| `-x` | negative |
| `*` `/` `%` | multiply, divide, remainder |
| `+` `-` | add, subtract (numbers only) |
| `==` `!=` `<` `<=` `>` `>=` | compare |
| `not` | not |
| `and` | both |
| `or` | either |

## Escapes inside text

`\n` new line · `\t` tab · `\"` quote · `\\` backslash · `\{` `\}` braces

## Reserved words

`say` `check` `var` `set` `when` `else` `true` `false` `and` `or` `not`

## Commands

| Command | What it does |
|---|---|
| `quill run FILE.ql` | run a program |
| `quill check FILE.ql` | find mistakes without running |
| `quill test` | run every test in this folder |
| `quill explain FILE.ql:N` | show how Quill read line N |
| `quill new NAME` | start a project |
| `quill tokens FILE.ql` | show the pieces Quill read |
| `quill update` | choose a version of Quill to install (also `quill upgrade`) |
| `quill update VERSION` | install that version, or `latest` |
| `quill help` | show help |
| `quill version` | show the version |
