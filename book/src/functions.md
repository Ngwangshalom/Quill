# Functions

A function names a piece of work so you can run it again. You write it once, then call it by name.

```quill
fn double(n: number) -> number
  return n * 2

say double(21)
```

```output
42
```

`fn` starts the function. `double` is its name. Inside the parentheses, `n: number` is a parameter: a name, then `:`, then the kind of value it takes. `-> number` is the kind the function gives back. The lines under `fn` are indented by exactly 2 spaces, and `return` gives the value back.

`double(21)` is a call. The value inside the parentheses is what `n` holds for that call.

## Why you use `return`

A function can work out an answer. `return` hands that answer to the line that called the function, so the rest of the program can use it.

`say` only prints. The next line cannot use what was printed. `return` gives the answer back, so you can print it, add to it, or check it:

```quill
fn shown(n: number)
  say n * 2

fn doubled(n: number) -> number
  return n * 2

shown(21)
var answer = doubled(21)
say answer + 1
```

```output
42
43
```

`shown(21)` prints `42` and gives nothing back. `doubled(21)` gives `42` back. That is what `answer` holds, so `answer + 1` is `43`.

`return` only goes inside a function. On its own line, outside any `fn`, use `say` to show a value.

The words in the parentheses and the word after `->` are separate. Each parameter names the kind you pass in. The arrow names the one kind that comes back. Those parameters can be different kinds from each other:

```quill
fn dataHope(name: text, age: number) -> text
  return "{name} is {age}"

say dataHope("Amina", 18)
```

```output
Amina is 18
```

`name` is text and `age` is a number. `-> text` means the call gives text back, so `return` has to give text. `"{name} is {age}"` is text: the number is written into the text, and the whole thing is one piece of text.

## The arrow names one kind

`-> number` means every `return` in that function gives a number. `-> text` means every `return` gives text. Quill will not turn a number into text, or text into a number, to make the kinds match.

```quill,error
fn double(n: number) -> number
  return "hi"
```

```output
Problem on line 2 of example.quill:

      return "hi"
             ^^^^
double gives back a Number, but "hi" is Text.
return has to give back a number.
```

A kind is `text`, `number`, `true/false`, or `list of` one of those:

```quill
fn owed(prices: list of number) -> number
  var total = 0
  for price in prices
    set total = total + price
  return total

say owed(list 3.50, 4.25)
```

```output
7.75
```

`var total` lives inside the function. It is created each time the function runs, and it disappears when the function returns. A parameter is not a `var`: `n` stays the value you passed in.

## A function that only does work

Leave off `->` when the function gives nothing back. Call it on its own line:

```quill
fn greet(name: text)
  say "Hello, {name}"

greet("Ngwang")
```

```output
Hello, Ngwang
```

There is no value to give back, so `return` cannot hand one out. A value after `return` is a mistake:

```quill,error
fn greet(name: text)
  return "Hello"
```

```output
Problem on line 2 of example.quill:

      return "Hello"
             ^^^^^^^
greet doesn't give a value back, so return stands on its own.
To give "Hello" back, write what greet gives back after ->.
```

To give `"Hello"` back, add `-> text` and keep the `return`.

`return` on its own, with no value after it, only stops the function early:

```quill
fn greet(name: text)
  when
    name == "" -> return
  say "Hello, {name}"

greet("")
greet("Ngwang")
```

```output
Hello, Ngwang
```

The check for `""` is only an example of when you might stop early. A function with no `->` does not have to test anything. Delete the `when`, and `greet("Ngwang")` still says hello. `greet("")` stops at `return` and says nothing. `greet("Ngwang")` skips that line, because the name is not empty, and reaches `say`.

## Calling a function from a function

A function can call one written above it, and it can call itself:

```quill
fn fact(n: number) -> number
  when
    n <= 1 -> return 1
    else -> return n * fact(n - 1)

say fact(5)
```

```output
120
```

Every path has to reach `return` when the function gives a value back. Here both cases of the `when` return, so `fact` always gives a number back.

A function does not see names from outside it. Pass those values in as parameters.

## Common mistakes

Forgetting `return` is a mistake. Quill has to know the function gives its kind back:

```quill,error
fn double(n: number) -> number
  say n
```

```output
Problem on line 1 of example.quill:

    fn double(n: number) -> number
       ^^^^^^
double has to give back a Number.
End every path with return, like return 1.

Did you mean:  return 1
```

A parameter's kind is written with the words Quill has today: `number`, `text`, `true/false`. Names like `Int` and `Text` arrive later, with types.

## Try it

1. Write `fn triple(n: number) -> number` that returns `n * 3`, and say `triple(5)`.
2. Write `fn hello(name: text)` that says hello to that name, and call it with your name.
3. Change `triple` so it returns text, and read what Quill says.
