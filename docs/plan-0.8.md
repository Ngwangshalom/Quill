# Plan for Quill 0.8.0

Quill 0.8.0 makes Quill strong for backends, money and AI. Everything below is
released together as **0.8.0**; there are no 0.5–0.7 releases.

**Status 21 October 2026.** Stages 1–8 are committed and pushed (550ad9f), with
`examples/` holding three sample projects. Done since, not yet committed: Quill
files end in `.quill` (`.ql` still runs, with a colored note to rename), `quill
test FOLDER`, a clearer problem for `try` on a line by itself, and the editor
extension at 0.2.0 (it removes older copies of itself).

**Goal: serve a small piece of the Kattegat backend with Quill in the first week of
November 2026 (2–6 Nov).** Kattegat is the user's real app (Express + Supabase,
money in AED fils); Kopsala is its consumer app inside it. The chosen piece is
**Kopsala Rewards** (`src/modules/kopsala/kopsala.rewards.rules.ts`): does a
check-in count, points into draw entries, and who wins the monthly draw. It's
plain numbers, so it runs on Quill today (prototyped 20 Oct); Node keeps the
database, logins, distances, Dubai times and the random ticket, and calls Quill
over HTTP. It needs headers (a shared secret) and a production-ready server
(items 2 and 3 below). Release 0.8.0 and test a deploy by 31 October.

Week plan: rewards rules ported with their 11 tests (to 25 Oct) → headers and
routes (26–28 Oct) → release and deploy (29–31 Oct) → Node calls Quill and
compares answers, then switches over (2–6 Nov).

## Done

Each stage has code, tests, its book chapter, `docs/spec.md`, the version history
("Unreleased"), README, quick reference, `quill help`, and editor colors.

| Stage | What | Book chapter | Spec |
|---|---|---|---|
| 1 | `type Booking` with fields and `where` rules; typed parameters | `types.md` | §8.11 |
| 2 | Money: `round ... to cents`, `split ... into/by`, `20% of`, `convert ... at`, currencies without cents (XAF, JPY), money shown as `90 USD` / `7.50 USD` | `money.md` | §4.3, §8.7 |
| 3 | `bring billing` / `bring "shared/money.quill"` (the import word is `bring`) | `bring.md` | §8.12 |
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
2. **Headers.** `request.headers`, so a route can read a login token, and a way
   to send headers with `respond`. Plus one line for CORS beside the permits:
   `allow origin "https://kopsala.app"`, covering every route. Needed for the
   Kattegat service.
3. **A server ready for production** (found reading `src/web.rs`, 21 Oct):
   - **Many requests at once.** `serve` answers one request at a time, so a route
     that calls `ask` or `fetch` makes every other visitor wait. Most important.
     Design: a pool of 32 workers in `std::thread::scope`, each looping on
     `server.recv()` (tiny_http's `Server` can be shared). The compiled `Chunk`
     has no `Rc` or `RefCell`, so it is shared by reference as it is; a trial
     build on 21 Oct compiled.
   - **A request size limit.** The body is read whole into memory; refuse large
     ones with `413`. Design: 1 MB. Check `body_length()` first, and read a body
     sent without a length (chunked) through `take(MAX + 1)`, so it's never read
     past the limit. Answer: "The request is larger than 1 MB, so it wasn't read.
     Send less at a time."
   - **One log line per request**, like `POST /orders 201 12ms`, so
     `journalctl -u myapi` shows what happened. Design: leave the query out of
     the log (it can hold a token), and print a request's `say` output, any
     problem, and its log line together in one write, so requests answered at
     once don't mix their lines.
   - Tests: a route that `fetch`es a service that waits 2 s, while `/fast` still
     answers at once; a 2 MB body with and without a length gets `413`; the log
     line has no query. Book: `web.md` (the server answers many requests at
     once, 413, the log line); spec §8.17; "Unreleased".
4. **Routes in brought files, made official.** They already work (a brought
   `routes.quill` is served), but spec §8.12 says a brought file holds only `fn`,
   `type` and `bring`. Allow `route` there in the spec and book; permits stay in
   the file that is run. This is what makes the Types, Rules, Routes layout work:
   one folder per module with `types.quill`, `rules.quill`, `routes.quill`.
5. **Book chapter "Putting it online"**: from desktop to production on a VPS
   (install, `git clone`, systemd, Caddy for HTTPS, firewall on `PORT`, secrets in
   `.env`, `git pull && quill check && systemctl restart` to update), on Render,
   and with Docker (`examples/myapi/Dockerfile`).
6. **`.gitattributes`** with `*.quill linguist-detectable=false` if GitHub still
   guesses another language for `.quill` files after the rename.
7. **Round half up**, e.g. `round half up fee to cents`, for money rules that
   match JavaScript's `Math.round` (Kattegat's `invoices.fees.ts`: 12,345 fils ×
   10% is 1,235 there and 1,234 with Quill's half to even).
8. **README "A small web API" section** still shows the old design (`use web`,
   `use db`, `Int`, `Text`). Rewrite it with real 0.8 code once `db` exists
   (`bring`, lowercase kinds, `route`/`respond`).
9. **Release 0.8.0**: move "Unreleased" notes in
   `book/src/appendix-version-history.md` under `v0.8.0`, add the release to the
   timeline in `book/src/story.md`, bump `Cargo.toml` to 0.8.0, update the
   `quill update` example in `getting-started.md`, run CI checks, commit, tag
   `v0.8.0`, push. Then the installer gives VPS and Docker users `bring web`.

## After 0.8.0

What the rest of Kopsala needs, in order of value:

1. **Dates and times**: `today`, `now in "Asia/Dubai"`, `+ 3 days`. Offers,
   "Happening now", reminders and live acts all depend on them.
2. **Text functions**: `upper`, `length`, `split`, for names, codes and search.
3. **Math**: `sqrt`, `sin`, `cos`, for the distance to a venue's map pin.
4. **Random numbers and hashing**, for codes, tokens and the monthly draw.
5. **Colored problems in the terminal**: "Problem" in red and the `^^^` marker,
   like the `.ql` rename note, which is the only colored output today.

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
