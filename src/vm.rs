//! The virtual machine: runs compiled bytecode.

use std::io::Write;

use crate::compiler::{Chunk, ContractInfo, Op};
use crate::decimal::{Decimal, MathError};
use crate::diagnostic::{Diagnostic, Span};
use crate::parser::Comparison;
use crate::value::Value;

/// One running function, or the main program when `function` is absent.
struct Frame {
    function: Option<usize>,
    slots: Vec<Option<Value>>,
    next: usize,
}

/// Runs `chunk`, writing everything `say` prints to `out`.
pub fn run(chunk: &Chunk, out: &mut impl Write) -> Result<(), Diagnostic> {
    run_with_stats(chunk, out, &mut RunStats::default())
}

/// Counts successful assertions, including those before a runtime failure.
#[derive(Default)]
pub struct RunStats {
    pub checks_passed: usize,
}

/// Runs a program and adds each successful assertion to `stats`.
pub fn run_with_stats(
    chunk: &Chunk,
    out: &mut impl Write,
    stats: &mut RunStats,
) -> Result<(), Diagnostic> {
    let mut stack: Vec<Value> = Vec::new();
    let mut frames = vec![Frame {
        function: None,
        slots: vec![None; chunk.slot_count],
        next: 0,
    }];

    loop {
        if frames.is_empty() {
            break;
        }
        let index = frames.len() - 1;
        let function = frames[index].function;
        let at = frames[index].next;
        let (code, spans, returns) = match function {
            None => (&chunk.code, &chunk.spans, false),
            Some(fn_index) => {
                let function = chunk.functions.get(fn_index).ok_or_else(|| {
                    internal(
                        Span {
                            line: 1,
                            column: 1,
                            length: 0,
                        },
                        "a call to a function that wasn't compiled",
                    )
                })?;
                (&function.code, &function.spans, function.returns)
            }
        };
        let Some(&op) = code.get(at) else {
            if returns {
                let name = function
                    .and_then(|fn_index| chunk.functions.get(fn_index))
                    .map(|function| function.name.as_str())
                    .unwrap_or("This function");
                return Err(Diagnostic::new(
                    Span {
                        line: 1,
                        column: 1,
                        length: 0,
                    },
                    format!(
                        "{name} ended without giving a value back.\n\
                         Add return, and make sure every path reaches it."
                    ),
                ));
            }
            frames.pop();
            continue;
        };
        let span = spans.get(at).copied().unwrap_or(Span {
            line: 1,
            column: 1,
            length: 0,
        });
        let mut next = at + 1;
        match op {
            Op::Constant(i) => {
                let value = chunk
                    .constants
                    .get(i)
                    .cloned()
                    .ok_or_else(|| internal(span, "a constant is missing"))?;
                stack.push(value);
            }
            Op::Store(slot) => {
                let value = pop(&mut stack, span)?;
                let target = frames[index]
                    .slots
                    .get_mut(slot)
                    .ok_or_else(|| internal(span, "a value slot is missing"))?;
                *target = Some(value);
            }
            Op::Load(slot) => {
                let value = frames[index]
                    .slots
                    .get(slot)
                    .cloned()
                    .flatten()
                    .ok_or_else(|| internal(span, "a value was read before it was set"))?;
                stack.push(value);
            }
            Op::Say => {
                let value = pop(&mut stack, span)?;
                writeln!(out, "{value}").map_err(|err| {
                    Diagnostic::new(
                        span,
                        format!(
                            "Quill couldn't print this line: {err}.\n\
                             The output was closed before the program finished."
                        ),
                    )
                })?;
            }
            Op::Check { op, example } => {
                let right = pop(&mut stack, span)?;
                let left = pop(&mut stack, span)?;
                if let Some(message) = check_failure(op, example, &left, &right) {
                    return Err(Diagnostic::new(span, message));
                }
                stats.checks_passed += 1;
            }
            Op::Add | Op::Subtract | Op::Multiply | Op::Divide | Op::Remainder => {
                let right = pop(&mut stack, span)?;
                let left = pop(&mut stack, span)?;
                stack.push(calculate(op, left, right, span)?);
            }
            Op::Negate => {
                let value = pop(&mut stack, span)?;
                let negated = match value {
                    Value::Number(number) => Value::Number(
                        number
                            .checked_neg()
                            .map_err(|err| math_error(err, op, span))?,
                    ),
                    Value::Money { amount, currency } => Value::Money {
                        amount: amount
                            .checked_neg()
                            .map_err(|err| math_error(err, op, span))?,
                        currency,
                    },
                    _ => {
                        return Err(internal(
                            span,
                            "a negative value that isn't a number or money",
                        ))
                    }
                };
                stack.push(negated);
            }
            Op::Not => {
                let value = pop_bool(&mut stack, span)?;
                stack.push(Value::Bool(!value));
            }
            Op::Compare(comparison) => {
                let right = pop(&mut stack, span)?;
                let left = pop(&mut stack, span)?;
                let result = compare(comparison, &left, &right)
                    .ok_or_else(|| internal(span, "two values of different kinds were compared"))?;
                stack.push(Value::Bool(result));
            }
            Op::Join(count) => {
                let start = stack
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| internal(span, "the value stack is empty"))?;
                let text: String = stack
                    .split_off(start)
                    .iter()
                    .map(Value::to_string)
                    .collect();
                stack.push(Value::Text(text));
            }
            Op::JumpIfFalse(target) => {
                if !pop_bool(&mut stack, span)? {
                    next = target;
                }
            }
            Op::Jump(target) => next = target,
            Op::List(count) => {
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(pop(&mut stack, span)?);
                }
                items.reverse();
                stack.push(Value::List(items));
            }
            Op::Record(index) => {
                let names = chunk
                    .records
                    .get(index)
                    .cloned()
                    .ok_or_else(|| internal(span, "a record's fields are missing"))?;
                let mut values = Vec::with_capacity(names.len());
                for _ in &names {
                    values.push(pop(&mut stack, span)?);
                }
                values.reverse();
                stack.push(Value::Record(names.into_iter().zip(values).collect()));
            }
            Op::Length => {
                let value = pop(&mut stack, span)?;
                let Value::List(items) = value else {
                    return Err(internal(span, "the length of something that isn't a list"));
                };
                stack.push(Value::Number(Decimal::from_int(items.len() as i128)));
            }
            Op::Append => {
                let item = pop(&mut stack, span)?;
                let Value::List(mut items) = pop(&mut stack, span)? else {
                    return Err(internal(
                        span,
                        "an item added to something that isn't a list",
                    ));
                };
                items.push(item);
                stack.push(Value::List(items));
            }
            Op::GetAt => {
                let index = pop_number(&mut stack, span)?;
                let list = pop(&mut stack, span)?;
                let position = list_position(&list, index, span)?;
                let Value::List(items) = list else {
                    return Err(internal(span, "an item from something that isn't a list"));
                };
                stack.push(items[position].clone());
            }
            Op::SetAt => {
                let new_item = pop(&mut stack, span)?;
                let index = pop_number(&mut stack, span)?;
                let list = pop(&mut stack, span)?;
                let position = list_position(&list, index, span)?;
                let Value::List(mut items) = list else {
                    return Err(internal(span, "a change to something that isn't a list"));
                };
                items[position] = new_item;
                stack.push(Value::List(items));
            }
            Op::GetField(index) => {
                let name = field_name(chunk, index, span)?;
                let record = pop(&mut stack, span)?;
                stack.push(record_field(&record, &name, span)?);
            }
            Op::SetField(index) => {
                let name = field_name(chunk, index, span)?;
                let new_value = pop(&mut stack, span)?;
                let record = pop(&mut stack, span)?;
                let Value::Record(mut fields) = record else {
                    return Err(internal(span, "a change to something that isn't a record"));
                };
                let Some(field) = fields.iter_mut().find(|(field, _)| field == &name) else {
                    return Err(Diagnostic::new(
                        span,
                        format!(
                            "This record has no field called {name}.\n\
                             Its fields are {}.",
                            fields
                                .iter()
                                .map(|(field, _)| field.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    ));
                };
                field.1 = new_value;
                stack.push(Value::Record(fields));
            }
            Op::Pluck(index) => {
                let name = field_name(chunk, index, span)?;
                let list = pop(&mut stack, span)?;
                stack.push(pluck(list, &name, span)?);
            }
            Op::Pick(index) => {
                let names = chunk
                    .picks
                    .get(index)
                    .cloned()
                    .ok_or_else(|| internal(span, "the fields of a get were not compiled"))?;
                let list = pop(&mut stack, span)?;
                stack.push(pick(list, &names, span)?);
            }
            Op::SumField { name, currency } => {
                let code = match currency {
                    Some(index) => Some(field_name(chunk, index, span)?),
                    None => None,
                };
                let field = field_name(chunk, name, span)?;
                let list = pop(&mut stack, span)?;
                stack.push(sum_field(list, &field, code.as_deref(), span)?);
            }
            Op::Gather { outer, inner } => {
                let nested = field_name(chunk, inner, span)?;
                let field = field_name(chunk, outer, span)?;
                let source = pop(&mut stack, span)?;
                stack.push(gather(source, &field, &nested, span)?);
            }
            Op::Dup => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or_else(|| internal(span, "the value stack is empty"))?;
                stack.push(value);
            }
            Op::Swap => {
                let top = pop(&mut stack, span)?;
                let under = pop(&mut stack, span)?;
                stack.push(top);
                stack.push(under);
            }
            Op::Call(fn_index) => {
                let function = chunk
                    .functions
                    .get(fn_index)
                    .ok_or_else(|| internal(span, "a call to a function that wasn't compiled"))?;
                let mut slots = vec![None; function.slot_count];
                for slot in (0..function.arity).rev() {
                    slots[slot] = Some(pop(&mut stack, span)?);
                }
                frames[index].next = next;
                frames.push(Frame {
                    function: Some(fn_index),
                    slots,
                    next: 0,
                });
                continue;
            }
            Op::Return(gives) => {
                let value = if gives {
                    Some(pop(&mut stack, span)?)
                } else {
                    None
                };
                frames.pop();
                if let Some(value) = value {
                    stack.push(value);
                }
                continue;
            }
            Op::WrapFound => {
                let value = pop(&mut stack, span)?;
                stack.push(Value::Found(Box::new(value)));
            }
            Op::MakeMissing => stack.push(Value::Missing),
            Op::MatchFound(target) => {
                let value = pop(&mut stack, span)?;
                match value {
                    Value::Missing => next = target,
                    Value::Found(inner) => stack.push(*inner),
                    _ => return Err(internal(span, "when opened a value that isn't a maybe")),
                }
            }
            Op::Fail => {
                let Value::Text(name) = pop(&mut stack, span)? else {
                    return Err(internal(span, "a failure without a name"));
                };
                frames.pop();
                if frames.is_empty() {
                    return Err(Diagnostic::new(
                        span,
                        format!(
                            "{name} wasn't handled.\n\
                             Add on {name} under the call, or fails {name} on the function that tries it."
                        ),
                    ));
                }
                stack.push(Value::Failure(name));
                continue;
            }
            Op::OnFail(constant, target) => {
                let name = match chunk.constants.get(constant) {
                    Some(Value::Text(name)) => name.clone(),
                    _ => return Err(internal(span, "a failure handler is missing its name")),
                };
                if matches!(stack.last(), Some(Value::Failure(failure)) if failure == &name) {
                    stack.pop();
                    next = target;
                }
            }
            Op::Contract(contract) => {
                if !pop_bool(&mut stack, span)? {
                    let contract = chunk
                        .contracts
                        .get(contract)
                        .ok_or_else(|| internal(span, "a contract is missing"))?;
                    return Err(Diagnostic::new(
                        span,
                        broken_contract(contract, &frames[index].slots, call_line(chunk, &frames)),
                    ));
                }
            }
        }
        if let Some(frame) = frames.get_mut(index) {
            frame.next = next;
        }
    }
    Ok(())
}

fn calculate(op: Op, left: Value, right: Value, span: Span) -> Result<Value, Diagnostic> {
    let numbers = |left: Decimal, right: Decimal| match op {
        Op::Add => left.checked_add(right),
        Op::Subtract => left.checked_sub(right),
        Op::Multiply => left.checked_mul(right),
        Op::Divide => left.checked_div(right),
        _ => left.checked_rem(right),
    };
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => Ok(Value::Number(
            numbers(left, right).map_err(|err| math_error(err, op, span))?,
        )),
        (
            Value::Money {
                amount: left,
                currency,
            },
            Value::Money {
                amount: right,
                currency: right_code,
            },
        ) if currency == right_code && matches!(op, Op::Add | Op::Subtract) => Ok(Value::Money {
            amount: numbers(left, right).map_err(|err| math_error(err, op, span))?,
            currency,
        }),
        (Value::Money { amount, currency }, Value::Number(right))
            if matches!(op, Op::Multiply | Op::Divide) =>
        {
            Ok(Value::Money {
                amount: numbers(amount, right).map_err(|err| math_error(err, op, span))?,
                currency,
            })
        }
        (Value::Number(left), Value::Money { amount, currency }) if matches!(op, Op::Multiply) => {
            Ok(Value::Money {
                amount: numbers(left, amount).map_err(|err| math_error(err, op, span))?,
                currency,
            })
        }
        _ => Err(internal(
            span,
            "a calculation on values that can't be combined",
        )),
    }
}

/// Whether `left op right` is true. `None` if the values can't be compared.
fn compare(op: Comparison, left: &Value, right: &Value) -> Option<bool> {
    match op {
        Comparison::Equal | Comparison::NotEqual => {
            if std::mem::discriminant(left) != std::mem::discriminant(right) {
                return None;
            }
            let equal = left == right;
            Some(if op == Comparison::Equal {
                equal
            } else {
                !equal
            })
        }
        _ => {
            let (left, right) = match (left, right) {
                (Value::Number(left), Value::Number(right)) => (left, right),
                (
                    Value::Money {
                        amount: left,
                        currency: left_code,
                    },
                    Value::Money {
                        amount: right,
                        currency: right_code,
                    },
                ) if left_code == right_code => (left, right),
                _ => return None,
            };
            Some(match op {
                Comparison::Less => left < right,
                Comparison::LessEqual => left <= right,
                Comparison::Greater => left > right,
                _ => left >= right,
            })
        }
    }
}

/// `None` when the check passes. Otherwise a message saying why it failed.
fn check_failure(op: Comparison, example: bool, left: &Value, right: &Value) -> Option<String> {
    let (word, a) = if example {
        ("example", "An")
    } else {
        ("check", "A")
    };
    match compare(op, left, right) {
        Some(true) => None,
        Some(false) => {
            let (l, r) = (left.written(), right.written());
            let why = match op {
                Comparison::Equal => format!(
                    "One side is {l} and the other is {r}.\n\
                     {a} {word} passes only when both sides are the same value, so change one of them to match."
                ),
                Comparison::NotEqual => format!(
                    "Both sides are {l}, but this {word} expects them to be different."
                ),
                Comparison::Less => format!("{l} is not less than {r}."),
                Comparison::LessEqual => format!("{l} is more than {r}."),
                Comparison::Greater => format!("{l} is not more than {r}."),
                Comparison::GreaterEqual => format!("{l} is less than {r}."),
            };
            Some(format!("This {word} failed.\n{why}"))
        }
        None => Some(format!(
            "This {word} compares {} ({}) to {} ({}).\n\
             Both sides have to be the same kind of value.\n\
             Quill won't turn one into the other, so make them both text or both numbers.",
            left.kind_name(),
            left.written(),
            right.kind_name(),
            right.written()
        )),
    }
}

/// The line that called the function running now.
fn call_line(chunk: &Chunk, frames: &[Frame]) -> Option<usize> {
    let caller = frames.get(frames.len().checked_sub(2)?)?;
    let spans = match caller.function {
        None => &chunk.spans,
        Some(fn_index) => &chunk.functions.get(fn_index)?.spans,
    };
    spans.get(caller.next.checked_sub(1)?).map(|span| span.line)
}

/// What a broken `require` or `ensure` says: the rule, the values it read, and whose
/// mistake it is.
fn broken_contract(
    contract: &ContractInfo,
    slots: &[Option<Value>],
    call_line: Option<usize>,
) -> String {
    let ContractInfo {
        function,
        ensure,
        condition,
        rule,
        names,
    } = contract;
    let values: Vec<String> = names
        .iter()
        .filter_map(|(name, slot)| {
            let value = slots.get(*slot)?.as_ref()?;
            Some(format!("{name} is {}", value.written()))
        })
        .collect();
    let found = match values.as_slice() {
        [] => "it was false".to_string(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    };
    if let Some((param, range)) = rule {
        let call = match call_line {
            Some(line) => format!("The call on line {line}"),
            None => "The call".to_string(),
        };
        return format!(
            "{function} takes {param} {range}, but {found}.\n\
             {call} gave {function} a value outside that range. \
             Check the value before calling {function}."
        );
    }
    if *ensure {
        format!(
            "{function} ensures {condition}, but {found}.\n\
             The mistake is inside {function}: it was about to give back a value \
             that breaks its own ensure."
        )
    } else {
        let call = match call_line {
            Some(line) => format!("The call on line {line}"),
            None => "The call".to_string(),
        };
        format!(
            "{function} requires {condition}, but {found}.\n\
             {call} gave {function} a value it doesn't accept. \
             Check the value before calling {function}."
        )
    }
}

fn math_error(err: MathError, op: Op, span: Span) -> Diagnostic {
    match err {
        MathError::DivideByZero => {
            let doing = if op == Op::Divide {
                "This divides by zero"
            } else {
                "This takes the remainder of dividing by zero"
            };
            Diagnostic::new(
                span,
                format!(
                    "{doing}, which has no answer.\n\
                     The number on the right was 0 when this line ran. \
                     Use when to handle zero before dividing."
                ),
            )
        }
        MathError::TooLarge => Diagnostic::new(
            span,
            "The result of this calculation is too big for a Quill number.\n\
             Numbers can have up to 38 digits in total, before and after the point.",
        ),
    }
}

/// Where `index` sits in `list`. Position 1 is the first item.
fn list_position(list: &Value, index: Decimal, span: Span) -> Result<usize, Diagnostic> {
    let Value::List(items) = list else {
        return Err(internal(span, "an item from something that isn't a list"));
    };
    let Some(position) = index.whole() else {
        return Err(Diagnostic::new(
            span,
            format!(
                "A list position has to be a whole number, but {index} isn't.\n\
                 Lists count from 1: the first item is at 1."
            ),
        ));
    };
    if position < 1 {
        return Err(Diagnostic::new(
            span,
            format!(
                "Lists count from 1, so {position} isn't a position in this list.\n\
                 The first item is at 1."
            ),
        ));
    }
    let Some(position) = usize::try_from(position).ok() else {
        return Err(Diagnostic::new(
            span,
            "This position is too big for a list.\n\
             Lists count from 1, and this one doesn't go that far."
                .to_string(),
        ));
    };
    if items.is_empty() {
        return Err(Diagnostic::new(
            span,
            "This list is empty, so it has no items to pick.\n\
             Add an item before asking for one.",
        ));
    }
    if position > items.len() {
        let last = items.len();
        return Err(Diagnostic::new(
            span,
            format!(
                "This list has {last} {}, so {position} is past the end.\n\
                 The last item is at {last}.",
                if last == 1 { "item" } else { "items" }
            ),
        ));
    }
    Ok(position - 1)
}

fn field_name(chunk: &Chunk, index: usize, span: Span) -> Result<String, Diagnostic> {
    match chunk.constants.get(index) {
        Some(Value::Text(name)) => Ok(name.clone()),
        _ => Err(internal(span, "a field name is missing")),
    }
}

/// The field of each record. A missing amount is left out. A found amount is unwrapped.
fn pluck(list: Value, field: &str, span: Span) -> Result<Value, Diagnostic> {
    if let Value::Record(_) = list {
        return record_field(&list, field, span);
    }
    let Value::List(items) = list else {
        return Err(internal(span, "a field from something that isn't a list"));
    };
    let mut values = Vec::new();
    for item in items {
        if let Some(value) = present(record_field(&item, field, span)?) {
            values.push(value);
        }
    }
    Ok(Value::List(values))
}

/// A record of the named fields, for each record in the list.
fn pick(list: Value, names: &[String], span: Span) -> Result<Value, Diagnostic> {
    if let Value::Record(_) = list {
        return Ok(Value::Record(picked_fields(&list, names, span)?));
    }
    let Value::List(items) = list else {
        return Err(internal(span, "fields from something that isn't a list"));
    };
    let mut records = Vec::new();
    for item in items {
        records.push(Value::Record(picked_fields(&item, names, span)?));
    }
    Ok(Value::List(records))
}

fn picked_fields(
    record: &Value,
    names: &[String],
    span: Span,
) -> Result<Vec<(String, Value)>, Diagnostic> {
    let mut fields = Vec::new();
    for name in names {
        fields.push((name.clone(), record_field(record, name, span)?));
    }
    Ok(fields)
}

/// Adds the field of each record. One record gives that one amount. A missing amount is left out.
fn sum_field(
    list: Value,
    field: &str,
    currency: Option<&str>,
    span: Span,
) -> Result<Value, Diagnostic> {
    let items = match list {
        Value::List(items) => items,
        Value::Record(_) => vec![list],
        _ => return Err(internal(span, "a sum of something that isn't a record")),
    };
    let mut total_number: Option<Decimal> = None;
    let mut total_money: Option<(Decimal, String)> = None;
    for item in items {
        let Some(value) = present(record_field(&item, field, span)?) else {
            continue;
        };
        match value {
            Value::Number(amount) => {
                total_number = Some(match total_number {
                    None => amount,
                    Some(so_far) => add_amounts(so_far, amount, span)?,
                });
            }
            Value::Money { amount, currency } => match total_money {
                None => total_money = Some((amount, currency)),
                Some((so_far, code)) if code == currency => {
                    total_money = Some((add_amounts(so_far, amount, span)?, code));
                }
                Some((_, code)) => {
                    return Err(Diagnostic::new(
                        span,
                        format!(
                            "This sum holds money in {code}, and this record holds money in {currency}.\n\
                             Add amounts in one currency: sum fee of roster."
                        ),
                    ));
                }
            },
            _ => return Err(internal(span, "a sum of something that isn't an amount")),
        }
    }
    if let Some((amount, code)) = total_money {
        return Ok(Value::Money {
            amount,
            currency: code,
        });
    }
    if let Some(amount) = total_number {
        return Ok(Value::Number(amount));
    }
    if let Some(code) = currency {
        return Ok(Value::Money {
            amount: Decimal::ZERO,
            currency: code.to_string(),
        });
    }
    Ok(Value::Number(Decimal::ZERO))
}

fn add_amounts(left: Decimal, right: Decimal, span: Span) -> Result<Decimal, Diagnostic> {
    left.checked_add(right).map_err(|_| {
        Diagnostic::new(
            span,
            "This sum is too big for a Quill number.\n\
             Numbers can have up to 38 digits in total, before and after the point.",
        )
    })
}

/// `found` opens into the value it holds. `missing` is left out.
fn present(value: Value) -> Option<Value> {
    match value {
        Value::Missing => None,
        Value::Found(inner) => Some(*inner),
        other => Some(other),
    }
}

/// The records named by `field`, each of which has `nested`, from a record or a list of records.
fn gather(source: Value, field: &str, nested: &str, span: Span) -> Result<Value, Diagnostic> {
    let lists = match &source {
        Value::Record(_) => vec![record_field(&source, field, span)?],
        Value::List(items) => {
            let mut lists = Vec::new();
            for item in items {
                lists.push(record_field(item, field, span)?);
            }
            lists
        }
        _ => {
            return Err(internal(
                span,
                "data from something that isn't a record or a list",
            ))
        }
    };
    let mut records = Vec::new();
    for list in lists {
        let Value::List(items) = list else {
            return Err(internal(span, "a list of records that wasn't a list"));
        };
        for item in items {
            record_field(&item, nested, span)?;
            records.push(item);
        }
    }
    Ok(Value::List(records))
}

fn record_field(record: &Value, name: &str, span: Span) -> Result<Value, Diagnostic> {
    let Value::Record(fields) = record else {
        return Err(internal(span, "a field of something that isn't a record"));
    };
    fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value.clone())
        .ok_or_else(|| {
            Diagnostic::new(
                span,
                format!(
                    "This record has no field called {name}.\n\
                     Its fields are {}.",
                    fields
                        .iter()
                        .map(|(field, _)| field.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
        })
}

fn pop(stack: &mut Vec<Value>, span: Span) -> Result<Value, Diagnostic> {
    stack
        .pop()
        .ok_or_else(|| internal(span, "the value stack is empty"))
}

fn pop_number(stack: &mut Vec<Value>, span: Span) -> Result<Decimal, Diagnostic> {
    match pop(stack, span)? {
        Value::Number(number) => Ok(number),
        _ => Err(internal(
            span,
            "a calculation got something that isn't a number",
        )),
    }
}

fn pop_bool(stack: &mut Vec<Value>, span: Span) -> Result<bool, Diagnostic> {
    match pop(stack, span)? {
        Value::Bool(value) => Ok(value),
        _ => Err(internal(span, "a condition wasn't true or false")),
    }
}

/// A mistake in Quill itself, reported instead of panicking.
fn internal(span: Span, what: &str) -> Diagnostic {
    Diagnostic::new(
        span,
        format!(
            "Quill made an internal mistake running this line ({what}).\n\
             This is a bug in Quill, not in your code. Please report it."
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(written: &str) -> Value {
        Value::Number(Decimal::parse(written).unwrap())
    }

    /// Compiles and runs Quill source, returning what it printed.
    fn run_source(source: &str) -> Result<String, Diagnostic> {
        let program = crate::parser::parse(&crate::lexer::lex(source).unwrap()).unwrap();
        let chunk = crate::compiler::compile(&program).unwrap();
        output(&chunk)
    }

    fn span() -> Span {
        Span {
            line: 1,
            column: 1,
            length: 1,
        }
    }

    fn chunk(code: Vec<Op>, constants: Vec<Value>, slot_count: usize) -> Chunk {
        Chunk {
            spans: vec![span(); code.len()],
            code,
            constants,
            records: Vec::new(),
            picks: Vec::new(),
            functions: Vec::new(),
            contracts: Vec::new(),
            slot_count,
        }
    }

    fn output(chunk: &Chunk) -> Result<String, Diagnostic> {
        let mut out = Vec::new();
        run(chunk, &mut out)?;
        Ok(String::from_utf8(out).unwrap())
    }

    #[test]
    fn says_constants() {
        let chunk = chunk(
            vec![Op::Constant(0), Op::Say, Op::Constant(1), Op::Say],
            vec![
                Value::Text("Hello".to_string()),
                Value::Number(Decimal::parse("0.10").unwrap()),
            ],
            0,
        );
        assert_eq!(output(&chunk).unwrap(), "Hello\n0.10\n");
    }

    #[test]
    fn stores_and_loads_values() {
        let chunk = chunk(
            vec![
                Op::Constant(0),
                Op::Store(0),
                Op::Load(0),
                Op::Say,
                Op::Load(0),
                Op::Say,
            ],
            vec![Value::Text("twice".to_string())],
            1,
        );
        assert_eq!(output(&chunk).unwrap(), "twice\ntwice\n");
    }

    #[test]
    fn broken_bytecode_is_an_error_not_a_panic() {
        let cases = vec![
            chunk(vec![Op::Say], vec![], 0),
            chunk(vec![Op::Constant(3)], vec![], 0),
            chunk(vec![Op::Load(0)], vec![], 1),
            chunk(
                vec![Op::Constant(0), Op::Store(5)],
                vec![Value::Number(Decimal::parse("1").unwrap())],
                0,
            ),
        ];
        for case in cases {
            let err = output(&case).unwrap_err();
            assert!(err.message.contains("internal mistake"), "{}", err.message);
        }
    }

    #[test]
    fn a_check_passes_quietly_when_both_sides_match() {
        let chunk = chunk(
            vec![
                Op::Constant(0),
                Op::Constant(1),
                Op::Check {
                    op: Comparison::Equal,
                    example: false,
                },
            ],
            vec![Value::Text("Hi".to_string()), Value::Text("Hi".to_string())],
            0,
        );
        assert_eq!(output(&chunk).unwrap(), "");
    }

    #[test]
    fn a_failed_check_names_both_sides() {
        let chunk = chunk(
            vec![
                Op::Constant(0),
                Op::Constant(1),
                Op::Check {
                    op: Comparison::Equal,
                    example: false,
                },
            ],
            vec![
                Value::Text("Hi".to_string()),
                Value::Text("Bye".to_string()),
            ],
            0,
        );
        let err = output(&chunk).unwrap_err();
        assert!(err.message.contains("This check failed"));
        assert!(err.message.contains("\"Hi\""));
        assert!(err.message.contains("\"Bye\""));
    }

    #[test]
    fn an_example_wont_compare_text_with_a_number() {
        let chunk = chunk(
            vec![
                Op::Constant(0),
                Op::Constant(1),
                Op::Check {
                    op: Comparison::Equal,
                    example: true,
                },
            ],
            vec![
                Value::Text("Hi".to_string()),
                Value::Number(Decimal::parse("1").unwrap()),
            ],
            0,
        );
        let err = output(&chunk).unwrap_err();
        assert!(err.message.contains("This example compares text"));
        assert!(err.message.contains("a number"));
    }

    #[test]
    fn closed_output_is_reported() {
        struct Closed;
        impl Write for Closed {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "pipe closed",
                ))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let chunk = chunk(
            vec![Op::Constant(0), Op::Say],
            vec![Value::Text("x".into())],
            0,
        );
        let err = run(&chunk, &mut Closed).unwrap_err();
        assert!(err.message.contains("couldn't print"));
    }

    #[test]
    fn arithmetic_is_exact() {
        assert_eq!(run_source("say 0.1 + 0.2").unwrap(), "0.3\n");
        assert_eq!(run_source("say 100 * 1.05").unwrap(), "105.00\n");
        assert_eq!(
            run_source("say 7 % 3\nsay 10 / 4\nsay -2 - 3").unwrap(),
            "1\n2.5\n-5\n"
        );
        assert_eq!(
            run_source("say 1 + 2 * 3\nsay (1 + 2) * 3").unwrap(),
            "7\n9\n"
        );
    }

    #[test]
    fn comparisons_and_logic() {
        assert_eq!(
            run_source("say 2 < 3\nsay 0.10 == 0.1\nsay \"a\" != \"a\"\nsay not (1 > 2) and true")
                .unwrap(),
            "true\ntrue\nfalse\ntrue\n"
        );
    }

    #[test]
    fn and_stops_before_dividing_by_zero() {
        assert_eq!(
            run_source("var n = 0\nsay n != 0 and 10 / n > 1\nsay n == 0 or 10 / n > 1").unwrap(),
            "false\ntrue\n"
        );
    }

    #[test]
    fn values_inside_text() {
        assert_eq!(
            run_source("name = \"Amina\"\nn = 2\nsay \"Hi {name}, {n * 3} {n > 1}\"").unwrap(),
            "Hi Amina, 6 true\n"
        );
    }

    #[test]
    fn var_changes_with_set() {
        assert_eq!(
            run_source(
                "var visits = 0\nset visits = visits + 1\nset visits = visits + 1\nsay visits"
            )
            .unwrap(),
            "2\n"
        );
    }

    #[test]
    fn when_runs_the_first_true_case() {
        let program = "n = 15\nwhen\n  n % 15 == 0 -> say \"FizzBuzz\"\n  n % 3 == 0 -> say \"Fizz\"\n  else -> say n";
        assert_eq!(run_source(program).unwrap(), "FizzBuzz\n");
        let program = "n = 7\nwhen\n  n % 3 == 0 -> say \"Fizz\"\n  else -> say n\nsay \"done\"";
        assert_eq!(run_source(program).unwrap(), "7\ndone\n");
        let program = "n = 7\nwhen\n  n > 10 -> say \"big\"\nsay \"done\"";
        assert_eq!(run_source(program).unwrap(), "done\n");
    }

    #[test]
    fn nested_when() {
        let program = "a = true\nb = false\nwhen\n  a -> when\n    b -> say 1\n    else -> say 2\n  else -> say 3";
        assert_eq!(run_source(program).unwrap(), "2\n");
    }

    #[test]
    fn dividing_by_zero_while_running_is_explained() {
        let err = run_source("var n = 0\nsay 10 / n").unwrap_err();
        assert_eq!((err.line, err.column, err.length), (2, 5, 6));
        assert!(err
            .message
            .contains("This divides by zero, which has no answer"));
        let err = run_source("var n = 0\nsay 10 % n").unwrap_err();
        assert!(err.message.contains("remainder"));
    }

    #[test]
    fn numbers_that_grow_too_big_are_explained() {
        let big = "9".repeat(37);
        let err = run_source(&format!("say {big} * {big}")).unwrap_err();
        assert!(err.message.contains("too big for a Quill number"));
    }

    #[test]
    fn checks_with_other_comparisons() {
        assert_eq!(
            run_source("check 1 < 2\ncheck 2 >= 2\ncheck 1 != 2").unwrap(),
            ""
        );
        let err = run_source("check 3 < 2").unwrap_err();
        assert!(err
            .message
            .contains("This check failed.\n3 is not less than 2."));
        let err = run_source("check 1 != 1").unwrap_err();
        assert!(err.message.contains("Both sides are 1"));
    }

    #[test]
    fn broken_jumps_and_kinds_are_errors_not_panics() {
        let cases = vec![
            chunk(vec![Op::JumpIfFalse(0)], vec![], 0),
            chunk(
                vec![Op::Constant(0), Op::JumpIfFalse(0)],
                vec![number("1")],
                0,
            ),
            chunk(
                vec![Op::Constant(0), Op::Negate],
                vec![Value::Bool(true)],
                0,
            ),
            chunk(vec![Op::Join(2)], vec![], 0),
            chunk(
                vec![
                    Op::Constant(0),
                    Op::Constant(1),
                    Op::Compare(Comparison::Less),
                ],
                vec![number("1"), Value::Text("x".into())],
                0,
            ),
        ];
        for case in cases {
            let err = output(&case).unwrap_err();
            assert!(err.message.contains("internal mistake"), "{}", err.message);
        }
        // A jump past the end just ends the program.
        assert_eq!(output(&chunk(vec![Op::Jump(99)], vec![], 0)).unwrap(), "");
    }

    #[test]
    fn a_for_loop_counts_through_both_ends() {
        assert_eq!(run_source("for i in 1..3\n  say i").unwrap(), "1\n2\n3\n");
        assert_eq!(
            run_source("var total = 0\nfor i in 1..3\n  set total = total + i\nsay total").unwrap(),
            "6\n"
        );
        assert_eq!(
            run_source("i = 10\nfor i in 1..2\n  say i\nsay i").unwrap(),
            "1\n2\n10\n"
        );
        assert_eq!(
            run_source("for i in 3..1\n  say i\nsay \"done\"").unwrap(),
            "done\n"
        );
        assert_eq!(run_source("for i in -1..1\n  say i").unwrap(), "-1\n0\n1\n");
    }

    #[test]
    fn a_list_keeps_values_in_order_and_counts_from_one() {
        assert_eq!(
            run_source("hobbies = list \"coding\", \"reading\", \"traveling\"\nsay hobbies\nsay hobbies at 1\nsay hobbies at 3").unwrap(),
            "coding, reading, traveling\ncoding\ntraveling\n"
        );
        assert_eq!(
            run_source("hobbies = list \"coding\", \"reading\"\nfor hobby in hobbies\n  say hobby")
                .unwrap(),
            "coding\nreading\n"
        );
        let err = run_source("hobbies = list \"coding\"\nsay hobbies at 2").unwrap_err();
        assert!(err.message.contains("past the end"));
        let err = run_source("empty = list of text\nsay empty at 1").unwrap_err();
        assert!(err.message.contains("empty"));
    }

    #[test]
    fn a_record_keeps_named_fields() {
        let source = "\
address = record
  street = \"123 Main St\"
  city = \"Anytown\"
say address.city
say address";
        assert_eq!(
            run_source(source).unwrap(),
            "Anytown\nstreet: 123 Main St\ncity: Anytown\n"
        );
        assert_eq!(
            run_source("var address = record city = \"Anytown\"\nset address.city = \"Springfield\"\nsay address.city").unwrap(),
            "Springfield\n"
        );
        assert_eq!(
            run_source("var hobbies = list \"coding\", \"reading\"\nset hobbies at 1 = \"writing\"\nsay hobbies").unwrap(),
            "writing, reading\n"
        );
    }

    #[test]
    fn a_function_can_be_called_and_can_call_itself() {
        assert_eq!(
            run_source("fn double(n: number) -> number\n  return n * 2\nsay double(21)").unwrap(),
            "42\n"
        );
        assert_eq!(
            run_source("fn greet(name: text)\n  say \"Hello, {name}\"\ngreet(\"Ngwang\")").unwrap(),
            "Hello, Ngwang\n"
        );
        assert_eq!(
            run_source(
                "fn fact(n: number) -> number\n  when\n    n <= 1 -> return 1\n    else -> return n * fact(n - 1)\nsay fact(5)"
            )
            .unwrap(),
            "120\n"
        );
    }

    #[test]
    fn money_maybe_and_failures() {
        assert_eq!(run_source("say 100 USD + 50 USD").unwrap(), "150 USD\n");
        assert_eq!(
            run_source(
                "fn find_price(name: text) -> maybe number\n  when\n    name == \"bread\" -> return found 2.50\n    else -> return missing\nwhen find_price(\"cake\")\n  found price -> say price\n  missing -> say \"none\""
            )
            .unwrap(),
            "none\n"
        );
        assert_eq!(
            run_source(
                "fn charge(amount: USD) -> text\n  fails Declined\n  when\n    amount <= 0 USD -> fail Declined\n    else -> return \"paid\"\nvar receipt = charge(0 USD)\n  on Declined -> say \"Card declined\"\nsay \"after\""
            )
            .unwrap(),
            "Card declined\n"
        );
    }

    #[test]
    fn get_and_sum_read_a_list_without_a_loop() {
        let roster = "\
sisters = record name = \"Nchang Sisters\", fee = 80000 USD
brass = record name = \"Bamenda Brass\", fee = 150000 USD
roster = list sisters, brass
say sum fee of roster
say get fee of roster
";
        assert_eq!(
            run_source(roster).unwrap(),
            "230000 USD\n80000 USD, 150000 USD\n"
        );
        let people = "\
sisters = record name = \"Nchang Sisters\", age = 24, fee = 80000 USD
brass = record name = \"Bamenda Brass\", age = 31, fee = 150000 USD
roster = list sisters, brass
people = get name, age, fee of roster
say (people at 1).name
say (people at 1).age
say (people at 2).fee
";
        assert_eq!(
            run_source(people).unwrap(),
            "Nchang Sisters\n24\n150000 USD\n"
        );
        let school = "\
school = record name = \"Harvard\", fees = 10000
data = get name, fees of school
say data.name
say data.fees
";
        assert_eq!(run_source(school).unwrap(), "Harvard\n10000\n");
        assert_eq!(
            run_source("school = record name = \"Harvard\", fees = 10000\nsay sum fees of school")
                .unwrap(),
            "10000\n"
        );
    }

    #[test]
    fn sum_leaves_out_a_missing_amount() {
        let source = "\
fn fee_for(name: text) -> maybe USD
  when
    name == \"Quiet Hour\" -> return missing
    else -> return found 40000 USD
quiet = record name = \"Quiet Hour\", fee = fee_for(\"Quiet Hour\")
drums = record name = \"Market Drums\", fee = fee_for(\"Market Drums\")
roster = list quiet, drums
say sum fee of roster
";
        assert_eq!(run_source(source).unwrap(), "40000 USD\n");
    }

    #[test]
    fn get_brings_embedded_records_from_one_place() {
        let source = "\
up = record place = \"Up Station\", acts = list \"Bamenda Brass\", \"Hilltop Duo\"
old = record place = \"Old Town\", acts = list \"Market Drums\"
roster = record bookings = list up, old
tonight = get bookings with acts in roster
say (tonight at 1).place
saturday = record day = \"Saturday\", bookings = list up
sunday = record day = \"Sunday\", bookings = list old
nights = list saturday, sunday
all = get bookings with acts in nights
say (all at 2).place
";
        assert_eq!(run_source(source).unwrap(), "Up Station\nOld Town\n");
    }

    #[test]
    fn an_empty_sum_of_money_keeps_the_currency() {
        let source = "\
fn fee_for(name: text) -> maybe USD
  return missing
quiet = record name = \"Quiet Hour\", fee = fee_for(\"Quiet Hour\")
roster = list quiet
say sum fee of roster
";
        assert_eq!(run_source(source).unwrap(), "0 USD\n");
    }

    const WITHDRAW: &str = "\
fn withdraw(balance: USD, amount: USD) -> USD
  require amount > 0 USD
  ensure result >= 0 USD
  return balance - amount
";

    #[test]
    fn contracts_that_hold_change_nothing() {
        let source = format!("{WITHDRAW}say withdraw(100 USD, 30 USD)\n");
        assert_eq!(run_source(&source).unwrap(), "70 USD\n");
    }

    #[test]
    fn a_broken_require_names_the_value_and_the_call() {
        let source = format!("{WITHDRAW}say \"before\"\nsay withdraw(100 USD, -5 USD)\n");
        let problem = run_source(&source).unwrap_err();
        assert_eq!(problem.line, 2);
        assert_eq!(
            problem.message,
            "withdraw requires amount > 0 USD, but amount is -5 USD.\n\
             The call on line 6 gave withdraw a value it doesn't accept. \
             Check the value before calling withdraw."
        );
    }

    #[test]
    fn a_value_outside_a_range_stops_the_call() {
        let source = "fn book(guests: number where 1..500) -> text\n  return \"Booked {guests}\"\nsay book(1)\nsay book(500)\nn = 501\nsay book(n)\n";
        let problem = run_source(source).unwrap_err();
        assert_eq!(problem.line, 1);
        assert_eq!(
            problem.message,
            "book takes guests from 1 through 500, but guests is 501.\n\
             The call on line 6 gave book a value outside that range. \
             Check the value before calling book."
        );
        let money = "fn pay(amount: USD where 1 USD..100 USD) -> USD\n  return amount\nbig = 101 USD\nsay pay(big)\n";
        let problem = run_source(money).unwrap_err();
        assert!(problem
            .message
            .starts_with("pay takes amount from 1 USD through 100 USD"));
    }

    #[test]
    fn where_keeps_the_records_that_match() {
        let roster = "a = record name = \"Amina\", score = 80, fee = 500 USD, paid = true\no = record name = \"Omar\", score = 64, fee = 300 USD, paid = false\nj = record name = \"John\", score = 45, fee = 200 USD, paid = false\nstudents = list a, o, j\n";
        let source = format!(
            "{roster}say get name of students where score >= 50\n\
             say sum fee of students where paid == false\n\
             say get name of students where score >= 50 where paid == false\n\
             say students where score > 100\n\
             score = 1000\n\
             say get name of students where score >= 70\n\
             say score\n\
             for s in students where paid\n  say s.name\n"
        );
        assert_eq!(
            run_source(&source).unwrap(),
            "Amina, Omar\n500 USD\nOmar\nan empty list\nAmina\n1000\nAmina\n"
        );
    }

    #[test]
    fn a_broken_ensure_blames_the_function() {
        let source = format!("{WITHDRAW}say withdraw(10 USD, 30 USD)\n");
        let problem = run_source(&source).unwrap_err();
        assert_eq!(problem.line, 3);
        assert_eq!(
            problem.message,
            "withdraw ensures result >= 0 USD, but result is -20 USD.\n\
             The mistake is inside withdraw: it was about to give back a value \
             that breaks its own ensure."
        );
    }

    #[test]
    fn ensure_is_checked_at_every_return() {
        let source = "\
fn grade(score: number) -> text
  ensure result != \"\"
  when
    score >= 50 -> return \"pass\"
  return \"\"
say grade(80)
say grade(10)
";
        let problem = run_source(source).unwrap_err();
        assert_eq!(problem.line, 2);
        assert!(
            problem.message.contains("but result is \"\""),
            "{}",
            problem.message
        );
    }

    #[test]
    fn a_broken_contract_lists_every_name_it_read() {
        let source = "\
fn split(total: number, people: number) -> number
  require people > 0 and total >= people
  return total / people
say split(3, 5)
";
        let problem = run_source(source).unwrap_err();
        assert!(
            problem.message.starts_with(
                "split requires people > 0 and total >= people, but people is 5 and total is 3.\n"
            ),
            "{}",
            problem.message
        );
    }

    #[test]
    fn a_value_called_result_in_the_body_is_left_alone() {
        let source = "\
fn double(n: number) -> number
  ensure result == n * 2
  result = n + n
  return result
say double(4)
";
        assert_eq!(run_source(source).unwrap(), "8\n");
    }

    const CHARGE: &str = "fn charge(amount: USD) -> text
  fails Declined, NetworkDown
  when
    amount <= 0 USD -> fail Declined
    amount > 1000 USD -> fail NetworkDown
  return \"paid\"
";

    #[test]
    fn on_with_a_value_carries_on() {
        let source = format!(
            "{CHARGE}receipt = charge(0 USD)\n  on Declined -> \"declined\"\n  on NetworkDown -> \"offline\"\nsay receipt\nsay \"after\"\n"
        );
        assert_eq!(run_source(&source).unwrap(), "declined\nafter\n");
    }

    #[test]
    fn on_with_a_value_is_skipped_when_the_call_succeeds() {
        let source = format!(
            "{CHARGE}var receipt = \"none\"\nset receipt = charge(5 USD)\n  on Declined -> \"declined\"\n  on NetworkDown -> \"offline\"\nsay receipt\n"
        );
        assert_eq!(run_source(&source).unwrap(), "paid\n");
    }

    #[test]
    fn a_value_and_a_stop_can_share_a_call() {
        let source = format!(
            "{CHARGE}receipt = charge(2000 USD)\n  on Declined -> \"declined\"\n  on NetworkDown -> say \"Try again\"\nsay \"never\"\n"
        );
        assert_eq!(run_source(&source).unwrap(), "Try again\n");
    }

    #[test]
    fn on_with_a_value_inside_a_function() {
        let source = format!(
            "{CHARGE}fn safe(amount: USD) -> text\n  receipt = charge(amount)\n    on Declined -> \"no\"\n    on NetworkDown -> \"later\"\n  return receipt\nsay safe(0 USD)\nsay safe(2000 USD)\nsay safe(5 USD)\n"
        );
        assert_eq!(run_source(&source).unwrap(), "no\nlater\npaid\n");
    }

    #[test]
    fn when_gives_the_value_of_the_first_true_case() {
        let source = "\
fn grade(score: number) -> text
  return when
    score >= 70 -> \"A\"
    score >= 50 -> \"Pass\"
    else -> \"Fail\"
say grade(90)
say grade(64)
say grade(10)
fee = when
  grade(90) == \"A\" -> 100 USD
  else -> 0 USD
say fee
say when
  fee > 50 USD -> \"high\"
  else -> \"low\"
";
        assert_eq!(
            run_source(source).unwrap(),
            "A\nPass\nFail\n100 USD\nhigh\n"
        );
    }

    #[test]
    fn when_can_give_a_maybe() {
        let source = "\
fn positive(n: number) -> maybe number
  return when
    n > 0 -> found n
    else -> missing
when positive(-2)
  found n -> say n
  missing -> say \"none\"
";
        assert_eq!(run_source(source).unwrap(), "none\n");
    }

    #[test]
    fn an_example_above_a_function_tests_it() {
        let source = "\
--- Adds 5% VAT to a price.
--- example: with_vat(100 USD) == 105 USD
fn with_vat(price: USD) -> USD
  return price * 1.05
say with_vat(20 USD)
";
        assert_eq!(run_source(source).unwrap(), "21.00 USD\n");
        let wrong = source.replace("== 105 USD", "== 106 USD");
        let problem = run_source(&wrong).unwrap_err();
        assert_eq!(problem.line, 2);
        assert!(problem.message.contains("An example passes only when"));
    }
}
