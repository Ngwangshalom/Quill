# {{name}}

A whole app for a small events company, in one service that a website, a desktop app, and a phone app can all call. It brings the parts of Quill together: a backend, JSON checked against types, exact money, data from a file, an AI assistant, and tests.

| File | What it holds |
|---|---|
| `server.quill` | the routes, the permits, and the AI assistant |
| `bookings.quill` | `type Booking` and the rules about bookings, with tests |
| `billing.quill` | prices, VAT, and payments in exact money, with tests |
| `data/bookings.json` | today's bookings |

## Run it

```bash
quill run server.quill
```

For the AI assistant, give it a key from [console.anthropic.com](https://console.anthropic.com). The key stays in a setting, never in the code:

```bash
ANTHROPIC_API_KEY=your-key quill run server.quill
```

Without a key, everything works except `/ask`, which answers 503.

## Try each route

In a second terminal:

```bash
curl localhost:8080/bookings
curl -X POST localhost:8080/quote -d '{"client": "Amina", "venue": "Town Hall", "guests": 120, "nights": 2, "paid": false}'
curl -X POST localhost:8080/bookings -d '{"client": "Omar", "venue": "Rose Garden", "guests": 400, "nights": 1, "paid": false}'
curl -X POST localhost:8080/ask -d '{"message": "Can I move my booking to Friday?"}'
```

- `GET /bookings` reads `data/bookings.json`, checks every booking, and answers with how many there are, how many are unpaid, the guests, and the clients, biggest first.
- `POST /quote` answers with the price before VAT, the VAT, the total, and three payments that add up to it.
- `POST /bookings` with 400 guests fits `type Booking`, which allows 500, but not the venue, which holds 300, so it's refused with 400.
- `POST /ask` sends the message to the model, and only an answer that fits `type Reply` comes back.

## Test it

```bash
quill test
```

The tests prove the booking and money rules. They don't start the server or ask the model, so they pass offline.

## Where each idea comes from

| In this app | Learn it in the book |
|---|---|
| `route`, `respond`, `request.body` | *Building a Web Service* |
| `as Booking`, `as list of Booking` | *Reading and Writing JSON* |
| `count of`, `sum`, `sorted by`, `where` | *Asking for data* |
| `round`, `split`, `% of` | *Money* |
| `require`, `ensure`, `where 1..500` | *Contracts* and *Types* |
| `ask ... as Reply` | *Asking an AI Model* |
| `permit read`, `read_file` | *Files, Settings and Permits* |

## Try next

1. Add a route `GET "/bookings/unpaid"` that answers with the clients who still have to pay.
2. Add a `deposit` to `Quote`: 20% of the total, with an example that proves it.
3. Add a field `email: text` to `Booking`, and refuse an email that doesn't contain `"@"`.

`QUILL.md` in this folder is a short tour of the language.
