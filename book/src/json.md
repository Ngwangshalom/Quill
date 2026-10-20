# Reading and Writing JSON

Programs talk to each other in JSON: a web form sends it, a payment service answers with it, a file stores it. It looks like this:

```text
{"venue": "Town Hall", "guests": 120, "fee": "500.00"}
```

JSON from outside can't be trusted. A field may be missing, a number may be text, a guest count may be 9000. Quill reads JSON **as a kind**, and checks all of it before your program uses any of it.

## Reading JSON as a type

Say a booking arrives in a file, `inbox/booking.json`:

```quill,part,file=inbox/booking.json
{"venue": "Town Hall", "guests": 120, "fee": "500.00"}
```

`as Booking` reads the text as a `Booking`:

```quill
permit read "inbox"

type Booking
  venue: text
  guests: number where 1..500
  fee: USD

body = read_file("inbox/booking.json")
  on FileMissing -> "none"
booking = body as Booking
  on Invalid -> Booking venue = "unknown", guests = 1, fee = 0 USD
say booking.venue
say booking.fee
```

```output
Town Hall
500 USD
```

`as Booking` checks that every field is there, that each one is the right kind, and that `guests` is from 1 through 500. Money is read exactly, from a number or from text like `"500.00"`, so no cent is lost. Fields the type doesn't name are left out.

When the text doesn't fit, `as` fails with `Invalid`, and the program says what happens, like any [failure](failures.md). Here a booking for 9000 guests is refused:

```quill,part,file=inbox/huge.json
{"venue": "Stadium", "guests": 9000, "fee": 1}
```

```quill
permit read "inbox"

type Booking
  venue: text
  guests: number where 1..500
  fee: USD

body = read_file("inbox/huge.json")
  on FileMissing -> "none"
booking = body as Booking
  on Invalid -> Booking venue = "refused", guests = 1, fee = 0 USD
say booking.venue
```

```output
refused
```

A program never holds a `Booking` that breaks its type's rules, wherever the data came from.

## Other kinds

`as` reads any kind a function can name: `as number`, `as text`, `as list of Booking`, `as maybe number`. A field that is `maybe` can be `null` or left out of the JSON, and is then `missing`:

```quill,part,file=inbox/prices.json
[1.10, 2.25, 0.10]
```

```quill
permit read "inbox"

body = read_file("inbox/prices.json")
  on FileMissing -> "[]"
prices = body as list of number
  on Invalid -> list of number
say prices
```

```output
1.10, 2.25, 0.10
```

## Writing JSON

`json` turns any value into JSON text: a record becomes an object, a list an array, money an exact number, and `missing` becomes `null`:

```quill
type Booking
  venue: text
  guests: number
  fee: USD

hall = Booking venue = "Town Hall", guests = 120, fee = 500.50 USD
say json(hall)
say json(list 1, 2, 3)
```

```output
{"venue":"Town Hall","guests":120,"fee":500.50}
[1,2,3]
```

## JSON written inside a program

JSON usually comes from outside: a file, a request, an answer. When you write it inside your own text, remember that `{` starts a value in Quill text. Write `\{` and `\}` for the braces themselves:

```quill
type Point
  x: number
  y: number

point = "\{\"x\": 3, \"y\": 4\}" as Point
  on Invalid -> Point x = 0, y = 0
say point.x + point.y
```

```output
7
```

## Common mistakes

Reading JSON can fail, so the failure has to be handled:

```quill,error
type Point
  x: number

point = "[]" as Point
say point.x
```

```output
Problem on line 4 of example.ql:

    point = "[]" as Point
                 ^^^^^^^^
Invalid isn't handled.
Reading text as Point can fail with Invalid, so the call needs on Invalid.

Did you mean:  on Invalid -> say "Invalid"
```

`as` reads text. A value that is already a number or a record doesn't need reading:

```quill,error
type Point
  x: number

point = 5 as Point
  on Invalid -> Point x = 0
```

```output
Problem on line 4 of example.ql:

    point = 5 as Point
            ^
as Point needs text for the JSON, but 5 is a Number.
as reads JSON text, like body as Point.
```

## Try it

1. Write `type Student` with `name: text` and `score: number where 0..100`, and a file with a student in JSON. Read it with `as Student`.
2. Change the score in the file to `120`, and see `on Invalid` run.
3. Read a JSON list of students with `as list of Student`, and say the names of those who passed with `get name of students where score >= 50`.
4. Say `json` of one of your students.
