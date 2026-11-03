# {{name}}

A program that asks an AI model to read customer reviews. The model must answer as `type Mood`, and Quill checks every answer before the program uses it.

| File | What it holds |
|---|---|
| `main.quill` | the question for the model, and what happens when it fails |
| `rules.quill` | the shape of an answer and our own rules about it, with tests |

## Run it

The program asks Claude, so it needs a key from [console.anthropic.com](https://console.anthropic.com). The key stays in a setting on your computer, never in the code:

```bash
ANTHROPIC_API_KEY=your-key quill run main.quill
```

Without a key, the program says so and stops. To use an OpenAI-compatible model instead, including one on your own computer, change the permits to `permit env "OPENAI_API_KEY"` and the host it uses. The book's chapter *Asking an AI Model* shows how.

## Test it

```bash
quill test
```

The tests check `needs_reply`, our own rule. They don't ask the model, so they pass without a key.

## Try next

1. Add your own reviews to the list.
2. Add a field `topic: text` to `Mood`, and say it for each review.
3. Change the range on `score` to `1..3` and watch `on Invalid` run when the model gives a 5.

`QUILL.md` in this folder is a short tour of the language.
