# Lists and records

Sometimes one value is really several. A person's hobbies are a **list**: values in order, all of the same kind. An address is a **record**: named fields kept together. Quill uses those two words, so the code says what it is.

## A list

Write `list`, then the items, separated by commas:

```quill
hobbies = list "coding", "reading", "traveling"
say hobbies
say hobbies at 1
```

```output
coding, reading, traveling
coding
```

- Every item is the same kind. A list of text stays a list of text.
- `at` picks one item. Lists count from 1, so `hobbies at 1` is `"coding"` and `hobbies at 3` is `"traveling"`.
- An empty list says its kind, because there is no item to learn it from: `list of text`, `list of number`, or `list of true/false`.

[Where `of` and `at` are used](#where-of-and-at-are-used) says when each word is required.

A list of numbers works the same way: `prices = list 12.50, 4, 8.25`.

Mixing kinds is a mistake:

```quill,error
hobbies = list "coding", 2
```

```output
Problem on line 1 of example.ql:

    hobbies = list "coding", 2
                             ^
Every item in a list has to be the same kind.
This list holds Text, but 2 is Number.
```

Square brackets are a mistake too. Quill's name for this is `list`:

```quill,error
hobbies = ["coding", "reading"]
```

```output
Problem on line 1 of example.ql:

    hobbies = ["coding", "reading"]
              ^
Quill writes a list with the word list, not with square brackets.
A list is values of one kind, in order: list "coding", "reading".

Did you mean:  list "coding", "reading"
```

## Where `of` and `at` are used

`of` and `at` are reserved words, so they can't be names. Each one has a single job, and in that job it is required.

### `at` picks one item

Write the list, then `at`, then the position. The position is a whole number, and `1` is the first item:

```quill
hobbies = list "coding", "reading", "traveling"
say hobbies at 2
```

```output
reading
```

Use `at` whenever you want one item by its place. To change that one item, the list has to be a `var`, and `at` stays in the `set`:

```quill
var hobbies = list "coding", "reading"
set hobbies at 2 = "writing"
say hobbies at 2
```

```output
writing
```

`at` is for one position. Walking every item is `for hobby in hobbies`, which uses `in`. A field of a record is a dot: `address.city`. Replacing the whole list is `set hobbies = list "cooking"`, with no `at`.

### `of` names an empty list

A list that already has items leaves `of` out. The items show the kind: `list "coding", "reading"` is a list of text.

An empty list has no items to learn from, so `of` is required. The kind after it is `text`, `number`, or `true/false`:

```quill
waiting = list of text
prices = list of number
flags = list of true/false
say waiting
say prices
say flags
```

```output
an empty list
an empty list
an empty list
```

A `list` with nothing after it is a mistake. Quill needs the items, or `of` and a kind:

```quill,error
waiting = list
```

```output
Problem on line 1 of example.ql:

    waiting = list
              ^^^^
list needs its items after it, separated by commas.
An empty list names its kind: list of text, list of number, or list of true/false.

Did you mean:  list "coding", "reading"
```

## Walking a list

`for` can walk a list, once for each item. The name holds the item, and it exists only under the `for`:

```quill
hobbies = list "coding", "reading", "traveling"

for hobby in hobbies
  say hobby
```

```output
coding
reading
traveling
```

The list itself is not changed. To keep a total, create a `var` before the loop, the same way a counting loop does.

## A record

A record is a group of named fields. Put `record` on its own line, then each field underneath, indented by exactly 2 spaces. A field is a name, then `=`, then a value:

```quill
address = record
  street = "123 Main St"
  city = "Anytown"
  state = "CA"
  zip = "12345"

say address.city
```

```output
Anytown
```

A dot reads one field: `address.city` is `"Anytown"`. `say address` shows every field:

```quill
address = record
  street = "123 Main St"
  city = "Anytown"

say address
```

```output
street: 123 Main St
city: Anytown
```

A short record can sit on one line: `address = record city = "Anytown", state = "CA"`. Braces are not how Quill writes a record. The word is `record`.

Fields can be different kinds. `city` can be text while another field is a number. Two records are the same kind when they have the same field names and each field has the same kind. The order you write the fields does not matter for that.

## When a value wraps another

One school is a record. The fields belong to that one school, so `sum` reads them directly:

```quill
school = record name = "Harvard", fees = 10000
say sum fees of school
```

```output
10000
```

A list wraps several records when they are the same kind of thing and one line should read all of them:

```quill
harvard = record name = "Harvard", fees = 10000
oxford = record name = "Oxford", fees = 12000
schools = list harvard, oxford
say sum fees of schools
```

```output
22000
```

`schools` exists so the two fees can be added together. One school has no list around it.

A record wraps a list when one thing owns many items:

```quill
up = record place = "Up Station"
old = record place = "Old Town"
desk = record bookings = list up, old
say get place of desk.bookings
```

```output
Up Station, Old Town
```

`desk` is one desk. `bookings` is the list it owns. `get place of desk.bookings` reads a field from that list.

## Changing one piece

The whole list or record has to be marked `var`. Then `set` can replace one item or one field:

```quill
var hobbies = list "coding", "reading"
set hobbies at 2 = "writing"
say hobbies

var address = record city = "Anytown"
set address.city = "Springfield"
say address.city
```

```output
coding, writing
Springfield
```

`set hobbies = list "cooking"` replaces the whole list. The new list still has to be a list of the same kind.

A record that isn't marked `var` stays as it was written. Change it by writing `var` where it is created, then `set` the field.

## Asking for one field

When the list holds records, `sum` and `get` read one field from all of them. One record gives that one amount: `sum fees of school`. `get name, fee of roster` keeps several fields together. A dot reaches a field further in: `get place of desk.bookings`, or `get gpa of school.acceptances`. [Asking for data](asking.md) shows each form. `for` stays when each record needs its own step.

```quill
sisters = record name = "Nchang Sisters", fee = 80000 USD
brass = record name = "Bamenda Brass", fee = 150000 USD
roster = list sisters, brass
say sum fee of roster
say get fee of roster
```

```output
230000 USD
80000 USD, 150000 USD
```

## Try it

1. Make a list of three prices and say the second one.
2. Walk that list with `for` and add the prices into a `var total`.
3. Make a record for a person, with a name and an age, and say the name.
