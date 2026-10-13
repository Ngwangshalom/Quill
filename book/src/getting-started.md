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

Quill gets better with every release. To update to the latest version, run:

```bash
quill update
```

You don't have to remember to check. At most once a day, Quill quietly asks whether a new version is out, and if one is, it tells you after your command finishes:

```text
Quill 0.3.0 is available (you have 0.2.0). Run quill update to get it.
```

Quill never updates itself without asking, so a program that works today won't change behaviour tomorrow unless you choose to update. The check runs in the background and never slows Quill down. If you'd rather Quill didn't check at all, set `QUILL_NO_UPDATE_CHECK=1` in your environment.

`quill update` works for copies installed with the commands above. If you built Quill from its source code with `cargo`, update it by getting the latest code and running `cargo install --path . --force` in the Quill folder.

## Hello, World!

It's a tradition to start with a program that shows the words "Hello, World!". In Quill, that program is one line.

Create a file called `hello.ql` (Quill programs always end in `.ql`) and write:

```quill
say "Hello, World!"
```

Then run it:

```bash
quill run hello.ql
```

```output
Hello, World!
```

`say` shows a value. Here the value is the text `"Hello, World!"`. The quotes mark where the text starts and ends; they aren't shown.

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

`quill new` creates a folder with a starter program in it:

```bash
quill new shop
```

```text
Created shop/hello.ql.
Run it with:  quill run shop/hello.ql
Test it with:  quill test
```

Inside `shop/hello.ql` you'll find:

```quill
greeting = "Hello, World!"
say greeting

check greeting == "Hello, World!"
```

```output
Hello, World!
```

The first line gives the text a name, `greeting`. The last line is a test, which you'll learn about in [Testing Your Code](testing.md).

## Comments

A comment is a note for people reading the code. Quill ignores it. A comment starts with `--` and runs to the end of the line:

```quill
-- Show a welcome message.
say "Welcome!"   -- this part is ignored too
```

```output
Welcome!
```

## The commands you'll use

| Command | What it does |
|---|---|
| `quill run FILE.ql` | Run a program |
| `quill check FILE.ql` | Look for mistakes without running anything |
| `quill test` | Run the tests in every file in this folder |
| `quill explain FILE.ql:N` | Show how Quill understood line N |
| `quill new NAME` | Start a new project |
| `quill update` | Update Quill to the latest version |
| `quill help` | Show all the commands |

## Try it

1. Write a program that says your name, then your favourite food, on two lines.
2. Add a comment above each line saying what it does.
3. Remove one of the quotes from your program and run it. Read what Quill says. You'll learn more about errors in [Reading Quill's Errors](errors.md).
