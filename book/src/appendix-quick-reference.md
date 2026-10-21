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

size = when                     -- a when that gives a value
  total > 20 -> "big"
  else       -> "small"
say size

for i in 1..3                   -- 1, then 2, then 3
  say i

hobbies = list "coding", "reading"
say hobbies at 1

address = record city = "Anytown"
say address.city

fn double(n: number) -> number
  return n * 2
say double(21)

fn half(n: number) -> number
  require n >= 0                -- what the caller must pass
  ensure result <= n            -- what half promises to give back
  return n / 2
say half(21)

fn book(guests: number where 1..500) -> text   -- 1 through 500
  return "{guests} guests"
say book(120)

type Act                        -- every Act has these fields
  name: text
  fee: USD where 1 USD..500000 USD

act = Act name = "Brass", fee = 150000 USD
roster = list act
say sum fee of roster
say get fee of roster
say get name of roster where fee > 100000 USD   -- only some records
say split 100 USD into 3                        -- no cent lost
say round 10.125 USD to cents                   -- half to even
say 20% of 500 USD
say convert 100 USD to EUR at 0.92              -- the rate is written

up = record place = "Up Station", acts = list "Bamenda Brass"
old = record place = "Old Town", acts = list "Market Drums"
desk = record bookings = list up, old
say get place of desk.bookings
```

```output
Hi Amina, total 25.00
She said "hi".
Delivery: 5
big
1
2
3
coding
Anytown
42
10.5
120 guests
150000 USD
150000 USD
Brass
33.34 USD, 33.33 USD, 33.33 USD
10.12 USD
100 USD
92 EUR
Up Station, Old Town
```

## Kinds of values

| Kind | Examples |
|---|---|
| Text | `"Hello"`, `"Hi {name}"`, `"""He said "hi"."""` |
| Number | `42`, `0.10`, `-3.5` |
| true/false | `true`, `false`, `total > 10` |
| list | `list "coding", "reading"`, `hobbies at 1` |
| record | `record city = "Anytown"`, `address.city` |

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

Blocks (`when`, `for`, `fn`, `record`) are indented by exactly 2 spaces.

## Reserved words

`say` `check` `var` `set` `when` `else` `for` `in` `list` `record` `of` `at` `get` `sum` `with` `fn` `return` `maybe` `found` `missing` `fails` `fail` `on` `try` `require` `ensure` `where` `type` `round` `split` `convert` `bring` `permit` `as` `ask` `route` `respond` `true` `false` `and` `or` `not`

`fn double(n: number) -> number` names a calculation. `return n * 2` is the value the call becomes, and `double(21)` calls it. Leave off `->` when the function only does work, and call it on its own line. Each parameter has its own kind (`text`, `number`, `true/false`, a currency such as `USD`, `maybe` one of those, or `list of` one of those). The kind after `->` is the only kind `return` may give back.

`100 USD + 50 USD` is money. Two currencies cannot be added. `-> maybe number` might be `found` or `missing`, and `when` has to handle both. `fails Declined` names a failure, `fail Declined` leaves with it, and the caller writes `on Declined` or `try`. `on Declined -> say "..."` runs and stops; `on Declined -> "declined"` uses that value and carries on. `require` says what has to be true when a function is called, and `ensure` says what is true when it gives back; `result` is the value it gives back. `guests: number where 1..500` gives a parameter its range, ends included. `round total to cents` rounds half to even (`round up`/`round down` choose a side); `split 100 USD into 3` and `split fee by list 70, 30` never lose a cent; `20% of fee` takes a percent; `convert 100 USD to EUR at 0.92` changes currency at the rate you write. `bring billing` uses the functions and types in billing.quill, next to the file. `permit read "config"` (also `write`, `net`, `env`) says what the program may touch; `read_file`, `write_file`, `file_exists` and `env` need it. `body as Booking` reads JSON as a type and fails with `Invalid` when it doesn't fit; `json(value)` writes JSON. `bring http` adds `fetch(url)`, `post(url, body)` and `send(method, url, body, headers)`; each gives back `status` and `body`, needs `permit net`, and fails with `NetworkDown`. `bring ai` adds `ask "question"` (text) and `ask "question" as Review` (checked against the type); it fails with `ModelFailed` (and `Invalid` with `as`), and `permit env "ANTHROPIC_API_KEY"` or `"OPENAI_API_KEY"` chooses the provider. `bring web` adds `route GET "/path/{name}"` and `respond 200, value` (text as is, anything else as JSON); `quill run` serves on `PORT` (8080). `type Act` names the fields every `Act` has; `Act name = ..., fee = ...` makes one, with every field and its rules checked.

`at` picks one list item: `hobbies at 1` reads the first, and `set hobbies at 2 = "writing"` changes that one item. `of` is required only for an empty list: `list of text`, `list of number`, or `list of true/false`. A list that already has items leaves `of` out.

`sum fee of roster` adds the fee of each record. `get fee of roster` takes that field as a list. `get place of desk.bookings` reads a field of a list on a record, and `get gpa of school.acceptances` reads a field of a nested record. Give that record a name before the dot: `first = desk.bookings at 1`, then `first.place`. `get bookings with acts in desk` is the longer form: it names the list and the field embedded in it. `roster where fee > 100000 USD` keeps the records whose condition is true; inside it, a field is read by its name, and it works after `get ... of` and `sum ... of`. A missing amount is left out. `for` stays when each record needs its own step.

## Commands

| Command | What it does |
|---|---|
| `quill run FILE.quill` | run a program |
| `quill check FILE.quill` | find mistakes without running |
| `quill test` | run every test in this folder |
| `quill test shop` | run the tests in the folder `shop` |
| `quill explain FILE.quill:N` | show how Quill read line N |
| `quill new NAME` | start a project |
| `quill tokens FILE.quill` | show the pieces Quill read |
| `quill update` | choose a version of Quill to install (also `quill upgrade`) |
| `quill update VERSION` | install that version, or `latest` |
| `quill help` | show help |
| `quill version` | show the version |
