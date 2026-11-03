# Getting Started

In this chapter you'll install Quill, write your first program, and learn the commands you'll use every day.

## Installing Quill

**On macOS or Linux**, open a terminal and run:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Ngwangshalom/Quill/releases/latest/download/quill-installer.sh | sh
```

**On Windows**, open PowerShell and run:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/Ngwangshalom/Quill/releases/latest/download/quill-installer.ps1 | iex"
```

You can also download Quill for your system from the [Releases page](https://github.com/Ngwangshalom/Quill/releases).

To check that it worked, open a new terminal and run:

```bash
quill version
```

It prints the version you installed, such as `quill 0.2.0`.

## Updating Quill

Quill gets better with every release. To update, run:

```bash
quill update
```

Quill shows every version you can install. Move with the ↑ and ↓ keys, press Enter to install the one you want, or press Esc to leave everything as it is:

```text
Which version of Quill do you want? (↑ ↓ to move, Enter to install, Esc to cancel):
> 0.4.8     3 Nov 2026   latest
  0.4.5     3 Nov 2026   installed
  0.4.2     3 Nov 2026
  0.4.0     20 Oct 2026
  0.3.1     14 Oct 2026
  0.3.0     13 Oct 2026
  0.2.0     13 Oct 2026
  0.1.0     13 Oct 2026
```

You can also name the version you want, which is handy in scripts:

```bash
quill update latest
quill update 0.2.0
```

Choosing an older version is how you go back if a new release causes a problem. Versions before 0.3.0 don't have `quill update`, so Quill asks before installing one of them: to come back to a newer version from there, you run the installer again.

`quill upgrade` does exactly the same as `quill update`, for anyone used to that word from other tools.

You don't have to remember to check. At most once a day, Quill quietly asks whether a new version is out, and if one is, it tells you after your command finishes:

```text
Quill 0.4.8 is available (you have 0.4.5). Run quill update to get it.
```

Quill never updates itself without asking, so a program that works today won't change behaviour tomorrow unless you choose to update. The check runs in the background and never slows Quill down. If you'd rather Quill didn't check at all, set `QUILL_NO_UPDATE_CHECK=1` in your environment.

`quill update` works for copies installed with the commands above. If you built Quill from its source code with `cargo`, update it by getting the latest code and running `cargo install --path . --force` in the Quill folder.

## Hello, World!

It's a tradition to start with a program that shows the words "Hello, World!". In Quill, that program is one line.

Create a file called `hello.quill` (Quill programs always end in `.quill`) and write:

```quill
say "Hello, World!"
```

Then run it:

```bash
quill run hello.quill
```

```output
Hello, World!
```

`say` shows a value. Here the value is the text `"Hello, World!"`. The quotes mark where the text starts and ends; they aren't shown.

> **Files from an older Quill.** Before 0.8, Quill files ended in `.ql`. They still run, and Quill reminds you to rename them:
>
> ```text
> Note: Quill files now end in .quill. Rename hello.ql to hello.quill.
> ```
>
> Rename each file once, and the note goes away. `bring billing` finds `billing.ql` until you do.

A program can have many lines. Quill runs them from top to bottom:

```quill
say "Hello, World!"
say "Welcome to Quill."
```

```output
Hello, World!
Welcome to Quill.
```

## Starting a project

`quill new` creates a folder with a program in it that runs and passes its tests straight away. It asks what kind of program you want:

```bash
quill new shop
```

```text
What kind of program is shop? (↑ ↓ to move, Enter to choose, Esc to cancel)
  Everything  app          a whole app: backend, AI, money, data, and tests
  Learning    learn        exercises with checks that fail until you fix them
> Learning    starter      one file: values, when, for, a function, a test
  Backend     backend      an API for web, desktop, and mobile apps
  Backend     integration  call another service and read its JSON safely
  Backend     webhook      receive payment notices and check every field
  AI          ai           ask a model and check its answer against a type
  AI          ai-backend   a service route that asks a model
  Business    finance      invoices with exact money, VAT, and contracts
  Business    banking      accounts that can never go below zero
  Automation  automation   read a file, count and sort, save a report
  Automation  library      reusable functions with tests, used with bring
```

Move with ↑ and ↓, and press Enter. To skip the menu, name the template after the folder:

```bash
quill new shop starter
quill new orders backend
```

```text
Created shop, a starter project.
Run it with:   quill run shop/hello.quill
Test it with:  quill test shop
Read shop/README.md to see what's inside, and shop/QUILL.md for a tour of Quill.
```

Every project has a `README.md` that says how to run it and what to try next, and a `QUILL.md` with a one-page tour of the language. When there's no menu to show, such as in a script, `quill new shop` makes a `starter` project.

| Template | Start here when you want to |
|---|---|
| `app` | see a whole app: a backend with bookings, prices with VAT, data from a file, an AI assistant, and tests, working together |
| `learn` | learn Quill by fixing small exercises until `quill test` passes |
| `starter` | try the language in one file |
| `backend` | build a service that web, desktop, and mobile apps call |
| `integration` | call another service and read its JSON safely |
| `webhook` | receive notices from a payment provider or another service |
| `ai` | ask an AI model and check its answer |
| `ai-backend` | put an AI model behind a service you can deploy |
| `finance` | work with invoices, VAT, and money that never loses a cent |
| `banking` | keep accounts whose rules can never be broken |
| `automation` | read a file, work something out, and save a report |
| `library` | write functions that other programs bring in |

Inside `shop/hello.quill`, the starter, you'll find:

```quill
-- Your first Quill program. Run it:  quill run hello.quill
-- Change a value, run it again, and watch what changes.

name = "Amina"
say "Hello, {name}!"

scores = list 80, 64, 45
say "{count of scores} scores"
for score in scores
  when
    score >= 50 -> say "{score}: a pass"
    else -> say "{score}: try again"

--- Doubles a number. The example below is a test: quill test runs it.
--- example: double(21) == 42
fn double(n: number) -> number
  return n * 2

say "Double 21 is {double(21)}"

check name == "Amina"
```

```output
Hello, Amina!
3 scores
80: a pass
64: a pass
45: try again
Double 21 is 42
```

Each part has a chapter of its own in this book. The `check` line and the `--- example:` are tests, which you'll learn about in [Testing Your Code](testing.md).

## Comments

A comment is a note for people reading the code. Quill ignores it. A comment starts with `--` and runs to the end of the line:

```quill
-- Show a welcome message.
say "Welcome!"   -- this part is ignored too
```

```output
Welcome!
```

## Colors in your editor

Quill has its own colors, taken from the feather, the nib, and ink. `say`, `when`, and `true` each have their own color. The first time you run Quill, it quietly gives those colors to Cursor and VS Code, puts the feather on every `.quill` file, and underlines mistakes while you type. Reload the editor window once to see it, and again after you update Quill. Other languages keep the colors they already had.

The feather follows the editor. On a dark window it is pale, so it shows against the dark file list. On a light window it is the deep blue feather from the logo.

On a dark editor:

| What | Color | Where it comes from |
|---|---|---|
| `say` | `#F0A35E` nib orange | the nib |
| `when` and `else` | `#7EAEF5` feather blue | the feather |
| `true` and `false` | `#5ECA8C` green | a yes on the page |
| `var` and `set` | `#C9A6F2` lilac | a name you can change |
| `check` | `#F0A0B4` rose | a test |
| `and`, `or`, `not` | `#5EC4C0` teal | words that join a question |
| `for` and `in` | `#C6D86A` yellow-green | a loop |
| `get`, `sum`, `with`, `count`, `average`, and `sorted` | `#6EC1D6` sea | asking for data |
| `trim`, `uppercase`, `lowercase`, `length`, `contains`, `starts with`, `ends with`, `join`, `replace` | `#949CF5` ink | working with text |
| `list` | `#D4896A` clay | values in order |
| `record` | `#A8C5A0` sage | named fields |
| `of` | `#E8C547` gold | the kind of an empty list |
| `at` | `#B07CFF` violet | one item, by its position |
| `fn` and `return` | `#E8A0D4` pink | a function |
| `maybe`, `found`, `missing` | `#F0B090` peach | a value that might be missing |
| `fails`, `fail`, `on`, `try` | `#E08A8A` red | a failure the caller must handle |
| `require`, `ensure`, and `result` in an `ensure` | `#C8A06A` brass | a function's rules |
| `where`, `type` and `as` | `#6AA8C8` steel | which values fit: a range, the records to keep, a kind of record |
| `round`, `split`, `convert` | `#8FD3B5` mint | money that never loses a cent |
| `bring`, `route`, `respond` | `#A8C0D8` mist | another file, a Quill module, a web route |
| `permit` | `#C47848` rust | what a program may touch |
| `ask` | `#E57BE0` spark | a question for an AI model |
| Numbers | `#E7C98A` sand | counted things |
| Text in quotes | `#F3E6D0` warm paper | the page |
| Comments | `#9A9186` faded ink | ink, faded |

On a light editor the same ideas use deeper shades, so they stay readable on paper: `say` `#C45C12`, `when` `#1D408A`, `true` `#157A42`, `list` `#8C4E32`, `record` `#3E5C32`, `of` `#8A5A00`, `at` `#5C2E9A`, `get` `#15556A`, `trim` `#3A44A0`, `fn` `#8A3D78`, `maybe` `#A85A38`, `fails` `#A33B3B`, `require` `#7A5A20`, `where` and `type` `#1A5270`, `round` `#2E7A5C`, `bring` `#3D5270`, `permit` `#8C4E20`, `ask` `#8E2C8A`, numbers `#9A4A12`, text `#6B3A22`. Where to write `of` and `at` is in [Lists and records](groups.md#where-of-and-at-are-used). Asking for a field is in [Asking for data](asking.md).

Every word Quill has today has its color. A word is painted only once the language has it, so a name you invent is never colored by mistake.

This is installed on each computer where Quill runs. Set `QUILL_NO_EDITOR_SETUP=1` to leave the editor alone.

Quill code inside a Markdown file, in a ` ```quill ` block like the ones in a project's `README.md` and `QUILL.md`, gets the same colors.

In a folder VS Code or Cursor doesn't trust yet, the editor shows *Restricted Mode*. Quill's colors and the feather still work there. Underlining mistakes runs the `quill` program, so it starts once you trust the folder: choose **Manage** on the Restricted Mode bar, then **Trust**.

Vim, Emacs, and Sublime also know `.quill` files. The files are in the `editors` folder. Any editor that can talk to a language server, including Neovim, Helix, and Zed, gets the underlines from `quill lsp`. That is the same check as `quill check`, and it does not run the program.

## The commands you'll use

| Command | What it does |
|---|---|
| `quill run FILE.quill` | Run a program |
| `quill check FILE.quill` | Look for mistakes without running anything |
| `quill lsp` | Underline those mistakes in an editor |
| `quill test` | Run the tests in every file in this folder |
| `quill explain FILE.quill:N` | Show how Quill understood line N |
| `quill new NAME` | Start a new project from a template |
| `quill update` | Choose a version of Quill to install |
| `quill help` | Show all the commands |

## Try it

1. Write a program that says your name, then your favourite food, on two lines.
2. Add a comment above each line saying what it does.
3. Remove one of the quotes from your program and run it. Read what Quill says. You'll learn more about errors in [Reading Quill's Errors](errors.md).
