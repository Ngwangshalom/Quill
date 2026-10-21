# Reading Quill's Errors

Everyone makes mistakes when programming, beginners and experts alike. What matters is how quickly you can understand and fix them. Quill's errors are designed to teach. In this chapter you'll learn how to read them and which tools help.

## The shape of an error

Every Quill error has the same parts:

```quill,error,file=shop.quill
greeting = "Hello"
say greting
```

```output
Problem on line 2 of shop.quill:

    say greting
        ^^^^^^^
There's no value called greting.
Values must be created with greting = ... before they can be used.

Did you mean:  say greeting
```

1. **Where:** the file and line, with `^^^` under the exact spot.
2. **What went wrong**, in the first sentence.
3. **Why** it's a problem, in the next sentence.
4. **Did you mean:** what Quill thinks you meant, when it can tell. Often you can copy it straight into your program.

## Quill checks before it runs

Before running anything, Quill reads your whole program and checks it. If it finds mistakes, it shows **all** of them and runs **nothing**, so a program never stops halfway through because of a typo on line 50:

```quill,error
say total
name = "Amina"
name = "Omar"
```

```output
Problem on line 1 of example.quill:

    say total
        ^^^^^
There's no value called total.
Values must be created with total = ... before they can be used.

Problem on line 3 of example.quill:

    name = "Omar"
    ^^^^
name already exists. It was created on line 2.
Values in Quill can't change once they're created, so give this one a different name.

Quill found 2 problems.
```

A few mistakes can only be found while running, like dividing by a value that turns out to be zero, or a failing `check`. Then the program stops at that line, with the same kind of explanation.

## quill check

`quill check` looks for mistakes without running anything. It's useful when a program does something you don't want to happen yet, like sending emails:

```text
$ quill check shop.quill
No problems found in shop.quill.
```

## quill explain

When you're not sure how Quill understood a line, ask it. `quill explain` takes the file and a line number:

```text
$ quill explain shop.quill:3

Line 3:  total = price * qty

  total         a new value (can't change)
  =             store the value on the right in total
  price * qty   a calculation, Number
  price         Number, created on line 1
  qty           Number, created on line 2
```

If the line has a problem, `quill explain` shows it too.

## Common mistakes

| Mistake | What Quill says | Fix |
|---|---|---|
| `say "Hello` | This text starts with a quote but never ends. | add the closing `"` |
| `say greting` | There's no value called greting. | fix the spelling |
| `total = price + "5"` | You're adding Number (price) to Text ("5"). | remove the quotes: `5` |
| `set name = "Omar"` | name can't change, because it isn't marked var. | create it with `var` |
| `sya "hi"` | Quill doesn't know what to do with sya here. | `say "hi"` |
| `x = 1 < y < 10` | Quill compares two values at a time. | `1 < y and y < 10` |

## If Quill ever crashes

Quill should never crash, whatever you type. If it does, that's a bug in Quill, not in your program. Please [report it](https://github.com/Ngwangshalom/Quill/issues) with the program that caused it.

## Try it

1. Make three different mistakes in one program and run `quill check`. Does Quill find all three?
2. Run `quill explain` on a line with a calculation.
3. Find an error message you think could be clearer, and suggest a better one on GitHub.
