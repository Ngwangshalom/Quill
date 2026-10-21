# Building a Web Service

A backend answers requests: a phone app asks for a booking, a website sends a new order, another service checks a price. Quill's `web` module turns a program into a web service with two words: `route` and `respond`.

```quill,ignore
bring web

route GET "/hello/{name}"
  respond 200, "Hello, {name}!"
```

Run it with `quill run hello.quill`, and Quill answers:

```text
Serving 1 routes on http://localhost:8080. Press Ctrl+C to stop.
```

Open `http://localhost:8080/hello/Amina` in a browser, and the page says `Hello, Amina!`.

- `bring web` brings Quill's `web` module.
- `route GET "/hello/{name}"` says what to do when a request asks for a page with `GET` at a path like `/hello/Amina`. `{name}` matches any part of the path, and inside the route it is a value: here, the text `"Amina"`.
- `respond 200, "Hello, {name}!"` answers. The number is the status: `200` means it worked. Text is sent as it is.

The examples in this chapter start a server that keeps running, so the book shows them without running them.

## Respond

Every route ends each path with `respond STATUS, VALUE`, the same way a function ends with `return`. The statuses you'll use most:

| Status | Means |
|---|---|
| `200` | It worked. |
| `201` | Something new was made. |
| `400` | The request was wrong, like a booking without a venue. |
| `404` | There's nothing there. |

Text is sent as plain text. Any other value, like a record or a list, is sent as [JSON](json.md):

```quill,ignore
bring web

type Act
  name: text
  fee: USD

route GET "/acts"
  brass = Act name = "Bamenda Brass", fee = 150000 USD
  sisters = Act name = "Nchang Sisters", fee = 80000 USD
  respond 200, list brass, sisters
```

`GET /acts` answers `[{"name":"Bamenda Brass","fee":150000},{"name":"Nchang Sisters","fee":80000}]`.

## The request

Inside a route, `request` holds what was asked: `request.method` (like `"POST"`), `request.path`, `request.query` (what comes after `?` in the address), and `request.body`, the text that was sent.

A body is usually JSON. Read it as a type with `as`, and every rule is checked before your code uses it:

```quill,ignore
bring web

type Booking
  venue: text
  guests: number where 1..500
  fee: USD

route POST "/bookings"
  booking = request.body as Booking
    on Invalid -> respond 400, "That isn't a booking."
  respond 201, booking
```

A request with 9000 guests gets `400 That isn't a booking.`, and the route never holds a `Booking` that breaks its rules. `on Invalid -> respond ...` answers and ends the route there, the same way `on` ends a function.

## When something goes wrong

Quill answers some requests by itself:

- A path no route has gets `404`.
- A path a route has, with another method, gets `405`.
- A route that stops with a problem, like dividing by zero, gets `500`, and the problem is shown where the server was started, with its line. The server keeps answering other requests.

## Putting it together

Routes can use everything else in Quill: functions, types, money, [files](permits.md), [other services](http.md), and [AI models](ai.md). Here a booking desk works out the deposit:

```quill,ignore
bring web

type Booking
  venue: text
  guests: number where 1..500
  fee: USD

fn deposit(booking: Booking) -> USD
  return round 20% of booking.fee to cents

say "The booking desk is open."

route POST "/quote"
  booking = request.body as Booking
    on Invalid -> respond 400, "Send a venue, guests from 1 to 500, and a fee."
  respond 200, "The deposit is {deposit(booking)}."
```

The lines outside routes run once, when the program starts. Then Quill serves the routes.

The server listens on port 8080. Set the setting `PORT` to use another one, like `PORT=3000 quill run desk.quill`.

## Growing your service

A first service fits in one file. As it grows, add one idea at a time, in this order:

1. **Rules in functions.** Work out prices, checks and decisions in plain `fn`s, and prove them with `--- example:` lines ([Testing](testing.md)). A route then only reads the request, calls a function, and responds.
2. **Split into files.** Move the types and functions into their own files and [bring](bring.md) them in. Keep `bring web` and the routes in the file you run.
3. **Files and settings.** Read a config file or a secret with [permits](permits.md). The permits go in the file you run, never in a brought file.
4. **Other services.** Call a payment service or a rates API with [`bring http`](http.md).
5. **AI.** Ask a model with [`bring ai`](ai.md), and read its answer as a type with `ask ... as Review`, the same way a request body is read with `as`.

Quill doesn't have a database yet; `bring db` is coming. Until then, a service keeps its data only while it runs, or in files it writes with `write_file`.

## Common mistakes

A program has to bring the module:

```quill,error
route GET "/hello"
  respond 200, "Hello"
```

```output
Problem on line 1 of example.quill:

    route GET "/hello"
          ^^^^^^^^^^^^
route is part of Quill's web module, which this program doesn't bring.
Add bring web at the top, so a reader sees that the program serves requests.

Did you mean:  bring web
```

A route has to respond on every path:

```quill,error
bring web

route GET "/hello"
  say "Someone said hello."
```

```output
Problem on line 3 of example.quill:

    route GET "/hello"
          ^^^^^^^^^^^^
GET /hello has to respond on every path.
End each path with respond, like respond 200, "OK".

Did you mean:  respond 200, "OK"
```

A route answers with `respond`, not `return`:

```quill,error
bring web

route GET "/hello"
  return "Hello"
```

```output
Problem on line 4 of example.quill:

      return "Hello"
      ^^^^^^
A route answers with respond, not return.
respond gives the status and what to send, like respond 200, booking.

Did you mean:  respond 200, "OK"
```

A route's method is written in capitals:

```quill,error
bring web

route post "/bookings"
  respond 201, "made"
```

```output
Problem on line 3 of example.quill:

    route post "/bookings"
          ^^^^
A route's method is written in capitals, like POST.

Did you mean:  POST
```

## Try it

1. Write a route `GET "/hello/{name}"` and open it in your browser with your own name.
2. Add `route GET "/time"` that responds with any text you like, and a request to a path you didn't write. Look at the `404`.
3. Write a `type Student` with `score: number where 0..100`, and a `POST "/students"` route that reads the body `as Student`. Send a score of `120` with a tool like `curl -X POST localhost:8080/students -d '{"name": "Amina", "score": 120}'`.
4. Respond with a list of records, and see the JSON your browser shows.
