# {{name}}

An automation: it reads data from a file, tidies it, works out a summary, and saves a report. Change the data and run it again.

| File | What it holds |
|---|---|
| `main.quill` | reads the data and writes the report |
| `report.quill` | the shape of the data and how a line looks, with tests |
| `data/students.json` | the data to read |

## Run it

```bash
quill run main.quill
```

It prints the summary and saves it to `reports/summary.txt`.

`permit read "data"` and `permit write "reports"` are the only folders the program may touch. Ask for any other file and Quill refuses.

## Test it

```bash
quill test
```

## Try next

1. Add a student to `data/students.json` and run it again. Give one a score of 120 and see what Quill says.
2. Add `Top student:` to the report with `students sorted by highest score`, then `at 1`.
3. List the names of those who still have to pay, with `join get name of students where paid == false with ", "`.

The book's chapters *Asking for data*, *Text*, and *Files, Settings and Permits* explain each part. `QUILL.md` in this folder is a short tour of the language.
