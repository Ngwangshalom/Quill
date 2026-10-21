# Talking to Web Services

Most backends talk to other services: an exchange rate service, a payment provider, a delivery company. They do it with web requests, and the answers usually come back as [JSON](json.md). Quill's `http` module sends them.

```quill,ignore
bring http
permit net "api.example.com"

type Rates
  usd_to_eur: number

answer = fetch("https://api.example.com/rates")
  on NetworkDown -> say "The rate service can't be reached."
when
  answer.status == 200 -> say "Got the rates."
  else -> say "The service answered {answer.status}."
rates = answer.body as Rates
  on Invalid -> Rates usd_to_eur = 0
say convert 100 USD to EUR at rates.usd_to_eur
```

Each part has a job:

- `bring http` brings in Quill's `http` module, so a reader sees at the top that the program uses the network.
- `permit net "api.example.com"` lets the program talk to that one host, and no other. Without it, the call is refused (see [Permits](permits.md)).
- `fetch(url)` asks for the page at `url` and gives back an **answer**: a record with `status`, a number like `200` or `404`, and `body`, the text that came back.
- `as Rates` reads the body as a type, checking every field (see [JSON](json.md)).

The examples in this chapter talk to services on the internet, so the book shows them without running them.

## When a request goes wrong

There are two different ways for a request to go wrong, and Quill keeps them apart:

- **The service can't be reached**: no network, a wrong host name, or no answer within 30 seconds. That's the failure `NetworkDown`, handled with `on NetworkDown` or `try`, like any failure.
- **The service answers with an error**, like `404` (not found) or `500` (the service broke). That's still an answer. Its `status` says what happened, and your program decides what to do.

Check the status before trusting the body. `200` to `299` mean it worked.

## Sending data: post and send

`post(url, body)` sends text to a service, usually JSON made with `json(...)`. It says the body is JSON for you:

```quill,ignore
bring http
permit net "api.example.com"

type Order
  item: text
  amount: USD

order = Order item = "Bamenda Brass", amount = 150000 USD
reply = post("https://api.example.com/orders", json(order))
  on NetworkDown -> say "Couldn't send the order."
say reply.status
```

`send(method, url, body, headers)` sends any kind of request, with your own headers. Each header is text with a name, a colon, and a value. A key from a setting goes in a header, not in the code:

```quill,ignore
bring http
permit net "api.example.com"
permit env "SHOP_KEY"

fn cancel_order(key: text) -> number
  reply = send("DELETE", "https://api.example.com/orders/7", "", list "Authorization: Bearer {key}")
    on NetworkDown -> return 0
  return reply.status

when env("SHOP_KEY")
  found key -> say cancel_order(key)
  missing -> say "Set SHOP_KEY first."
```

The methods are `GET`, `POST`, `PUT`, `PATCH`, `DELETE` and `HEAD`. Use `""` for no body.

## Common mistakes

A program has to bring the module before it uses it:

```quill,error
permit net "api.example.com"

answer = fetch("https://api.example.com/rates")
  on NetworkDown -> say "offline"
```

```output
Problem on line 3 of example.quill:

    answer = fetch("https://api.example.com/rates")
             ^^^^^
fetch is part of Quill's http module, which this program doesn't bring.
Add bring http at the top, so a reader sees what the program uses.

Did you mean:  bring http
```

Without a permit for the host, the call is refused before the program runs:

```quill,error
bring http

answer = fetch("https://api.example.com/rates")
  on NetworkDown -> say "offline"
```

```output
Problem on line 3 of example.quill:

    answer = fetch("https://api.example.com/rates")
                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
fetch can't reach https://api.example.com/rates, because the program has no permit for it.
A program touches only what its permits allow. If it should, add this at the top of the program.

Did you mean:  permit net "api.example.com"
```

A web address starts with `https://` or `http://`:

```quill,error
bring http
permit net "api.example.com"

answer = fetch("api.example.com/rates")
  on NetworkDown -> say "offline"
```

```output
Problem on line 4 of example.quill:

    answer = fetch("api.example.com/rates")
                   ^^^^^^^^^^^^^^^^^^^^^^^
api.example.com/rates isn't a web address.
A web address starts with https:// or http://, like https://api.example.com/rates.
```

## Try it

1. Write a program that brings `http`, permits one host, and fetches a page from it. Say the status.
2. Remove the permit and read what Quill says.
3. Fetch a page that doesn't exist on that host, and handle the `404` with `when`.
4. Make a record, and send it with `post(url, json(record))` to a test service like `https://httpbin.org/post`.
