//! The virtual machine: runs compiled bytecode.

use std::io::Write;

use crate::compiler::{Chunk, Op};
use crate::decimal::{Decimal, MathError};
use crate::diagnostic::{Diagnostic, Span};
use crate::parser::Comparison;
use crate::value::Value;

/// Runs `chunk`, writing everything `say` prints to `out`.
pub fn run(chunk: &Chunk, out: &mut impl Write) -> Result<(), Diagnostic> {
    let mut stack: Vec<Value> = Vec::new();
    let mut slots: Vec<Option<Value>> = vec![None; chunk.slot_count];
    let mut next = 0;

    while let Some(&op) = chunk.code.get(next) {
        let span = chunk.spans.get(next).copied().unwrap_or(Span {
            line: 1,
            column: 1,
            length: 0,
        });
        next += 1;
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
                let target = slots
                    .get_mut(slot)
                    .ok_or_else(|| internal(span, "a value slot is missing"))?;
                *target = Some(value);
            }
            Op::Load(slot) => {
                let value = slots
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
            }
            Op::Add | Op::Subtract | Op::Multiply | Op::Divide | Op::Remainder => {
                let right = pop_number(&mut stack, span)?;
                let left = pop_number(&mut stack, span)?;
                let result = match op {
                    Op::Add => left.checked_add(right),
                    Op::Subtract => left.checked_sub(right),
                    Op::Multiply => left.checked_mul(right),
                    Op::Divide => left.checked_div(right),
                    _ => left.checked_rem(right),
                };
                stack.push(Value::Number(
                    result.map_err(|err| math_error(err, op, span))?,
                ));
            }
            Op::Negate => {
                let number = pop_number(&mut stack, span)?;
                let result = number
                    .checked_neg()
                    .map_err(|err| math_error(err, op, span))?;
                stack.push(Value::Number(result));
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
        }
    }
    Ok(())
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
            let (Value::Number(left), Value::Number(right)) = (left, right) else {
                return None;
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
    let word = if example { "example" } else { "check" };
    match compare(op, left, right) {
        Some(true) => None,
        Some(false) => {
            let (l, r) = (left.written(), right.written());
            let why = match op {
                Comparison::Equal => format!(
                    "One side is {l} and the other is {r}.\n\
                     A {word} passes only when both sides are the same value, so change one of them to match."
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
}
