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
    /// Pop this many values and push them as a list. The first item was pushed first.
    List(usize),
    /// Pop one value for each field of `records[index]` and push the record.
    Record(usize),
    /// Pop a list and push how many items it has.
    Length,
    /// Pop an index and a list, and push the item. Position 1 is the first item.
    GetAt,
    /// Pop a new item, an index and a list, and push the list with that item replaced.
    SetAt,
    /// Pop a record and push `constants[index]`'s field. The constant is the field's name as text.
    GetField(usize),
    /// Pop a list of records and push a list of one field. A missing amount is left out.
    /// The constant is the field's name as text.
    Pluck(usize),
    /// Pop a list of records and push a list of records that keep `picks[index]`, in that order.
    Pick(usize),
    /// Pop a list of records and push the sum of one field. A missing amount is left out.
    /// `name` is the field. `currency` is the code when the field is money, so an empty list
    /// still has a currency.
    SumField {
        name: usize,
        currency: Option<usize>,
    },
    /// Pop a record or a list of records and push the list named by `outer`, after checking
    /// that each record has `inner`. Both constants are field names as text.
    Gather {
        outer: usize,
        inner: usize,
    },
    /// Pop a new value and a record, and push the record with that field replaced.
    /// The constant is the field's name as text.
    SetField(usize),
    /// Push a copy of the top value.
    Dup,
    /// Swap the top two values.
    Swap,
    /// Call `functions[index]`. The arguments are on the stack, first pushed first.
    Call(usize),
    /// Leave the current function. `true` when a value is given back, on top of the stack.
    Return(bool),
    /// Pop a value and push it wrapped as `found`.
    WrapFound,
    /// Push `missing`.
    MakeMissing,
    /// Pop a maybe. If it is missing, jump. If it is found, push the inner value.
    MatchFound(usize),
    /// Pop a failure name and leave the function with that failure.
    Fail,
    /// If the top value is the failure named by this constant, pop it and jump.
    OnFail(usize, usize),
}

/// A compiled function. Its jumps point into `code`, not the main program.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub arity: usize,
    /// `true` when the function gives a value back.
    pub returns: bool,
    /// Failure names this function can leave with.
    pub fails: Vec<String>,
    pub slot_count: usize,
    pub code: Vec<Op>,
    pub spans: Vec<Span>,
}

/// A compiled program.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Chunk {
    pub code: Vec<Op>,
    /// Where in the source each instruction came from, for runtime errors.
    pub spans: Vec<Span>,
    pub constants: Vec<Value>,
    /// Field names for each `Op::Record`, in the order the values were pushed.
    pub records: Vec<Vec<String>>,
    /// Field names for each `Op::Pick`, in the order they were written.
    pub picks: Vec<Vec<String>>,
    /// How many named values the program creates.
    pub slot_count: usize,
    pub functions: Vec<Function>,
}

pub fn compile(program: &Program) -> Result<Chunk, Diagnostic> {
    let mut compiler = Compiler {
        sum_currencies: crate::checker::sum_currencies(program),
        chunk: Chunk::default(),
        code: Vec::new(),
        spans: Vec::new(),
        slots: HashMap::new(),
        slot_count: 0,
        functions: HashMap::new(),
        fn_returns: None,
        halts: Vec::new(),
    };
    for statement in &program.statements {
        compiler.statement(statement)?;
    }
    let end = compiler.code.len();
    for index in compiler.halts {
        if let Some(Op::Jump(to)) = compiler.code.get_mut(index) {
            *to = end;
        }
    }
    compiler.chunk.code = compiler.code;
    compiler.chunk.spans = compiler.spans;
    compiler.chunk.slot_count = compiler.slot_count;
    Ok(compiler.chunk)
}

struct Compiler {
    /// Currency of each money `sum`, keyed by the span of the whole `sum`.
    sum_currencies: Vec<(Span, String)>,
    chunk: Chunk,
    /// Instructions for the code being compiled: the main program, or one function.
    code: Vec<Op>,
    spans: Vec<Span>,
    slots: HashMap<String, usize>,
    slot_count: usize,
    /// Function name to its index in `chunk.functions`.
    functions: HashMap<String, usize>,
    /// `Some` while compiling a function: whether that function gives a value back.
    fn_returns: Option<bool>,
    /// Jumps that should leave the whole program, patched once `code` is finished.
    halts: Vec<usize>,
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
                handlers,
                ..
            } => {
                self.expression(value)?;
                self.handlers(handlers, *name_span)?;
                let slot = self.slot_count;
                self.slot_count += 1;
                self.slots.insert(name.clone(), slot);
                self.emit(Op::Store(slot), *name_span);
            }
            Statement::Set {
                name,
                name_span,
                steps,
                value,
                handlers,
                ..
            } => {
                if !handlers.is_empty() && steps.is_empty() {
                    self.expression(value)?;
                    self.handlers(handlers, *name_span)?;
                    let slot = self.slot(name, *name_span)?;
                    self.emit(Op::Store(slot), *name_span);
                    return Ok(());
                }
                let slot = self.slot(name, *name_span)?;
                if steps.is_empty() {
                    self.expression(value)?;
                    self.emit(Op::Store(slot), *name_span);
                } else {
                    self.emit(Op::Load(slot), *name_span);
                    for step in &steps[..steps.len() - 1] {
                        self.emit(Op::Dup, *name_span);
                        self.get_step(step)?;
                    }
                    self.expression(value)?;
                    self.set_step(steps.last().unwrap())?;
                    for step in steps[..steps.len() - 1].iter().rev() {
                        self.set_step(step)?;
                    }
                    self.emit(Op::Store(slot), *name_span);
                }
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
            Statement::For {
                name,
                name_span,
                start,
                end,
                body,
                span,
            } => {
                if end.is_none() {
                    return self.for_list(name, *name_span, start, body, *span);
                }
                let end = end.as_ref().unwrap();
                // Count from start through end, including both. The end is saved
                // once, so changing a name inside the loop doesn't move the finish.
                self.expression(start)?;
                let slot = self.slot_count;
                self.slot_count += 1;
                let previous = self.slots.insert(name.clone(), slot);
                self.emit(Op::Store(slot), *name_span);

                self.expression(end)?;
                let end_slot = self.slot_count;
                self.slot_count += 1;
                self.emit(Op::Store(end_slot), end.span);

                let loop_start = self.code.len();
                self.emit(Op::Load(slot), *name_span);
                self.emit(Op::Load(end_slot), end.span);
                self.emit(Op::Compare(Comparison::Greater), *span);
                let to_body = self.emit(Op::JumpIfFalse(0), *span);
                let leave = self.emit(Op::Jump(0), *span);
                self.patch(to_body);
                for statement in body {
                    self.statement(statement)?;
                }
                self.emit(Op::Load(slot), *name_span);
                self.constant(Value::Number(Decimal::from_int(1)), *span);
                self.emit(Op::Add, *span);
                self.emit(Op::Store(slot), *name_span);
                let back = self.emit(Op::Jump(0), *span);
                if let Some(Op::Jump(to)) = self.code.get_mut(back) {
                    *to = loop_start;
                }
                self.patch(leave);

                match previous {
                    Some(old) => {
                        self.slots.insert(name.clone(), old);
                    }
                    None => {
                        self.slots.remove(name);
                    }
                }
            }
            Statement::Function {
                name,
                params,
                result,
                fails,
                body,
                span,
                ..
            } => self.function(name, params, result.is_some(), fails, body, *span)?,
            Statement::Match {
                subject,
                arms,
                span,
            } => self.open_maybe(subject, arms, *span)?,
            Statement::Fail { name, span } => {
                self.constant(Value::Text(name.clone()), *span);
                self.emit(Op::Fail, *span);
            }
            Statement::Return { value, span } => match value {
                Some(value) => {
                    self.expression(value)?;
                    self.emit(Op::Return(true), *span);
                }
                None => {
                    self.emit(Op::Return(false), *span);
                }
            },
            Statement::Call {
                name,
                name_span,
                args,
                handlers,
                span,
            } => {
                self.call(name, *name_span, args, *span)?;
                self.handlers(handlers, *span)?;
            }
        }
        Ok(())
    }

    fn expression(&mut self, expr: &Expr) -> Result<(), Diagnostic> {
        let span = expr.span;
        match &expr.kind {
            ExprKind::Text(text) => self.constant(Value::Text(text.clone()), span),
            ExprKind::Money { amount, currency } => match Decimal::parse(amount) {
                Some(amount) => self.constant(
                    Value::Money {
                        amount,
                        currency: currency.clone(),
                    },
                    span,
                ),
                None => return Err(too_many_digits(amount, span)),
            },
            ExprKind::Found(inner) => {
                self.expression(inner)?;
                self.emit(Op::WrapFound, span);
            }
            ExprKind::Missing => {
                self.emit(Op::MakeMissing, span);
            }
            ExprKind::Try(inner) => self.try_call(inner)?,
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
            ExprKind::List { items, .. } => {
                for item in items {
                    self.expression(item)?;
                }
                self.emit(Op::List(items.len()), span);
            }
            ExprKind::Record { fields } => {
                let names = fields.iter().map(|field| field.name.clone()).collect();
                for field in fields {
                    self.expression(&field.value)?;
                }
                let index = self.chunk.records.len();
                self.chunk.records.push(names);
                self.emit(Op::Record(index), span);
            }
            ExprKind::At { list, index, .. } => {
                self.expression(list)?;
                self.expression(index)?;
                self.emit(Op::GetAt, span);
            }
            ExprKind::Field { record, name, .. } => {
                self.expression(record)?;
                let index = self.field_name(name);
                self.emit(Op::GetField(index), span);
            }
            ExprKind::Get { fields, list, .. } => {
                self.expression(list)?;
                if fields.len() == 1 {
                    let index = self.field_name(&fields[0].name);
                    self.emit(Op::Pluck(index), span);
                } else {
                    let names = fields.iter().map(|field| field.name.clone()).collect();
                    let index = self.chunk.picks.len();
                    self.chunk.picks.push(names);
                    self.emit(Op::Pick(index), span);
                }
            }
            ExprKind::Sum { field, list, .. } => {
                self.expression(list)?;
                let name = self.field_name(field);
                let currency = self
                    .sum_currencies
                    .iter()
                    .find(|(sum_span, _)| *sum_span == span)
                    .map(|(_, code)| code.clone());
                let currency = currency.map(|code| self.field_name(&code));
                self.emit(Op::SumField { name, currency }, span);
            }
            ExprKind::GetWith {
                field,
                nested,
                source,
                ..
            } => {
                self.expression(source)?;
                let outer = self.field_name(field);
                let inner = self.field_name(nested);
                self.emit(Op::Gather { outer, inner }, span);
            }
            ExprKind::Call {
                name,
                name_span,
                args,
            } => self.call(name, *name_span, args, span)?,
        }
        Ok(())
    }

    /// Compiles a function into `chunk.functions` and remembers its index.
    fn function(
        &mut self,
        name: &str,
        params: &[crate::parser::Param],
        returns: bool,
        fails: &[(String, Span)],
        body: &[Statement],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let index = self.chunk.functions.len();
        self.functions.insert(name.to_string(), index);
        let saved_code = std::mem::take(&mut self.code);
        let saved_spans = std::mem::take(&mut self.spans);
        let saved_slots = std::mem::take(&mut self.slots);
        let saved_count = self.slot_count;
        self.slot_count = 0;
        let saved_returns = self.fn_returns;
        self.fn_returns = Some(returns);
        for param in params {
            let slot = self.slot_count;
            self.slot_count += 1;
            self.slots.insert(param.name.clone(), slot);
        }
        for statement in body {
            self.statement(statement)?;
        }
        if !returns {
            self.emit(Op::Return(false), span);
        }
        let function = Function {
            name: name.to_string(),
            arity: params.len(),
            returns,
            fails: fails.iter().map(|(name, _)| name.clone()).collect(),
            slot_count: self.slot_count,
            code: std::mem::take(&mut self.code),
            spans: std::mem::take(&mut self.spans),
        };
        self.fn_returns = saved_returns;
        self.code = saved_code;
        self.spans = saved_spans;
        self.slots = saved_slots;
        self.slot_count = saved_count;
        self.chunk.functions.push(function);
        Ok(())
    }

    /// Pushes the arguments and calls `name`.
    fn call(
        &mut self,
        name: &str,
        name_span: Span,
        args: &[Expr],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let Some(&index) = self.functions.get(name) else {
            return Err(Diagnostic::new(
                name_span,
                format!(
                    "There's no function called {name}.\n\
                     Write fn {name}(...) above the line that calls it."
                ),
            ));
        };
        for arg in args {
            self.expression(arg)?;
        }
        self.emit(Op::Call(index), span);
        Ok(())
    }

    /// `for name in list`: the name is the item, and the list is kept as it was.
    fn for_list(
        &mut self,
        name: &str,
        name_span: Span,
        list: &Expr,
        body: &[Statement],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.expression(list)?;
        let list_slot = self.fresh_slot();
        self.emit(Op::Store(list_slot), list.span);

        let index_slot = self.fresh_slot();
        self.constant(Value::Number(Decimal::from_int(1)), span);
        self.emit(Op::Store(index_slot), span);

        let item_slot = self.fresh_slot();
        let previous = self.slots.insert(name.to_string(), item_slot);

        let loop_start = self.code.len();
        self.emit(Op::Load(index_slot), span);
        self.emit(Op::Load(list_slot), list.span);
        self.emit(Op::Length, list.span);
        self.emit(Op::Compare(Comparison::Greater), span);
        let to_body = self.emit(Op::JumpIfFalse(0), span);
        let leave = self.emit(Op::Jump(0), span);
        self.patch(to_body);

        self.emit(Op::Load(list_slot), list.span);
        self.emit(Op::Load(index_slot), name_span);
        self.emit(Op::GetAt, name_span);
        self.emit(Op::Store(item_slot), name_span);
        for statement in body {
            self.statement(statement)?;
        }
        self.emit(Op::Load(index_slot), span);
        self.constant(Value::Number(Decimal::from_int(1)), span);
        self.emit(Op::Add, span);
        self.emit(Op::Store(index_slot), span);
        let back = self.emit(Op::Jump(0), span);
        if let Some(Op::Jump(to)) = self.code.get_mut(back) {
            *to = loop_start;
        }
        self.patch(leave);

        match previous {
            Some(old) => {
                self.slots.insert(name.to_string(), old);
            }
            None => {
                self.slots.remove(name);
            }
        }
        Ok(())
    }

    fn get_step(&mut self, step: &crate::parser::Access) -> Result<(), Diagnostic> {
        match step {
            crate::parser::Access::Field { name, span } => {
                let index = self.field_name(name);
                self.emit(Op::GetField(index), *span);
            }
            crate::parser::Access::At { index, span } => {
                self.expression(index)?;
                self.emit(Op::GetAt, *span);
            }
        }
        Ok(())
    }

    /// The stack holds the container, then the new value. An `at` step also needs its index.
    fn set_step(&mut self, step: &crate::parser::Access) -> Result<(), Diagnostic> {
        match step {
            crate::parser::Access::Field { name, span } => {
                let index = self.field_name(name);
                self.emit(Op::SetField(index), *span);
            }
            crate::parser::Access::At { index, span } => {
                self.expression(index)?;
                self.emit(Op::Swap, *span);
                self.emit(Op::SetAt, *span);
            }
        }
        Ok(())
    }

    fn field_name(&mut self, name: &str) -> usize {
        self.chunk.constants.push(Value::Text(name.to_string()));
        self.chunk.constants.len() - 1
    }

    fn fresh_slot(&mut self) -> usize {
        let slot = self.slot_count;
        self.slot_count += 1;
        slot
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
        self.code.push(op);
        self.spans.push(span);
        self.code.len() - 1
    }

    /// Points the jump at `index` to the next instruction to be emitted.
    fn patch(&mut self, index: usize) {
        let target = self.code.len();
        match self.code.get_mut(index) {
            Some(Op::Jump(to) | Op::JumpIfFalse(to) | Op::MatchFound(to)) => *to = target,
            Some(Op::OnFail(_, to)) => *to = target,
            _ => {}
        }
    }

    /// On failure, run the matching handler and leave. On success, the value stays.
    fn handlers(
        &mut self,
        handlers: &[crate::parser::Handler],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if handlers.is_empty() {
            return Ok(());
        }
        let mut jumps = Vec::new();
        for handler in handlers {
            let name = self.chunk.constants.len();
            self.chunk.constants.push(Value::Text(handler.name.clone()));
            jumps.push(self.emit(Op::OnFail(name, 0), handler.span));
        }
        let success = self.emit(Op::Jump(0), span);
        for (handler, jump) in handlers.iter().zip(jumps) {
            self.patch(jump);
            self.statement(&handler.body)?;
            match self.fn_returns {
                None => {
                    let halt = self.emit(Op::Jump(0), handler.span);
                    self.halts.push(halt);
                }
                Some(false) => {
                    self.emit(Op::Return(false), handler.span);
                }
                Some(true) => {}
            }
        }
        self.patch(success);
        Ok(())
    }

    fn open_maybe(
        &mut self,
        subject: &Expr,
        arms: &[crate::parser::Catch],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.expression(subject)?;
        let missing_at = self.emit(Op::MatchFound(0), span);
        let mut end_jumps = Vec::new();
        if let Some(found) = arms.iter().find(|arm| arm.found.is_some()) {
            let (name, name_span) = found.found.clone().unwrap();
            let slot = self.slot_count;
            self.slot_count += 1;
            let previous = self.slots.insert(name.clone(), slot);
            self.emit(Op::Store(slot), name_span);
            self.statement(&found.body)?;
            match previous {
                Some(old) => {
                    self.slots.insert(name, old);
                }
                None => {
                    self.slots.remove(&name);
                }
            }
            end_jumps.push(self.emit(Op::Jump(0), span));
        }
        self.patch(missing_at);
        if let Some(missing) = arms.iter().find(|arm| arm.found.is_none()) {
            self.statement(&missing.body)?;
        }
        for jump in end_jumps {
            self.patch(jump);
        }
        Ok(())
    }

    fn try_call(&mut self, inner: &Expr) -> Result<(), Diagnostic> {
        let crate::parser::ExprKind::Call {
            name,
            name_span,
            args,
        } = &inner.kind
        else {
            self.expression(inner)?;
            return Ok(());
        };
        self.call(name, *name_span, args, inner.span)?;
        let Some(&index) = self.functions.get(name) else {
            return Ok(());
        };
        let fails = self
            .chunk
            .functions
            .get(index)
            .map(|function| function.fails.clone())
            .unwrap_or_default();
        let mut jumps = Vec::new();
        for failure in &fails {
            let name_index = self.chunk.constants.len();
            self.chunk.constants.push(Value::Text(failure.clone()));
            jumps.push(self.emit(Op::OnFail(name_index, 0), inner.span));
        }
        let success = self.emit(Op::Jump(0), inner.span);
        for (failure, jump) in fails.iter().zip(jumps) {
            self.patch(jump);
            self.constant(Value::Text(failure.clone()), inner.span);
            self.emit(Op::Fail, inner.span);
        }
        self.patch(success);
        Ok(())
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
    fn a_for_loop_counts_until_the_number_passes_the_end() {
        let chunk = compile_source("for i in 1..2\n  say i").unwrap();
        assert_eq!(
            chunk.code,
            vec![
                Op::Constant(0),
                Op::Store(0),
                Op::Constant(1),
                Op::Store(1),
                Op::Load(0),
                Op::Load(1),
                Op::Compare(Comparison::Greater),
                Op::JumpIfFalse(9),
                Op::Jump(16),
                Op::Load(0),
                Op::Say,
                Op::Load(0),
                Op::Constant(2),
                Op::Add,
                Op::Store(0),
                Op::Jump(4),
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
