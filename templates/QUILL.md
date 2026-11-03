# About Quill

Quill is a programming language where intent is clear, failures are explicit, and correctness is part of the code. It is for backend services, financial software, automation, reliable business applications, and AI. A beginner can read it, and a team can trust it.

## Quill on one page

```quill
-- A comment starts with two dashes.
name = "Amina"                  -- a value that can't change
var visits = 0                  -- a value that can change
set visits = visits + 1         -- changing it is always visible

say "Hello, {name}!"            -- {} puts a value inside text

when                            -- the first true case runs
  visits > 10 -> say "A regular!"
  else -> say "Welcome."

for i in 1..3                   -- 1, then 2, then 3
  say i

price = 12.50                   -- numbers are exact: 0.1 + 0.2 is 0.3
fee = 100 USD + 50 USD          -- money keeps its currency

fn double(n: number) -> number  -- a function names its kinds
  return n * 2

check double(21) == 42          -- a test, right next to the code
```

## What makes it different

- **Mistakes are found before the program runs.** Adding text to a number, a misspelled name, a missing case: Quill says what went wrong, why, and what you probably meant.
- **Nothing is missing by surprise.** There is no `null`. `maybe` says a value might be missing, and `when` must handle `found` and `missing`.
- **Failures can't be ignored.** `fails Declined` names how a function can fail, and the caller handles it with `on`, or passes it on with `try`.
- **Money is exact.** `split 100 USD into 3` gives `33.34, 33.33, 33.33`. Dollars and euros can't be added by accident.
- **Data reads like a sentence.** `count of students where score >= 50`, `average fee of roster`, `students sorted by highest score`, `trim name`, `split line by ","`.
- **A program says what it touches.** `permit read "data"`, `permit net "api.example.com"`, `permit env "API_KEY"`. Anything else is refused.
- **Rules are part of the code.** `require`, `ensure`, and `where 1..500` say what must be true, and Quill checks them.

## Commands

| Command | What it does |
|---|---|
| `quill run FILE.quill` | run a program |
| `quill check FILE.quill` | find mistakes without running anything |
| `quill test` | run every `check` and `--- example:` in this folder |
| `quill explain FILE.quill:LINE` | show how Quill reads one line |
| `quill new NAME` | start another project from a template |
| `quill update` | install a newer version of Quill |
| `quill help` | show everything |

## Learn more

- The Quill book teaches the language from the very beginning: <https://ngwangshalom.github.io/Quill/>
- The language specification says exactly what each line means: <https://ngwangshalom.github.io/Quill/appendix-specification.html>
- Questions, ideas, and bugs: <https://github.com/Ngwangshalom/Quill/issues>

Your editor colors Quill and underlines mistakes as you type, in Cursor and VS Code, and in Vim, Emacs, and Sublime with the files in Quill's `editors` folder.
