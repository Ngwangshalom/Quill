# Values That Change

So far, every value has stayed the same once it was created. But some values need to change: a visitor counter, a running total, a game score. In this chapter you'll learn how Quill handles change.

## `var` and `set`

Mark a value with `var` when you create it, and you can change it later with `set`:

```quill
var visits = 0
say visits

set visits = visits + 1
say visits

set visits = visits + 1
say visits
```

```output
0
1
2
```

Read `set visits = visits + 1` as: "work out `visits + 1`, then make that the new value of `visits`".

## Why two words?

Most languages let you change any value at any time. Quill makes you mark changes for two reasons:

1. **You can see every change.** Search for `set` to find every place a value changes. A value without `var` never changes, so you never need to check.
2. **Mistakes are caught.** Changing a value that wasn't meant to change is usually a bug.

The README calls this "every mutation is visible when you read the code".

## Values without `var` can't be set

```quill,error
name = "Amina"
set name = "Omar"
```

```output
Problem on line 2 of example.quill:

    set name = "Omar"
        ^^^^
name can't change, because it isn't marked var.
It was created on line 1 as a value that stays the same. To let it change, write line 1 as:

    var name = "Amina"
```

## `set` only changes, it never creates

A value must be created with `var` before `set` can change it. When you misspell its name, Quill suggests the one you meant:

```quill,error
var visits = 0
set vistis = 2
```

```output
Problem on line 2 of example.quill:

    set vistis = 2
        ^^^^^^
There's no value called vistis.
set changes a value that was created with var, so create it first.

Did you mean:  set visits = 2
```

Creating a `var` a second time is also a mistake. Use `set` to change it:

```quill,error
var count = 0
count = 5
```

```output
Problem on line 2 of example.quill:

    count = 5
    ^^^^^
count already exists. It was created on line 1 with var.
To change it, use set instead of creating it again.

Did you mean:  set count = 5
```

## A value keeps its kind

A `var` can change its value, but not its kind. A Number stays a Number:

```quill,error
var total = 0
set total = "10"
```

```output
Problem on line 2 of example.quill:

    set total = "10"
                ^^^^
total holds a Number, so it can't be set to Text ("10").
A value keeps the same kind for the whole program.

Did you mean:  set total = 10
```

## A running total

Here's a typical use: adding up a bill one item at a time.

```quill
var total = 0
set total = total + 3.50   -- coffee
set total = total + 4.25   -- cake
set total = total + 3.50   -- another coffee

say "Total: {total}"
```

```output
Total: 11.25
```

## Try it

1. Make a `var` called `score` starting at 0. Add 10 three times, then say the score.
2. Try to `set` a value that wasn't created with `var`. Read the error and fix your program the way it suggests.
3. Make a `var` holding text, and try to set it to a number.
