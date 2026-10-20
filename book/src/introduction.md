# Introduction

Welcome to **The Quill Programming Language**, a book for learning Quill from the very beginning.

Quill is a programming language where intent is clear, failures are explicit, and correctness is part of the code. It is for backend engineering, financial software, automation, reliable business applications, and AI and machine learning. The short form of that idea is **write what you mean**.

The words stay close to English. `when` chooses a path. `found` and `missing` both have to be handled. `fails` and `on` name a failure and what to do about it. `check` is a test written next to the code. Those words are Quill's voice.

A bank withdrawal is one kind of service. An account, a booking, an order, a school record, and a job that can fail are others. Asking a model is the same kind of care: the answer may be missing, and the call may fail. Quill can [serve requests](web.md), [read and write JSON](json.md), [call other services](http.md), and [ask a model](ai.md). Storing rows in a database is the next step.

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

- **Money is exact.** `100 USD + 50 USD` is `150 USD`, and `0.1 + 0.2` is exactly `0.3`. Dollars and euros cannot be added.
- **Ask for the data.** `sum fee of roster` adds each fee. `get place of desk.bookings` reads a field of a list on a record, and `get gpa of school.acceptances` reads a field of a nested record. [Asking for data](asking.md) shows each form. `for` stays when each record needs its own step.
- **A missing answer has to be handled.** `maybe` says a value might not be there. `found` and `missing` are both required.
- **A failure has to be handled.** `fails Declined` names it, `fail Declined` leaves with it, and the caller writes `on Declined`.
- **Functions state their rules.** `require amount > 0 USD` says what a function needs, and `ensure result >= 0 USD` says what it promises. `guests: number where 1..500` gives a parameter its range. When one is broken, the program stops and says which value broke it.
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
