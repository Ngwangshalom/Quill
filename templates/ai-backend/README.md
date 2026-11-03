# {{name}}

An AI service you can deploy. An app sends a customer's message as JSON, a model answers in a fixed shape, and the service checks that answer before sending it back.

| File | What it holds |
|---|---|
| `server.quill` | the question for the model, the shape of its answer, the routes, and what each failure answers |
| `rules.quill` | our own rules, with tests |

## Run it

It asks Claude, so it needs a key from [console.anthropic.com](https://console.anthropic.com):

```bash
ANTHROPIC_API_KEY=your-key quill run server.quill
```

In a second terminal:

```bash
curl -X POST localhost:8080/answer -d '{"message": "Can I change my booking to Friday?"}'
curl -X POST localhost:8080/answer -d '{"text": "hello"}'
```

The first gets a topic, whether it's urgent, and a reply. The second has no `message`, so it gets 400. When the model can't be reached, or answers in the wrong shape, the service answers 502, and without a key it answers 503: the app always gets an answer it can rely on.

## Test it

```bash
quill test
```

The tests don't call the model, so they pass without a key.

## Try next

1. Add a field `language: text` to `Answer`, and ask the model to reply in the customer's language.
2. Add a route `GET "/topics"` that answers with a list of the topics you handle.
3. Run it against a model on your own computer: see *Asking an AI Model* in the book.

`QUILL.md` in this folder is a short tour of the language.
