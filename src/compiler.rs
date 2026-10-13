//! The bytecode compiler: turns checked statements into instructions for the VM.

use std::collections::HashMap;

use crate::decimal::Decimal;
use crate::diagnostic::{Diagnostic, Span};
use crate::parser::{Expr, ExprKind, Program, Statement};
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
    /// Pop two values and fail if they are not the same.
    /// `example` is set when the check came from a doc example.
    Check { example: bool },
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
            } => {
                self.expression(value)?;
                let slot = self.chunk.slot_count;
                self.chunk.slot_count += 1;
                self.slots.insert(name.clone(), slot);
                self.emit(Op::Store(slot), *name_span);
            }
            Statement::Check {
                left,
                right,
                span,
                example,
            } => {
                self.expression(left)?;
                self.expression(right)?;
                self.emit(Op::Check { example: *example }, *span);
            }
        }
        Ok(())
    }

    fn expression(&mut self, expr: &Expr) -> Result<(), Diagnostic> {
        let op = match &expr.kind {
            ExprKind::Text(text) => self.constant(Value::Text(text.clone())),
            ExprKind::Number(number) => match Decimal::parse(number) {
                Some(number) => self.constant(Value::Number(number)),
                None => return Err(too_many_digits(number, expr.span)),
            },
            ExprKind::Name(name) => match self.slots.get(name) {
                Some(&slot) => Op::Load(slot),
                // The checker reports this first; this keeps the compiler from
                // panicking if it is ever called on unchecked code.
                None => {
                    return Err(Diagnostic::new(
                        expr.span,
                        format!(
                            "There's no value called {name}.\n\
                             Values must be created with {name} = ... before they can be used."
                        ),
                    ))
                }
            },
        };
        self.emit(op, expr.span);
        Ok(())
    }

    fn constant(&mut self, value: Value) -> Op {
        self.chunk.constants.push(value);
        Op::Constant(self.chunk.constants.len() - 1)
    }

    fn emit(&mut self, op: Op, span: Span) {
        self.chunk.code.push(op);
        self.chunk.spans.push(span);
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
            vec![
                Value::Text("Hi".to_string()),
                Value::Number(Decimal::parse("7").unwrap())
            ]
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
    fn compiles_a_check_and_an_example() {
        let chunk = compile_source("check \"Hi\" == \"Hi\"\n--- example: 1 == 1").unwrap();
        assert_eq!(
            chunk.code,
            vec![
                Op::Constant(0),
                Op::Constant(1),
                Op::Check { example: false },
                Op::Constant(2),
                Op::Constant(3),
                Op::Check { example: true },
            ]
        );
    }

    #[test]
    fn unchecked_unknown_name_is_an_error_not_a_panic() {
        let err = compile_source("say ghost").unwrap_err();
        assert_eq!((err.line, err.column), (1, 5));
    }
}
