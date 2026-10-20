# Files, Settings and Permits

So far, programs only worked with values written inside them. Real programs read a price list from a file, write a report, and use a secret key that must never be written in the code. Quill can do all of that, but only what the program **permits**.

## Permits

A `permit` line at the top of a program says one thing it may touch outside itself:

```quill,ignore
permit read "config"            -- read files in the config folder
permit write "reports"          -- write files in the reports folder
permit env "STRIPE_KEY"         -- read the setting STRIPE_KEY
permit net "api.stripe.com"     -- talk to this host (used by http and ai)
```

Anything else is refused. A reader sees everything a program can reach in its first lines, and a mistake, or a file you brought in from someone else, can't quietly read your secrets or send your data somewhere.

A folder permit covers everything inside it: `permit read "config"` allows `config/rates.txt` and `config/2026/may.txt`. `permit read "."` allows the program's own folder and everything below it. Paths are counted from the folder the program is in.

## Reading a file

Say `config/rates.txt` holds one line:

```quill,part,file=config/rates.txt
USD 1
```

`read_file` gives back the whole file as text:

```quill
permit read "config"

rates = read_file("config/rates.txt")
  on FileMissing -> "none"
say "Rates: {rates}"
```

```output
Rates: USD 1

```

The file ends with a new line, like most text files, so `say` shows an empty line after it.

A file might not be there, so `read_file` can fail with `FileMissing`, and the program has to say what happens then, the same as any [failure](failures.md). Here the program carries on with `"none"`.

`file_exists` answers whether a file is there, without reading it:

```quill
permit read "config"

say file_exists("config/rates.txt")
say file_exists("config/old.txt")
```

```output
true
false
```

## Writing a file

`write_file` writes text to a file, replacing what was there. It makes the folder first if it isn't there yet:

```quill
permit write "reports"
permit read "reports"

write_file("reports/today.txt", "All 3 bookings confirmed.")
  on CantWrite -> say "Couldn't save the report."
done = read_file("reports/today.txt")
  on FileMissing -> "nothing"
say done
```

```output
All 3 bookings confirmed.
```

Writing can fail too, for example when the disk is full, so `write_file` fails with `CantWrite`.

## Settings and secrets: env

A secret key belongs outside the code, in a setting of the computer the program runs on (an *environment variable*). `env` reads one. A setting might not be set, so `env` gives back a [maybe](maybe.md):

```quill
permit env "QUILL_BOOK_EXAMPLE_KEY"

when env("QUILL_BOOK_EXAMPLE_KEY")
  found key -> say "The key is set."
  missing -> say "Set QUILL_BOOK_EXAMPLE_KEY before running."
```

```output
Set QUILL_BOOK_EXAMPLE_KEY before running.
```

Keys don't belong in code, because code gets shared, copied and published. A setting stays on the one computer that needs it.

## When a permit is missing

When the path or name is written in the call, Quill finds a missing permit before the program runs, and says which line to add:

```quill,error
key = read_file("secrets/key.txt")
  on FileMissing -> "none"
say key
```

```output
Problem on line 1 of example.ql:

    key = read_file("secrets/key.txt")
                    ^^^^^^^^^^^^^^^^^
read_file can't read secrets/key.txt, because the program has no permit for it.
A program touches only what its permits allow. If it should, add this at the top of the program.

Did you mean:  permit read "secrets"
```

When the path is only known while the program runs, the call stops the program instead. A path can't climb out of a permitted folder with `..`:

```quill,error
permit read "config"

path = "config/../secrets.txt"
key = read_file(path)
  on FileMissing -> "none"
say key
```

```output
Problem on line 4 of example.ql:

    key = read_file(path)
          ^^^^^^^^^^^^^^^
read_file can't read config/../secrets.txt, because the program has no permit for it.
A program touches only what its permits allow. If it should, add this at the top of the program: permit read "secrets.txt"
```

Permits belong to the program you run. A file you [bring in](bring.md) can't give itself permits, so everything a program may touch is in one place.

## Common mistakes

A failure has to be handled. Forgetting `on FileMissing` is found before running:

```quill,error
permit read "config"

say read_file("config/rates.txt")
```

```output
Problem on line 3 of example.ql:

    say read_file("config/rates.txt")
        ^^^^^^^^^
read_file can fail with FileMissing.
Handle each failure with on under the call, or write try read_file(...) inside a function that fails the same way.
```

`permit net` names a host, not a web address:

```quill,error
permit net "https://api.stripe.com/v1"
```

```output
Problem on line 1 of example.ql:

    permit net "https://api.stripe.com/v1"
               ^^^^^^^^^^^^^^^^^^^^^^^^^^^
permit net names a host, like api.stripe.com, without https:// or a path.
Every path on that host is allowed.

Did you mean:  permit net "api.stripe.com"
```

`read_file`, `write_file`, `file_exists` and `env` are Quill's own functions, so a function you write can't use those names.

## Try it

1. Make a file `notes/hello.txt`, and a program that permits reading `notes` and says what is in it.
2. Remove the permit, and read what Quill says.
3. Write a program that writes the date you choose into `out/today.txt`, then reads it back.
4. Read a setting with `env`, and say whether it is set.
