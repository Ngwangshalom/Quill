# Chance: random, pick, and hash

A draw needs a ticket number. A code needs a fingerprint of a secret. Quill gives three tools for that: `random` for a number in a range, `pick from` for one item from a list, and `hash` for a fingerprint of text. `seed` makes the draws the same every run, so tests can check them.

None of these words is reserved. `var random = 1` and `fn hash(...)` still work. The tools only kick in when the word starts a value the Quill way: `random 1..100`, `pick from names`, `hash secret`.

## A number in a range: random

`random 1..100` draws a whole number from 1 through 100, both ends included. The same range shape as `for i in 1..3`.

```quill
seed 1
say random 1..10
```

```output
9
```

Both ends have to be whole numbers, and the smaller one comes first. A part of a number, or a range written the wrong way round, is a mistake.

## One item from a list: pick from

`pick from names` chooses one item. The list has to have at least one:

```quill
seed 1
names = list "Amina", "Omar", "Sola"
say pick from names
```

```output
Omar
```

An empty list stops with a problem that says there is nothing to choose.

## A fingerprint: hash

`hash secret` turns text into 64 lowercase hex letters. The same text always gives the same letters. That is SHA-256, the usual fingerprint for codes and tokens:

```quill
say hash "desk"
```

```output
49be417ad74080a0031b636b44cfc26fdd0065492d6cd3b033960c4414955cf7
```

`hash` needs text. A number or a list is a mistake.

## The same draws every run: seed

Without `seed`, each run of `random` and `pick from` can give a different answer. That is right for a live draw. For a `check` or a book example, fix the draws first:

```quill
seed 1
check random 1..10 == 9
check pick from list "a", "b", "c" == "a"
```

`seed` takes a whole number. Later `random` and `pick` in that run follow from it. `hash` does not need a seed: it is always the same for the same text.

A web route that never writes `seed` gets a fresh source of chance on each request.

## Common mistakes

`random` needs `..` between the ends:

```quill,error
say random 1
```

```text
Problem on line 1:

    say random 1
                ^
random needs .. between the smallest and the largest, but found the end of the line.
random 1..100 draws a whole number from 1 through 100.

Did you mean:  random 1..1
```

`pick from` needs a list with something in it:

```quill,error
seed 1
say pick from list of text
```

```text
pick from needs at least one item in the list.
An empty list has nothing to choose.
```

`hash(x)` with no space calls a function named `hash`. The tool needs a space, or no brackets: `hash x`.

## Try it

1. Draw a ticket from 1 through 50 with `seed 2`, and check the number you get.
2. Put three acts in a list, pick a winner with the same seed, and say who won.
3. Hash a short password and check that hashing it again gives the same text.
4. Run the [draw](https://github.com/Ngwangshalom/Quill/tree/main/examples/draw) example: `quill run examples/draw/draw.quill`, then `quill test examples/draw/draw.quill`.

`random`, `pick`, `hash`, and `seed` are ember (`#E6A07A` on a dark editor, `#9A4E28` on a light one).
