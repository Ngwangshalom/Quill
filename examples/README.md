# Examples

Small Quill projects, each in its own folder. Run them from the top of the repository.

| Folder | What it shows | Run it |
|---|---|---|
| [events](events) | A venue booking system split across files: types with ranges, `bring`, money, failures, contracts and tests | `quill run examples/events/main.ql` |
| [myapi](myapi) | A first web service with JSON orders, and a Dockerfile for hosting it on Render, Railway or Fly.io | `cd examples/myapi && quill run server.ql` |
| [reviews](reviews) | A web service that reads JSON, asks a model to rate it with `ask ... as Rating`, and answers with JSON | `ANTHROPIC_API_KEY=... quill run examples/reviews/server.ql` |

Test them all with `quill test examples`.
