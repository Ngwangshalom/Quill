# Repeating with for

Some work has to happen more than once: say every number from 1 through 10, add up a list of prices, print a row for each guest. A `for` loop repeats the lines under it, once for each number in a range. When those things are already gathered together, [Lists and records](groups.md) shows how `for` walks a list. When you want one field, a total, or the records inside other records, [Asking for data](asking.md) does that in one line. `for` stays for the step each record needs on its own.

## Counting from one number to another

```quill
for i in 1..3
  say i
```

```output
1
2
3
```

Read it as: "for each number `i` in 1 through 3, say `i`."

- `i` is the number for this pass. The first pass it is `1`, then `2`, then `3`.
- `1..3` includes both ends. The loop runs for 1, 2, and 3, and then stops.
- The lines to repeat go **below** `for`, indented by **exactly 2 spaces**.
- Each pass, `i` moves to the next number by itself. You don't write `set` for it.

A range that starts after it ends does nothing. The lines under it are skipped, and the program continues:

```quill
for i in 3..1
  say i
say "done"
```

```output
done
```

`for i in 5..5` runs once, with `i` as `5`.

## Keeping a total

The loop's own name can't be changed with `set`, and a line inside the loop can't create a new value, because that value would be created again on every pass. Create a `var` before the loop, then change it inside:

```quill
var total = 0
for n in 1..4
  set total = total + n
say total
```

```output
10
```

That adds 1, then 2, then 3, then 4. `10` is their sum.

The end of the range is decided once, before the first pass. Changing a value inside the loop doesn't move the finish line.

## The name lives inside the loop

`i` exists on the lines under the `for`, and nowhere else:

```quill,error
for i in 1..3
  say i
say i
```

If a value with that name already exists, the loop borrows the name while it runs. Afterwards the earlier value is still there:

```quill
i = 10
for i in 1..2
  say i
say i
```

```output
1
2
10
```

The range itself sees the earlier value. `for i in i..12`, when `i` is `10`, counts from 10 through 12.

## Common mistakes

The repeated lines have to be indented by exactly 2 spaces. On the same line as `for`, or back at the left edge, they aren't part of the loop.

A range is two dots. Three dots is a mistake:

```quill,error
for i in 1...3
  say i
```

Both ends of the range have to be numbers:

```quill,error
for i in 1.."3"
  say i
```

And the loop can't create a value:

```quill,error
for i in 1..3
  label = i
```

Create `label` before the loop with `var`, then use `set` inside.

## Try it

1. Say the numbers from 1 through 5.
2. Add those numbers into a `var total`, and say the total when the loop is finished. It should be 15.
3. Try `for i in 5..5`. How many lines does it print?
