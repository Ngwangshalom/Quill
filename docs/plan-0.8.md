# Plan for Quill 0.8.0

Quill 0.8.0 makes Quill strong for backends, money and AI. Everything below is
released together as **0.8.0**; there are no 0.5–0.7 releases.

**Stopped on 20 October 2026, after stage 8 (web server).** Nothing from this plan
is committed yet. `cargo test`, `cargo clippy --all-targets -- -D warnings` and the
book tests all pass at this point.

## Done

Each stage has code, tests, its book chapter, `docs/spec.md`, the version history
("Unreleased"), README, quick reference, `quill help`, and editor colors.

| Stage | What | Book chapter | Spec |
|---|---|---|---|
| 1 | `type Booking` with fields and `where` rules; typed parameters | `types.md` | §8.11 |
| 2 | Money: `round ... to cents`, `split ... into/by`, `20% of`, `convert ... at`, currencies without cents (XAF, JPY), money shown as `90 USD` / `7.50 USD` | `money.md` | §4.3, §8.7 |
| 3 | `bring billing` / `bring "shared/money.ql"` (the import word is `bring`) | `bring.md` | §8.12 |
| 4 | `permit read/write/net/env`, `read_file`, `write_file`, `file_exists`, `env` | `permits.md` | §8.13 |
| 5 | JSON: `body as Booking` (fails `Invalid`), `json(value)` | `json.md` | §8.14 |
| 6 | `bring http`: `fetch`, `post`, `send` (fails `NetworkDown`) | `http.md` | §8.15 |
| 7 | `bring ai`: `ask "..."` and `ask "..." as Review` for Claude and OpenAI-compatible services (fails `ModelFailed`, `Invalid`) | `ai.md` | §8.16 |
| 8 | `bring web`: `route GET "/path/{name}"`, `respond 200, value`, served by `quill run` on `PORT` | `web.md` | §8.17 |

Also new: `Span` and `Diagnostic` carry a file number, so problems in brought files
name the right file; the book test harness supports `quill,part,file=NAME` blocks.

## Still to do

1. **Stage 9: database (`bring db`), SQLite only**, built into the binary
   (`rusqlite` with the bundled feature). Planned shape: save and find records of a
   type, and reuse `where` for queries, e.g. `bookings where paid == false`. Needs a
   `permit` for the database file. Book chapter, spec §8.18, tests, colors.
2. **README "A small web API" section** still shows the old design (`use web`,
   `use db`, `Int`, `Text`). Rewrite it with real 0.8 code once `db` exists
   (`bring`, lowercase kinds, `route`/`respond`).
3. **Book "What's Coming Next"** (`book/src/whats-next.md`): update for 0.8.
4. **Release 0.8.0**: move "Unreleased" notes in
   `book/src/appendix-version-history.md` under `v0.8.0`, add the release to the
   timeline in `book/src/story.md`, bump `Cargo.toml` to 0.8.0, update the
   `quill update` example in `getting-started.md`, run CI checks, commit, tag
   `v0.8.0`, push.

## Decisions made along the way

- Kinds stay lowercase (`number`, `text`); no `Int` / `Text` aliases.
- `round`, `split`, `convert`, `bring`, `permit`, `as`, `ask`, `route`,
  `respond`, `type` are reserved words.
- New colors: `where`/`type`/`as` steel `#6AA8C8`/`#1A5270`; `round`/`split`/`convert`
  mint `#8FD3B5`/`#2E7A5C`; `bring`/`route`/`respond` mist `#A8C0D8`/`#3D5270`;
  `permit` rust `#C47848`/`#8C4E20`; `ask` spark `#E57BE0`/`#8E2C8A`.
- The permitted key chooses the AI provider: `ANTHROPIC_API_KEY` → Claude
  (`claude-opus-5-5`, server-side fallbacks on), `OPENAI_API_KEY` → OpenAI-compatible
  (needs `QUILL_AI_MODEL`).
- Permits live only in the program you run, never in a brought file.
