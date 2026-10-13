# Quill Language Specification

**Version 0.1 draft**, describing Quill as implemented in release v0.2.0.

This document says exactly what Quill programs mean. The [README](https://github.com/Ngwangshalom/Quill#readme) shows where the language is going; this spec covers what works today, and marks planned features clearly in [§12](#12-planned-not-yet-implemented). When the implementation and this document disagree, that is a bug in one of them. Please [open an issue](https://github.com/Ngwangshalom/Quill/issues).

## Contents

1. [Programs and files](#1-programs-and-files)
2. [Lines, indentation and comments](#2-lines-indentation-and-comments)
3. [Words and names](#3-words-and-names)
4. [Kinds of values](#4-kinds-of-values)
5. [Writing values](#5-writing-values)
6. [Calculations and conditions](#6-calculations-and-conditions)
7. [Statements](#7-statements)
8. [`when`](#8-when)
9. [Tests: `check` and doc examples](#9-tests-check-and-doc-examples)
10. [When Quill finds mistakes](#10-when-quill-finds-mistakes)
11. [The `quill` tool](#11-the-quill-tool)
12. [Planned, not yet implemented](#12-planned-not-yet-implemented)
13. [Grammar summary](#13-grammar-summary)

---

## 1. Programs and files

A Quill program is a text file ending in `.ql`, written in UTF-8. Quill reads it from top to bottom and runs each statement in order.

Before anything runs, Quill reads the whole file and checks it (§10). If it finds any mistake, nothing runs at all.

## 2. Lines, indentation and comments

### 2.1 One statement per line

Each statement sits on its own line. There are no semicolons and no braces for blocks. A line ends at a new line character (`\n`, or `\r\n` on Windows).

```quill
name = "Amina"
say name
```

Blank lines are ignored.

### 2.2 Indentation

Indentation marks a block: lines that belong to the line above them. Today, only the cases of a `when` (§8) are indented.

- A line that is indented further than the line above it starts a block. It is only allowed where a block is expected.
- A block ends when a line returns to an earlier indentation. That line must line up exactly with an earlier level.
- Indent with spaces. A line whose indentation mixes tabs and spaces is a mistake, because editors show tabs at different widths.
- Blank lines and comment lines never start or end a block.

```quill
when
  total > 100 -> say "Free delivery"    -- in the block
  else -> say "Delivery: 5"             -- in the block
say "Thanks!"                           -- back at the left: the block has ended
```

### 2.3 Comments

- `--` starts a comment. Everything after it on the line is ignored.
- `---` starts a doc comment, which documents the code that follows it. It is ignored when the program runs.
- `--- example:` is not a comment. It is a test (§9.2).

```quill
--- Adds 5% VAT to a price.
--- example: 100 * 1.05 == 105
vat = 1.05   -- the UAE rate
```

## 3. Words and names

### 3.1 Names

A name starts with a letter or `_`, followed by letters, digits or `_`. Letters from any language are allowed (`café`, `prix`). Names are case-sensitive: `total` and `Total` are different names.

A name can't start with a digit: `2x` is a mistake.

### 3.2 Reserved words

These words have a meaning in Quill and can't be used as names:

```
say  check  var  set  when  else  true  false  and  or  not
```

## 4. Kinds of values

Every value has one of three kinds. The checker knows the kind of every value before the program runs, so it can catch mistakes like adding text to a number.

| Kind | Examples | What it's for |
|---|---|---|
| **Text** | `"Amina"`, `"Total: {total}"` | words and sentences |
| **Number** | `42`, `0.10`, `-3.5` | exact decimal numbers |
| **true/false** | `true`, `false`, `total > 10` | conditions |

A name keeps the kind of the value it was created with for the whole program (§7.4).

Quill never turns one kind into another by itself. `"5"` is text, and `5` is a number.

### 4.1 Numbers are exact

Numbers are decimals, not binary floating point, so they never round by surprise:

```quill
say 0.1 + 0.2      -- 0.3
say 100 * 1.05     -- 105.00
```

- A number holds up to **38 digits** in total, before and after the point. A number written with more digits is a mistake, and a calculation whose result would need more stops the program with an error (§10.2).
- **Adding, subtracting and multiplying are always exact.**
- **Dividing** keeps up to **20 digits** after the point. When the answer needs more, the last digit is rounded half to even: `1 / 3` is `0.33333333333333333333`, and `2 / 3` is `0.66666666666666666667`.
- **The remainder** `a % b` has the sign of `a`: `7 % 3` is `1` and `-7 % 3` is `-1`. It works on decimals too: `7.5 % 2` is `1.5`.

### 4.2 How numbers are shown

A number keeps the digits after the point that it was written with, so prices stay looking like prices:

| Calculation | Shows | Rule |
|---|---|---|
| `0.10` | `0.10` | as written |
| `0.10 + 0.2` | `0.30` | adding and subtracting keep the most digits after the point |
| `2.5 - 0.5` | `2.0` | |
| `1.50 * 2` | `3.00` | multiplying keeps at least as many digits as the longer side |
| `10 / 4` | `2.5` | dividing drops trailing zeros... |
| `9.00 / 3` | `3.00` | ...but keeps at least as many digits as the longer side |

Comparing ignores trailing zeros: `0.10 == 0.1` is `true`.

### 4.3 How values are shown

`say` and `{}` inside text show values like this:

- Text: its characters, without quotes.
- Number: as described in §4.2.
- true/false: the word `true` or `false`.

## 5. Writing values

### 5.1 Text

Text is written between double quotes on one line: `"Hello, World!"`. Single quotes are a mistake.

Inside text, a backslash starts an escape:

| Write | To get |
|---|---|
| `\n` | a new line |
| `\t` | a tab |
| `\"` | `"` |
| `\\` | `\` |
| `\{` | `{` |
| `\}` | `}` |

Any other character after a backslash is a mistake.

**Text with quotes inside.** Text between three quotes holds `"` as itself, with no backslash needed. Escapes and `{}` still work:

```quill
say """She said "hi"."""
```

### 5.2 Values inside text

`{` and `}` inside text hold a value. Quill works out the value and puts it into the text, shown as in §4.3:

```quill
name = "Amina"
qty = 3
say "Hi {name}, that's {qty * 2} items"     -- Hi Amina, that's 6 items
```

- Any value or calculation can go inside `{}`, as long as it fits on the line.
- Only one value goes inside each pair of braces.
- Braces can't be empty, can't hold another `{`, and can't hold text in quotes. To use text, give it a name first.
- A `}` that doesn't close a `{` is a mistake. Write `\{` and `\}` to show braces themselves.

### 5.3 Numbers

A number is written as digits, with an optional decimal point followed by more digits: `42`, `0.10`, `3.14159`.

- There must be a digit before the point: `.5` is not a number. Write `0.5`.
- A point must be followed by a digit: `3.` is a mistake.
- Negative numbers are written with `-` in front (§6.1).

### 5.4 true and false

`true` and `false` are the two true/false values. Comparisons (§6.2) also produce them.

## 6. Calculations and conditions

### 6.1 Operators

From the tightest binding to the loosest:

| Level | Operators | Works on | Gives |
|---|---|---|---|
| 1 | `( ... )` | anything | the value inside |
| 2 | `-a` | Number | Number |
| 3 | `a * b`, `a / b`, `a % b` | Number, Number | Number |
| 4 | `a + b`, `a - b` | Number, Number | Number |
| 5 | `a == b`, `a != b` | two values of the same kind | true/false |
| 5 | `a < b`, `a <= b`, `a > b`, `a >= b` | Number, Number | true/false |
| 6 | `not a` | true/false | true/false |
| 7 | `a and b` | true/false, true/false | true/false |
| 8 | `a or b` | true/false, true/false | true/false |

Operators on the same level work from left to right: `10 - 4 - 3` is `(10 - 4) - 3`, which is `3`.

So `1 + 2 * 3` is `7`, `n % 15 == 0` compares the remainder with `0`, and `not a == b` means `not (a == b)`.

### 6.2 Comparing

- `==` and `!=` compare two values of the same kind. Comparing different kinds, like `5 == "5"`, is a mistake rather than `false`, because it is almost never what was meant.
- `<`, `<=`, `>` and `>=` compare numbers only.
- Comparisons can't be chained: `1 < x < 10` is a mistake. Write `1 < x and x < 10`.

### 6.3 `and`, `or` and `not`

`and` and `or` stop as soon as the answer is known:

- In `a and b`, if `a` is `false`, `b` is not worked out.
- In `a or b`, if `a` is `true`, `b` is not worked out.

So a condition can protect a calculation:

```quill
say count != 0 and total / count > 10     -- never divides by zero
```

### 6.4 `+` doesn't join text

`+` only adds numbers. To build text from values, put them inside text (§5.2):

```quill
say "Hi " + name      -- mistake: Quill won't guess whether you meant to add or join
say "Hi {name}"       -- right
```

## 7. Statements

### 7.1 `say`

```quill
say VALUE
```

Shows the value (§4.3), followed by a new line.

### 7.2 Creating a value: `name = value`

```quill
total = price * qty
```

Creates a new name for a value. This value **can't change**.

- A name must be created before it is used. Quill reads from top to bottom.
- A name can only be created once in a program.
- A value can't be made from itself: `x = x + 1` is a mistake when `x` doesn't exist yet.

### 7.3 Values that can change: `var`

```quill
var visits = 0
```

Creates a value that can be changed later with `set`. Otherwise it follows the same rules as §7.2.

### 7.4 Changing a value: `set`

```quill
set visits = visits + 1
```

Changes a value created with `var`.

- The name must already exist, and it must have been created with `var`. Changing a value that isn't marked `var` is a mistake. This makes every change visible when you read the code.
- The new value must be the same kind as the old one. A Number stays a Number.
- `name = ...` on a name that already exists is a mistake, even for a `var`. Use `set` to change it.

### 7.5 `check` and `--- example:`

Tests. See §9.

### 7.6 `when`

Chooses what to do. See §8.

### 7.7 A value on its own

A line holding only a value or calculation, like `price * 2`, is a mistake, because nothing would happen to it. Quill suggests `say price * 2`, or `check ...` for a comparison.

## 8. `when`

`when` runs the first case whose condition is true:

```quill
when
  total > 100 -> say "Free delivery"
  total > 30  -> say "Delivery: 5"
  else        -> say "Delivery: 10"
```

### 8.1 Cases and `->`

The cases go on the lines below `when`, indented further than the `when`. Each case is one line:

```
CONDITION -> STATEMENT
```

**`->` separates the question from the answer.** It reads as "then": when `total > 100` is true, then `say "Free delivery"`.

- On the left of `->` is a condition, which must be true/false.
- On the right of `->` is the statement to run when that case is chosen, on the same line.
- `->` is only used inside `when`. Anywhere else it is a mistake.

`->` is used instead of `:` or `then` so the condition and the action are clearly separated even when lined up in a column, as above. The README uses the same arrow for matching `maybe` values and failures (§12), so it will always mean "in this case, do this".

### 8.2 How a `when` runs

1. Quill tests each condition in order, from the top.
2. The first case whose condition is `true` runs. No other case runs, even if its condition is also true.
3. `else` matches when no case above it was true. It is optional, and if present it must be the last case.
4. If no case is true and there is no `else`, nothing happens.

### 8.3 What a case can do

The statement after `->` can be `say`, `set`, `check`, `--- example:` or another `when`:

```quill
when
  logged_in -> when
    is_admin -> say "Welcome, admin"
    else     -> say "Welcome back"
  else -> say "Please log in"
```

A case **can't create a value** (`name = ...` or `var name = ...`), because the value would only exist when that case runs. Create it before the `when` with `var`, then change it inside with `set`:

```quill
var label = ""
when
  total > 100 -> set label = "big order"
  else        -> set label = "order"
say label
```

### 8.4 Rules

- A `when` must have at least one case.
- `when` stands on its own line: its cases can't be written on the same line.
- A case that comes after `else` can never run, so it is a mistake.

## 9. Tests: `check` and doc examples

### 9.1 `check`

```quill
check total == 37.80
check visits >= 1
```

A `check` compares two values with any comparison from §6.2. When it is true, nothing happens. When it is false, the program stops and Quill says what each side was:

```
This check failed.
One side is 37.75 and the other is 37.80.
```

Both sides of a `check` are calculations. A `check` can't use `and`, `or` or `not` directly: use two checks instead.

### 9.2 `--- example:`

```quill
--- example: 0.1 + 0.2 == 0.3
```

A doc example is a `check` written inside documentation. It runs exactly like `check`, so examples in documentation can't drift out of date.

### 9.3 Running tests

`check` lines run whenever the program runs. `quill test` (§11) runs every file in a folder that contains tests.

## 10. When Quill finds mistakes

### 10.1 Before running

Quill reads the whole program and checks it before running anything. If there are mistakes, it reports **all of them** and runs nothing. It finds:

- text it can't read: text that never ends, unknown escapes, unknown characters, indentation that doesn't line up
- lines that aren't valid statements
- names that don't exist, are used before they are created, or are created twice
- `set` on a value that isn't marked `var`, or that changes its kind
- operations on the wrong kinds of values, like `price + "5"`
- division by a written `0`
- `when` conditions that aren't true/false

### 10.2 While running

A few mistakes can only be found while running. When one happens, the program stops, and the lines before it have already run:

- dividing by a value that turns out to be zero
- a calculation whose result is too big for a number (§4.1)
- a `check` or `--- example:` that is false

### 10.3 How mistakes are reported

Every problem is reported the same way: where it is, what went wrong, why, and, when Quill can tell, what you probably meant:

```
Problem on line 2 of shop.ql:

    total = price + "5"
                    ^^^
You're adding Number (price) to Text ("5").
Quill won't guess which one you meant.

Did you mean:  total = price + 5
```

Quill never crashes on a mistake in a program. If it ever does, that is a bug in Quill.

## 11. The `quill` tool

| Command | What it does |
|---|---|
| `quill run FILE.ql` | Check the program, then run it |
| `quill FILE.ql` | Same as `quill run` |
| `quill check FILE.ql` | Check for mistakes without running anything |
| `quill test` | Run every `.ql` file in this folder and its subfolders that contains a `check` or `--- example:`, and report how many passed. Folders starting with `.`, and `target`, are skipped. |
| `quill explain FILE.ql:N` | Show how Quill read line N: what each part means, its kind, and where names were created |
| `quill new NAME` | Create a folder `NAME` holding a starter `hello.ql` |
| `quill tokens FILE.ql` | Show the pieces Quill read the file as, with their line and column |
| `quill update [VERSION]` | Install a release of Quill. With no version, shows a menu of every release to choose from with the arrow keys (or installs the latest when there's no terminal). `VERSION` is a version like `0.2.0`, or `latest`. Works for copies installed with the installer; a copy built from source explains how to update it with `cargo`. `quill upgrade` is another name for this command. |
| `quill help` | Show help |
| `quill version` | Show the version |

`quill` exits with code `0` on success and `1` when it reports a problem.

**Update check.** At most once a day, `quill` asks GitHub for the latest release, in a separate background process so commands are never slowed down, and remembers the answer in `~/.quill/update-check`. When a newer version exists, commands end with a one-line notice on standard error. Quill never updates itself. The check is skipped when standard error isn't a terminal, when the `CI` environment variable is set, when `curl` isn't available, and whenever `QUILL_NO_UPDATE_CHECK` is set.

## 12. Planned, not yet implemented

The README describes these features. They are part of Quill's design but not of this version of the language. Using them today is a mistake.

| Feature | README section |
|---|---|
| `for` loops and ranges (`1..15`) | Functions, `when` and tests |
| Functions (`fn`) and `return` | Functions, `when` and tests |
| `when` as a value, and `when` on a subject (`when find_user(42)`) | No null: `maybe` |
| `maybe`, `found` and `missing` | No null: `maybe` |
| Failures: `fails`, `on` and `try` | Failures you can't ignore |
| Types (`type`), fields and `where` constraints | Constrained types and exact money |
| `Money<CURRENCY>` | Constrained types and exact money |
| Contracts: `require` and `ensure` | Contracts |
| Permissions: `permit` | Permissions |
| Modules: `use`, `web`, `db`, `route`, `respond` | A small web API |

## 13. Grammar summary

This grammar describes the shape of programs. The rules above add the meaning and the mistakes Quill checks for. `NEWLINE`, `INDENT` and `DEDENT` come from §2.

```
program     = { statement | NEWLINE } ;

statement   = say | create | change | check | when ;
say         = "say" value ;
create      = [ "var" ] NAME "=" value ;
change      = "set" NAME "=" value ;
check       = ( "check" | "--- example:" ) sum COMPARE sum ;
when        = "when" NEWLINE INDENT case { case } DEDENT ;
case        = ( value | "else" ) "->" case_body NEWLINE ;
case_body   = say | change | check | when ;

value       = or ;
or          = and { "or" and } ;
and         = not { "and" not } ;
not         = "not" not | comparison ;
comparison  = sum [ COMPARE sum ] ;
sum         = product { ( "+" | "-" ) product } ;
product     = negative { ( "*" | "/" | "%" ) negative } ;
negative    = "-" negative | simple ;
simple      = TEXT | NUMBER | "true" | "false" | NAME | "(" value ")" ;

COMPARE     = "==" | "!=" | "<" | "<=" | ">" | ">=" ;
TEXT        = '"' { character | escape | "{" value "}" } '"'
            | '"""' { character | escape | "{" value "}" } '"""' ;
NUMBER      = digit { digit } [ "." digit { digit } ] ;
NAME        = ( letter | "_" ) { letter | digit | "_" } ;   (not a reserved word)
```
