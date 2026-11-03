# {{name}}

A program that talks to another service: it asks a free exchange rate service for today's rates, reads the JSON answer safely, and changes a price into euros and pounds.

| File | What it holds |
|---|---|
| `main.quill` | the request, and what to do when it goes wrong |
| `rates.quill` | the shape of the answer and the conversions, with tests |

## Run it

```bash
quill run main.quill
```

It needs the internet. Without it, the program says the service can't be reached: `NetworkDown` is a failure Quill makes you handle.

`permit net "api.frankfurter.dev"` is the only host the program may talk to. Point `fetch` at another host and Quill refuses before the program starts.

## Test it

```bash
quill test
```

The tests use rates written in the examples, so they pass without the internet.

## Try next

1. Add `JPY: number` to `Prices`, ask for `to=EUR,GBP,JPY`, and show the price in yen.
2. Change `price`, or read it from a setting with `env("PRICE")` and `permit env "PRICE"`.
3. Send something with `post(url, json(record))` to a test service such as `https://httpbin.org/post`.

The book's chapters *Talking to Web Services* and *Reading and Writing JSON* explain each part. `QUILL.md` in this folder is a short tour of the language.
