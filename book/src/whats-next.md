# What's Coming Next

Quill is young, and these features are designed but not built yet. The examples on this page show how they're planned to work. They don't run yet.

Functions, [money](money.md), [maybe](maybe.md), [failures](failures.md), [contracts](contracts.md) with ranges like `where 1..500`, and [asking for data](asking.md) with `get` and `sum` are in the language now. `for` is still there when each record needs its own step. A `when` can give a value, and `on` can give a value and carry on. What is still ahead is kind names like `Int` and `Text`. Today a parameter's kind is `number`, `text`, `true/false`, a currency such as `USD`, `maybe` one of those, or `list of` one of those.

## And more

Types with rules on their fields (`type Booking` with `guests: Int where 1..500`), contracts that Quill proves before the program runs, permissions (`permit net "api.stripe.com"`), and built-in modules for web servers and databases. See the [README](https://github.com/Ngwangshalom/Quill#readme) for the full design.

Each of these will get its own chapter in this book as it's built. Want to help build one? Read the [contributor guide](https://github.com/Ngwangshalom/Quill/blob/main/CONTRIBUTING.md).
