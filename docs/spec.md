# Quill Language Specification

**Version 0.1 draft**, describing Quill as implemented in release v0.3.0.

This document says exactly what Quill programs mean. The [README](https://github.com/Ngwangshalom/Quill#readme) shows where the language is going; this spec covers what works today, and marks planned features clearly in [§12](#12-planned-not-yet-implemented). When the implementation and this document disagree, that is a bug in one of them. Please [open an issue](https://github.com/Ngwangshalom/Quill/issues).

## Contents

1. [Programs and files](#1-programs-and-files)
2. [Lines, indentation and comments](#2-lines-indentation-and-comments)
3. [Words and names](#3-words-and-names)
4. [Kinds of values](#4-kinds-of-values)
5. [Writing values](#5-writing-values)
6. [Calculations and conditions](#6-calculations-and-conditions)
7. [Statements](#7-statements)
8. [`when` and `for`](#8-when)
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

Indentation marks a block: lines that belong to the line above them. Today, the cases of a `when`, the body of a `for` (§8), and the fields of a `record` (§5.5) are indented.

- A block moves in by **exactly 2 spaces** from the line above it. One space, four spaces, or any other width is a mistake.
- A deeper block moves in by another 2 spaces: 2, then 4, then 6.
- A block ends when a line returns to an earlier level. That line must land on a level of 2 spaces, in the same column as the line that opened the block.
- Indent with spaces. A tab is a mistake, because editors show tabs at different widths.
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
say  check  var  set  when  else  for  in  list  record  of  at  fn  return  maybe  found  missing  fails  fail  on  try  true  false  and  or  not
```

## 4. Kinds of values

Every value has a kind. The checker knows the kind of every value before the program runs, so it can catch mistakes like adding text to a number.

| Kind | Examples | What it's for |
|---|---|---|
| **Text** | `"Amina"`, `"Total: {total}"` | words and sentences |
| **Number** | `42`, `0.10`, `-3.5` | exact decimal numbers |
| **true/false** | `true`, `false`, `total > 10` | conditions |
| **list** | `list "coding", "reading"`, `list of text` | values in order, all one kind |
| **record** | `record city = "Anytown"` | named fields kept together |

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
- list: the items separated by commas, or `an empty list` when there are none. `list "coding", "reading"` is shown as `coding, reading`.
- record: one field per line, as `name: value`.

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
- A point with no digit after it is a field dot (§5.5), not part of the number. `3.` with no field name is a mistake. Write `3.0` for a number.
- Negative numbers are written with `-` in front (§6.1).

### 5.4 true and false

`true` and `false` are the two true/false values. Comparisons (§6.2) also produce them.

### 5.5 Lists and records

A **list** is values in order, all of one kind. The first item is at position 1.

```quill
hobbies = list "coding", "reading", "traveling"
say hobbies at 1
```

- Items are separated by commas. A comma with nothing after it is a mistake.
- Every item has the same kind. `list "coding", 2` is a mistake.
- An empty list names its kind: `list of text`, `list of number`, or `list of true/false`.
- `VALUE at POSITION` reads one item. `POSITION` is a whole number, and `1` is the first item. `0` and numbers that aren't whole are mistakes. A position past the end is found while running, unless both the list and the position are written out, in which case it is found before running.
- Square brackets are a mistake. The word is `list`.

A **record** is named fields.

```quill
address = record
  street = "123 Main St"
  city = "Anytown"
```

- On its own line, `record` is followed by an indented block. Each field is `NAME = value`, indented by exactly 2 spaces (§2.2).
- A short record can be written on one line: `record city = "Anytown", state = "CA"`.
- A record has at least one field. The same field name twice is a mistake.
- `VALUE.NAME` reads one field. A field that isn't there is a mistake, and Quill names the fields the record does have.
- Two records are the same kind when they have the same field names and each matching field has the same kind. The order of the fields does not change the kind.
- Braces are a mistake here. The word is `record`. Braces inside text still show a value (§5.2).

`set` can replace one item or one field when the list or record was created with `var`:

```quill
set hobbies at 2 = "writing"
set address.city = "Springfield"
```

The new value must match the item's or the field's kind. `for NAME in LIST` walks a list (§8.5).

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
- `set NAME.FIELD = value` and `set NAME at POSITION = value` change one field or one item. The list or record itself must be marked `var`. The new value must match that field or item.
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
- A `when` whose cases are conditions stands on its own line. `when VALUE` on one line opens a maybe (§8.8).
- A case that comes after `else` can never run, so it is a mistake.

### 8.5 `for`

`for` repeats the lines under it for each number from the start through the end, including both:

```quill
for i in 1..3
  say i
```

This shows `1`, then `2`, then `3`.

```
for NAME in START .. END
  STATEMENT
```

- `START` and `END` are numbers. They are worked out once, before the first pass.
- The range is two dots. Three dots (`1...3`) is a mistake; write `1..3`.
- `NAME` holds the number for this pass. It exists only on the lines under the `for`. After the loop, the name is gone, unless a value with that name already existed. In that case the loop uses the name while it runs, and the earlier value is unchanged afterwards. The range sees the earlier value: `for i in i..12` when `i` is `10` counts from 10 through 12.
- The loop moves `NAME` to the next number by adding 1. `set` on that name is a mistake. To keep a total, create a `var` before the loop and change it with `set`.
- A line under `for` can't create a value (`name = ...` or `var name = ...`).
- If `START` is greater than `END`, the body never runs. `for i in 5..5` runs once.
- The body is indented further than `for`, and `for` stands on its own line.

`for` also walks a list, once for each item:

```quill
for hobby in hobbies
  say hobby
```

- `hobbies` is a list. It is worked out once, before the first pass.
- `hobby` holds the item for this pass. It exists only on the lines under the `for`, and `set` on that name is a mistake.
- The first pass takes the item at position 1. An empty list runs the body zero times.
- `for i in 1` with no `..` and no list is a mistake. A range needs `..`.

### 8.6 Functions

A function names a piece of work. A call runs it.

```quill
fn double(n: number) -> number
  return n * 2

say double(21)
```

This shows `42`.

```
fn NAME "(" [ parameter { "," parameter } ] ")" [ "->" kind ]
  STATEMENT

parameter = NAME ":" kind
kind      = "text" | "number" | "true/false" | "list" "of" kind
```

- The body is indented further than `fn`, and `fn` stands on its own line.
- `-> kind` means the function gives a value of that kind back. Every path through the body has to reach `return VALUE`, and that value has to be the named kind. Parameters may each have their own kind. The kind after `->` is the only kind `return` may give back. Quill does not convert one kind into another.
- With no `-> kind`, the function gives nothing back. Call it on its own line. `return` on its own leaves the function early. A value after `return` is a mistake until the function names a kind after `->`.
- A parameter exists only inside the function. It is not a `var`. Names created in the body exist only inside the function, and they are created again on every call.
- A function does not see values created outside it. Pass them in as parameters.
- A function can call itself, and it can call a function written above it.
- `return` outside a function is a mistake.
- Kind names such as `Int` and `Text` are not part of this version. A parameter's kind is `text`, `number`, `true/false`, a currency such as `AED`, `maybe` one of those, or `list of` one of those.
- The first line of the body may be `fails NAME { "," NAME }`. A failure name starts with a capital letter. `fail NAME` leaves the function with that failure, and the name has to be in the list.

### 8.7 Money

An amount is a number, a space, and a currency of three capital letters.

```quill
say 100 AED + 50 AED
```

This shows `150 AED`.

- `+` and `-` need two amounts in the same currency. Different currencies are a mistake.
- `*` and `/` can combine an amount with a plain number: `100 AED * 0.90`, `100 AED / 2`. A number can also come first in a multiplication.
- The currency is the kind in a function: `price: AED`.
- A currency written in lowercase, such as `aed`, is a mistake. Write `AED`.

### 8.8 Maybe

`-> maybe KIND` means the function gives back that kind, or gives back nothing.

- `return found VALUE` gives the value back. The value's kind is `KIND`.
- `return missing` gives nothing back. It is allowed when the function's result is a maybe.
- `when VALUE` opens a maybe. The cases are `found NAME -> STATEMENT` and `missing -> STATEMENT`. Both are required. `NAME` exists only on the `found` line.
- Using a maybe with `say`, or as a number, is a mistake. Open it with `when` first.

### 8.9 Failures

A function names the failures it can leave with. The caller handles each one, or passes them on.

```quill
fn charge(amount: AED) -> text
  fails Declined
  when
    amount <= 0 AED -> fail Declined
    else -> return "paid"
```

- `fails` is the first line under `fn`.
- `var name = charge(...)`, then indented `on NAME -> STATEMENT` for every declared failure. On success the variable is set and the program continues. On failure the `on` line runs and the program stops, so later lines cannot use a value the call never gave back.
- `try call` is an expression inside a function whose `fails` list includes every failure of that call. It passes those failures upward.
- A call that can fail, with no `on` and no `try`, is a mistake.

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
- a `for` range whose ends aren't numbers, a `for` that doesn't walk a list, or `set` on the loop's own name
- a list whose items aren't the same kind, a list position that isn't a whole number from 1, or a record field that isn't there

### 10.2 While running

A few mistakes can only be found while running. When one happens, the program stops, and the lines before it have already run:

- dividing by a value that turns out to be zero
- a calculation whose result is too big for a number (§4.1)
- a `check` or `--- example:` that is false
- a list position past the end, or a position in an empty list

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
| `quill lsp` | Speak the Language Server Protocol on standard input and output. Report the same mistakes as `quill check`, so an editor can underline them. Do not run the program. |
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
| `when` as a value | No null: `maybe` |
| Parameter kinds written as `Int` or `Text` | Functions, `when` and tests |
| Named types such as `User`, and `Money<AED>` as a type | Constrained types and exact money |
| Types (`type`), fields and `where` constraints | Constrained types and exact money |
| Contracts: `require` and `ensure` | Contracts |
| Permissions: `permit` | Permissions |
| Modules: `use`, `web`, `db`, `route`, `respond` | A small web API |

## 13. Grammar summary

This grammar describes the shape of programs. The rules above add the meaning and the mistakes Quill checks for. `NEWLINE`, `INDENT` and `DEDENT` come from §2.

```
program     = { statement | NEWLINE } ;

statement   = say | create | change | check | when | match | for | function | return | call | fail ;
say         = "say" value ;
create      = [ "var" ] NAME "=" value ;
change      = "set" NAME { "." NAME | "at" sum } "=" value ;
check       = ( "check" | "--- example:" ) sum COMPARE sum ;
when        = "when" NEWLINE INDENT case { case } DEDENT ;
case        = ( value | "else" ) "->" case_body NEWLINE ;
case_body   = say | change | check | when | for ;
for         = "for" NAME "in" ( value ".." value | value ) NEWLINE INDENT { for_body } DEDENT ;
for_body    = say | change | check | when | for | return ;
function    = "fn" NAME "(" [ parameter { "," parameter } ] ")" [ "->" kind ] NEWLINE INDENT { fn_body } DEDENT ;
parameter   = NAME ":" kind ;
kind        = "text" | "number" | "true/false" | CURRENCY | "maybe" kind | "list" "of" kind ;
match       = "when" value NEWLINE INDENT catch { catch } DEDENT ;
catch       = ( "found" NAME | "missing" ) "->" statement ;
fail        = "fail" NAME ;
CURRENCY    = three capital letters ;
fn_body     = say | create | change | check | when | for | return ;
return      = "return" [ value ] ;
call        = NAME "(" [ value { "," value } ] ")" ;

value       = or ;
or          = and { "or" and } ;
and         = not { "and" not } ;
not         = "not" not | comparison ;
comparison  = sum [ COMPARE sum ] ;
sum         = product { ( "+" | "-" ) product } ;
product     = negative { ( "*" | "/" | "%" ) negative } ;
negative    = "-" negative | atom { "." NAME | "at" sum } ;
atom        = TEXT | NUMBER | "true" | "false" | NAME | call | "(" value ")" | list | record ;
list        = "list" ( "of" ( "text" | "number" | "true/false" ) | sum { "," sum } ) ;
record      = "record" ( NEWLINE INDENT field { field } DEDENT | field { "," field } ) ;
field       = NAME "=" value ;

COMPARE     = "==" | "!=" | "<" | "<=" | ">" | ">=" ;
TEXT        = '"' { character | escape | "{" value "}" } '"'
            | '"""' { character | escape | "{" value "}" } '"""' ;
NUMBER      = digit { digit } [ "." digit { digit } ] ;
NAME        = ( letter | "_" ) { letter | digit | "_" } ;   (not a reserved word)
```
