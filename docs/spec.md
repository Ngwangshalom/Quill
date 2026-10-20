# Quill Language Specification

**Version 0.1 draft**, describing Quill as implemented in release v0.4.0.

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
say  check  var  set  when  else  for  in  list  record  of  at  get  sum  with  fn  return  maybe  found  missing  fails  fail  on  try  require  ensure  where  type  round  split  convert  bring  permit  as  ask  route  respond  true  false  and  or  not
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
- Money: the amount, a space, and the currency. The amount is shown in whole units (`90 USD`), or with every digit of the currency's smallest unit (`7.50 USD`, not `7.5 USD`; `1.500 KWD`). Digits smaller than that unit stay visible (`0.125 USD`). Most currencies have 2 digits. BIF, CLP, DJF, GNF, ISK, JPY, KMF, KRW, PYG, RWF, UGX, VND, VUV, XAF, XOF and XPF have none. BHD, IQD, JOD, KWD, LYD, OMR and TND have 3.
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

### 5.6 Asking for a field

`get` and `sum` read a list of records without a loop. `for` still walks the list when each record needs its own step (§8.5). A program can use either.

```quill
sisters = record name = "Nchang Sisters", fee = 80000 USD
brass = record name = "Bamenda Brass", fee = 150000 USD
roster = list sisters, brass
till = sum fee of roster
fees = get fee of roster
say till
say fees
```

```output
230000 USD
80000 USD, 150000 USD
```

A missing amount is left out of the sum:

```quill
fn fee_for(name: text) -> maybe USD
  when
    name == "Quiet Hour" -> return missing
    else -> return found 40000 USD

quiet = record name = "Quiet Hour", fee = fee_for("Quiet Hour")
drums = record name = "Market Drums", fee = fee_for("Market Drums")
roster = list quiet, drums
say sum fee of roster
```

```output
40000 USD
```

Several fields stay together on a smaller record. Name the record before the dot:

```quill
sisters = record name = "Nchang Sisters", age = 24, fee = 80000 USD
people = get name, age, fee of list sisters
person = people at 1
say person.name
say person.fee
```

```output
Nchang Sisters
80000 USD
```

A dot after `of` reads a field further in. One nested record gives that one value. A list on a record gives a list:

```quill
school = record
  name = "Harvard"
  acceptances = record gpa = 4.0

say get gpa of school.acceptances

up = record place = "Up Station", acts = list "Bamenda Brass", "Hilltop Duo"
old = record place = "Old Town", acts = list "Market Drums"
desk = record bookings = list up, old
say get place of desk.bookings
```

```output
4.0
Up Station, Old Town
```

`get` can also name the list and the field embedded in each record. One record, or a list of records, can hold that list:

```quill
up = record place = "Up Station", acts = list "Bamenda Brass", "Hilltop Duo"
old = record place = "Old Town", acts = list "Market Drums"
desk = record bookings = list up, old
tonight = get bookings with acts in desk
first = tonight at 1
say first.place

saturday = record day = "Saturday", bookings = list up
sunday = record day = "Sunday", bookings = list old
nights = list saturday, sunday
all = get bookings with acts in nights
second = all at 2
say second.place
```

```output
Up Station
Old Town
```

- `sum NAME of LIST` adds `NAME` from each record. `NAME` is a number or money in one currency. The result is that number, or money in that currency. When the value after `of` is one record, the sum is that one amount: `sum fees of school`.
- `get NAME of LIST` gives a list of `NAME` from each record.
- `get NAME { "," NAME } of LIST` gives a list of records. Each record keeps only the named fields, in the order they were written. The same field twice is a mistake. When the value after `of` is one record, one field comes back as that value, and several fields come back as one record. A dot after `of` reaches a field further in: `get gpa of school.acceptances`, `get place of desk.bookings`. `record = list at 1` then `record.name` reads a field of that item. `(list at 1).name` is the same value.
- A field that is `maybe` contributes only when it is `found`. `missing` is left out. When every money amount is missing, `sum` is `0` in that currency.
- `get NAME with NESTED in SOURCE` gives the list `NAME`. Each record in that list has the field `NESTED`. `SOURCE` is one record, or a list of records that each hold `NAME`. A list of records is gathered into one list.
- `LIST where CONDITION` gives the records of `LIST` whose `CONDITION` is true, in their order. Inside `CONDITION`, each field of the records is a name, and it hides a value of the same name; other values are read as usual. `CONDITION` must be true/false. When nothing matches, the result is an empty list. `where` may follow again: `students where score >= 50 where paid` keeps records for which both are true.
- `where` binds after a whole value, so its condition takes in `and` and `or`: `students where score >= 50 and paid`. After `of`, a `where` applies to the list before `get` or `sum` reads it: `sum fee of roster where paid == false` adds the unpaid fees.
- The field names are checked before anything runs. A missing field, a list that is not records, or a `sum` of text is a mistake, and nothing runs. In a `where`, a name that is neither a field nor a value, a list that is not records, or a condition that is not true/false is a mistake.

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

A case **can't create a value** (`name = ...` or `var name = ...`), because the value would only exist when that case runs. Create it before the `when` with `var`, then change it inside with `set`, or use a `when` that gives a value (§8.4.1):

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

#### 8.4.1 A `when` that gives a value

`when` at the end of a line, in the place of a value, chooses a value instead of a statement:

```quill
delivery = when
  total > 100 -> 0
  total > 30  -> 5
  else        -> 10
```

- It can stand wherever a whole value is written at the end of a line: after `=`, `var NAME =`, `set NAME =`, `say` and `return`. The cases are on the lines below, indented.
- Each case is `CONDITION -> VALUE` or `else -> VALUE`. The value is an expression, not a statement: `say`, `set` and the other statement words are a mistake there.
- The last case must be `else`, so there is always a value. A case after `else` is a mistake.
- Every condition must be true/false. Every case must give the same kind of value. `missing` fits only when the other cases give `found` values; the `when` is then a maybe.
- The cases are tested in order, and the value of the first true case is the value of the `when`. Only that case's value is worked out.
- `when VALUE` on one line still opens a maybe (§8.8); it does not give a value.

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

parameter = NAME ":" kind [ "where" bound ".." bound ]
kind      = "text" | "number" | "true/false" | "list" "of" kind
```

- The body is indented further than `fn`, and `fn` stands on its own line.
- `-> kind` means the function gives a value of that kind back. Every path through the body has to reach `return VALUE`, and that value has to be the named kind. Parameters may each have their own kind. The kind after `->` is the only kind `return` may give back. Quill does not convert one kind into another.
- With no `-> kind`, the function gives nothing back. Call it on its own line. `return` on its own leaves the function early. A value after `return` is a mistake until the function names a kind after `->`.
- A parameter exists only inside the function. It is not a `var`. Names created in the body exist only inside the function, and they are created again on every call.
- A function does not see values created outside it. Pass them in as parameters.
- A function can call itself, and it can call a function written above it.
- `return` outside a function is a mistake.
- A parameter's kind is `text`, `number`, `true/false`, a currency such as `USD`, a type such as `Booking` (§8.11), `maybe` one of those, or `list of` one of those. Kinds are written in lowercase; `Int` and `Text` are not kinds.
- The first line of the body may be `fails NAME { "," NAME }`. A failure name starts with a capital letter. `fail NAME` leaves the function with that failure, and the name has to be in the list.

### 8.7 Money

An amount is a number, a space, and a currency of three capital letters.

```quill
say 100 USD + 50 USD
```

This shows `150 USD`.

- `+` and `-` need two amounts in the same currency. Different currencies are a mistake.
- `*` and `/` can combine an amount with a plain number: `100 USD * 0.90`, `100 USD / 2`. A number can also come first in a multiplication.
- The currency is the kind in a function: `price: USD`.
- A currency written in lowercase, such as `usd`, is a mistake. Write `USD`.
- "Cents" below means the currency's smallest unit (§4.3).

```
round [ "up" | "down" ] VALUE [ "to" ( "cents" | VALUE "places" ) ]
split VALUE "into" VALUE
split VALUE "by" VALUE
VALUE "%" "of" VALUE
convert VALUE "to" CURRENCY "at" VALUE
```

- `round VALUE` rounds a number or money to a whole number. `to N places` keeps `N` digits after the point, with `N` a whole number from 0 to 20; the result has exactly `N` digits. `to cents` is for money and keeps the currency's smallest unit. Plain `round` goes to the nearest value, and exactly half goes to the even digit (`10.125` → `10.12`, `10.135` → `10.14`). `round up` goes to the next larger value, and `round down` to the next smaller one (`round up -2.1` is `-2`).
- `split AMOUNT into COUNT` gives a list of `COUNT` amounts in whole cents that add up to `AMOUNT`. `COUNT` is a whole number, at least 1. Each part is `AMOUNT / COUNT` rounded down to cents, and the cents left over go one each to the first parts: `split 100 USD into 3` is `33.34 USD, 33.33 USD, 33.33 USD`.
- `split AMOUNT by SHARES` gives one amount for each number in the list `SHARES`, in proportion to it. Shares are zero or more, and at least one is above zero. Each part is rounded down to cents, and the cents left over go one each to the first parts.
- `split` needs an amount with no part smaller than a cent; otherwise the program stops. Round it first.
- `P% of VALUE` is `VALUE * P / 100`, for a number or money. `P` is a number.
- `convert AMOUNT to CURRENCY at RATE` is money in `CURRENCY`: `AMOUNT * RATE`. `RATE` is a number above zero. Converting to the same currency is a mistake.
- These are mistakes found before running: `round` of something that isn't a number or money; `to cents` on a number; places that aren't a whole number from 0 to 20; `split` of something that isn't money, `into` a count that isn't a whole number of at least 1, or `by` something that isn't a list of numbers; `% of` a value that isn't a number or money; `convert` of something that isn't money, to the same currency, at a rate that isn't a number or is a written value of zero or less. When those values are only known while running, the same mistakes stop the program (§10.2).
- `round`, `split` and `convert` are reserved words (§3.2). `up`, `down`, `to`, `cents`, `places`, `into` and `by` are ordinary names everywhere else.

### 8.8 Maybe

`-> maybe KIND` means the function gives back that kind, or gives back nothing.

- `return found VALUE` gives the value back. The value's kind is `KIND`.
- `return missing` gives nothing back. It is allowed when the function's result is a maybe.
- `when VALUE` opens a maybe. The cases are `found NAME -> STATEMENT` and `missing -> STATEMENT`. Both are required. `NAME` exists only on the `found` line.
- Using a maybe with `say`, or as a number, is a mistake. Open it with `when` first.

### 8.9 Failures

A function names the failures it can leave with. The caller handles each one, or passes them on.

```quill
fn charge(amount: USD) -> text
  fails Declined
  when
    amount <= 0 USD -> fail Declined
    else -> return "paid"
```

- `fails` goes at the top of the function, with any `require` and `ensure` lines (§8.10).
- `var name = charge(...)` (or `name =`, or `set name =`), then indented `on NAME -> ...` for every declared failure. On success the value is stored and the program continues. On failure the matching `on` line decides:
  - `on NAME -> STATEMENT` (`say`, `set`, `check`, `when`, `fail`, `return`, ...): the statement runs and the program stops, so later lines cannot use a value the call never gave back. Inside a function, it leaves the function, and a function that gives a value back must `return` on that line.
  - `on NAME -> VALUE`: the value is stored in place of the call's answer, and the program continues. It must be the kind the call gives back (`missing` fits a maybe).
  - Each `on` line chooses for itself, so one failure can give a value while another stops.
- A call on its own line gives nothing back, so the `on` lines under it run statements. A call after `->` there, like `on Oops -> log("x")`, is run as a statement; any other value is a mistake.
- `try call` is an expression inside a function whose `fails` list includes every failure of that call. It passes those failures upward. `try` on a line by itself is a mistake (§7.7); a call on its own line passes a failure upward with `on NAME -> fail NAME`.
- A call that can fail, with no `on` and no `try`, is a mistake.

### 8.10 Contracts: `require`, `ensure` and `where`

A function can state what has to be true when it is called, and what it promises is true when it gives back.

```quill
fn withdraw(balance: USD, amount: USD) -> USD
  require amount > 0 USD
  ensure result >= 0 USD
  return balance - amount
```

- `require VALUE` and `ensure VALUE` go at the top of the function, before its first other line. They may be mixed with `fails` in any order. A function can have any number of each.
- `VALUE` is a condition: it must be true/false (§6). Only one condition fits on a line; join conditions with `and`.
- `require` can read the parameters. Every `require` is tested in order when the function is called, before its first line runs.
- `ensure` can read the parameters and `result`, the value the function gives back. Every `ensure` is tested in order at each `return`, after the value is worked out and before the function leaves. A function that leaves with `fail` does not test its `ensure` lines.
- `result` is defined only inside `ensure`. Elsewhere it is an ordinary name.
- A parameter can carry a range: `guests: number where 1..500`. The parameter's kind must be `number` or a currency. Each end is a written number or amount, optionally with a leading `-` (`-40`, `0 USD`), of the same kind as the parameter, and the first is not larger than the second. The range includes both ends. When the function is called, each parameter's range is tested in order, before every `require`.
- When a call passes a written number or amount outside a range, that is a mistake found before running (§10.1). Any other value outside the range stops the program (§10.2) with the function's name, the parameter, the range, its value, and the line of the call.
- When a contract is true, nothing happens. When it is false, the program stops (§10.2) with the function's name, the condition, and the value of every name it read. A broken `require` also names the line of the call.
- These are mistakes: `ensure` on a function with no `->`; `require` that reads `result`; a parameter called `result` in a function with an `ensure`; a condition that isn't true/false; `require` or `ensure` outside a function or below its first other line; a second `fails` line; `where` on a parameter that isn't a number or a currency, on the kind after `->`, with an end that isn't a written value, with an end of another kind, or with its ends in the wrong order.
- Contracts are not tests: `quill test` does not count them.

### 8.11 Types

A type names the fields every record of that kind has.

```quill
type Booking
  venue: text
  guests: number where 1..500
  fee: USD

hall = Booking venue = "Town Hall", guests = 120, fee = 500 USD
```

```
type NAME NEWLINE INDENT { NAME ":" kind [ "where" bound ".." bound ] NEWLINE } DEDENT
NAME field { "," field }
NAME NEWLINE INDENT { field NEWLINE } DEDENT
```

- A type is written at the top level of a file. Its name starts with a capital letter and is not three capitals (those are currencies). Each field is on its own line, with a kind as in §8.6 and an optional range as in §8.10. A type can be used above the line that writes it.
- `NAME field, ...` makes a record of the type. It has every field of the type, each of the field's kind, and no others. A `maybe` field is given `missing` or `found VALUE`. The record's fields are kept in the type's order.
- A record of a type is a record (§5.5): `.`, `get`, `sum` and `where` read it, and it is shown like any record.
- A field's range is tested each time a record of the type is made. A written number or amount outside it is a mistake found before running; any other value stops the program (§10.2), naming the type, the field, the range and the value.
- Where a kind names a type (`b: Booking`, `-> Booking`, `list of Booking`), a record with the same fields and kinds fits. A function with a parameter of a type tests that type's ranges when it is called, before its own `require` lines.
- These are mistakes: a type name that isn't capitalized; two types with the same name; a type with no fields, or two fields with the same name; a type inside a function; a kind naming a type that doesn't exist; a type that holds itself, directly or through `maybe`, `list of` or another type; a record of a type with a field missing, an unknown field, or a field of the wrong kind.

### 8.12 Bringing in files: `bring`

A program can use the functions and types written in other files.

```quill
bring billing
bring "shared/money.ql"
```

- `bring NAME` reads `NAME.ql` in the folder of the file that writes it. `bring "PATH"` reads the file at `PATH`, counted from that folder, with `/` between folders; it must end in `.ql`. `bring` goes at the top level of a file.
- A brought file holds `fn`, `type` and `bring` lines, comments, and its own tests. Its functions and types can be used by name in the program, as if written above its first line. Its `check` and `--- example:` lines run only when that file itself is run or tested. Any other line in a brought file is a mistake.
- A file brought twice, directly or through other files, is read once. Files that bring each other in a circle are a mistake.
- A function or type name written in two different files is a mistake.
- A problem in a brought file is reported with that file's name and line.
- `web`, `db`, `ai` and `http` name Quill's modules. `bring web` brings the module, not a file.

### 8.13 Permits, files and settings

A program touches nothing outside itself unless it permits it.

```quill
permit read "config"
permit write "reports"
permit env "STRIPE_KEY"
permit net "api.stripe.com"
```

- `permit KIND "TARGET"` goes at the top level of the program that is run. `KIND` is `read` or `write` (a file or folder), `net` (a host), or `env` (a setting). A brought file (§8.12) can't contain a permit.
- A file or folder permit covers that path and everything inside it, with `.` and `..` worked out first, so `..` can't leave a permitted folder. `"."` covers the program's folder and everything below it. Paths are counted from the program's folder.
- `permit net` names a host such as `api.stripe.com`, or `*.example.com` for every host below that domain, without `https://` or a path. A host without a port allows any port.
- `permit env` names one setting, made of letters, digits and `_`.
- Quill's own functions:

| Function | Gives back | Fails with | Needs |
|---|---|---|---|
| `read_file(path: text)` | the file's text | `FileMissing` | `permit read` |
| `write_file(path: text, text: text)` | nothing; makes missing folders | `CantWrite` | `permit write` |
| `file_exists(path: text)` | `true/false` | | `permit read` |
| `env(name: text)` | `maybe text` | | `permit env` |
| `json(value)` | the value as JSON text (§8.14) | | |

- A call whose path or name is written as text in the call is checked against the permits before running; without a permit it is a mistake. Any other value is checked when the call runs, and without a permit the program stops (§10.2).
- These names belong to Quill: a function can't be called `read_file`, `write_file`, `file_exists`, `env`, `json`, `fetch`, `post` or `send`.

### 8.14 JSON

```quill
booking = body as Booking
  on Invalid -> say "That isn't a booking."
say json(booking)
```

- `TEXT as KIND` reads JSON text as `KIND`, any kind a function can name (§8.6). It is a call that can fail with `Invalid`, so it needs `on Invalid` or `try` (§8.9). `as` follows a value, like `.` and `at`.
- A JSON number or text such as `"500.00"` is read as money of the kind's currency, digit for digit. Numbers are never read through floating point; a number with an exponent, like `1e3`, is `Invalid`.
- An object is read as a record of a type: every field of the type must be there, except `maybe` fields, which may be `null` or left out and are then `missing`. Fields the type doesn't name are left out. Each field's range (§8.11) must hold. A JSON array is read as `list of KIND`.
- Anything else that doesn't fit is `Invalid`: text that isn't JSON, a value of another kind, a missing field, or a value outside a range.
- `json(VALUE)` gives back the value as JSON text. Text becomes a string, numbers and money exact numbers, true/false booleans, lists arrays, records objects with their fields in order, a `found` value its value, and `missing` `null`.

### 8.15 Web requests: `bring http`

```quill
bring http
permit net "api.example.com"

answer = fetch("https://api.example.com/rates")
  on NetworkDown -> say "offline"
say answer.status
```

| Function | Sends |
|---|---|
| `fetch(url: text)` | a `GET` request |
| `post(url: text, body: text)` | a `POST` request with the header `Content-Type: application/json` |
| `send(method: text, url: text, body: text, headers: list of text)` | any of `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`; `""` is no body; each header is `"Name: value"` |

- Each needs `bring http` (§8.12) and a `permit net` (§8.13) covering the URL's host. A URL starts with `https://` or `http://`.
- Each gives back an answer: a record with `status`, a number, and `body`, text. Any answer the service gives is an answer, including statuses such as `404` and `500`.
- Each fails with `NetworkDown` when the service can't be reached or doesn't answer within 30 seconds.
- A method or header that isn't valid stops the program (§10.2).

### 8.16 Asking a model: `bring ai`

```quill
bring ai
permit net "api.anthropic.com"
permit env "ANTHROPIC_API_KEY"

summary = ask "Summarize: {text}"
  on ModelFailed -> "none"
review = ask "Rate it: {text}" as Review
  on ModelFailed -> say "no answer"
  on Invalid -> say "an answer that broke Review's rules"
```

- `ask VALUE` sends the text `VALUE` to a model and gives back its answer as text. `ask VALUE as KIND` gives back the answer as `KIND`, any kind a function can name (§8.6). `VALUE` is everything up to `as` or the end of the value.
- `ask` is a call. It fails with `ModelFailed` when the model can't be reached, answers with an error, declines, or gives no text. `ask ... as KIND` also fails with `Invalid` when the answer doesn't fit `KIND` as `as` reads it (§8.14), including a `where` range.
- It needs `bring ai`, and a `permit env` for a key, which chooses the provider: `ANTHROPIC_API_KEY` asks Claude; otherwise `OPENAI_API_KEY` asks a service that speaks the OpenAI chat format. The service's host needs a `permit net`. For Claude, a program with no `permit net` at all is a mistake found before running.
- Settings read without a permit, because they only choose where the request goes: `QUILL_AI_MODEL` (the model; Claude's default is `claude-opus-5-5`, and an OpenAI-compatible service needs it), `ANTHROPIC_BASE_URL` and `OPENAI_BASE_URL` (the service's address). The request still needs a `permit net` for that address.
- With `as KIND`, the model is sent the kind as a JSON schema: each field is required, a `maybe` field may be `null`, and a range is described in words. A kind that isn't a record is asked for as `{"value": ...}`.
- When Claude's own API declines a question for a model that offers it, Anthropic's recommended model answers instead.
- A key permitted but not set, or an OpenAI-compatible service with no `QUILL_AI_MODEL`, stops the program (§10.2).

### 8.17 A web service: `bring web`

```quill
bring web

route GET "/hello/{name}"
  respond 200, "Hello, {name}!"
```

```
route METHOD "PATH" NEWLINE INDENT { statement } DEDENT
respond STATUS "," VALUE
```

- A route is written at the top level of a program that brings `web`. `METHOD` is `GET`, `POST`, `PUT`, `PATCH` or `DELETE`. `PATH` starts with `/`; each `{NAME}` in it matches one part of a requested path and is a text value named `NAME` inside the route, with `%` escapes worked out.
- Inside a route, `request` is a record with `method`, `path`, `query` (after `?`, or `""`) and `body`, all text. A route doesn't see values created outside it, like a function (§8.6).
- `respond STATUS, VALUE` ends the route with its answer, as `return` ends a function. `STATUS` is a whole number from 100 through 599. Text is sent as `text/plain`; any other value as JSON (§8.14), as `application/json`. Every path through a route must reach `respond`, which may also stand after `on ... ->`. `return` in a route and `respond` outside one are mistakes.
- `Request` and `Response` are types the web module gives; a program can't write types with those names.
- `quill run` runs the program's top level, then answers requests on port `PORT` (8080 when the setting isn't set) until it is stopped. A request runs the route whose method and path match. A path no route has is answered `404`; a path a route has with another method `405`. A route that stops the program (§10.2) is answered `500`, its problem is shown where the server runs, and the server keeps answering. `quill check` and `quill test` don't serve.

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

An example runs where it is written, with one exception: examples directly above a `fn` (with only comments and blank lines between) document that function, and run just after the function is written, so they can call it. When an example fails, Quill points at the line where it was written.

```quill
--- example: with_vat(100 USD) == 105 USD
fn with_vat(price: USD) -> USD
  return price * 1.05
```

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
- `require`, `ensure` and `where` that break the rules in §8.10, and a written value passed outside a parameter's `where` range
- a `for` range whose ends aren't numbers, a `for` that doesn't walk a list, or `set` on the loop's own name
- a list whose items aren't the same kind, a list position that isn't a whole number from 1, or a record field that isn't there

### 10.2 While running

A few mistakes can only be found while running. When one happens, the program stops, and the lines before it have already run:

- dividing by a value that turns out to be zero
- a calculation whose result is too big for a number (§4.1)
- a `check` or `--- example:` that is false
- a `require` or `ensure` that is false, or a value outside a parameter's `where` range (§8.10)
- a list position past the end, or a position in an empty list
- a file, host or setting that no permit covers (§8.13)

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

Integer remainder overflow produces an arithmetic diagnostic rather than a host panic.

Quill never crashes on a mistake in a program. If it ever does, that is a bug in Quill.

## 11. The `quill` tool

| Command | What it does |
|---|---|
| `quill run FILE.ql` | Check the program, then run it |
| `quill FILE.ql` | Same as `quill run` |
| `quill check FILE.ql` | Check for mistakes without running anything |
| `quill lsp` | Speak the Language Server Protocol on standard input and output. Report the same mistakes as `quill check`, so an editor can underline them. Do not run the program. |
| `quill test FILE.ql` | Run the selected file, including files with no checks. Missing files and non-`.ql` paths are errors. |
| `quill test` | Run every `.ql` file in this folder and its subfolders that contains a `check` or `--- example:`, and report how many passed. Folders starting with `.`, and `target`, are skipped. |
| `quill test FOLDER` | The same as `quill test`, for the files in `FOLDER` and its subfolders. |
| `quill explain FILE.ql:N` | Show how Quill read line N: what each part means, its kind, and where names were created |
| `quill new NAME` | Create a folder `NAME` holding a starter `hello.ql` |
| `quill tokens FILE.ql` | Show the pieces Quill read the file as, with their line and column |
| `quill update [VERSION]` | Install a release of Quill. With no version, shows a menu of every release to choose from with the arrow keys (or installs the latest when there's no terminal). `VERSION` is a version like `0.2.0`, or `latest`. Works for copies installed with the installer; a copy built from source explains how to update it with `cargo`. `quill upgrade` is another name for this command. |
| `quill help` | Show help |
| `quill version` | Show the version |

Test reports count successful assertions actually executed, including doc examples, function calls and each loop iteration. Unexecuted assertions do not count. A failing file stops at its first failure; its earlier passes remain in the summary. Project testing continues with other files.

`quill` exits with code `0` on success and `1` when it reports a problem.

**Update check.** At most once a day, `quill` asks GitHub for the latest release, in a separate background process so commands are never slowed down, and remembers the answer in `~/.quill/update-check`. When a newer version exists, commands end with a one-line notice on standard error. Quill never updates itself. The check is skipped when standard error isn't a terminal, when the `CI` environment variable is set, when `curl` isn't available, and whenever `QUILL_NO_UPDATE_CHECK` is set.

## 12. Planned, not yet implemented

The README describes these features. They are part of Quill's design but not of this version of the language. Using them today is a mistake.

| Feature | README section |
|---|---|
| `require` and `ensure` proven before running (static verification) | Contracts |
| Permissions: `permit` | Permissions |
| Modules: `use`, `web`, `db`, `route`, `respond` | A small web API |

## 13. Grammar summary

This grammar describes the shape of programs. The rules above add the meaning and the mistakes Quill checks for. `NEWLINE`, `INDENT` and `DEDENT` come from §2.

```
program     = { statement | NEWLINE } ;

statement   = say | create | change | check | when | match | for | function | type | return | call | fail ;
type        = "type" NAME NEWLINE INDENT parameter NEWLINE { parameter NEWLINE } DEDENT ;
say         = "say" value ;
create      = [ "var" ] NAME "=" value ;
change      = "set" NAME { "." NAME | "at" sum } "=" value ;
check       = ( "check" | "--- example:" ) sum COMPARE sum ;
when        = "when" NEWLINE INDENT case { case } DEDENT ;
case        = ( value | "else" ) "->" case_body NEWLINE ;
when_value  = "when" NEWLINE INDENT value_case { value_case } DEDENT ;   (the last case is else)
value_case  = ( value | "else" ) "->" value NEWLINE ;
handler     = "on" NAME "->" ( statement | value ) NEWLINE ;
case_body   = say | change | check | when | for ;
for         = "for" NAME "in" ( value ".." value | value ) NEWLINE INDENT { for_body } DEDENT ;
for_body    = say | change | check | when | for | return ;
function    = "fn" NAME "(" [ parameter { "," parameter } ] ")" [ "->" kind ] NEWLINE INDENT { header } { fn_body } DEDENT ;
header      = ( "fails" NAME { "," NAME } | "require" value | "ensure" value ) NEWLINE ;
parameter   = NAME ":" kind [ "where" bound ".." bound ] ;
bound       = [ "-" ] NUMBER [ CURRENCY ] ;
kind        = "text" | "number" | "true/false" | CURRENCY | TYPE | "maybe" kind | "list" "of" kind ;
match       = "when" value NEWLINE INDENT catch { catch } DEDENT ;
catch       = ( "found" NAME | "missing" ) "->" statement ;
fail        = "fail" NAME ;
CURRENCY    = three capital letters ;
fn_body     = say | create | change | check | when | for | return ;
return      = "return" [ value ] ;
call        = NAME "(" [ value { "," value } ] ")" ;

value       = or { "where" or } ;
or          = and { "or" and } ;
and         = not { "and" not } ;
not         = "not" not | comparison ;
comparison  = sum [ COMPARE sum ] ;
sum         = product { ( "+" | "-" ) product } ;
product     = negative { ( "*" | "/" | "%" ) negative | "%" "of" negative } ;
negative    = "-" negative | atom { "." NAME | "at" sum } ;
atom        = TEXT | NUMBER | "true" | "false" | NAME | call | "(" value ")" | list | record | typed | ask | total | when_value | money ;
money       = "round" [ "up" | "down" ] sum [ "to" ( "cents" | negative "places" ) ]
            | "split" sum ( "into" | "by" ) sum
            | "convert" sum "to" CURRENCY "at" negative ;
typed       = TYPE ( field { "," field } | NEWLINE INDENT field { field } DEDENT ) ;
ask         = "get" NAME { "," NAME } "of" negative { "where" or } | "get" NAME "with" NAME "in" negative ;
total       = "sum" NAME "of" negative { "where" or } ;
list        = "list" ( "of" ( "text" | "number" | "true/false" ) | sum { "," sum } ) ;
record      = "record" ( NEWLINE INDENT field { field } DEDENT | field { "," field } ) ;
field       = NAME "=" value ;

COMPARE     = "==" | "!=" | "<" | "<=" | ">" | ">=" ;
TEXT        = '"' { character | escape | "{" value "}" } '"'
            | '"""' { character | escape | "{" value "}" } '"""' ;
NUMBER      = digit { digit } [ "." digit { digit } ] ;
NAME        = ( letter | "_" ) { letter | digit | "_" } ;   (not a reserved word)
```
