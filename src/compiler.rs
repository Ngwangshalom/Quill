//! The bytecode compiler: turns checked statements into instructions for the VM.

use std::collections::HashMap;

use crate::decimal::Decimal;
use crate::diagnostic::{Diagnostic, Span};
use crate::parser::{Arithmetic, Comparison, Expr, ExprKind, Logic, Part, Program, Statement};
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    /// Push `constants[index]` onto the stack.
    Constant(usize),
    /// Pop the top of the stack into a value slot.
    Store(usize),
    /// Push a copy of a value slot.
    Load(usize),
    /// Pop the top of the stack and print it.
    Say,
    /// Pop two values and fail unless `left op right` is true.
    /// `example` is set when the check came from a doc example.
    Check {
        op: Comparison,
        example: bool,
    },
    /// Pop two numbers and push the result.
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    /// Pop a number and push it with the opposite sign.
    Negate,
    /// Pop true/false and push the opposite.
    Not,
    /// Pop two values and push whether `left op right` is true.
    Compare(Comparison),
    /// Pop this many values and push them joined into one text.
    Join(usize),
    /// Pop true/false, and jump to the instruction at this index if it was false.
    JumpIfFalse(usize),
    /// Continue at the instruction at this index.
    Jump(usize),
}

/// A compiled program.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Chunk {
    pub code: Vec<Op>,
    /// Where in the source each instruction came from, for runtime errors.
    pub spans: Vec<Span>,
    pub constants: Vec<Value>,
    /// How many named values the program creates.
    pub slot_count: usize,
}

pub fn compile(program: &Program) -> Result<Chunk, Diagnostic> {
    let mut compiler = Compiler {
        chunk: Chunk::default(),
        slots: HashMap::new(),
    };
    for statement in &program.statements {
        compiler.statement(statement)?;
    }
    Ok(compiler.chunk)
}

struct Compiler {
    chunk: Chunk,
    slots: HashMap<String, usize>,
}

impl Compiler {
    fn statement(&mut self, statement: &Statement) -> Result<(), Diagnostic> {
        match statement {
            Statement::Say { value, span } => {
                self.expression(value)?;
                self.emit(Op::Say, *span);
            }
            Statement::Let {
                name,
                name_span,
                value,
                ..
            } => {
                self.expression(value)?;
                let slot = self.chunk.slot_count;
                self.chunk.slot_count += 1;
                self.slots.insert(name.clone(), slot);
                self.emit(Op::Store(slot), *name_span);
            }
            Statement::Set {
                name,
                name_span,
                value,
                ..
            } => {
                self.expression(value)?;
                let slot = self.slot(name, *name_span)?;
                self.emit(Op::Store(slot), *name_span);
            }
            Statement::Check {
                left,
                op,
                right,
                span,
                example,
            } => {
                self.expression(left)?;
                self.expression(right)?;
                self.emit(
                    Op::Check {
                        op: *op,
                        example: *example,
                    },
                    *span,
                );
            }
            Statement::When { arms, .. } => {
                // Each case: test the condition, skip to the next case if false,
                // otherwise run the body and jump past the rest of the when.
                let mut to_end = Vec::new();
                for arm in arms {
                    match &arm.condition {
                        Some(condition) => {
                            self.expression(condition)?;
                            let skip = self.emit(Op::JumpIfFalse(0), arm.span);
                            self.statement(&arm.body)?;
                            to_end.push(self.emit(Op::Jump(0), arm.span));
                            self.patch(skip);
                        }
                        None => self.statement(&arm.body)?,
                    }
                }
                for jump in to_end {
                    self.patch(jump);
                }
            }
        }
        Ok(())
    }

    fn expression(&mut self, expr: &Expr) -> Result<(), Diagnostic> {
        let span = expr.span;
        match &expr.kind {
            ExprKind::Text(text) => self.constant(Value::Text(text.clone()), span),
            ExprKind::Number(number) => match Decimal::parse(number) {
                Some(number) => self.constant(Value::Number(number), span),
                None => return Err(too_many_digits(number, span)),
            },
            ExprKind::Bool(value) => self.constant(Value::Bool(*value), span),
            ExprKind::Name(name) => {
                let slot = self.slot(name, span)?;
                self.emit(Op::Load(slot), span);
            }
            ExprKind::Template(parts) => {
                for part in parts {
                    match part {
                        Part::Text(text) => self.constant(Value::Text(text.clone()), span),
                        Part::Value(value) => self.expression(value)?,
                    }
                }
                self.emit(Op::Join(parts.len()), span);
            }
            ExprKind::Group(inner) => self.expression(inner)?,
            ExprKind::Negate(inner) => {
                self.expression(inner)?;
                self.emit(Op::Negate, span);
            }
            ExprKind::Not(inner) => {
                self.expression(inner)?;
                self.emit(Op::Not, span);
            }
            ExprKind::Arithmetic {
                op, left, right, ..
            } => {
                self.expression(left)?;
                self.expression(right)?;
                let op = match op {
                    Arithmetic::Add => Op::Add,
                    Arithmetic::Subtract => Op::Subtract,
                    Arithmetic::Multiply => Op::Multiply,
                    Arithmetic::Divide => Op::Divide,
                    Arithmetic::Remainder => Op::Remainder,
                };
                self.emit(op, span);
            }
            ExprKind::Compare {
                op, left, right, ..
            } => {
                self.expression(left)?;
                self.expression(right)?;
                self.emit(Op::Compare(*op), span);
            }
            // `and` and `or` stop early, so `n != 0 and 10 / n > 1` never divides by zero.
            ExprKind::Logic {
                op: Logic::And,
                left,
                right,
                ..
            } => {
                self.expression(left)?;
                let left_false = self.emit(Op::JumpIfFalse(0), span);
                self.expression(right)?;
                let done = self.emit(Op::Jump(0), span);
                self.patch(left_false);
                self.constant(Value::Bool(false), span);
                self.patch(done);
            }
            ExprKind::Logic {
                op: Logic::Or,
                left,
                right,
                ..
            } => {
                self.expression(left)?;
                let left_false = self.emit(Op::JumpIfFalse(0), span);
                self.constant(Value::Bool(true), span);
                let done = self.emit(Op::Jump(0), span);
                self.patch(left_false);
                self.expression(right)?;
                self.patch(done);
            }
        }
        Ok(())
    }

    /// The slot of an existing value.
    fn slot(&self, name: &str, span: Span) -> Result<usize, Diagnostic> {
        // The checker reports unknown names first; this keeps the compiler from
        // panicking if it is ever called on unchecked code.
        self.slots.get(name).copied().ok_or_else(|| {
            Diagnostic::new(
                span,
                format!(
                    "There's no value called {name}.\n\
                     Values must be created with {name} = ... before they can be used."
                ),
            )
        })
    }

    fn constant(&mut self, value: Value, span: Span) {
        self.chunk.constants.push(value);
        let index = self.chunk.constants.len() - 1;
        self.emit(Op::Constant(index), span);
    }

    /// Adds an instruction and returns its index.
    fn emit(&mut self, op: Op, span: Span) -> usize {
        self.chunk.code.push(op);
        self.chunk.spans.push(span);
        self.chunk.code.len() - 1
    }

    /// Points the jump at `index` to the next instruction to be emitted.
    fn patch(&mut self, index: usize) {
        let target = self.chunk.code.len();
        if let Some(Op::Jump(to) | Op::JumpIfFalse(to)) = self.chunk.code.get_mut(index) {
            *to = target;
        }
    }
}

/// A number literal that doesn't fit in a Quill number.
pub fn too_many_digits(number: &str, span: Span) -> Diagnostic {
    Diagnostic::new(
        span,
        format!(
            "{number} has more digits than a Quill number can hold.\n\
             Numbers can have up to 38 digits in total, before and after the point."
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;

    fn compile_source(source: &str) -> Result<Chunk, Diagnostic> {
        compile(&parse(&lex(source).unwrap()).unwrap())
    }

    fn number(written: &str) -> Value {
        Value::Number(Decimal::parse(written).unwrap())
    }

    #[test]
    fn compiles_values_and_say() {
        let chunk = compile_source("greeting = \"Hi\"\nsay greeting\nsay 7").unwrap();
        assert_eq!(
            chunk.code,
            vec![
                Op::Constant(0),
                Op::Store(0),
                Op::Load(0),
                Op::Say,
                Op::Constant(1),
                Op::Say,
            ]
        );
        assert_eq!(
            chunk.constants,
            vec![Value::Text("Hi".to_string()), number("7")]
        );
        assert_eq!(chunk.slot_count, 1);
        assert_eq!(chunk.spans.len(), chunk.code.len());
    }

    #[test]
    fn each_value_gets_its_own_slot() {
        let chunk = compile_source("a = 1\nb = a\nsay b").unwrap();
        assert_eq!(
            chunk.code,
            vec![
                Op::Constant(0),
                Op::Store(0),
                Op::Load(0),
                Op::Store(1),
                Op::Load(1),
                Op::Say,
            ]
        );
        assert_eq!(chunk.slot_count, 2);
    }

    #[test]
    fn set_stores_into_the_existing_slot() {
        let chunk = compile_source("var n = 1\nset n = n + 1").unwrap();
        assert_eq!(
            chunk.code,
            vec![
                Op::Constant(0),
                Op::Store(0),
                Op::Load(0),
                Op::Constant(1),
                Op::Add,
                Op::Store(0),
            ]
        );
        assert_eq!(chunk.slot_count, 1);
    }

    #[test]
    fn compiles_a_check_and_an_example() {
        let chunk = compile_source("check \"Hi\" == \"Hi\"\n--- example: 1 < 2").unwrap();
        assert_eq!(
            chunk.code,
            vec![
                Op::Constant(0),
                Op::Constant(1),
                Op::Check {
                    op: Comparison::Equal,
                    example: false
                },
                Op::Constant(2),
                Op::Constant(3),
                Op::Check {
                    op: Comparison::Less,
                    example: true
                },
            ]
        );
    }

    #[test]
    fn calculations_follow_the_order_of_operations() {
        let chunk = compile_source("say 1 + 2 * -3").unwrap();
        assert_eq!(
            chunk.code,
            vec![
                Op::Constant(0),
                Op::Constant(1),
                Op::Constant(2),
                Op::Negate,
                Op::Multiply,
                Op::Add,
                Op::Say,
            ]
        );
    }

    #[test]
    fn values_inside_text_are_joined() {
        let chunk = compile_source("n = 1\nsay \"n is {n}!\"").unwrap();
        assert_eq!(
            &chunk.code[2..],
            &[
                Op::Constant(1),
                Op::Load(0),
                Op::Constant(2),
                Op::Join(3),
                Op::Say,
            ]
        );
    }

    #[test]
    fn when_jumps_past_the_other_cases() {
        let chunk = compile_source("when\n  true -> say 1\n  else -> say 2").unwrap();
        assert_eq!(
            chunk.code,
            vec![
                Op::Constant(0),
                Op::JumpIfFalse(5),
                Op::Constant(1),
                Op::Say,
                Op::Jump(7),
                Op::Constant(2),
                Op::Say,
            ]
        );
    }

    #[test]
    fn and_and_or_stop_early() {
        let chunk = compile_source("say false and true").unwrap();
        assert_eq!(
            chunk.code,
            vec![
                Op::Constant(0),
                Op::JumpIfFalse(4),
                Op::Constant(1),
                Op::Jump(5),
                Op::Constant(2),
                Op::Say,
            ]
        );
        let chunk = compile_source("say true or false").unwrap();
        assert_eq!(
            chunk.code,
            vec![
                Op::Constant(0),
                Op::JumpIfFalse(4),
                Op::Constant(1),
                Op::Jump(5),
                Op::Constant(2),
                Op::Say,
            ]
        );
    }

    #[test]
    fn unchecked_unknown_name_is_an_error_not_a_panic() {
        let err = compile_source("say ghost").unwrap_err();
        assert_eq!((err.line, err.column), (1, 5));
        let err = compile_source("set ghost = 1").unwrap_err();
        assert_eq!((err.line, err.column), (1, 5));
    }
}
