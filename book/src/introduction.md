# Introduction

Welcome to **The Quill Programming Language**, a book for learning Quill from the very beginning.

Quill is a programming language built around one idea: **write what you mean**. Code should read like a description of what it does, and when something is wrong, the language should explain the problem as patiently as a good teacher would.

```quill
price = 12.50
qty = 3
total = price * qty

say "You bought {qty} items for {total}"
```

```output
You bought 3 items for 37.50
```

## Who this book is for

This book is for anyone who wants to learn Quill:

- **New programmers.** You don't need to know any other language. Every idea is explained the first time it appears.
- **Experienced programmers.** You can move quickly through the early chapters. [Appendix A](appendix-quick-reference.md) is a one-page summary of the language.

## What makes Quill different

Many languages let small mistakes slip through until a program is already running, sometimes in front of real customers. Quill tries to catch them first:

- **Numbers are exact.** `0.1 + 0.2` is exactly `0.3`, so prices and money never round by surprise.
- **Nothing changes by accident.** A value can only change if you mark it with `var`, so every change is visible when you read the code.
- **Mistakes are found before anything runs.** If you add text to a number, Quill tells you before the program starts.
- **Errors teach.** Every error says what went wrong, why, and usually what you probably meant.
- **Tests live with the code.** A `check` line is a test, written right next to the code it tests.

## How to read this book

The chapters build on each other, so read them in order the first time. Each one introduces a few ideas with small examples, shows the mistakes people commonly make, and ends with something to try.

Every example in this book is run by Quill's own test suite each time Quill changes. If the book shows that a program prints something, that is exactly what it prints.

Code you can type looks like this:

```quill
say "Hello!"
```

And what it prints looks like this:

```output
Hello!
```

## Quill is young

Quill is in early development. This book grows with the language: as each feature is built, a chapter explains it. [What's Coming Next](whats-next.md) lists the features that are designed but not built yet, and [Appendix C](appendix-version-history.md) records what changed in each version.

Let's begin by installing Quill and running your first program.
