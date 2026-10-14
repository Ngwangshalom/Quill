# Maybe

Some questions have no answer. A price list might not contain the name you asked for. `maybe` says the answer might be missing, and Quill will not run until you handle both cases.

```quill
fn find_price(name: text) -> maybe number
  when
    name == "bread" -> return found 2.50
    else -> return missing

when find_price("bread")
  found price -> say price
  missing -> say "none"
```

```output
2.50
```

`-> maybe number` means the function gives back a number, or it gives back nothing. `found 2.50` is the number. `missing` is the gap. `return missing` is allowed only inside a function that gives a maybe back.

`when` followed by the call opens that maybe. `found price` holds the number for that one line. `missing` is the other case. Both cases are required:

```quill,error
fn find_price(name: text) -> maybe number
  when
    name == "bread" -> return found 2.50
    else -> return missing

when find_price("bread")
  found price -> say price
```

```output
Problem on line 6 of example.ql:

    when find_price("bread")
    ^^^^
This when doesn't handle missing.
A maybe can be missing, and that case has to be handled too.

Did you mean:  missing -> say "none"
```

`say find_price("bread")` is a mistake too. `say` would hide the gap. Open the maybe with `when` first.

The kind after `maybe` can be `text`, `number`, `true/false`, a currency such as `USD`, or a list of one of those.

## Try it

1. Call `find_price("cake")` and handle both cases. The cake is missing.
2. Change the function so `"cake"` is found at `4`.
3. Remove the `missing` case and read what Quill says.
