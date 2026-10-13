//! The virtual machine: runs compiled bytecode.

use std::io::Write;

use crate::compiler::{Chunk, Op};
use crate::diagnostic::{Diagnostic, Span};
use crate::value::Value;

/// Runs `chunk`, writing everything `say` prints to `out`.
pub fn run(chunk: &Chunk, out: &mut impl Write) -> Result<(), Diagnostic> {
    let mut stack: Vec<Value> = Vec::new();
    let mut slots: Vec<Option<Value>> = vec![None; chunk.slot_count];

    for (index, op) in chunk.code.iter().enumerate() {
        let span = chunk.spans.get(index).copied().unwrap_or(Span {
            line: 1,
            column: 1,
            length: 0,
        });
        match *op {
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
        }
    }
    Ok(())
}

fn pop(stack: &mut Vec<Value>, span: Span) -> Result<Value, Diagnostic> {
    stack
        .pop()
        .ok_or_else(|| internal(span, "the value stack is empty"))
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
                Value::Number("0.10".to_string()),
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
                vec![Value::Number("1".into())],
                0,
            ),
        ];
        for case in cases {
            let err = output(&case).unwrap_err();
            assert!(err.message.contains("internal mistake"), "{}", err.message);
        }
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
}
