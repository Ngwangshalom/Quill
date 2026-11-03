# {{name}}

A webhook: a small service that a payment provider calls each time something happens to a payment. It checks every field of the notice before trusting it, then decides what to do.

| File | What it holds |
|---|---|
| `server.quill` | the route that receives notices |
| `payments.quill` | the shape of a notice and the decision, with tests |

## Run it

```bash
quill run server.quill
```

In a second terminal, send it a notice:

```bash
curl -X POST localhost:8080/payments -d '{"id": "pay_1001", "customer": "Amina", "amount": "250.00", "status": "paid"}'
curl -X POST localhost:8080/payments -d '{"id": "pay_1002", "customer": "Omar"}'
```

The first is received, and the server says `Thank Amina for 250 USD`. The second has no amount, so it's refused with 400 before any rule runs.

## Test it

```bash
quill test
```

## Try next

1. Add a status `"disputed"` with its own decision and example.
2. Tighten the range on `amount` to `0.01 USD..10000 USD`, and send a notice for more. It is refused with 400.
3. Write each decision to a file with `permit write "logs"` and `write_file`.

The book's chapters *Building a Web Service* and *Reading and Writing JSON* explain each part. `QUILL.md` in this folder is a short tour of the language.
