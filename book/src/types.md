# Types

A booking always has a venue, a number of guests, and a fee. A `record` can hold those, but nothing stops one booking from forgetting its fee, or another from having 9000 guests in a hall for 500. A `type` writes down, once, what every booking has and what each field allows.

```quill
type Booking
  venue: text
  guests: number where 1..500
  fee: USD

hall = Booking venue = "Town Hall", guests = 120, fee = 500 USD
say hall.venue
say hall.guests
```

```output
Town Hall
120
```

`type Booking` names a kind of record. Each line under it is one field: a name, then `:`, then its kind, the same way a function writes a parameter. A field can have a range with `where`, as in [Contracts](contracts.md#a-range-on-a-parameter-where).

`Booking venue = ..., guests = ..., fee = ...` makes a new booking. It reads like `record`, with the type's name in place of the word `record`. A type's name starts with a capital letter, so it stands apart from values.

## Making a record of a type

Every field has to be there, and each one has to be the right kind. A long record can put one field on each line:

```quill
type Booking
  venue: text
  guests: number where 1..500
  fee: USD

garden = Booking
  venue = "Garden"
  guests = 40
  fee = 200 USD

say garden
```

```output
venue: Garden
guests: 40
fee: 200 USD
```

A field that might not have a value is a `maybe`, and you write `missing` or `found` for it, as in [Maybe](maybe.md):

```quill
type Guest
  name: text
  phone: maybe text

amina = Guest name = "Amina", phone = found "555-0101"
omar = Guest name = "Omar", phone = missing
say omar.name
```

```output
Omar
```

## Types in functions

A function can take a type, and give one back. Write the type's name where a kind goes:

```quill
type Booking
  venue: text
  guests: number where 1..500
  fee: USD

fn deposit(b: Booking) -> USD
  return b.fee / 5

fn bigger(b: Booking, more: number) -> Booking
  return Booking venue = b.venue, guests = b.guests + more, fee = b.fee

hall = Booking venue = "Town Hall", guests = 120, fee = 500 USD
say deposit(hall)
say bigger(hall, 30).guests
```

```output
100 USD
150
```

A list of a type is written `list of Booking`. Everything in [Asking for data](asking.md) works on it:

```quill
type Booking
  venue: text
  guests: number where 1..500
  fee: USD

hall = Booking venue = "Town Hall", guests = 120, fee = 500 USD
garden = Booking venue = "Garden", guests = 40, fee = 200 USD
bookings = list hall, garden

say get venue of bookings where guests > 100
say sum fee of bookings
```

```output
Town Hall
700 USD
```

## The rules always hold

A field's `where` is checked every time a record of the type is made. When the value is written right there, Quill finds the mistake before the program runs:

```quill,error,file=hall.quill
type Booking
  venue: text
  guests: number where 1..500

big = Booking venue = "Stadium", guests = 9000
```

```output
Problem on line 5 of hall.quill:

    big = Booking venue = "Stadium", guests = 9000
                                              ^^^^
A Booking takes guests from 1 through 500, but this one gives 9000.
Line 3 says guests where 1..500.
```

When the value is worked out while the program runs, making the record stops the program:

```quill,error,file=hall.quill
type Booking
  venue: text
  guests: number where 1..500

crowd = 400 + 200
big = Booking venue = "Stadium", guests = crowd
```

```output
Problem on line 6 of hall.quill:

    big = Booking venue = "Stadium", guests = crowd
          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
A Booking takes guests from 1 through 500, but guests is 600.
A Booking can't be made with a value outside that range. Check the value before making the Booking.
```

A function that takes a `Booking` checks the rules too, so even a plain `record` with the same fields can't bring a value outside the range into it. Inside the function, `b.guests` is always from 1 through 500.

## Common mistakes

Every field has to be there:

```quill,error
type Booking
  venue: text
  guests: number

hall = Booking venue = "Town Hall"
```

```output
Problem on line 5 of example.quill:

    hall = Booking venue = "Town Hall"
           ^^^^^^^
This Booking is missing the field guests.
Every Booking has all of its fields. Add it, or use missing for a maybe field.
```

A misspelled field is found before the program runs:

```quill,error
type Booking
  venue: text
  guests: number

hall = Booking venue = "Town Hall", gests = 120
```

```output
Problem on line 5 of example.quill:

    hall = Booking venue = "Town Hall", gests = 120
                                        ^^^^^
A Booking has no field called gests.
Its fields are venue, guests.

Did you mean:  guests
```

A type can't hold itself, because that one would hold another, and so on forever. Hold something that names the other one, like its id:

```quill,error
type Person
  name: text
  manager: maybe Person
```

```output
Problem on line 3 of example.quill:

      manager: maybe Person
               ^^^^^^^^^^^^
A Person can't hold a Person inside it, because that one would hold another, forever.
Hold something that names the other Person instead, like its id.
```

`quill explain` shows what a type line means. On `type Booking`, it lists each field and when its range is checked.

## Try it

1. Write `type Student` with `name: text` and `score: number where 0..100`. Make two students and `say` each one.
2. Make a student with a score of `120` and read what Quill says.
3. Write `fn passed(s: Student) -> true/false` that gives back whether the score is 50 or more.
4. Put your students in a list, and `say get name of students where score >= 50`.
