# Asking an AI Model

A program can ask an AI model a question in one line: summarize a message, sort a support request, pull the details out of an email, or rate a review. Quill's `ai` module does it with one word, `ask`.

```quill,ignore
bring ai
permit net "api.anthropic.com"
permit env "ANTHROPIC_API_KEY"

message = "Hi! We'd like the brass band for our wedding on 12 June, about 150 guests."
summary = ask "Summarize this booking request in one sentence: {message}"
  on ModelFailed -> "No summary right now."
say summary
```

Each line has a job:

- `bring ai` brings Quill's `ai` module, so a reader sees at the top that the program asks a model.
- `permit net "api.anthropic.com"` lets the program reach the model's service.
- `permit env "ANTHROPIC_API_KEY"` lets it read the key, and says which provider it uses: Claude. The key stays in a setting on your computer, never in the code (see [Permits](permits.md)).
- `ask "..."` sends the question and gives back the answer as text.

The examples in this chapter need a real model and a key, so the book shows them without running them.

## A model can fail

A model lives somewhere else. It can be out of reach, too busy, or it can decline a question. All of these are the failure `ModelFailed`, and like every [failure](failures.md), the program has to say what happens then: carry on with a value, stop, or pass it on with `try`.

## Answers you can trust: ask ... as

Text is fine for a person to read, but a program needs answers it can use. Add `as` and a kind, and the model must answer in that shape:

```quill,ignore
bring ai
permit net "api.anthropic.com"
permit env "ANTHROPIC_API_KEY"

type Request
  act: text
  guests: number where 1..500
  date: text

email = "Hi! We'd like the brass band for our wedding on 12 June, about 150 guests."
request = ask "Pull out the act, the number of guests and the date: {email}" as Request
  on ModelFailed -> Request act = "unknown", guests = 1, date = "unknown"
  on Invalid -> Request act = "check by hand", guests = 1, date = "unknown"
say "{request.act} for {request.guests} guests on {request.date}"
```

`as Request` does two things:

1. Quill sends the shape of `Request` to the model, so it answers with exactly those fields.
2. Quill reads the answer the same way `as` reads any [JSON](json.md): every field must be there, the right kind, and inside its `where` range.

A model can still get something wrong, like 900 guests for a hall of 500. Then `ask` fails with `Invalid`, and the program never holds a `Request` that breaks its rules. Any kind works after `as`: `as number`, `as true/false`, `as list of text`, or a type.

## Choosing the provider

The key a program permits chooses who answers:

| Permit | Asks | Settings |
|---|---|---|
| `permit env "ANTHROPIC_API_KEY"` | Claude, at `api.anthropic.com` | The model is `claude-opus-5-5` unless `QUILL_AI_MODEL` names another. |
| `permit env "OPENAI_API_KEY"` | any service that speaks the OpenAI chat format | `QUILL_AI_MODEL` names the model. `OPENAI_BASE_URL` points at the service, like `http://localhost:11434/v1` for a model running on your own computer. |

The host still needs its `permit net`: `"api.anthropic.com"`, `"api.openai.com"`, or `"localhost"` for a model on your computer.

When Claude's safeguards decline a question, Quill asks Anthropic's recommended model to answer instead, so a cautious answer doesn't stop your program. If no model answers, it is `ModelFailed`.

## Putting a model to work

A model answers well when the question says exactly what you want and gives it the facts. Put the facts in with `{}`, and keep the kind small:

```quill,ignore
bring ai
permit net "api.anthropic.com"
permit env "ANTHROPIC_API_KEY"

type Mood
  happy: true/false
  reason: text

fn mood_of(review: text) -> Mood
  fails ModelFailed, Invalid
  return try ask "Is this customer happy? Give a short reason. Review: {review}" as Mood

reviews = list "The band was wonderful!", "They arrived two hours late."
var mood = Mood happy = false, reason = "not asked yet"
for review in reviews
  set mood = mood_of(review)
    on ModelFailed -> Mood happy = false, reason = "no answer"
    on Invalid -> Mood happy = false, reason = "unclear"
  say "{mood.happy}: {mood.reason}"
```

`try` passes a failure up to the function's caller, so each review decides for itself what an unanswered question means.

## Common mistakes

A program has to bring the module:

```quill,error
permit env "ANTHROPIC_API_KEY"

answer = ask "Hello"
  on ModelFailed -> "no answer"
```

```output
Problem on line 3 of example.quill:

    answer = ask "Hello"
             ^^^
ask is part of Quill's ai module, which this program doesn't bring.
Add bring ai at the top, so a reader sees that the program asks a model.

Did you mean:  bring ai
```

It needs a permit for a key, so a reader knows which provider it uses:

```quill,error
bring ai

answer = ask "Hello"
  on ModelFailed -> "no answer"
```

```output
Problem on line 3 of example.quill:

    answer = ask "Hello"
             ^^^
ask needs a key for a model, and the program permits none.
Add permit env "ANTHROPIC_API_KEY" to ask Claude, or permit env "OPENAI_API_KEY" for an OpenAI-compatible service. The key itself stays in the setting, out of the code.

Did you mean:  permit env "ANTHROPIC_API_KEY"
```

And a permit to reach the service:

```quill,error
bring ai
permit env "ANTHROPIC_API_KEY"

answer = ask "Hello"
  on ModelFailed -> "no answer"
```

```output
Problem on line 4 of example.quill:

    answer = ask "Hello"
             ^^^
ask sends the question to Claude at api.anthropic.com, and the program has no permit to reach it.
Add a permit for the host at the top of the program.

Did you mean:  permit net "api.anthropic.com"
```

`ask` can fail, so the failure has to be handled:

```quill,error
bring ai
permit net "api.anthropic.com"
permit env "ANTHROPIC_API_KEY"

say ask "Hello"
```

```output
Problem on line 5 of example.quill:

    say ask "Hello"
        ^^^
Asking a model can fail with ModelFailed: the model may be out of reach, or say no.
Handle each with on under this line, or write try in front, inside a function that fails the same way.
```

## Try it

1. Set `ANTHROPIC_API_KEY` on your computer, and ask a model to write one sentence about Quill.
2. Write `type City` with `name: text` and `country: text`, and ask for `as list of City` with "Name three cities in Cameroon."
3. Add a `where` range to a number field, and ask a question that tempts the model to break it. Watch `on Invalid` run.
4. Run the same program against a model on your own computer, with `permit env "OPENAI_API_KEY"`, `OPENAI_BASE_URL` and `QUILL_AI_MODEL`.
