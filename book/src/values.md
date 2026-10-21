# Values

Programs work with **values**: pieces of information like a name, a price, or whether a customer is logged in. In this chapter you'll learn how to give values names and what kinds of values there are.

## Giving a value a name

Write a name, then `=`, then the value:

```quill
name = "Amina"
age = 34

say name
say age
```

```output
Amina
34
```

After this line runs, `name` means `"Amina"` everywhere below it. Using a name instead of repeating the value makes programs easier to read and to change.

Names can use letters, digits and `_`, but must start with a letter or `_`. Good names say what the value means: `total_price` is clearer than `tp`. Names are case-sensitive, so `total` and `Total` are different names.

## Values don't change

In Quill, a value created with `=` **can't change**. Once `name` is `"Amina"`, it stays `"Amina"`. Creating the same name again is a mistake:

```quill,error
name = "Amina"
name = "Omar"
```

```output
Problem on line 2 of example.quill:

    name = "Omar"
    ^^^^
name already exists. It was created on line 1.
Values in Quill can't change once they're created, so give this one a different name.
```

This might seem strict, but it makes programs much easier to understand: when you read `name`, you know it is the value from the line that created it. When you do need a value that changes, like a counter, you say so with `var`. You'll learn how in [Values That Change](changing-values.md).

## Create before you use

Quill reads a program from top to bottom, so a name must be created before it's used:

```quill,error
say greeting
greeting = "Hello"
```

```output
Problem on line 1 of example.quill:

    say greeting
        ^^^^^^^^
greeting is used here, but it isn't created until line 2.
Quill reads from top to bottom, so create values before using them.
```

## Kinds of values

Every value has a **kind**. Quill starts with three:

| Kind | Examples | Used for |
|---|---|---|
| **Text** | `"Amina"`, `"Hello, World!"` | words and sentences |
| **Number** | `34`, `12.50`, `-3` | counting and calculating |
| **true/false** | `true`, `false` | answers to yes-or-no questions |

```quill
city = "Dubai"
guests = 120
confirmed = true

say city
say guests
say confirmed
```

```output
Dubai
120
true
```

A list and a record gather several values into one. They have their own chapter, [Lists and records](groups.md).

A name keeps its kind for the whole program. Quill knows the kind of every value before the program runs, which is how it catches mistakes like adding text to a number. You'll see that in the next chapter.

Quill never turns one kind into another on its own: `"5"` is text, while `5` is a number.

## Try it

1. Create values for a product: its name, its price, and whether it's in stock. Show each one with `say`.
2. Try to create the same name twice. Read the error.
3. Use a name before creating it. What does Quill say?
