# {{name}}

A backend service. It answers web requests with JSON, so a website, a desktop app, and a phone app can all use it.

| File | What it holds |
|---|---|
| `server.quill` | the routes: what the service does for each request |
| `orders.quill` | the types and the rules, with tests |

## Run it

```bash
quill run server.quill
```

The service listens on port 8080. Set `PORT` to use another one: `PORT=3000 quill run server.quill`. In a second terminal, try:

```bash
curl localhost:8080/health
curl localhost:8080/hello/Amina
curl -X POST localhost:8080/orders -d '{"item": "Notebook", "quantity": 3, "price": "4.50"}'
curl -X POST localhost:8080/orders -d '{"item": "Notebook", "quantity": 500, "price": "4.50"}'
```

The last order asks for 500, and an order allows 1 to 100, so the service answers 400. Quill checked the JSON against `type Order` before any of your code saw it.

## Test it

```bash
quill test
```

The `--- example:` lines above `total` are the tests. The server itself isn't started by the tests.

## Try next

1. Add a field `customer: text` to `Order` and `Receipt`, and send it in the JSON.
2. Add a route `GET "/price/{item}"` that answers with a price.
3. Change the discount to 15% and update the example, so the test still passes.

The book's chapters *Building a Web Service* and *Reading and Writing JSON* explain each part. `QUILL.md` in this folder is a short tour of the language.
