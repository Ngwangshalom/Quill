# Asking for data

A large program, such as a bank, often wants one field from many records, or the total of those amounts, or the records that sit inside other records. `get` and `sum` ask for that data in one line. `for` is still there when each record needs its own step. One record stays a record. A list wraps several records, and a record wraps a list when one thing owns many items. [When a value wraps another](groups.md#when-a-value-wraps-another) shows each case.

`get` and `sum` are the usual choice for reading and totaling. You name the field. Quill walks the list. A missing amount is left out. `for` is the choice when this row is confirmed and the next row is declined, because each row does different work.

## The total

`sum` adds one field of every record in a list:

```quill
sisters = record name = "Nchang Sisters", fee = 80000 USD
brass = record name = "Bamenda Brass", fee = 150000 USD
roster = list sisters, brass

say sum fee of roster
```

```output
230000 USD
```

Read it as: "sum the fee of the roster." Each record has `fee`. Quill adds the amounts. You do not write the adding.

The same line reads one record. The sum is that one amount:

```quill
school = record name = "Harvard", fees = 10000

say sum fees of school
```

```output
10000
```

When an amount might be missing, `sum` adds the ones that are there:

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

Quiet Hour has no fee, so it is left out. The till is the money that exists. If every amount is missing, the sum is `0` in that currency.

`sum` only adds a number or an amount of money:

```quill,error
sisters = record name = "Nchang Sisters", fee = 80000 USD
roster = list sisters
say sum name of roster
```

```output
Problem on line 3 of example.quill:

    say sum name of roster
            ^^^^
sum adds amounts, but name is Text.
Name a number or an amount of money: sum fee of roster.
```

## One field from every record

`get` takes one field and gives you a list:

```quill
sisters = record name = "Nchang Sisters", fee = 80000 USD
brass = record name = "Bamenda Brass", fee = 150000 USD
roster = list sisters, brass

say get fee of roster
```

```output
80000 USD, 150000 USD
```

A missing amount is left out, the same way `sum` leaves it out. The list holds the fees that exist.

## Several fields

Name every field you need, separated by commas. `get` gives a list of records that keep only those fields, in the order you wrote them:

```quill
sisters = record name = "Nchang Sisters", age = 24, fee = 80000 USD
brass = record name = "Bamenda Brass", age = 31, fee = 150000 USD
roster = list sisters, brass

people = get name, age, fee of roster
person = people at 1
say person.name
say person.age
say person.fee
```

```output
Nchang Sisters
24
80000 USD
```

`person` is the first record. `person.name` reads its name. `(people at 1).name` is the same value. The parentheses are there because a dot binds to the value just before it, and that value would be the number `1`.

The first record in `people` has `name`, `age`, and `fee`. It does not have any other field the original record had.

The same line reads one record. The result is one record with only the fields you named:

```quill
school = record
  name = "Harvard"
  location = "United Kingdom"
  year = 2026
  fees = 10000

data = get name, location, year, fees of school
say data.name
say data.fees
```

```output
Harvard
10000
```

Name each field once:

```quill,error
sisters = record name = "Nchang Sisters", fee = 80000 USD
roster = list sisters
say get name, name of roster
```

```output
Problem on line 3 of example.quill:

    say get name, name of roster
                  ^^^^
name is already in this get.
Name each field once: get name, fee of roster.
```

## A field further in

The value after `of` can be a field of another record. Write the path.

```quill
school = record
  name = "Harvard"
  acceptances = record gpa = 4.0, sat = 1500

say get gpa of school.acceptances
```

```output
4.0
```

`school.acceptances` is one record, so this is that one number.

A list can sit on a record the same way. The place and the acts are already on each booking:

```quill
up = record place = "Up Station", acts = list "Bamenda Brass", "Hilltop Duo"
old = record place = "Old Town", acts = list "Market Drums"
desk = record bookings = list up, old

say get place of desk.bookings

first = desk.bookings at 1
say first.place
say first.acts
```

```output
Up Station, Old Town
Up Station
Bamenda Brass, Hilltop Duo
```

`desk.bookings` is the list. `get place of desk.bookings` is every place. `first` is the first booking, so `first.place` and `first.acts` need no parentheses.

## Records inside other records

`with` and `in` name the list and the field embedded in it, starting from the record that holds the list. The acts come back with the bookings:

```quill
up = record place = "Up Station", acts = list "Bamenda Brass", "Hilltop Duo"
old = record place = "Old Town", acts = list "Market Drums"
roster = record bookings = list up, old

tonight = get bookings with acts in roster
first = tonight at 1
say first.place
```

```output
Up Station
```

`bookings` is the list you want. `acts` is the data embedded in each booking. `in roster` is where those records live. `tonight` is the bookings, and each one still has its acts.

The same line works when the bookings are inside a list of records. Quill gathers them into one list:

```quill
up = record place = "Up Station", acts = list "Bamenda Brass"
old = record place = "Old Town", acts = list "Market Drums"
saturday = record day = "Saturday", bookings = list up
sunday = record day = "Sunday", bookings = list old
nights = list saturday, sunday

all = get bookings with acts in nights
second = all at 2
say second.place
```

```output
Old Town
```

The embedded field has to be there:

```quill,error
up = record place = "Up Station", acts = list "Bamenda Brass"
roster = record bookings = list up
say get bookings with guest in roster
```

```output
Problem on line 3 of example.quill:

    say get bookings with guest in roster
                          ^^^^^
This record has no field called guest.
Its fields are place, acts.
```

## Only some records: where

Often you want only some of the records: the students who passed, the fees that are not paid yet. `where` keeps the records whose condition is true, in the same order:

```quill
amina = record name = "Amina", score = 80, fee = 500 USD, paid = true
omar = record name = "Omar", score = 64, fee = 300 USD, paid = false
john = record name = "John", score = 45, fee = 200 USD, paid = false
students = list amina, omar, john

passed = students where score >= 50
say get name of passed
```

```output
Amina, Omar
```

Read it as: "the students where the score is 50 or more." Inside the condition, `score` is the field of each record. You don't write `student.score`, and you don't write the loop.

`where` works right inside `get` and `sum`, so one line can pick the records and ask for their data:

```quill
amina = record name = "Amina", score = 80, fee = 500 USD, paid = true
omar = record name = "Omar", score = 64, fee = 300 USD, paid = false
john = record name = "John", score = 45, fee = 200 USD, paid = false
students = list amina, omar, john

say get name of students where score >= 50
say sum fee of students where paid == false
```

```output
Amina, Omar
500 USD
```

Join conditions with `and` and `or`, or write `where` again. Both lines below keep only Omar:

```quill
amina = record name = "Amina", score = 80, fee = 500 USD, paid = true
omar = record name = "Omar", score = 64, fee = 300 USD, paid = false
john = record name = "John", score = 45, fee = 200 USD, paid = false
students = list amina, omar, john

say get name of students where score >= 50 and paid == false
say get name of students where score >= 50 where paid == false
```

```output
Omar
Omar
```

The condition can read values from outside, too. A field comes first: if the records have a `score`, then `score` in the condition is the record's score.

```quill
amina = record name = "Amina", score = 80
omar = record name = "Omar", score = 64
students = list amina, omar

pass_mark = 70
say get name of students where score >= pass_mark
```

```output
Amina
```

When no record matches, the answer is an empty list, not a problem:

```quill
amina = record name = "Amina", score = 80
students = list amina

say students where score > 100
```

```output
an empty list
```

`for` can walk the records that match: `for s in students where paid == false`.

## When each row needs its own step

`for` still walks every item. Use it when the work is different for each one: say the name, or decide that this booking is declined.

```quill
sisters = record name = "Nchang Sisters", fee = 80000 USD
brass = record name = "Bamenda Brass", fee = 150000 USD
roster = list sisters, brass

for act in roster
  say act.name
```

```output
Nchang Sisters
Bamenda Brass
```

Ask with `get` or `sum` when you want the data or the total, and add `where` to keep only some records. Repeat with `for` when each record needs a step of its own. A program can use both.

## Common mistakes

A dot binds to the value just before it. `people at 1.name` asks the number `1` for a field:

```quill,error
sisters = record name = "Nchang Sisters", age = 24, fee = 80000 USD
roster = list sisters
people = get name, age, fee of roster
say people at 1.name
```

```output
Problem on line 4 of example.quill:

    say people at 1.name
                    ^^^^
1 is Number, so it has no fields.
A dot reads a field of a record, like address.city.
```

Give the record a name first: `person = people at 1`, then `say person.name`.

A name in a `where` condition has to be a field of the records, or a value you made:

```quill,error
amina = record name = "Amina", score = 80
students = list amina

say students where scor >= 50
```

```output
Problem on line 4 of example.quill:

    say students where scor >= 50
                 ^^^^^
The records in students have no field called scor, and there's no value called scor.
Their fields are name, score.

Did you mean:  score
```

A `where` condition runs to the end of the line. In a `check`, name the answer first, so the comparison isn't read as part of the condition:

```quill
amina = record name = "Amina", score = 80
omar = record name = "Omar", score = 64
students = list amina, omar

pass_total = sum score of students where score >= 50
check pass_total == 144
```

`where` keeps records, so it needs a list of records. For a list of numbers or text, use `for` and `when`:

```quill,error
scores = list 80, 64, 45

say scores where scores > 50
```

```output
Problem on line 3 of example.quill:

    say scores where scores > 50
               ^^^^^
where keeps the records whose fields match, but scores holds Number.
Use where on a list of records, like students where score >= 50. To pick from other lists, use for and when.
```

## Try it

1. Make two records with a `fee` in USD, put them in a list, and `say` the `sum`.
2. `get` the fees into their own list and `say` that list.
3. Add a record whose fee is `missing`, and check that the sum left it out.
4. Put two bookings on a desk and `say get place of desk.bookings`.
5. Give each record a `paid` field. `say sum fee of roster where paid == false` to see what is still owed.
