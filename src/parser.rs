//! The parser: turns the lexer's tokens into a tree of statements.
//!
//! Statements, one per line:
//!   say VALUE
//!   NAME = VALUE                 a value that can't change
//!   var NAME = VALUE             a value that can change
//!   set NAME = VALUE             change a var
//!   check VALUE == VALUE         a test (also !=, <, <=, >, >=)
//!   --- example: VALUE == VALUE  a doc example, run as a test
//!   when                         run the first case whose condition is true
//!     CONDITION -> STATEMENT
//!     else -> STATEMENT
//!
//! Values, from loosest to tightest binding:
//!   a or b
//!   a and b
//!   not a
//!   a == b   a != b   a < b   a <= b   a > b   a >= b
//!   a + b    a - b
//!   a * b    a / b    a % b
//!   -a
//!   text, "text with {values}", numbers, true, false, names, (VALUE)

use std::fmt;

use crate::decimal::Decimal;
use crate::diagnostic::{closest_match, Diagnostic, Span};
use crate::lexer::{Piece, Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// `say value`. `span` covers the `say` keyword.
    Say { value: Expr, span: Span },
    /// `name = value`, or `var name = value` when `mutable`.
    Let {
        name: String,
        name_span: Span,
        value: Expr,
        mutable: bool,
        /// `on Declined -> ...` when `value` is a call that can fail.
        handlers: Vec<Handler>,
    },
    /// `set name = value`, or `set name.field = value`, or `set name at 1 = value`.
    /// `span` covers the `set` keyword.
    Set {
        name: String,
        name_span: Span,
        /// Steps after the name: a field, or an item at a position.
        steps: Vec<Access>,
        value: Expr,
        span: Span,
        /// `on Declined -> ...` when `value` is a call that can fail.
        handlers: Vec<Handler>,
    },
    /// `check left == right`, or a doc example that means the same thing.
    Check {
        left: Expr,
        op: Comparison,
        right: Expr,
        /// Covers the whole statement, so a failure can underline it.
        span: Span,
        /// `true` when this came from `--- example:`.
        example: bool,
    },
    /// `when` with its cases. `span` covers the `when` keyword.
    When { arms: Vec<Arm>, span: Span },
    /// `for name in start..end`, or `for name in list`.
    /// `end` is absent when `start` is the list to walk.
    /// `span` covers the `for` keyword.
    For {
        name: String,
        name_span: Span,
        start: Expr,
        end: Option<Expr>,
        body: Vec<Statement>,
        span: Span,
    },
    /// `fn name(n: number) -> number`, then the indented lines.
    /// `result` is absent when the function gives nothing back.
    Function {
        name: String,
        name_span: Span,
        params: Vec<Param>,
        result: Option<SigKind>,
        /// Failures this function can leave with, written `fails Declined, NetworkDown`.
        fails: Vec<(String, Span)>,
        /// `require` and `ensure` lines, in the order they were written.
        contracts: Vec<Contract>,
        body: Vec<Statement>,
        span: Span,
    },
    /// `return value`, or `return` when the function gives nothing back.
    Return { value: Option<Expr>, span: Span },
    /// `greet("Amina")` on its own line, for a function that gives nothing back.
    Call {
        name: String,
        name_span: Span,
        args: Vec<Expr>,
        span: Span,
        /// `on Declined -> ...` under a call that can fail.
        handlers: Vec<Handler>,
    },
    /// `when value`, then `found name` and `missing`.
    Match {
        subject: Expr,
        arms: Vec<Catch>,
        span: Span,
    },
    /// `fail Declined`, leaving the function with a declared failure.
    Fail { name: String, span: Span },
}

/// `require amount > 0 USD` or `ensure result >= 0 USD`, under a `fn`.
#[derive(Debug, Clone, PartialEq)]
pub struct Contract {
    /// `true` for `ensure`, checked when the function gives back.
    /// `false` for `require`, checked when the function is called.
    pub ensure: bool,
    pub condition: Expr,
    /// Covers the whole line, so a broken contract can underline it.
    pub span: Span,
}

impl Contract {
    pub fn keyword(&self) -> &'static str {
        if self.ensure {
            "ensure"
        } else {
            "require"
        }
    }
}

/// A function parameter: `n: number`.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub name_span: Span,
    pub kind: SigKind,
    pub kind_span: Span,
    /// `where 1..500`, the smallest and largest value the parameter takes.
    pub rule: Option<Rule>,
}

/// `where 1..500` after a parameter's kind. Both ends are written values, and both
/// are included.
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub low: Expr,
    pub high: Expr,
    /// Covers `where` through the last value.
    pub span: Span,
}

impl Rule {
    /// `from 1 through 500`, as a broken rule says it.
    pub fn range(&self) -> String {
        format!("from {} through {}", self.low, self.high)
    }

    /// The rule as a condition on the parameter: `guests >= 1 and guests <= 500`.
    pub fn condition(&self, name: &str) -> Expr {
        let span = self.span;
        let param = || {
            Box::new(Expr {
                kind: ExprKind::Name(name.to_string()),
                span,
            })
        };
        let compare = |op, bound: &Expr| Expr {
            kind: ExprKind::Compare {
                op,
                left: param(),
                right: Box::new(bound.clone()),
                op_span: span,
            },
            span,
        };
        Expr {
            kind: ExprKind::Logic {
                op: Logic::And,
                left: Box::new(compare(Comparison::GreaterEqual, &self.low)),
                right: Box::new(compare(Comparison::LessEqual, &self.high)),
                op_span: span,
            },
            span,
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "where {}..{}", self.low, self.high)
    }
}

/// The kind written in a function: `text`, `number`, `true/false`, or `list of` one of those.
#[derive(Debug, Clone, PartialEq)]
pub enum SigKind {
    Text,
    Number,
    Bool,
    List(Box<SigKind>),
    /// Money in one currency, written `USD`.
    Money(String),
    /// A value that might be missing, written `maybe number`.
    Maybe(Box<SigKind>),
}

impl SigKind {
    /// The kind as it is written in a function.
    pub fn written(&self) -> String {
        match self {
            SigKind::Text => "text".to_string(),
            SigKind::Number => "number".to_string(),
            SigKind::Bool => "true/false".to_string(),
            SigKind::List(item) => format!("list of {}", item.written()),
            SigKind::Money(code) => code.clone(),
            SigKind::Maybe(inner) => format!("maybe {}", inner.written()),
        }
    }
}

/// One case under `when value`: `found name` or `missing`.
#[derive(Debug, Clone, PartialEq)]
pub struct Catch {
    /// The name bound by `found`. Absent for `missing`.
    pub found: Option<(String, Span)>,
    pub body: Box<Statement>,
    pub span: Span,
}

/// `on Declined -> say "Card declined"`, or `on Declined -> "declined"`.
#[derive(Debug, Clone, PartialEq)]
pub struct Handler {
    pub name: String,
    pub name_span: Span,
    pub body: Recovery,
    pub span: Span,
}

/// What an `on` line does when its failure happens.
#[derive(Debug, Clone, PartialEq)]
pub enum Recovery {
    /// `on Declined -> say "Card declined"`: run the line, then stop the program
    /// (or leave the function).
    Stop(Box<Statement>),
    /// `on Declined -> "declined"`: use this value in place of the call's answer,
    /// and carry on.
    Value(Expr),
}

/// A currency code is three capital letters, as in `USD`.
pub fn is_currency(name: &str) -> bool {
    name.len() == 3 && name.chars().all(|c| c.is_ascii_uppercase())
}

/// Three letters that would be a currency if they were capitals.
fn currency_letters(name: &str) -> bool {
    name.len() == 3 && name.chars().all(|c| c.is_ascii_alphabetic()) && !is_currency(name)
}

fn capitalize(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => name.to_string(),
    }
}

fn attach_handlers(statement: Statement, handlers: Vec<Handler>) -> Statement {
    match statement {
        Statement::Let {
            name,
            name_span,
            value,
            mutable,
            ..
        } => Statement::Let {
            name,
            name_span,
            value,
            mutable,
            handlers,
        },
        Statement::Set {
            name,
            name_span,
            steps,
            value,
            span,
            ..
        } => Statement::Set {
            name,
            name_span,
            steps,
            value,
            span,
            handlers,
        },
        Statement::Call {
            name,
            name_span,
            args,
            span,
            ..
        } => Statement::Call {
            name,
            name_span,
            args,
            span,
            // A call on its own line gives nothing back, so `on ... -> greet("x")`
            // runs that call.
            handlers: handlers.into_iter().map(call_as_statement).collect(),
        },
        other => other,
    }
}

fn call_as_statement(handler: Handler) -> Handler {
    let Recovery::Value(Expr {
        kind: ExprKind::Call {
            name,
            name_span,
            args,
        },
        span,
    }) = handler.body
    else {
        return handler;
    };
    Handler {
        body: Recovery::Stop(Box::new(Statement::Call {
            name,
            name_span,
            args,
            span,
            handlers: Vec::new(),
        })),
        ..handler
    }
}

/// One step into a list or a record: `name.field` or `name at 1`.
#[derive(Debug, Clone, PartialEq)]
pub enum Access {
    Field { name: String, span: Span },
    At { index: Expr, span: Span },
}

/// One case of a `when`: `condition -> body`, or `else -> body`.
#[derive(Debug, Clone, PartialEq)]
pub struct Arm {
    /// `None` for `else`.
    pub condition: Option<Expr>,
    pub body: Box<Statement>,
    /// Covers the condition, or the `else` keyword.
    pub span: Span,
}

impl Statement {
    /// The line this statement starts on.
    pub fn line(&self) -> usize {
        match self {
            Statement::Say { span, .. } => span.line,
            Statement::Let { name_span, .. } => name_span.line,
            Statement::Set { span, .. } => span.line,
            Statement::Check { span, .. } => span.line,
            Statement::When { span, .. } => span.line,
            Statement::For { span, .. } => span.line,
            Statement::Function { span, .. } => span.line,
            Statement::Return { span, .. } => span.line,
            Statement::Call { span, .. } => span.line,
            Statement::Match { span, .. } | Statement::Fail { span, .. } => span.line,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Comparison {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl Comparison {
    pub fn symbol(self) -> &'static str {
        match self {
            Comparison::Equal => "==",
            Comparison::NotEqual => "!=",
            Comparison::Less => "<",
            Comparison::LessEqual => "<=",
            Comparison::Greater => ">",
            Comparison::GreaterEqual => ">=",
        }
    }

    /// Whether the comparison only makes sense for numbers.
    pub fn orders(self) -> bool {
        !matches!(self, Comparison::Equal | Comparison::NotEqual)
    }

    fn from_token(kind: &TokenKind) -> Option<Comparison> {
        Some(match kind {
            TokenKind::EqualEqual => Comparison::Equal,
            TokenKind::NotEqual => Comparison::NotEqual,
            TokenKind::Less => Comparison::Less,
            TokenKind::LessEqual => Comparison::LessEqual,
            TokenKind::Greater => Comparison::Greater,
            TokenKind::GreaterEqual => Comparison::GreaterEqual,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Arithmetic {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

impl Arithmetic {
    pub fn symbol(self) -> &'static str {
        match self {
            Arithmetic::Add => "+",
            Arithmetic::Subtract => "-",
            Arithmetic::Multiply => "*",
            Arithmetic::Divide => "/",
            Arithmetic::Remainder => "%",
        }
    }

    /// The verb for an error message: "You're adding ...".
    pub fn verb(self) -> &'static str {
        match self {
            Arithmetic::Add => "adding",
            Arithmetic::Subtract => "subtracting",
            Arithmetic::Multiply => "multiplying",
            Arithmetic::Divide => "dividing",
            Arithmetic::Remainder => "taking the remainder of",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Logic {
    And,
    Or,
}

impl Logic {
    pub fn word(self) -> &'static str {
        match self {
            Logic::And => "and",
            Logic::Or => "or",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Text(String),
    /// Text with values inside: `"Hello, {name}"`.
    Template(Vec<Part>),
    Number(String),
    /// `100 USD`.
    Money {
        amount: String,
        currency: String,
    },
    Bool(bool),
    Name(String),
    /// `-value`
    Negate(Box<Expr>),
    /// `not value`
    Not(Box<Expr>),
    Arithmetic {
        op: Arithmetic,
        left: Box<Expr>,
        right: Box<Expr>,
        /// The operator itself, for errors like dividing by zero.
        op_span: Span,
    },
    Compare {
        op: Comparison,
        left: Box<Expr>,
        right: Box<Expr>,
        op_span: Span,
    },
    Logic {
        op: Logic,
        left: Box<Expr>,
        right: Box<Expr>,
        op_span: Span,
    },
    /// `(value)`, kept so values are written back the way they were typed.
    Group(Box<Expr>),
    /// `list "coding", "reading"`, or `list of text` when `empty` is set.
    List {
        items: Vec<Expr>,
        empty: Option<SigKind>,
    },
    /// `record` with `name = value` fields.
    Record {
        fields: Vec<Field>,
    },
    /// `found 2.50`, a maybe that holds a value.
    Found(Box<Expr>),
    /// `missing`, a maybe that holds nothing.
    Missing,
    /// `try charge(amount)`, passing failures upward.
    Try(Box<Expr>),
    /// `double(4)`.
    Call {
        name: String,
        name_span: Span,
        args: Vec<Expr>,
    },
    /// `hobbies at 1`. Position 1 is the first item.
    At {
        list: Box<Expr>,
        index: Box<Expr>,
        at_span: Span,
    },
    /// `address.city`.
    Field {
        record: Box<Expr>,
        name: String,
        name_span: Span,
    },
    /// `sum fee of roster`. Adds that field of each record. A missing amount is left out.
    Sum {
        field: String,
        field_span: Span,
        of_span: Span,
        list: Box<Expr>,
    },
    /// `get fee of roster`, or `get name, age, fee of roster`.
    /// One field is a list of those values. Several fields are a list of records
    /// that keep only those fields. A missing amount is left out of a single field.
    Get {
        fields: Vec<Asked>,
        of_span: Span,
        list: Box<Expr>,
    },
    /// `when` with indented cases that each give a value. The last case is `else`.
    When(Vec<ValueArm>),
    /// `students where score >= 50`. The records whose condition is true, in order.
    /// Inside the condition, a field of the records is read by its name.
    Where {
        list: Box<Expr>,
        condition: Box<Expr>,
        where_span: Span,
    },
    /// `get bookings with acts in roster`. The bookings, each with its acts, from one place.
    GetWith {
        field: String,
        field_span: Span,
        nested: String,
        nested_span: Span,
        source: Box<Expr>,
    },
}

/// One case of a `when` that gives a value: `score >= 70 -> "A"`, or `else -> "Fail"`.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueArm {
    /// `None` for `else`.
    pub condition: Option<Expr>,
    pub value: Expr,
    /// Covers the condition, or the `else` keyword.
    pub span: Span,
}

/// A field named by `get`: `name` in `get name, fee of roster`.
#[derive(Debug, Clone, PartialEq)]
pub struct Asked {
    pub name: String,
    pub span: Span,
}

/// One field of a record: `city = "Anytown"`.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub name_span: Span,
    pub value: Expr,
}

/// One part of a text with values inside.
#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Text(String),
    Value(Expr),
}

impl Expr {
    /// The names this expression reads, each once, in the order they are written.
    /// Field names and called functions are not included.
    pub fn names(&self) -> Vec<String> {
        let mut names = Vec::new();
        self.collect_names(&mut names);
        names
    }

    fn collect_names(&self, names: &mut Vec<String>) {
        match &self.kind {
            ExprKind::Name(name) => {
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
            ExprKind::Template(parts) => {
                for part in parts {
                    if let Part::Value(value) = part {
                        value.collect_names(names);
                    }
                }
            }
            ExprKind::Negate(inner)
            | ExprKind::Not(inner)
            | ExprKind::Group(inner)
            | ExprKind::Found(inner)
            | ExprKind::Try(inner) => inner.collect_names(names),
            ExprKind::Arithmetic { left, right, .. }
            | ExprKind::Compare { left, right, .. }
            | ExprKind::Logic { left, right, .. } => {
                left.collect_names(names);
                right.collect_names(names);
            }
            ExprKind::At { list, index, .. } => {
                list.collect_names(names);
                index.collect_names(names);
            }
            ExprKind::List { items, .. } => {
                for item in items {
                    item.collect_names(names);
                }
            }
            ExprKind::Record { fields } => {
                for field in fields {
                    field.value.collect_names(names);
                }
            }
            ExprKind::Call { args, .. } => {
                for arg in args {
                    arg.collect_names(names);
                }
            }
            ExprKind::Field { record, .. } => record.collect_names(names),
            ExprKind::Sum { list, .. } | ExprKind::Get { list, .. } => list.collect_names(names),
            ExprKind::GetWith { source, .. } => source.collect_names(names),
            ExprKind::Where {
                list, condition, ..
            } => {
                list.collect_names(names);
                condition.collect_names(names);
            }
            ExprKind::When(arms) => {
                for arm in arms {
                    if let Some(condition) = &arm.condition {
                        condition.collect_names(names);
                    }
                    arm.value.collect_names(names);
                }
            }
            ExprKind::Text(_)
            | ExprKind::Number(_)
            | ExprKind::Money { .. }
            | ExprKind::Bool(_)
            | ExprKind::Missing => {}
        }
    }

    /// The amount of a written number or money, like `500`, `-5` or `0 USD`.
    /// `None` for anything that has to be worked out.
    pub fn written_amount(&self) -> Option<Decimal> {
        match &self.kind {
            ExprKind::Number(number) => Decimal::parse(number),
            ExprKind::Money { amount, .. } => Decimal::parse(amount),
            ExprKind::Negate(inner) => inner.written_amount()?.checked_neg().ok(),
            ExprKind::Group(inner) => inner.written_amount(),
            _ => None,
        }
    }

    /// The expression inside any parentheses.
    pub fn ungrouped(&self) -> &Expr {
        match &self.kind {
            ExprKind::Group(inner) => inner.ungrouped(),
            _ => self,
        }
    }
}

/// Writes an expression back out as Quill source, for suggestions.
impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            ExprKind::Text(text) => write!(f, "\"{}\"", escape(text)),
            ExprKind::Template(parts) => {
                write!(f, "\"")?;
                for part in parts {
                    match part {
                        Part::Text(text) => write!(f, "{}", escape(text))?,
                        Part::Value(value) => write!(f, "{{{value}}}")?,
                    }
                }
                write!(f, "\"")
            }
            ExprKind::Number(number) => write!(f, "{number}"),
            ExprKind::Money { amount, currency } => write!(f, "{amount} {currency}"),
            ExprKind::Bool(value) => write!(f, "{value}"),
            ExprKind::Name(name) => write!(f, "{name}"),
            ExprKind::Negate(inner) => write!(f, "-{inner}"),
            ExprKind::Not(inner) => write!(f, "not {inner}"),
            ExprKind::Arithmetic {
                op, left, right, ..
            } => write!(f, "{left} {} {right}", op.symbol()),
            ExprKind::Compare {
                op, left, right, ..
            } => write!(f, "{left} {} {right}", op.symbol()),
            ExprKind::Logic {
                op, left, right, ..
            } => write!(f, "{left} {} {right}", op.word()),
            ExprKind::Group(inner) => write!(f, "({inner})"),
            ExprKind::List { items, empty } => {
                write!(f, "list")?;
                if let Some(kind) = empty {
                    write!(f, " of {}", kind.written())?;
                }
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, " {item}")?;
                }
                Ok(())
            }
            ExprKind::Record { fields } => {
                write!(f, "record")?;
                for (index, field) in fields.iter().enumerate() {
                    if index > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, " {} = {}", field.name, field.value)?;
                }
                Ok(())
            }
            ExprKind::Found(value) => write!(f, "found {value}"),
            ExprKind::Missing => write!(f, "missing"),
            ExprKind::Try(inner) => write!(f, "try {inner}"),
            ExprKind::Call { name, args, .. } => {
                write!(f, "{name}(")?;
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                write!(f, ")")
            }
            ExprKind::At { list, index, .. } => write!(f, "{list} at {index}"),
            ExprKind::Field { record, name, .. } => write!(f, "{record}.{name}"),
            ExprKind::Sum { field, list, .. } => write!(f, "sum {field} of {list}"),
            ExprKind::Get { fields, list, .. } => {
                write!(f, "get ")?;
                for (index, field) in fields.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", field.name)?;
                }
                write!(f, " of {list}")
            }
            ExprKind::GetWith {
                field,
                nested,
                source,
                ..
            } => write!(f, "get {field} with {nested} in {source}"),
            ExprKind::When(_) => write!(f, "when"),
            ExprKind::Where {
                list, condition, ..
            } => write!(f, "{list} where {condition}"),
        }
    }
}

/// Text written back inside quotes, with escapes for characters that need them.
fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            c => out.push(c),
        }
    }
    out
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Statement::Say { value, .. } => write!(f, "say {value}"),
            Statement::Let {
                name,
                value,
                mutable,
                ..
            } => {
                if *mutable {
                    write!(f, "var {name} = {value}")
                } else {
                    write!(f, "{name} = {value}")
                }
            }
            Statement::Set {
                name, steps, value, ..
            } => {
                write!(f, "set {name}")?;
                write_steps(f, steps)?;
                write!(f, " = {value}")
            }
            Statement::Check {
                left,
                op,
                right,
                example,
                ..
            } => {
                let keyword = if *example { "--- example:" } else { "check" };
                write!(f, "{keyword} {left} {} {right}", op.symbol())
            }
            Statement::When { .. } => write!(f, "when"),
            Statement::For {
                name, start, end, ..
            } => match end {
                Some(end) => write!(f, "for {name} in {start}..{end}"),
                None => write!(f, "for {name} in {start}"),
            },
            Statement::Function {
                name,
                params,
                result,
                ..
            } => {
                write!(f, "fn {name}(")?;
                for (index, param) in params.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", param.name, param.kind.written())?;
                }
                write!(f, ")")?;
                if let Some(result) = result {
                    write!(f, " -> {}", result.written())?;
                }
                Ok(())
            }
            Statement::Return { value, .. } => match value {
                Some(value) => write!(f, "return {value}"),
                None => write!(f, "return"),
            },
            Statement::Call { name, args, .. } => {
                write!(f, "{name}(")?;
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                write!(f, ")")
            }
            Statement::Match { .. } => write!(f, "when"),
            Statement::Fail { name, .. } => write!(f, "fail {name}"),
        }
    }
}

pub fn parse(tokens: &[Token]) -> Result<Program, Diagnostic> {
    let mut statements = Vec::new();
    if tokens.is_empty() {
        return Ok(Program { statements });
    }

    let mut parser = Parser {
        tokens,
        pos: 0,
        in_function: false,
    };
    loop {
        match parser.peek().kind {
            TokenKind::EndOfFile => break,
            TokenKind::Newline | TokenKind::Dedent => {
                parser.advance();
            }
            _ => statements.push(parser.full_statement()?),
        }
    }
    Ok(Program {
        statements: examples_after_their_function(statements),
    })
}

/// Doc examples written directly above a `fn` document that function, so they run
/// just after it is written, once it can be called. Their spans don't change, so a
/// failure still points at the line where the example was written.
fn examples_after_their_function(statements: Vec<Statement>) -> Vec<Statement> {
    let mut ordered = Vec::with_capacity(statements.len());
    let mut examples = Vec::new();
    for statement in statements {
        match statement {
            Statement::Check { example: true, .. } => examples.push(statement),
            Statement::Function { .. } => {
                ordered.push(statement);
                ordered.append(&mut examples);
            }
            other => {
                ordered.append(&mut examples);
                ordered.push(other);
            }
        }
    }
    ordered.append(&mut examples);
    ordered
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    /// `true` while reading the lines under a `fn`.
    in_function: bool,
}

impl<'a> Parser<'a> {
    /// The current token. Past the end, keeps returning the last token (EndOfFile).
    fn peek(&self) -> &'a Token {
        self.peek_at(0)
    }

    fn peek_at(&self, offset: usize) -> &'a Token {
        let last = self.tokens.len() - 1;
        &self.tokens[(self.pos + offset).min(last)]
    }

    fn advance(&mut self) -> &'a Token {
        let token = self.peek();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        token
    }

    /// A statement and the end of its line.
    fn full_statement(&mut self) -> Result<Statement, Diagnostic> {
        let mut statement = self.statement()?;
        // A `when` ends with its indented block, so its line has already ended.
        if !matches!(
            statement,
            Statement::When { .. }
                | Statement::Match { .. }
                | Statement::For { .. }
                | Statement::Function { .. }
        ) && !contains_record(&statement)
        {
            self.end_of_line(&statement)?;
            statement = self.maybe_handlers(statement)?;
        }
        Ok(statement)
    }

    fn statement(&mut self) -> Result<Statement, Diagnostic> {
        let token = self.peek();
        match &token.kind {
            TokenKind::Say => self.say(),
            TokenKind::Check => self.check_statement(false),
            TokenKind::Example => self.check_statement(true),
            TokenKind::Var => self.var_statement(),
            TokenKind::Set => self.set_statement(),
            TokenKind::When => self.when_statement(),
            TokenKind::For => self.for_statement(),
            TokenKind::Fn => self.function_statement(),
            TokenKind::Return => self.return_statement(),
            TokenKind::Fail => self.fail_statement(),
            TokenKind::Fails => Err(Diagnostic::new(
                token.span(),
                "fails names how a function can fail, at the top of the function, under fn.\n\
                 fn charge(amount: USD) -> text, then fails Declined.",
            )
            .suggest("fails Declined")),
            TokenKind::Require | TokenKind::Ensure => {
                let word = describe(&token.kind);
                let why = "require says what has to be true when the function is called, \
                           and ensure says what is true when it gives back.";
                if self.in_function {
                    return Err(Diagnostic::new(
                        token.span(),
                        format!(
                            "{word} goes at the top of a function, right under the fn line.\n\
                             {why} Move this line up, above the function's other lines."
                        ),
                    ));
                }
                Err(Diagnostic::new(
                    token.span(),
                    format!("{word} goes inside a function, at the top.\n{why}"),
                )
                .suggest("fn withdraw(amount: USD) -> USD\n  require amount > 0 USD"))
            }
            TokenKind::On => Err(Diagnostic::new(
                token.span(),
                "on handles one failure of the call above it.\n\
                 Indent it under the call: on Declined -> say \"Card declined\".",
            )),
            TokenKind::Found | TokenKind::Missing => Err(Diagnostic::new(
                token.span(),
                "found and missing go under when, to open a maybe.\n\
                 when find_price(\"bread\"), then found price -> say price, and missing -> say \"none\".",
            )),
            TokenKind::Maybe => Err(Diagnostic::new(
                token.span(),
                "maybe names a kind that might be missing.\n\
                 It goes after ->, as in -> maybe number.",
            )
            .suggest("-> maybe number")),
            TokenKind::Identifier(name) if self.peek_at(1).kind == TokenKind::LeftParen => {
                let name = name.clone();
                self.call_statement(name)
            }
            TokenKind::Identifier(name) if self.peek_at(1).kind == TokenKind::Equals => {
                self.let_statement(name, false)
            }
            TokenKind::Identifier(name) => Err(self.stray_name(name, token)),
            TokenKind::Else => Err(Diagnostic::new(
                token.span(),
                "else only goes inside a when, as its last case.\n\
                 It runs when none of the cases above it are true.",
            )),
            TokenKind::Indent => {
                let first = self.peek_at(1);
                Err(Diagnostic::new(
                    first.span(),
                    "This line is indented, but nothing above it starts a block.\n\
                     Only the cases under a when, the lines under a for, the lines under a function, \
                     and the fields of a record are indented, by exactly 2 spaces. \
                     Move this line back to the left.",
                ))
            }
            TokenKind::EqualEqual => Err(Diagnostic::new(
                token.span(),
                "This line starts with ==, but == needs a value on its left.\n\
                 To test that two values are the same, use check.",
            )
            .suggest("check name == \"Hello\"")),
            TokenKind::Equals => Err(Diagnostic::new(
                token.span(),
                "This line starts with =, but = needs a name on its left.\n\
                 To create a value, write its name, then =, then the value.",
            )
            .suggest("name = \"Amina\"")),
            kind if starts_value(kind) => {
                let value = self.expression()?;
                Err(self.bare_expression(&value))
            }
            other => Err(Diagnostic::new(
                token.span(),
                format!(
                    "A line can't start with {}.\n\
                     A line can start with say, check, var, set, when, for, fn, return, \
                     a call, or a name and =.",
                    describe(other)
                ),
            )),
        }
    }

    fn check_statement(&mut self, example: bool) -> Result<Statement, Diagnostic> {
        let keyword = self.advance();
        let word = if example { "example" } else { "check" };
        let a = if example { "An" } else { "A" };
        let written = if example { "--- example:" } else { "check" };
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                keyword.span(),
                format!(
                    "{word} needs two values with == between them.\n\
                     {a} {word} passes when both sides are the same value."
                ),
            )
            .suggest(format!("{written} name == \"Hello\"")));
        }
        let left = self.sum()?;
        let Some(op) = Comparison::from_token(&self.peek().kind) else {
            let found = self.peek();
            return Err(Diagnostic::new(
                found.span(),
                format!(
                    "{word} compares with ==, but found {}.\n\
                     Write the value you have, then ==, then the value you expect. \
                     (!=, <, <=, > and >= work too.)",
                    describe(&found.kind)
                ),
            )
            .suggest(format!("{written} {left} == \"expected\"")));
        };
        let op_token = self.advance();
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                op_token.span(),
                format!(
                    "{} needs a value on its right.\n\
                     A check compares two values, so write the one you expect.",
                    op.symbol()
                ),
            )
            .suggest(format!("{written} {left} {} \"expected\"", op.symbol())));
        }
        let right = self.sum()?;
        Ok(Statement::Check {
            span: cover(keyword.span(), right.span),
            left,
            op,
            right,
            example,
        })
    }

    fn say(&mut self) -> Result<Statement, Diagnostic> {
        let say = self.advance();
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                say.span(),
                "say needs something to say.\n\
                 Put text, a number or the name of a value after it.",
            )
            .suggest("say \"Hello, World!\""));
        }
        if self.peek().kind == TokenKind::Arrow {
            let arrow = self.advance().clone();
            let suggestion = if starts_value(&self.peek().kind) {
                match self.expression() {
                    Ok(value) => format!("say {value}"),
                    Err(_) => "say \"Hello, World!\"".to_string(),
                }
            } else {
                "say \"Hello, World!\"".to_string()
            };
            return Err(Diagnostic::new(
                arrow.span(),
                "-> belongs inside a when, between a condition and what to do.\n\
                 say shows a value, so write that value right after say.",
            )
            .suggest(suggestion));
        }
        let value = self.expression()?;
        Ok(Statement::Say {
            value,
            span: say.span(),
        })
    }

    fn let_statement(&mut self, name: &str, mutable: bool) -> Result<Statement, Diagnostic> {
        let name_token = self.advance();
        let equals = self.advance();
        let written = if mutable {
            format!("var {name}")
        } else {
            name.to_string()
        };
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                equals.span(),
                format!(
                    "{written} = needs a value after the =.\n\
                     A value can be text, a number, a calculation or the name of another value."
                ),
            )
            .suggest(format!("{written} = \"some text\"")));
        }
        let value = self.expression()?;
        Ok(Statement::Let {
            name: name.to_string(),
            name_span: name_token.span(),
            value,
            mutable,
            handlers: Vec::new(),
        })
    }

    fn var_statement(&mut self) -> Result<Statement, Diagnostic> {
        let var = self.advance();
        let (name, _) = self.assignment_target(var, "var", "count", "0")?;
        self.let_statement(&name, true)
    }

    fn set_statement(&mut self) -> Result<Statement, Diagnostic> {
        let set = self.advance();
        let name_token = self.peek().clone();
        let TokenKind::Identifier(name) = name_token.kind.clone() else {
            return Err(self
                .assignment_target(set, "set", "count", "count + 1")
                .unwrap_err());
        };
        let name_span = name_token.span();
        self.advance();
        let steps = self.access_steps()?;
        if self.peek().kind != TokenKind::Equals {
            let place = write_steps_string(&steps);
            return Err(Diagnostic::new(
                name_span,
                format!(
                    "set {name}{place} needs = and a value after it.\n\
                     set changes a value that was created with var."
                ),
            )
            .suggest(if steps.is_empty() {
                format!("set {name} = {name} + 1")
            } else {
                format!("set {name}{place} = {name}{place}")
            }));
        }
        let equals = self.advance();
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                equals.span(),
                format!(
                    "set {name} = needs the new value after the =.\n\
                     set changes a value that was created with var."
                ),
            )
            .suggest(format!("set {name} = {name} + 1")));
        }
        let value = self.expression()?;
        Ok(Statement::Set {
            name,
            name_span,
            steps,
            value,
            span: set.span(),
            handlers: Vec::new(),
        })
    }

    /// `.field` and `at index` steps, stopping before `=`.
    fn access_steps(&mut self) -> Result<Vec<Access>, Diagnostic> {
        let mut steps = Vec::new();
        loop {
            match self.peek().kind {
                TokenKind::Dot => {
                    let dot = self.advance().clone();
                    let field = self.peek().clone();
                    let TokenKind::Identifier(name) = field.kind.clone() else {
                        return Err(Diagnostic::new(
                            dot.span(),
                            format!(
                                "A dot needs the field's name after it, but found {}.\n\
                                 A record's fields have names: address.city.",
                                describe(&field.kind)
                            ),
                        )
                        .suggest("address.city"));
                    };
                    self.advance();
                    steps.push(Access::Field {
                        name,
                        span: field.span(),
                    });
                }
                TokenKind::At => {
                    let at = self.advance().clone();
                    if !starts_value(&self.peek().kind) {
                        return Err(Diagnostic::new(
                            at.span(),
                            format!(
                                "at needs a position after it, but found {}.\n\
                                 Lists count from 1: hobbies at 1 is the first hobby.",
                                describe(&self.peek().kind)
                            ),
                        )
                        .suggest("hobbies at 1"));
                    }
                    let index = self.sum()?;
                    steps.push(Access::At {
                        index,
                        span: at.span(),
                    });
                }
                _ => return Ok(steps),
            }
        }
    }

    /// Checks that `NAME =` follows `var` or `set`, without consuming it.
    fn assignment_target(
        &self,
        keyword: &Token,
        word: &str,
        example_name: &str,
        example_value: &str,
    ) -> Result<(String, Span), Diagnostic> {
        let purpose = if word == "var" {
            "var creates a value that can change later"
        } else {
            "set changes a value that was created with var"
        };
        let TokenKind::Identifier(name) = &self.peek().kind else {
            let found = self.peek();
            let span = if self.at_end_of_line() {
                keyword.span()
            } else {
                found.span()
            };
            return Err(Diagnostic::new(
                span,
                format!(
                    "{word} needs a name, then =, then a value, but found {}.\n\
                     {purpose}.",
                    describe(&found.kind)
                ),
            )
            .suggest(format!("{word} {example_name} = {example_value}")));
        };
        let after = self.peek_at(1);
        if after.kind != TokenKind::Equals {
            let value = if word == "set" {
                format!("{name} + 1")
            } else {
                example_value.to_string()
            };
            return Err(Diagnostic::new(
                self.peek().span(),
                format!(
                    "{word} {name} needs = and a value after it.\n\
                     {purpose}."
                ),
            )
            .suggest(format!("{word} {name} = {value}")));
        }
        Ok((name.clone(), self.peek().span()))
    }

    fn when_statement(&mut self) -> Result<Statement, Diagnostic> {
        let when = self.advance().clone();
        // Lined up under "Did you mean:  when", with the cases indented.
        let example = "when\n                 count > 10 -> say \"big\"\n                 else -> say \"small\"";
        if !self.at_end_of_line() {
            return self.match_maybe(when.span());
        }
        while self.peek().kind == TokenKind::Newline {
            self.advance();
        }
        if self.peek().kind != TokenKind::Indent {
            return Err(Diagnostic::new(
                when.span(),
                "This when has no cases.\n\
                 Its cases go on the lines below it, indented further than the when.",
            )
            .suggest(example));
        }
        self.advance(); // Indent

        let mut arms: Vec<Arm> = Vec::new();
        loop {
            match self.peek().kind {
                TokenKind::Newline => {
                    self.advance();
                    continue;
                }
                TokenKind::Dedent => {
                    self.advance();
                    break;
                }
                TokenKind::EndOfFile => break,
                _ => {}
            }
            let arm = self.arm()?;
            if let Some(last) = arms.last() {
                if last.condition.is_none() {
                    return Err(Diagnostic::new(
                        arm.span,
                        "This case comes after else, so it can never run.\n\
                         else catches everything the cases above it didn't, so it must be last.",
                    ));
                }
            }
            arms.push(arm);
        }
        Ok(Statement::When {
            arms,
            span: when.span(),
        })
    }

    /// `for name in start..end`, then the indented lines to repeat.
    fn for_statement(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.advance();
        let suggest = "for i in 1..3\n  say i";
        let name_token = self.peek().clone();
        let name = match &name_token.kind {
            TokenKind::Identifier(name) => name.clone(),
            _ => {
                return Err(Diagnostic::new(
                    name_token.span(),
                    "for needs a name after it, then in, then a range.\n\
                     The name holds the number for this pass: for i in 1..3.",
                )
                .suggest(suggest));
            }
        };
        self.advance();
        if self.peek().kind != TokenKind::In {
            let found = self.peek();
            return Err(Diagnostic::new(
                found.span(),
                format!(
                    "for needs in after {name}, but found {}.\n\
                     Write the name, then in, then the first number, .., and the last.",
                    describe(&found.kind)
                ),
            )
            .suggest(format!("for {name} in 1..3")));
        }
        self.advance();
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                keyword.span(),
                "for needs a range after in: the first number, .., then the last.\n\
                 for i in 1..3 counts 1, then 2, then 3.",
            )
            .suggest(suggest));
        }
        let start = self.expression()?;
        let end = if self.peek().kind == TokenKind::DotDot {
            let dots = self.advance();
            if self.at_end_of_line() {
                return Err(Diagnostic::new(
                    dots.span(),
                    "This range is missing the last number, after ..\n\
                     for i in 1..3 counts from 1 through 3, including both ends.",
                )
                .suggest(format!("for {name} in {start}..{start}")));
            }
            Some(self.expression()?)
        } else if self.at_end_of_line() {
            None
        } else {
            let found = self.peek();
            return Err(Diagnostic::new(
                found.span(),
                format!(
                    "A for range needs .. between the first number and the last, but found {}.\n\
                     for i in 1..3 counts from 1 through 3. for hobby in hobbies walks a list.",
                    describe(&found.kind)
                ),
            )
            .suggest(format!("for {name} in {start}..{start}")));
        };
        let end_shown = match &end {
            Some(end) => format!("{start}..{end}"),
            None => start.to_string(),
        };
        if !self.at_end_of_line() {
            let found = self.peek();
            return Err(Diagnostic::new(
                found.span(),
                "The lines a for repeats go below it, indented, one statement per line.\n\
                 Finish the range, then put the work on the next lines.",
            )
            .suggest(format!("for {name} in {end_shown}\n  say {name}")));
        }
        while self.peek().kind == TokenKind::Newline {
            self.advance();
        }
        if self.peek().kind != TokenKind::Indent {
            return Err(Diagnostic::new(
                keyword.span(),
                "This for has nothing to repeat.\n\
                 The lines it repeats go below it, indented further than the for.",
            )
            .suggest(format!("for {name} in {end_shown}\n  say {name}")));
        }
        self.advance();

        let mut body = Vec::new();
        loop {
            match &self.peek().kind {
                TokenKind::Newline => {
                    self.advance();
                    continue;
                }
                TokenKind::Dedent => {
                    self.advance();
                    break;
                }
                TokenKind::EndOfFile => break,
                TokenKind::Var => {
                    let var = self.advance().clone();
                    return Err(self.value_created_in_loop(&var));
                }
                TokenKind::Identifier(_) if self.peek_at(1).kind == TokenKind::Equals => {
                    let found = self.peek().clone();
                    return Err(self.value_created_in_loop(&found));
                }
                _ => body.push(self.full_statement()?),
            }
        }
        if body.is_empty() {
            return Err(Diagnostic::new(
                keyword.span(),
                "This for has nothing to repeat.\n\
                 The lines it repeats go below it, indented further than the for.",
            )
            .suggest(format!("for {name} in {end_shown}\n  say {name}")));
        }
        Ok(Statement::For {
            name,
            name_span: name_token.span(),
            start,
            end,
            body,
            span: keyword.span(),
        })
    }

    /// `fn name(n: number) -> number`, then the indented lines.
    fn function_statement(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.advance().clone();
        if self.in_function {
            return Err(Diagnostic::new(
                keyword.span(),
                "A function can't contain another function.\n\
                 Write each function on its own, at the left edge.",
            ));
        }
        let name_token = self.peek().clone();
        let TokenKind::Identifier(name) = name_token.kind.clone() else {
            return Err(Diagnostic::new(
                name_token.span(),
                "fn needs a name after it, then the parameters in parentheses.\n\
                 fn double(n: number) -> number doubles a number.",
            )
            .suggest("fn double(n: number) -> number\n  return n * 2"));
        };
        self.advance();
        if self.peek().kind != TokenKind::LeftParen {
            return Err(Diagnostic::new(
                self.peek().span(),
                format!(
                    "fn {name} needs parentheses after the name, even when it takes nothing.\n\
                     The parameters go inside: fn {name}(n: number)."
                ),
            )
            .suggest(format!("fn {name}(n: number) -> number")));
        }
        self.advance();
        let mut params = Vec::new();
        if self.peek().kind != TokenKind::RightParen {
            loop {
                params.push(self.parameter()?);
                if self.peek().kind == TokenKind::Comma {
                    let comma = self.advance().clone();
                    if self.peek().kind == TokenKind::RightParen {
                        return Err(Diagnostic::new(
                            comma.span(),
                            "This comma has no parameter after it.\n\
                             Write the next parameter, or remove the comma.",
                        ));
                    }
                    continue;
                }
                break;
            }
        }
        if self.peek().kind != TokenKind::RightParen {
            return Err(Diagnostic::new(
                self.peek().span(),
                format!(
                    "fn {name} needs ) after its parameters, but found {}.",
                    describe(&self.peek().kind)
                ),
            ));
        }
        self.advance();
        let mut names = Vec::new();
        for param in &params {
            if param.name == name {
                return Err(Diagnostic::new(
                    param.name_span,
                    format!(
                        "The parameter {name} has the same name as the function.\n\
                         Give the parameter its own name."
                    ),
                ));
            }
            if names.contains(&param.name) {
                return Err(Diagnostic::new(
                    param.name_span,
                    format!(
                        "{name} takes {} twice.\n\
                         Each parameter needs its own name.",
                        param.name
                    ),
                ));
            }
            names.push(param.name.clone());
        }
        let result = if self.peek().kind == TokenKind::Arrow {
            self.advance();
            if self.at_end_of_line() {
                return Err(Diagnostic::new(
                    self.peek().span(),
                    format!(
                        "fn {name} needs the kind it gives back, after ->.\n\
                         A kind is text, number, true/false, or list of one of those."
                    ),
                )
                .suggest(format!("fn {name}() -> number")));
            }
            let kind = self.signature_kind()?;
            if self.peek().kind == TokenKind::Where {
                return Err(Diagnostic::new(
                    self.peek().span(),
                    format!(
                        "where goes on a parameter, not on the kind {name} gives back.\n\
                         To promise something about the value it gives back, write ensure \
                         under the fn line."
                    ),
                )
                .suggest("ensure result >= 0"));
            }
            Some(kind)
        } else {
            None
        };
        if !self.at_end_of_line() {
            return Err(Diagnostic::new(
                self.peek().span(),
                "The lines of a function go below it, indented, one statement per line.\n\
                 Finish the first line, then put the work underneath.",
            )
            .suggest(format!("fn {name}() -> number\n  return 1")));
        }
        while self.peek().kind == TokenKind::Newline {
            self.advance();
        }
        if self.peek().kind != TokenKind::Indent {
            return Err(Diagnostic::new(
                keyword.span(),
                format!(
                    "fn {name} has nothing to do.\n\
                     The lines it runs go below it, indented further than the fn."
                ),
            )
            .suggest(format!("fn {name}() -> number\n  return 1")));
        }
        self.advance();
        // `fails`, `require` and `ensure` lines come first, in any order.
        let mut fails = Vec::new();
        let mut contracts = Vec::new();
        loop {
            match self.peek().kind {
                TokenKind::Fails if fails.is_empty() => fails = self.failure_list()?,
                TokenKind::Fails => {
                    return Err(Diagnostic::new(
                        self.peek().span(),
                        format!(
                            "fn {name} already has a fails line.\n\
                             Name every failure on one line, separated by commas."
                        ),
                    )
                    .suggest("fails Declined, NetworkDown"));
                }
                TokenKind::Require | TokenKind::Ensure => contracts.push(self.contract()?),
                TokenKind::Newline => {
                    self.advance();
                }
                _ => break,
            }
        }
        self.in_function = true;
        let mut body = Vec::new();
        let body_result = self.function_body(&mut body);
        self.in_function = false;
        body_result?;
        if body.is_empty() {
            return Err(Diagnostic::new(
                keyword.span(),
                format!(
                    "fn {name} has nothing to do.\n\
                     The lines it runs go below it, indented further than the fn."
                ),
            )
            .suggest(format!("fn {name}() -> number\n  return 1")));
        }
        Ok(Statement::Function {
            name,
            name_span: name_token.span(),
            params,
            result,
            fails,
            contracts,
            body,
            span: keyword.span(),
        })
    }

    /// `require CONDITION` or `ensure CONDITION`, at the top of a function.
    fn contract(&mut self) -> Result<Contract, Diagnostic> {
        let keyword = self.advance().clone();
        let ensure = keyword.kind == TokenKind::Ensure;
        let word = if ensure { "ensure" } else { "require" };
        if self.at_end_of_line() {
            let (what, example) = if ensure {
                (
                    "what is true when the function gives back",
                    "ensure result >= 0",
                )
            } else {
                (
                    "what has to be true when the function is called",
                    "require amount > 0",
                )
            };
            return Err(Diagnostic::new(
                keyword.span(),
                format!("{word} needs a condition: {what}."),
            )
            .suggest(example));
        }
        let condition = self.expression()?;
        let span = cover(
            keyword.span(),
            self.tokens[self.pos.saturating_sub(1)].span(),
        );
        if !self.at_end_of_line() {
            return Err(Diagnostic::new(
                self.peek().span(),
                format!(
                    "Quill expected the line to end after `{word} {condition}`, but found {}.\n\
                     Each {word} line holds one condition. Join two with and.",
                    describe(&self.peek().kind)
                ),
            ));
        }
        Ok(Contract {
            ensure,
            condition,
            span,
        })
    }

    fn function_body(&mut self, body: &mut Vec<Statement>) -> Result<(), Diagnostic> {
        loop {
            match &self.peek().kind {
                TokenKind::Newline => {
                    self.advance();
                }
                TokenKind::Dedent => {
                    self.advance();
                    break;
                }
                TokenKind::EndOfFile => break,
                _ => body.push(self.full_statement()?),
            }
        }
        Ok(())
    }

    /// `n: number` inside a function's parentheses.
    fn parameter(&mut self) -> Result<Param, Diagnostic> {
        let name_token = self.peek().clone();
        let TokenKind::Identifier(name) = name_token.kind.clone() else {
            return Err(Diagnostic::new(
                name_token.span(),
                "A parameter is a name, then :, then its kind.\n\
                 fn double(n: number) -> number takes one number.",
            )
            .suggest("n: number"));
        };
        self.advance();
        if self.peek().kind != TokenKind::Colon {
            return Err(Diagnostic::new(
                name_token.span(),
                format!(
                    "The parameter {name} needs a kind after it.\n\
                     Write {name}: text, {name}: number, {name}: true/false, \
                     or {name}: list of one of those."
                ),
            )
            .suggest(format!("{name}: number")));
        }
        self.advance();
        let kind_span_start = self.peek().span();
        let kind = self.signature_kind()?;
        let kind_span = cover(
            kind_span_start,
            self.tokens[self.pos.saturating_sub(1)].span(),
        );
        let rule = if self.peek().kind == TokenKind::Where {
            Some(self.rule(&name)?)
        } else {
            None
        };
        Ok(Param {
            name,
            name_span: name_token.span(),
            kind,
            kind_span,
            rule,
        })
    }

    /// `where 1..500` after a parameter's kind.
    fn rule(&mut self, name: &str) -> Result<Rule, Diagnostic> {
        let keyword = self.advance().clone();
        let example = format!("{name}: number where 1..500");
        let why = "where gives the smallest and the largest value, with .. between them.";
        if matches!(
            self.peek().kind,
            TokenKind::Comma | TokenKind::RightParen | TokenKind::Newline
        ) {
            return Err(Diagnostic::new(
                keyword.span(),
                format!("where needs a range after it.\n{why}"),
            )
            .suggest(example));
        }
        let low = self.rule_value()?;
        if self.peek().kind != TokenKind::DotDot {
            let found = self.peek();
            return Err(Diagnostic::new(
                found.span(),
                format!(
                    "This range needs .. after {low}, but found {}.\n{why}",
                    describe(&found.kind)
                ),
            )
            .suggest(example));
        }
        let dots = self.advance().clone();
        if matches!(
            self.peek().kind,
            TokenKind::Comma | TokenKind::RightParen | TokenKind::Newline
        ) {
            return Err(Diagnostic::new(
                dots.span(),
                format!(
                    "This range is missing the largest value, after ..\n\
                     where {low}..500 takes {low} through 500, including both ends."
                ),
            )
            .suggest(example));
        }
        let high = self.rule_value()?;
        let span = cover(
            keyword.span(),
            self.tokens[self.pos.saturating_sub(1)].span(),
        );
        Ok(Rule { low, high, span })
    }

    /// One end of a `where` range: a written number or amount of money.
    fn rule_value(&mut self) -> Result<Expr, Diagnostic> {
        let value = self.unary()?;
        if value.written_amount().is_none() {
            return Err(Diagnostic::new(
                value.span,
                format!(
                    "Each end of a where range is a written value, like 1 or 0 USD, \
                     but {value} has to be worked out.\n\
                     For a rule that reads other values, write require under the fn line."
                ),
            ));
        }
        Ok(value)
    }

    /// `text`, `number`, `true/false`, or `list of` one of those.
    fn signature_kind(&mut self) -> Result<SigKind, Diagnostic> {
        let token = self.peek().clone();
        let kind = match &token.kind {
            TokenKind::Identifier(name) if name == "text" => {
                self.advance();
                SigKind::Text
            }
            TokenKind::Identifier(name) if name == "number" => {
                self.advance();
                SigKind::Number
            }
            TokenKind::True
                if self.peek_at(1).kind == TokenKind::Slash
                    && matches!(self.peek_at(2).kind, TokenKind::False) =>
            {
                self.advance();
                self.advance();
                self.advance();
                SigKind::Bool
            }
            TokenKind::List => {
                self.advance();
                if self.peek().kind != TokenKind::Of {
                    return Err(Diagnostic::new(
                        token.span(),
                        "A list parameter names the kind of its items.\n\
                         Write list of text, list of number, list of true/false, or list of USD.",
                    )
                    .suggest("list of number"));
                }
                self.advance();
                SigKind::List(Box::new(self.signature_kind()?))
            }
            TokenKind::Maybe => {
                self.advance();
                if self.at_end_of_line() {
                    return Err(Diagnostic::new(
                        token.span(),
                        "maybe needs the kind that might be missing.\n\
                         Write maybe number, maybe text, maybe true/false, or maybe USD.",
                    )
                    .suggest("maybe number"));
                }
                SigKind::Maybe(Box::new(self.signature_kind()?))
            }
            TokenKind::Identifier(name) if is_currency(name) => {
                let code = name.clone();
                self.advance();
                SigKind::Money(code)
            }
            TokenKind::Identifier(name) if currency_letters(name) => {
                let code = name.to_uppercase();
                return Err(Diagnostic::new(
                    token.span(),
                    format!(
                        "A currency is written in capitals, like {code}.\n\
                         {name} is the same three letters, in capitals."
                    ),
                )
                .suggest(code));
            }
            _ => {
                return Err(Diagnostic::new(
                    token.span(),
                    format!(
                        "A function's kind is text, number, true/false, maybe one of those, \
                         a currency like USD, or a list of those, but found {}.\n\
                         Int and Text arrive later, with types. Today, write number or text.",
                        describe(&token.kind)
                    ),
                )
                .suggest("number"));
            }
        };
        Ok(kind)
    }

    /// `return value`, or `return` on its own.
    fn return_statement(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.advance().clone();
        if !self.in_function {
            return Err(Diagnostic::new(
                keyword.span(),
                "return only goes inside a function, to give a value back.\n\
                 On its own line, use say to show a value.",
            )
            .suggest("say \"Hello\""));
        }
        let value = if self.at_end_of_line() {
            None
        } else {
            Some(self.expression()?)
        };
        Ok(Statement::Return {
            value,
            span: keyword.span(),
        })
    }

    /// `greet("Amina")` as a whole line.
    fn call_statement(&mut self, name: String) -> Result<Statement, Diagnostic> {
        let name_token = self.advance().clone();
        let (args, end) = self.arguments()?;
        if !self.at_end_of_line() {
            return Err(Diagnostic::new(
                self.peek().span(),
                format!(
                    "This line calls {name} and then keeps going.\n\
                     A call that gives a value back belongs inside say, check, or =. \
                     A call that does work stands on its own line."
                ),
            )
            .suggest(format!("say {name}()")));
        }
        Ok(Statement::Call {
            name,
            name_span: name_token.span(),
            args,
            span: cover(name_token.span(), end),
            handlers: Vec::new(),
        })
    }

    /// The values inside `(` and `)`, and the span of the closing parenthesis.
    /// The current token is `(`.
    fn arguments(&mut self) -> Result<(Vec<Expr>, Span), Diagnostic> {
        let open = self.peek().clone();
        if open.kind != TokenKind::LeftParen {
            return Err(Diagnostic::new(
                open.span(),
                "A call needs ( after the function's name.\n\
                 double(4) calls double with 4.",
            )
            .suggest("double(4)"));
        }
        self.advance();
        if self.peek().kind == TokenKind::RightParen {
            let close = self.advance().clone();
            return Ok((Vec::new(), close.span()));
        }
        let mut args = Vec::new();
        loop {
            if !starts_value(&self.peek().kind) {
                return Err(Diagnostic::new(
                    self.peek().span(),
                    format!(
                        "A call needs a value here, but found {}.\n\
                         double(4) calls double with 4.",
                        describe(&self.peek().kind)
                    ),
                )
                .suggest("double(4)"));
            }
            args.push(self.expression()?);
            if self.peek().kind == TokenKind::Comma {
                let comma = self.advance().clone();
                if self.peek().kind == TokenKind::RightParen {
                    return Err(Diagnostic::new(
                        comma.span(),
                        "This comma has no value after it.\n\
                         Write the next value, or remove the comma.",
                    ));
                }
                continue;
            }
            if self.peek().kind == TokenKind::RightParen {
                let close = self.advance().clone();
                return Ok((args, close.span()));
            }
            return Err(Diagnostic::new(
                self.peek().span(),
                format!(
                    "A call needs ) after its values, but found {}.\n\
                     double(4) calls double with 4.",
                    describe(&self.peek().kind)
                ),
            ));
        }
    }

    /// `name = value` or `var name = value` inside a `for`, which isn't allowed.
    fn value_created_in_loop(&mut self, start: &Token) -> Diagnostic {
        let name = match &self.peek().kind {
            TokenKind::Identifier(name) => name.clone(),
            _ => "name".to_string(),
        };
        self.advance();
        self.advance();
        let value = match self.expression() {
            Ok(value) => value.to_string(),
            Err(_) => "...".to_string(),
        };
        Diagnostic::new(
            start.span(),
            format!(
                "A for loop can't create a new value.\n\
                 {name} would be created again on every pass. Create it before the for \
                 with var {name} = ..., then change it here with set."
            ),
        )
        .suggest(format!("set {name} = {value}"))
    }

    fn arm(&mut self) -> Result<Arm, Diagnostic> {
        let (condition, span, written) = if self.peek().kind == TokenKind::Else {
            let token = self.advance();
            (None, token.span(), "else".to_string())
        } else {
            let condition = self.expression()?;
            let span = condition.span;
            let written = condition.to_string();
            (Some(condition), span, written)
        };

        if self.peek().kind != TokenKind::Arrow {
            let found = self.peek();
            let message = if self.at_end_of_line() {
                format!(
                    "This case of the when needs -> after {written}, then what to do.\n\
                     Each case is a condition, then ->, then a statement on the same line."
                )
            } else {
                format!(
                    "This case of the when needs -> after {written}, but found {}.\n\
                     Each case is a condition, then ->, then a statement on the same line.",
                    describe(&found.kind)
                )
            };
            let at = if self.at_end_of_line() {
                span
            } else {
                found.span()
            };
            return Err(Diagnostic::new(at, message).suggest(format!("{written} -> say \"...\"")));
        }
        let arrow = self.advance();
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                arrow.span(),
                "-> needs something to do after it, on the same line.\n\
                 Write the statement that runs when this case is chosen.",
            )
            .suggest(format!("{written} -> say \"...\"")));
        }

        let body = match &self.peek().kind {
            TokenKind::Var => {
                let var = self.advance().clone();
                return Err(self.value_created_in_case(&var));
            }
            TokenKind::Identifier(_) if self.peek_at(1).kind == TokenKind::Equals => {
                let name = self.peek().clone();
                return Err(self.value_created_in_case(&name));
            }
            _ => self.full_statement()?,
        };
        Ok(Arm {
            condition,
            body: Box::new(body),
            span,
        })
    }

    /// `when` in the place of a value, with indented cases that each give one.
    fn when_value(&mut self) -> Result<Expr, Diagnostic> {
        let when = self.advance().clone();
        let example = "label = when\n                 score >= 50 -> \"Pass\"\n                 else -> \"Fail\"";
        if !self.at_end_of_line() {
            return Err(Diagnostic::new(
                when.span(),
                "A when that gives a value has its cases on the lines below it.\n\
                 Each case is a condition, then ->, then the value. else comes last.",
            )
            .suggest(example));
        }
        while self.peek().kind == TokenKind::Newline {
            self.advance();
        }
        if self.peek().kind != TokenKind::Indent {
            return Err(Diagnostic::new(
                when.span(),
                "This when has no cases.\n\
                 Its cases go on the lines below it, indented further than the line with when.",
            )
            .suggest(example));
        }
        self.advance();
        let mut arms: Vec<ValueArm> = Vec::new();
        loop {
            match self.peek().kind {
                TokenKind::Newline => {
                    self.advance();
                    continue;
                }
                TokenKind::Dedent => {
                    self.advance();
                    break;
                }
                TokenKind::EndOfFile => break,
                _ => {}
            }
            let arm = self.value_arm()?;
            if arms.last().is_some_and(|last| last.condition.is_none()) {
                return Err(Diagnostic::new(
                    arm.span,
                    "This case comes after else, so it can never be chosen.\n\
                     else catches everything the cases above it didn't, so it must be last.",
                ));
            }
            arms.push(arm);
        }
        if arms.last().is_some_and(|last| last.condition.is_some()) {
            return Err(Diagnostic::new(
                when.span(),
                "A when that gives a value needs else as its last case.\n\
                 Without else, no case might be true, and there would be no value.",
            )
            .suggest("else -> \"...\""));
        }
        let span = when.span();
        Ok(Expr {
            kind: ExprKind::When(arms),
            span,
        })
    }

    /// `condition -> value` or `else -> value`, in a when that gives a value.
    fn value_arm(&mut self) -> Result<ValueArm, Diagnostic> {
        let (condition, span, written) = if self.peek().kind == TokenKind::Else {
            let token = self.advance();
            (None, token.span(), "else".to_string())
        } else {
            let condition = self.expression()?;
            let span = condition.span;
            let written = condition.to_string();
            (Some(condition), span, written)
        };
        if self.peek().kind != TokenKind::Arrow {
            let at = if self.at_end_of_line() {
                span
            } else {
                self.peek().span()
            };
            return Err(Diagnostic::new(
                at,
                format!(
                    "This case needs -> after {written}, then the value it gives.\n\
                     Each case is a condition, then ->, then a value on the same line."
                ),
            )
            .suggest(format!("{written} -> \"...\"")));
        }
        let arrow = self.advance();
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                arrow.span(),
                "-> needs the value this case gives, on the same line.",
            )
            .suggest(format!("{written} -> \"...\"")));
        }
        if starts_statement(&self.peek().kind) {
            let word = describe(&self.peek().kind);
            return Err(Diagnostic::new(
                self.peek().span(),
                format!(
                    "This when gives a value, so each case gives a value after ->, not {word}.\n\
                     To run a statement in each case instead, put when on its own line."
                ),
            )
            .suggest(format!("{written} -> \"...\"")));
        }
        let value = self.expression()?;
        if !self.at_end_of_line() {
            return Err(Diagnostic::new(
                self.peek().span(),
                format!(
                    "Quill expected the case to end after `{written} -> {value}`, but found {}.\n\
                     Each case gives one value.",
                    describe(&self.peek().kind)
                ),
            ));
        }
        Ok(ValueArm {
            condition,
            value,
            span,
        })
    }

    /// `name = value` or `var name = value` after `->`, which isn't allowed.
    fn value_created_in_case(&mut self, start: &Token) -> Diagnostic {
        let name = match &self.peek().kind {
            TokenKind::Identifier(name) => name.clone(),
            _ => "name".to_string(),
        };
        // Skip `name =` to read the value for the suggestion.
        self.advance();
        self.advance();
        let value = match self.expression() {
            Ok(value) => value.to_string(),
            Err(_) => "...".to_string(),
        };
        Diagnostic::new(
            start.span(),
            format!(
                "A case of a when can't create a new value.\n\
                 {name} would only exist when this case runs. Create it before the when \
                 with var {name} = ..., then change it here with set."
            ),
        )
        .suggest(format!("set {name} = {value}"))
    }

    fn expression(&mut self) -> Result<Expr, Diagnostic> {
        let value = self.or()?;
        self.filters(value)
    }

    /// `students where score >= 50`, and any further `where` after it.
    fn filters(&mut self, mut list: Expr) -> Result<Expr, Diagnostic> {
        while self.peek().kind == TokenKind::Where {
            let keyword = self.advance().clone();
            if !starts_value(&self.peek().kind) {
                return Err(Diagnostic::new(
                    keyword.span(),
                    format!(
                        "where needs a condition after it.\n\
                         {list} where score >= 50 keeps the records whose score is 50 or more."
                    ),
                )
                .suggest(format!("{list} where score >= 50")));
            }
            let condition = self.or()?;
            list = Expr {
                span: cover(list.span, condition.span),
                kind: ExprKind::Where {
                    list: Box::new(list),
                    condition: Box::new(condition),
                    where_span: keyword.span(),
                },
            };
        }
        Ok(list)
    }

    fn or(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.and()?;
        while self.peek().kind == TokenKind::Or {
            let op = self.advance();
            self.expect_operand(op)?;
            let right = self.and()?;
            left = logic(Logic::Or, left, right, op.span());
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.not()?;
        while self.peek().kind == TokenKind::And {
            let op = self.advance();
            self.expect_operand(op)?;
            let right = self.not()?;
            left = logic(Logic::And, left, right, op.span());
        }
        Ok(left)
    }

    fn not(&mut self) -> Result<Expr, Diagnostic> {
        if self.peek().kind != TokenKind::Not {
            return self.comparison();
        }
        let not = self.advance();
        self.expect_operand(not)?;
        let inner = self.not()?;
        Ok(Expr {
            span: cover(not.span(), inner.span),
            kind: ExprKind::Not(Box::new(inner)),
        })
    }

    fn comparison(&mut self) -> Result<Expr, Diagnostic> {
        let left = self.sum()?;
        let Some(op) = Comparison::from_token(&self.peek().kind) else {
            return Ok(left);
        };
        let op_token = self.advance();
        self.expect_operand(op_token)?;
        let right = self.sum()?;

        if let Some(second) = Comparison::from_token(&self.peek().kind) {
            let second_token = self.advance();
            let suggestion = match self.sum() {
                Ok(third) => Some(format!(
                    "{left} {} {right} and {right} {} {third}",
                    op.symbol(),
                    second.symbol()
                )),
                Err(_) => None,
            };
            let problem = Diagnostic::new(
                second_token.span(),
                "Quill compares two values at a time.\n\
                 To check more than one thing, join the comparisons with and.",
            );
            return Err(match suggestion {
                Some(suggestion) => problem.suggest(suggestion),
                None => problem,
            });
        }

        Ok(Expr {
            span: cover(left.span, right.span),
            kind: ExprKind::Compare {
                op,
                left: Box::new(left),
                right: Box::new(right),
                op_span: op_token.span(),
            },
        })
    }

    fn sum(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.product()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Plus => Arithmetic::Add,
                TokenKind::Minus => Arithmetic::Subtract,
                _ => return Ok(left),
            };
            let op_token = self.advance();
            self.expect_operand(op_token)?;
            let right = self.product()?;
            left = arithmetic(op, left, right, op_token.span());
        }
    }

    fn product(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Star => Arithmetic::Multiply,
                TokenKind::Slash => Arithmetic::Divide,
                TokenKind::Percent => Arithmetic::Remainder,
                _ => return Ok(left),
            };
            let op_token = self.advance();
            self.expect_operand(op_token)?;
            let right = self.unary()?;
            left = arithmetic(op, left, right, op_token.span());
        }
    }

    fn unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.peek().kind != TokenKind::Minus {
            return self.unary_value();
        }
        let minus = self.advance();
        self.expect_operand(minus)?;
        let inner = self.unary()?;
        Ok(Expr {
            span: cover(minus.span(), inner.span),
            kind: ExprKind::Negate(Box::new(inner)),
        })
    }

    /// A value, including `.field` and `at`, but not a leading minus.
    fn unary_value(&mut self) -> Result<Expr, Diagnostic> {
        let expr = self.primary()?;
        self.postfix(expr)
    }

    /// `.field` and `at position` after a value.
    fn postfix(&mut self, mut expr: Expr) -> Result<Expr, Diagnostic> {
        loop {
            match self.peek().kind {
                TokenKind::LeftParen => {
                    let ExprKind::Name(name) = expr.kind.clone() else {
                        return Err(Diagnostic::new(
                            self.peek().span(),
                            "A call starts with the function's name, then (.\n\
                             double(4) calls double with 4.",
                        )
                        .suggest("double(4)"));
                    };
                    let name_span = expr.span;
                    let (args, end) = self.arguments()?;
                    expr = Expr {
                        span: cover(name_span, end),
                        kind: ExprKind::Call {
                            name,
                            name_span,
                            args,
                        },
                    };
                }
                TokenKind::Dot => {
                    let dot = self.advance().clone();
                    let field = self.peek().clone();
                    let TokenKind::Identifier(name) = field.kind.clone() else {
                        return Err(Diagnostic::new(
                            dot.span(),
                            format!(
                                "A dot needs the field's name after it, but found {}.\n\
                                 A record's fields have names: address.city.",
                                describe(&field.kind)
                            ),
                        )
                        .suggest("address.city"));
                    };
                    self.advance();
                    expr = Expr {
                        span: cover(expr.span, field.span()),
                        kind: ExprKind::Field {
                            record: Box::new(expr),
                            name,
                            name_span: field.span(),
                        },
                    };
                }
                TokenKind::At => {
                    let at = self.advance().clone();
                    if !starts_value(&self.peek().kind) {
                        return Err(Diagnostic::new(
                            at.span(),
                            format!(
                                "at needs a position after it, but found {}.\n\
                                 Lists count from 1: hobbies at 1 is the first hobby.",
                                describe(&self.peek().kind)
                            ),
                        )
                        .suggest("hobbies at 1"));
                    }
                    let index = self.sum()?;
                    expr = Expr {
                        span: cover(expr.span, index.span),
                        kind: ExprKind::At {
                            list: Box::new(expr),
                            index: Box::new(index),
                            at_span: at.span(),
                        },
                    };
                }
                _ => return Ok(expr),
            }
        }
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek();
        let kind = match &token.kind {
            TokenKind::Text(text) => ExprKind::Text(text.clone()),
            TokenKind::Template(pieces) => ExprKind::Template(template(pieces)?),
            TokenKind::Number(_) => return self.number_or_money(),
            TokenKind::Found => return self.found_expr(),
            TokenKind::Missing => {
                let span = token.span();
                self.advance();
                return Ok(Expr {
                    kind: ExprKind::Missing,
                    span,
                });
            }
            TokenKind::Try => return self.try_expr(),
            TokenKind::True => ExprKind::Bool(true),
            TokenKind::False => ExprKind::Bool(false),
            TokenKind::Identifier(name) => ExprKind::Name(name.clone()),
            TokenKind::List => return self.list_expr(),
            TokenKind::Record => return self.record_expr(),
            TokenKind::When => return self.when_value(),
            TokenKind::Get | TokenKind::Sum => return self.ask_expr(),
            TokenKind::LeftParen => return self.group(),
            TokenKind::Of => {
                return Err(Diagnostic::new(
                    token.span(),
                    "of names the kind of an empty list.\n\
                     It follows list: list of text, list of number, or list of true/false.",
                )
                .suggest("list of text"))
            }
            TokenKind::At => {
                return Err(Diagnostic::new(
                    token.span(),
                    "at picks an item out of a list, so the list comes first.\n\
                     Lists count from 1: hobbies at 1 is the first hobby.",
                )
                .suggest("hobbies at 1"))
            }
            TokenKind::Say => {
                return Err(Diagnostic::new(
                    token.span(),
                    "say can't be used as a value.\n\
                     It shows a value, so it has to start its own line.",
                ))
            }
            TokenKind::Check => {
                return Err(Diagnostic::new(
                    token.span(),
                    "check can't be used as a value.\n\
                     It compares two values, so it has to start its own line.",
                )
                .suggest("check name == \"Hello\""))
            }
            TokenKind::Example => {
                return Err(Diagnostic::new(
                    token.span(),
                    "An example can't be used as a value.\n\
                     It compares two values, so it stays on its own line.",
                ))
            }
            other => {
                return Err(Diagnostic::new(
                    token.span(),
                    format!(
                        "Quill expected a value here, but found {}.\n\
                         A value can be text, a number, true, false, a name, a list, a record or a calculation.",
                        describe(other)
                    ),
                ))
            }
        };
        self.advance();
        Ok(Expr {
            kind,
            span: token.span(),
        })
    }

    /// `sum fee of roster`, `get fee of roster`, or `get bookings with acts in roster`.
    fn ask_expr(&mut self) -> Result<Expr, Diagnostic> {
        let keyword = self.advance().clone();
        let summing = keyword.kind == TokenKind::Sum;
        let field_token = self.peek().clone();
        let TokenKind::Identifier(field) = field_token.kind.clone() else {
            let message = if summing {
                "sum needs the name of the amount.\n\
                 sum fee of roster adds the fee of each record."
            } else {
                "get needs the name of the data.\n\
                 get fee of roster takes the fee of each record. \
                 get bookings with acts in roster brings the bookings and their acts."
            };
            let suggestion = if summing {
                "sum fee of roster"
            } else {
                "get fee of roster"
            };
            return Err(Diagnostic::new(field_token.span(), message).suggest(suggestion));
        };
        self.advance();
        let mut fields = vec![Asked {
            name: field.clone(),
            span: field_token.span(),
        }];
        while self.peek().kind == TokenKind::Comma {
            let comma = self.advance().clone();
            if summing {
                return Err(Diagnostic::new(
                    comma.span(),
                    "sum adds one amount.\n\
                     sum fee of roster adds the fee of each record.",
                )
                .suggest(format!("sum {field} of roster")));
            }
            let next = self.peek().clone();
            let TokenKind::Identifier(name) = next.kind.clone() else {
                return Err(Diagnostic::new(
                    comma.span(),
                    format!(
                        "This comma needs the next field, but found {}.\n\
                         get name, fee of roster takes those fields of each record.",
                        describe(&next.kind)
                    ),
                )
                .suggest("get name, fee of roster"));
            };
            self.advance();
            fields.push(Asked {
                name,
                span: next.span(),
            });
        }
        let written_fields = fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        match self.peek().kind {
            TokenKind::Of => {
                let of_token = self.advance().clone();
                if !starts_value(&self.peek().kind) {
                    return Err(Diagnostic::new(
                        self.peek().span(),
                        format!(
                            "of needs the list after it, but found {}.\n\
                             {} {written_fields} of roster reads that field of each record.",
                            describe(&self.peek().kind),
                            if summing { "sum" } else { "get" }
                        ),
                    )
                    .suggest(format!(
                        "{} {written_fields} of roster",
                        if summing { "sum" } else { "get" }
                    )));
                }
                let list = self.unary()?;
                let list = self.filters(list)?;
                let kind = if summing {
                    ExprKind::Sum {
                        field,
                        field_span: field_token.span(),
                        of_span: of_token.span(),
                        list: Box::new(list.clone()),
                    }
                } else {
                    ExprKind::Get {
                        fields,
                        of_span: of_token.span(),
                        list: Box::new(list.clone()),
                    }
                };
                Ok(Expr {
                    span: cover(keyword.span(), list.span),
                    kind,
                })
            }
            TokenKind::With => {
                if fields.len() > 1 {
                    return Err(Diagnostic::new(
                        self.peek().span(),
                        "with brings one list and the data embedded in it.\n\
                         get bookings with acts in roster. Several fields use of: \
                         get name, fee of roster.",
                    )
                    .suggest("get name, fee of roster"));
                }
                if summing {
                    return Err(Diagnostic::new(
                        self.peek().span(),
                        "sum adds one field of a list.\n\
                         sum fee of roster adds the fee of each record.",
                    )
                    .suggest(format!("sum {field} of roster")));
                }
                self.advance();
                let nested_token = self.peek().clone();
                let TokenKind::Identifier(nested) = nested_token.kind.clone() else {
                    return Err(Diagnostic::new(
                        nested_token.span(),
                        "with needs the name of the embedded data.\n\
                         get bookings with acts in roster brings each booking and its acts.",
                    )
                    .suggest("get bookings with acts in roster"));
                };
                self.advance();
                if self.peek().kind != TokenKind::In {
                    return Err(Diagnostic::new(
                        self.peek().span(),
                        format!(
                            "in names where the data is, but found {}.\n\
                             get bookings with acts in roster.",
                            describe(&self.peek().kind)
                        ),
                    )
                    .suggest(format!("get {field} with {nested} in roster")));
                }
                self.advance();
                if !starts_value(&self.peek().kind) {
                    return Err(Diagnostic::new(
                        self.peek().span(),
                        format!(
                            "in needs the data after it, but found {}.\n\
                             get bookings with acts in roster.",
                            describe(&self.peek().kind)
                        ),
                    )
                    .suggest(format!("get {field} with {nested} in roster")));
                }
                let source = self.unary()?;
                Ok(Expr {
                    span: cover(keyword.span(), source.span),
                    kind: ExprKind::GetWith {
                        field,
                        field_span: field_token.span(),
                        nested,
                        nested_span: nested_token.span(),
                        source: Box::new(source),
                    },
                })
            }
            _ => {
                let found = self.peek().clone();
                let message = if summing {
                    format!(
                        "sum {field} needs of, and then the list.\n\
                         sum {field} of roster adds that field of each record."
                    )
                } else {
                    format!(
                        "get {written_fields} needs of, or with.\n\
                         get fee of roster takes one field. get name, fee of roster takes several. \
                         get bookings with acts in roster brings the acts with the bookings."
                    )
                };
                let suggestion = if summing {
                    format!("sum {field} of roster")
                } else {
                    format!("get {field} of roster")
                };
                Err(Diagnostic::new(found.span(), message).suggest(suggestion))
            }
        }
    }

    /// `list "coding", "reading"` or `list of text`.
    fn list_expr(&mut self) -> Result<Expr, Diagnostic> {
        let keyword = self.advance().clone();
        if self.peek().kind == TokenKind::Of {
            self.advance();
            let empty = Some(self.signature_kind()?);
            return Ok(Expr {
                span: cover(
                    keyword.span(),
                    self.tokens[self.pos.saturating_sub(1)].span(),
                ),
                kind: ExprKind::List {
                    items: Vec::new(),
                    empty,
                },
            });
        }
        if !starts_value(&self.peek().kind) {
            return Err(Diagnostic::new(
                keyword.span(),
                "list needs its items after it, separated by commas.\n\
                 An empty list names its kind: list of text, list of number, or list of true/false.",
            )
            .suggest("list \"coding\", \"reading\""));
        }
        let mut items = vec![self.sum()?];
        while self.peek().kind == TokenKind::Comma {
            let comma = self.advance().clone();
            if !starts_value(&self.peek().kind) {
                return Err(Diagnostic::new(
                    comma.span(),
                    "This comma has no item after it.\n\
                     Write the next item, or remove the comma.",
                ));
            }
            items.push(self.sum()?);
        }
        let span = cover(keyword.span(), items.last().unwrap().span);
        Ok(Expr {
            span,
            kind: ExprKind::List { items, empty: None },
        })
    }

    /// An indented record, or `record city = "Anytown", street = "123 Main St"`.
    fn record_expr(&mut self) -> Result<Expr, Diagnostic> {
        let keyword = self.advance().clone();
        if self.at_end_of_line() {
            return self.indented_record(keyword);
        }
        let mut fields = vec![self.record_field()?];
        while self.peek().kind == TokenKind::Comma {
            let comma = self.advance().clone();
            if !matches!(self.peek().kind, TokenKind::Identifier(_)) {
                return Err(Diagnostic::new(
                    comma.span(),
                    "This comma has no field after it.\n\
                     A field is a name, then =, then a value.",
                ));
            }
            fields.push(self.record_field()?);
        }
        self.reject_duplicate_fields(&fields)?;
        Ok(Expr {
            span: cover(keyword.span(), fields.last().unwrap().value.span),
            kind: ExprKind::Record { fields },
        })
    }

    fn indented_record(&mut self, keyword: Token) -> Result<Expr, Diagnostic> {
        while self.peek().kind == TokenKind::Newline {
            self.advance();
        }
        if self.peek().kind != TokenKind::Indent {
            return Err(Diagnostic::new(
                keyword.span(),
                "This record has no fields.\n\
                 Put each field on the next lines, indented by 2 spaces: a name, then =, then a value.",
            )
            .suggest("record\n  city = \"Anytown\""));
        }
        self.advance();
        let mut fields = Vec::new();
        loop {
            match &self.peek().kind {
                TokenKind::Newline => {
                    self.advance();
                }
                TokenKind::Dedent => {
                    self.advance();
                    break;
                }
                TokenKind::EndOfFile => break,
                TokenKind::Identifier(_) => fields.push(self.record_field()?),
                _ => {
                    return Err(Diagnostic::new(
                        self.peek().span(),
                        format!(
                            "A record's field starts with its name, but found {}.\n\
                             Each field is a name, then =, then a value.",
                            describe(&self.peek().kind)
                        ),
                    )
                    .suggest("city = \"Anytown\""));
                }
            }
        }
        if fields.is_empty() {
            return Err(Diagnostic::new(
                keyword.span(),
                "This record has no fields.\n\
                 Put each field on the next lines, indented by 2 spaces: a name, then =, then a value.",
            )
            .suggest("record\n  city = \"Anytown\""));
        }
        self.reject_duplicate_fields(&fields)?;
        Ok(Expr {
            span: keyword.span(),
            kind: ExprKind::Record { fields },
        })
    }

    fn record_field(&mut self) -> Result<Field, Diagnostic> {
        let name_token = self.peek().clone();
        let TokenKind::Identifier(name) = name_token.kind.clone() else {
            return Err(Diagnostic::new(
                name_token.span(),
                "A field starts with its name, then =, then a value.",
            )
            .suggest("city = \"Anytown\""));
        };
        self.advance();
        if self.peek().kind != TokenKind::Equals {
            return Err(Diagnostic::new(
                name_token.span(),
                format!(
                    "The field {name} needs = and a value after it.\n\
                     A field keeps one value under that name."
                ),
            )
            .suggest(format!("{name} = \"Anytown\"")));
        }
        self.advance();
        if !starts_value(&self.peek().kind) {
            return Err(Diagnostic::new(
                name_token.span(),
                format!(
                    "{name} = needs a value after the =.\n\
                     A field keeps one value under that name."
                ),
            )
            .suggest(format!("{name} = \"Anytown\"")));
        }
        let value = self.expression()?;
        Ok(Field {
            name,
            name_span: name_token.span(),
            value,
        })
    }

    fn reject_duplicate_fields(&self, fields: &[Field]) -> Result<(), Diagnostic> {
        for (index, field) in fields.iter().enumerate() {
            if let Some(first) = fields
                .iter()
                .take(index)
                .find(|earlier| earlier.name == field.name)
            {
                return Err(Diagnostic::new(
                    field.name_span,
                    format!(
                        "This record already has a field called {}.\n\
                         A record keeps one value under each name. The first {} is on line {}.",
                        field.name, field.name, first.name_span.line
                    ),
                ));
            }
        }
        Ok(())
    }

    /// `( value )`
    fn group(&mut self) -> Result<Expr, Diagnostic> {
        let open = self.advance();
        if self.peek().kind == TokenKind::RightParen {
            let close = self.peek();
            return Err(Diagnostic::new(
                cover(open.span(), close.span()),
                "These brackets are empty.\nPut a value or a calculation between them.",
            ));
        }
        self.expect_operand(open)?;
        let inner = self.expression()?;
        if self.peek().kind != TokenKind::RightParen {
            return Err(Diagnostic::new(
                open.span(),
                format!(
                    "This ( is never closed.\n\
                     Every ( needs a ) after the value inside it, on the same line. Quill found {} instead.",
                    describe(&self.peek().kind)
                ),
            )
            .suggest(format!("({inner})")));
        }
        let close = self.advance();
        Ok(Expr {
            span: cover(open.span(), close.span()),
            kind: ExprKind::Group(Box::new(inner)),
        })
    }

    /// After an operator, makes sure a value follows it.
    fn expect_operand(&self, op: &Token) -> Result<(), Diagnostic> {
        let next = self.peek();
        if starts_value(&next.kind) {
            return Ok(());
        }
        let what = match op.kind {
            TokenKind::LeftParen => "( needs a value after it".to_string(),
            _ => format!("{} needs a value on its right", op.kind),
        };
        Err(Diagnostic::new(
            op.span(),
            format!(
                "{what}, but found {}.\n\
                 Write a value after it: text, a number, true, false, a name, a list, a record or a calculation.",
                describe(&next.kind)
            ),
        ))
    }

    /// A line that starts with a name but isn't `name = value`.
    fn stray_name(&mut self, name: &str, token: &Token) -> Diagnostic {
        let start = self.pos;
        self.advance();
        if self.at_end_of_line() {
            return Diagnostic::new(
                token.span(),
                format!(
                    "{name} is on a line by itself, so nothing happens to it.\n\
                     Use say to show it."
                ),
            )
            .suggest(format!("say {name}"));
        }

        // A name followed directly by a value: a misspelled keyword or a missing =.
        if matches!(
            self.peek().kind,
            TokenKind::Text(_)
                | TokenKind::Template(_)
                | TokenKind::Number(_)
                | TokenKind::Identifier(_)
                | TokenKind::True
                | TokenKind::False
        ) {
            let diagnostic = Diagnostic::new(
                token.span(),
                format!(
                    "Quill doesn't know what to do with {name} here.\n\
                     A line can start with say, check, var, set, when, for, or a name and = to create a value."
                ),
            );
            let keyword = closest_match(name, ["say", "set", "var", "check", "when", "for"]);
            return match (self.expression(), keyword) {
                (Ok(value), Some("say")) => diagnostic.suggest(format!("say {value}")),
                (_, Some(keyword)) => diagnostic.suggest(keyword),
                (Ok(value), None) => diagnostic.suggest(format!("{name} = {value}")),
                (Err(_), None) => diagnostic,
            };
        }

        // Otherwise the whole line is a calculation or a comparison.
        self.pos = start;
        match self.expression() {
            Ok(value) => self.bare_expression(&value),
            Err(problem) => problem,
        }
    }

    /// A value on a line by itself.
    fn bare_expression(&self, value: &Expr) -> Diagnostic {
        if matches!(value.ungrouped().kind, ExprKind::Compare { .. }) {
            return Diagnostic::new(
                value.span,
                "This comparison is on a line by itself, so nothing happens to it.\n\
                 To test it, use check. To do something only when it's true, use when.",
            )
            .suggest(format!("check {value}"));
        }
        Diagnostic::new(
            value.span,
            "This value is on a line by itself, so nothing happens to it.\n\
             Use say to show it, or give it a name to keep it.",
        )
        .suggest(format!("say {value}"))
    }

    /// `100 USD`, or a plain number when no currency follows.
    fn number_or_money(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.advance().clone();
        let TokenKind::Number(amount) = token.kind.clone() else {
            return Err(Diagnostic::new(
                token.span(),
                "Quill expected a number here.",
            ));
        };
        let next = self.peek().clone();
        if next.line == token.line {
            if let TokenKind::Identifier(name) = &next.kind {
                if is_currency(name) {
                    let currency = name.clone();
                    self.advance();
                    return Ok(Expr {
                        span: cover(token.span(), next.span()),
                        kind: ExprKind::Money { amount, currency },
                    });
                }
                if currency_letters(name) {
                    let code = name.to_uppercase();
                    return Err(Diagnostic::new(
                        next.span(),
                        format!(
                            "A currency is written in capitals, like {code}.\n\
                             {amount} {name} writes the amount, then the currency."
                        ),
                    )
                    .suggest(format!("{amount} {code}")));
                }
            }
        }
        Ok(Expr {
            kind: ExprKind::Number(amount),
            span: token.span(),
        })
    }

    /// `found 2.50`.
    fn found_expr(&mut self) -> Result<Expr, Diagnostic> {
        let keyword = self.advance().clone();
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                keyword.span(),
                "found needs the value that was found.\n\
                 found 2.50 holds a number. missing holds nothing.",
            )
            .suggest("found 2.50"));
        }
        let value = self.sum()?;
        Ok(Expr {
            span: cover(keyword.span(), value.span),
            kind: ExprKind::Found(Box::new(value)),
        })
    }

    /// `try charge(amount)`.
    fn try_expr(&mut self) -> Result<Expr, Diagnostic> {
        let keyword = self.advance().clone();
        if self.at_end_of_line() || !starts_value(&self.peek().kind) {
            return Err(Diagnostic::new(
                keyword.span(),
                "try needs a call after it.\n\
                 try charge(amount) runs charge and passes its failures upward.",
            )
            .suggest("try charge(amount)"));
        }
        let inner = self.unary()?;
        Ok(Expr {
            span: cover(keyword.span(), inner.span),
            kind: ExprKind::Try(Box::new(inner)),
        })
    }

    /// `when find_price("bread")`, then `found` and `missing`.
    fn match_maybe(&mut self, span: Span) -> Result<Statement, Diagnostic> {
        let subject = self.expression()?;
        if self.peek().kind == TokenKind::Arrow {
            return Err(Diagnostic::new(
                subject.span,
                "The cases of a when go on the lines below it, one per line, indented.\n\
                 Each case is a condition, then ->, then what to do.",
            )
            .suggest("when\n  x > 1 -> say 1"));
        }
        if !self.at_end_of_line() {
            return Err(Diagnostic::new(
                self.peek().span(),
                "The found and missing cases go on the lines below when.\n\
                 Each case is found name -> ..., or missing -> ....",
            ));
        }
        while self.peek().kind == TokenKind::Newline {
            self.advance();
        }
        if self.peek().kind != TokenKind::Indent {
            return Err(Diagnostic::new(
                span,
                "This when opens a maybe, but it has no cases.\n\
                 Under it, handle found and missing.",
            )
            .suggest(
                "when find_price(\"bread\")\n  found price -> say price\n  missing -> say \"none\"",
            ));
        }
        self.advance();
        let mut arms = Vec::new();
        loop {
            match self.peek().kind {
                TokenKind::Newline => {
                    self.advance();
                    continue;
                }
                TokenKind::Dedent => {
                    self.advance();
                    break;
                }
                TokenKind::EndOfFile => break,
                _ => arms.push(self.catch_arm()?),
            }
        }
        if arms.is_empty() {
            return Err(Diagnostic::new(
                span,
                "This when opens a maybe, but it has no cases.\n\
                 Under it, handle found and missing.",
            ));
        }
        Ok(Statement::Match {
            subject,
            arms,
            span,
        })
    }

    /// `found price -> say price`, or `missing -> say "none"`.
    fn catch_arm(&mut self) -> Result<Catch, Diagnostic> {
        let token = self.peek().clone();
        let found = match token.kind {
            TokenKind::Found => {
                self.advance();
                let name_token = self.peek().clone();
                let TokenKind::Identifier(name) = name_token.kind.clone() else {
                    return Err(Diagnostic::new(
                        name_token.span(),
                        "found needs the name that holds the value.\n\
                         found price -> say price.",
                    )
                    .suggest("found price -> say price"));
                };
                self.advance();
                Some((name, name_token.span()))
            }
            TokenKind::Missing => {
                self.advance();
                None
            }
            _ => {
                return Err(Diagnostic::new(
                    token.span(),
                    "A maybe opens with found or missing.\n\
                     found price -> say price handles the value. missing -> say \"none\" handles the gap.",
                )
                .suggest("missing -> say \"none\""));
            }
        };
        let written = match &found {
            Some((name, _)) => format!("found {name}"),
            None => "missing".to_string(),
        };
        let span = token.span();
        if self.peek().kind != TokenKind::Arrow {
            return Err(Diagnostic::new(
                self.peek().span(),
                format!(
                    "{written} needs ->, then what to do.\n\
                     {written} -> say \"...\"."
                ),
            )
            .suggest(format!("{written} -> say \"...\"")));
        }
        self.advance();
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                span,
                format!("{written} needs what to do after ->, on the same line."),
            )
            .suggest(format!("{written} -> say \"...\"")));
        }
        let body = self.full_statement()?;
        Ok(Catch {
            found,
            body: Box::new(body),
            span,
        })
    }

    /// `fails Declined, NetworkDown` on the first line of a function.
    fn failure_list(&mut self) -> Result<Vec<(String, Span)>, Diagnostic> {
        let keyword = self.advance().clone();
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                keyword.span(),
                "fails needs the names of the failures, like Declined.\n\
                 A failure's name starts with a capital letter.",
            )
            .suggest("fails Declined"));
        }
        let mut names = Vec::new();
        loop {
            let (name, span) = self.failure_name()?;
            if names.iter().any(|(found, _)| found == &name) {
                return Err(Diagnostic::new(
                    span,
                    format!(
                        "{name} is already in this fails list.\n\
                         Each failure is named once."
                    ),
                ));
            }
            names.push((name, span));
            if self.peek().kind == TokenKind::Comma {
                let comma = self.advance().clone();
                if self.at_end_of_line() {
                    return Err(Diagnostic::new(
                        comma.span(),
                        "This comma has no failure after it.\n\
                         Write the next name, or remove the comma.",
                    ));
                }
                continue;
            }
            break;
        }
        self.end_of_line(&Statement::Fail {
            name: names[0].0.clone(),
            span: keyword.span(),
        })?;
        Ok(names)
    }

    /// `fail Declined`.
    fn fail_statement(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.advance().clone();
        if !self.in_function {
            return Err(Diagnostic::new(
                keyword.span(),
                "fail only goes inside a function that declares fails.\n\
                 The function names the failure, and the caller handles it with on.",
            ));
        }
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                keyword.span(),
                "fail needs the failure's name after it.\n\
                 fail Declined leaves the function with that failure.",
            )
            .suggest("fail Declined"));
        }
        let (name, span) = self.failure_name()?;
        Ok(Statement::Fail { name, span })
    }

    fn failure_name(&mut self) -> Result<(String, Span), Diagnostic> {
        let token = self.peek().clone();
        let TokenKind::Identifier(name) = token.kind.clone() else {
            return Err(Diagnostic::new(
                token.span(),
                format!(
                    "A failure's name starts with a capital letter, but found {}.\n\
                     Declined and NetworkDown are failure names.",
                    describe(&token.kind)
                ),
            )
            .suggest("Declined"));
        };
        self.advance();
        let mut chars = name.chars();
        if !chars.next().is_some_and(|c| c.is_ascii_uppercase()) {
            return Err(Diagnostic::new(
                token.span(),
                format!(
                    "A failure's name starts with a capital letter.\n\
                     Write {} instead of {name}.",
                    capitalize(&name)
                ),
            )
            .suggest(capitalize(&name)));
        }
        Ok((name, token.span()))
    }

    /// An indented `on` block under a call, when one follows this statement.
    fn maybe_handlers(&mut self, statement: Statement) -> Result<Statement, Diagnostic> {
        let saved = self.pos;
        while self.peek().kind == TokenKind::Newline {
            self.advance();
        }
        if self.peek().kind != TokenKind::Indent || self.peek_at(1).kind != TokenKind::On {
            self.pos = saved;
            return Ok(statement);
        }
        self.advance();
        let mut handlers = Vec::new();
        loop {
            match self.peek().kind {
                TokenKind::Newline => {
                    self.advance();
                    continue;
                }
                TokenKind::Dedent => {
                    self.advance();
                    break;
                }
                TokenKind::EndOfFile => break,
                TokenKind::On => handlers.push(self.handler()?),
                _ => {
                    return Err(Diagnostic::new(
                        self.peek().span(),
                        "Under a call, an indented line handles a failure with on.\n\
                         on Declined -> say \"Card declined\".",
                    ));
                }
            }
        }
        Ok(attach_handlers(statement, handlers))
    }

    fn handler(&mut self) -> Result<Handler, Diagnostic> {
        let keyword = self.advance().clone();
        let (name, name_span) = self.failure_name()?;
        if self.peek().kind != TokenKind::Arrow {
            return Err(Diagnostic::new(
                self.peek().span(),
                format!(
                    "on {name} needs ->, then what to do.\n\
                     on {name} -> say \"{name}\"."
                ),
            )
            .suggest(format!("on {name} -> say \"{name}\"")));
        }
        self.advance();
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                keyword.span(),
                format!("on {name} needs what to do after ->, on the same line."),
            )
            .suggest(format!("on {name} -> say \"{name}\"")));
        }
        let gives_value = !starts_statement(&self.peek().kind)
            && !(matches!(self.peek().kind, TokenKind::Identifier(_))
                && self.peek_at(1).kind == TokenKind::Equals);
        let body = if gives_value {
            let value = self.expression()?;
            if !self.at_end_of_line() {
                return Err(Diagnostic::new(
                    self.peek().span(),
                    format!(
                        "Quill expected the line to end after `on {name} -> {value}`, but found {}.\n\
                         The value after -> takes the place of the call's answer.",
                        describe(&self.peek().kind)
                    ),
                ));
            }
            Recovery::Value(value)
        } else {
            Recovery::Stop(Box::new(self.full_statement()?))
        };
        Ok(Handler {
            name,
            name_span,
            body,
            span: keyword.span(),
        })
    }

    fn at_end_of_line(&self) -> bool {
        matches!(
            self.peek().kind,
            TokenKind::Newline | TokenKind::EndOfFile | TokenKind::Dedent
        )
    }

    fn end_of_line(&mut self, statement: &Statement) -> Result<(), Diagnostic> {
        if self.at_end_of_line() {
            return Ok(());
        }
        let extra = self.peek();
        let hint = match extra.kind {
            TokenKind::Arrow => "\n-> only goes inside a when, between a condition and what to do.",
            _ => "",
        };
        Err(Diagnostic::new(
            extra.span(),
            format!(
                "Quill expected the line to end after `{statement}`, but found {}.\n\
                 Each line holds one statement.{hint}",
                describe(&extra.kind)
            ),
        ))
    }
}

/// The parts of a text with values inside.
fn template(pieces: &[Piece]) -> Result<Vec<Part>, Diagnostic> {
    let mut parts = Vec::new();
    for piece in pieces {
        match piece {
            Piece::Text(text) => parts.push(Part::Text(text.clone())),
            Piece::Code { tokens, .. } => {
                if tokens.is_empty() {
                    continue;
                }
                let mut parser = Parser {
                    tokens,
                    pos: 0,
                    in_function: false,
                };
                let value = parser.expression()?;
                let next = parser.peek();
                if next.kind != TokenKind::EndOfFile {
                    return Err(Diagnostic::new(
                        next.span(),
                        format!(
                            "Only one value can go inside {{}}, but found {} after {value}.\n\
                             To show more than one value, give each its own {{}}.",
                            describe(&next.kind)
                        ),
                    ));
                }
                parts.push(Part::Value(value));
            }
        }
    }
    Ok(parts)
}

fn arithmetic(op: Arithmetic, left: Expr, right: Expr, op_span: Span) -> Expr {
    Expr {
        span: cover(left.span, right.span),
        kind: ExprKind::Arithmetic {
            op,
            left: Box::new(left),
            right: Box::new(right),
            op_span,
        },
    }
}

fn logic(op: Logic, left: Expr, right: Expr, op_span: Span) -> Expr {
    Expr {
        span: cover(left.span, right.span),
        kind: ExprKind::Logic {
            op,
            left: Box::new(left),
            right: Box::new(right),
            op_span,
        },
    }
}

/// Whether a token can be the first token of a value.
fn starts_value(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Text(_)
            | TokenKind::Template(_)
            | TokenKind::Number(_)
            | TokenKind::Identifier(_)
            | TokenKind::True
            | TokenKind::False
            | TokenKind::LeftParen
            | TokenKind::Minus
            | TokenKind::Not
            | TokenKind::List
            | TokenKind::Record
            | TokenKind::Get
            | TokenKind::Sum
            | TokenKind::Found
            | TokenKind::Missing
            | TokenKind::Try
    )
}

fn write_steps(f: &mut fmt::Formatter, steps: &[Access]) -> fmt::Result {
    for step in steps {
        match step {
            Access::Field { name, .. } => write!(f, ".{name}")?,
            Access::At { index, .. } => write!(f, " at {index}")?,
        }
    }
    Ok(())
}

fn write_steps_string(steps: &[Access]) -> String {
    let mut out = String::new();
    for step in steps {
        match step {
            Access::Field { name, .. } => out.push_str(&format!(".{name}")),
            Access::At { index, .. } => out.push_str(&format!(" at {index}")),
        }
    }
    out
}

/// Whether this statement's value includes a record or a `when` block, which already
/// ended its line.
fn contains_record(statement: &Statement) -> bool {
    match statement {
        Statement::Say { value, .. }
        | Statement::Let { value, .. }
        | Statement::Set { value, .. } => contains_record_expr(value),
        Statement::Check { left, right, .. } => {
            contains_record_expr(left) || contains_record_expr(right)
        }
        Statement::When { .. }
        | Statement::Match { .. }
        | Statement::For { .. }
        | Statement::Function { .. }
        | Statement::Fail { .. } => false,
        Statement::Return { value, .. } => value.as_ref().is_some_and(contains_record_expr),
        Statement::Call { args, .. } => args.iter().any(contains_record_expr),
    }
}

fn contains_record_expr(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Record { .. } | ExprKind::When(_) => true,
        ExprKind::List { items, .. } => items.iter().any(contains_record_expr),
        ExprKind::At { list, index, .. } => {
            contains_record_expr(list) || contains_record_expr(index)
        }
        ExprKind::Field { record, .. } => contains_record_expr(record),
        ExprKind::Sum { list, .. } | ExprKind::Get { list, .. } => contains_record_expr(list),
        ExprKind::GetWith { source, .. } => contains_record_expr(source),
        ExprKind::Where {
            list, condition, ..
        } => contains_record_expr(list) || contains_record_expr(condition),
        ExprKind::Call { args, .. } => args.iter().any(contains_record_expr),
        ExprKind::Negate(inner) | ExprKind::Not(inner) | ExprKind::Group(inner) => {
            contains_record_expr(inner)
        }
        ExprKind::Arithmetic { left, right, .. }
        | ExprKind::Compare { left, right, .. }
        | ExprKind::Logic { left, right, .. } => {
            contains_record_expr(left) || contains_record_expr(right)
        }
        ExprKind::Template(parts) => parts.iter().any(|part| match part {
            Part::Value(value) => contains_record_expr(value),
            Part::Text(_) => false,
        }),
        ExprKind::Found(inner) | ExprKind::Try(inner) => contains_record_expr(inner),
        ExprKind::Text(_)
        | ExprKind::Number(_)
        | ExprKind::Money { .. }
        | ExprKind::Bool(_)
        | ExprKind::Name(_)
        | ExprKind::Missing => false,
    }
}

/// Words that start a statement, not a value.
fn starts_statement(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Say
            | TokenKind::Check
            | TokenKind::Example
            | TokenKind::Var
            | TokenKind::Set
            | TokenKind::When
            | TokenKind::For
            | TokenKind::Fn
            | TokenKind::Return
            | TokenKind::Fail
            | TokenKind::Fails
            | TokenKind::Require
            | TokenKind::Ensure
            | TokenKind::On
    )
}

fn describe(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Text(text) => format!("the text {text:?}"),
        TokenKind::Template(_) => "a text".to_string(),
        TokenKind::Number(number) => format!("the number {number}"),
        TokenKind::Identifier(name) => format!("the name {name}"),
        TokenKind::Example => "a doc example".to_string(),
        TokenKind::Indent => "an indented line".to_string(),
        TokenKind::Dedent => "the end of an indented block".to_string(),
        TokenKind::Newline => "the end of the line".to_string(),
        TokenKind::EndOfFile => "the end of the file".to_string(),
        other => other.to_string(),
    }
}

/// A span from the start of `start` through the end of `end`, on the same line.
fn cover(start: Span, end: Span) -> Span {
    if end.line != start.line {
        return start;
    }
    Span {
        line: start.line,
        column: start.column,
        length: (end.column + end.length).saturating_sub(start.column),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse_source(source: &str) -> Result<Program, Diagnostic> {
        parse(&lex(source).expect("source should lex"))
    }

    /// Parses `source` and writes each statement back out as Quill code.
    fn statements(source: &str) -> Vec<String> {
        parse_source(source)
            .expect("source should parse")
            .statements
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn error(source: &str) -> Diagnostic {
        parse_source(source).expect_err("source should not parse")
    }

    fn span(line: usize, column: usize, length: usize) -> Span {
        Span {
            line,
            column,
            length,
        }
    }

    #[test]
    fn empty_program() {
        assert_eq!(statements(""), Vec::<String>::new());
        assert_eq!(parse(&[]).unwrap().statements, vec![]);
    }

    #[test]
    fn say_and_values() {
        assert_eq!(
            statements("greeting = \"Hello\"\nyear = 2026\nsay greeting\nsay 1.5\n"),
            vec![
                "greeting = \"Hello\"",
                "year = 2026",
                "say greeting",
                "say 1.5"
            ]
        );
    }

    #[test]
    fn blank_lines_are_skipped() {
        assert_eq!(
            statements("\n\nsay 1\n\n\nsay 2\n\n"),
            vec!["say 1", "say 2"]
        );
    }

    #[test]
    fn statements_keep_positions() {
        let program = parse_source("x = \"hi\"\nsay   x").unwrap();
        assert_eq!(
            program.statements,
            vec![
                Statement::Let {
                    name: "x".to_string(),
                    name_span: span(1, 1, 1),
                    value: Expr {
                        kind: ExprKind::Text("hi".to_string()),
                        span: span(1, 5, 4),
                    },
                    mutable: false,
                    handlers: Vec::new(),
                },
                Statement::Say {
                    value: Expr {
                        kind: ExprKind::Name("x".to_string()),
                        span: span(2, 7, 1),
                    },
                    span: span(2, 1, 3),
                },
            ]
        );
    }

    #[test]
    fn say_needs_a_value() {
        let err = error("say\n");
        assert_eq!((err.line, err.column, err.length), (1, 1, 3));
        assert!(err.message.contains("say needs something to say"));
    }

    #[test]
    fn let_needs_a_value() {
        let err = error("total =");
        assert_eq!((err.line, err.column), (1, 7));
        assert_eq!(err.suggestion.as_deref(), Some("total = \"some text\""));
    }

    #[test]
    fn one_statement_per_line() {
        let err = error("say \"a\" \"b\"");
        assert_eq!((err.line, err.column, err.length), (1, 9, 3));
        assert!(err
            .message
            .contains("expected the line to end after `say \"a\"`"));
    }

    #[test]
    fn equals_needs_a_name() {
        let err = error("= 5");
        assert_eq!((err.line, err.column), (1, 1));
    }

    #[test]
    fn value_on_its_own_suggests_say() {
        let err = error("\"hello\"");
        assert_eq!(err.suggestion.as_deref(), Some("say \"hello\""));
        assert_eq!(error("42").suggestion.as_deref(), Some("say 42"));
    }

    #[test]
    fn name_on_its_own_suggests_say() {
        let err = error("x = 1\nx");
        assert_eq!((err.line, err.column), (2, 1));
        assert_eq!(err.suggestion.as_deref(), Some("say x"));
    }

    #[test]
    fn misspelled_say_is_caught() {
        let err = error("sya \"hi\"");
        assert_eq!((err.line, err.column, err.length), (1, 1, 3));
        assert_eq!(err.suggestion.as_deref(), Some("say \"hi\""));
    }

    #[test]
    fn missing_equals_is_caught() {
        let err = error("total 5");
        assert_eq!(err.suggestion.as_deref(), Some("total = 5"));
    }

    #[test]
    fn say_cannot_be_a_value() {
        let err = error("x = say");
        assert_eq!((err.line, err.column), (1, 5));
    }

    #[test]
    fn double_equals_is_not_a_value() {
        let err = error("x = = 1");
        assert_eq!((err.line, err.column), (1, 5));
        assert!(err.message.contains("found ="));
    }

    #[test]
    fn checks_and_examples() {
        assert_eq!(
            statements(
                "greeting = \"Hello\"\n\
                 check greeting == \"Hello\"\n\
                 --- a note\n\
                 --- example: greeting == \"Hello\"\n"
            ),
            vec![
                "greeting = \"Hello\"",
                "check greeting == \"Hello\"",
                "--- example: greeting == \"Hello\""
            ]
        );
    }

    #[test]
    fn comments_do_not_become_statements() {
        assert_eq!(statements("say \"hi\" -- note\n"), vec!["say \"hi\""]);
    }

    #[test]
    fn check_needs_a_comparison() {
        let err = error("check\n");
        assert!(err.message.contains("needs two values"));
        assert_eq!(err.suggestion.as_deref(), Some("check name == \"Hello\""));

        let err = error("check greeting \"Hello\"");
        assert!(err.message.contains("compares with =="));

        let err = error("check greeting ==");
        assert!(err.message.contains("needs a value on its right"));
    }

    #[test]
    fn a_bare_comparison_suggests_check() {
        let err = error("greeting == \"Hello\"");
        assert_eq!(
            err.suggestion.as_deref(),
            Some("check greeting == \"Hello\"")
        );
    }

    #[test]
    fn suggestions_write_text_back_as_quill() {
        let err = error("\"a\\\"b\\n\"");
        assert_eq!(err.suggestion.as_deref(), Some("say \"a\\\"b\\n\""));
    }

    /// Parses one value (the right side of `x = ...`) and writes it back with
    /// brackets around every operation, to show how it was grouped.
    fn grouping(value: &str) -> String {
        fn show(expr: &Expr) -> String {
            match &expr.kind {
                ExprKind::Arithmetic {
                    op, left, right, ..
                } => format!("({} {} {})", show(left), op.symbol(), show(right)),
                ExprKind::Compare {
                    op, left, right, ..
                } => format!("({} {} {})", show(left), op.symbol(), show(right)),
                ExprKind::Logic {
                    op, left, right, ..
                } => format!("({} {} {})", show(left), op.word(), show(right)),
                ExprKind::Negate(inner) => format!("(-{})", show(inner)),
                ExprKind::Not(inner) => format!("(not {})", show(inner)),
                ExprKind::Group(inner) => show(inner),
                _ => expr.to_string(),
            }
        }
        let program = parse_source(&format!("x = {value}")).unwrap();
        match &program.statements[0] {
            Statement::Let { value, .. } => show(value),
            other => panic!("expected a value, got {other:?}"),
        }
    }

    #[test]
    fn arithmetic_follows_the_usual_order() {
        assert_eq!(grouping("1 + 2 * 3"), "(1 + (2 * 3))");
        assert_eq!(grouping("(1 + 2) * 3"), "((1 + 2) * 3)");
        assert_eq!(grouping("10 - 4 - 3"), "((10 - 4) - 3)");
        assert_eq!(grouping("8 / 2 % 3"), "((8 / 2) % 3)");
        assert_eq!(grouping("-a * b"), "((-a) * b)");
        assert_eq!(grouping("- -5"), "(-(-5))");
    }

    #[test]
    fn comparisons_and_logic_bind_looser_than_arithmetic() {
        assert_eq!(grouping("n % 15 == 0"), "((n % 15) == 0)");
        assert_eq!(grouping("a + 1 >= b * 2"), "((a + 1) >= (b * 2))");
        assert_eq!(
            grouping("a < b and b < c or d"),
            "(((a < b) and (b < c)) or d)"
        );
        assert_eq!(grouping("not a == b"), "(not (a == b))");
        assert_eq!(grouping("not a and b"), "((not a) and b)");
    }

    #[test]
    fn values_are_written_back_as_typed() {
        assert_eq!(
            statements("x = (price + 5) * -2 >= 10 and not done"),
            vec!["x = (price + 5) * -2 >= 10 and not done"]
        );
        assert_eq!(
            statements("say \"Hi {name}, total {a + b}\""),
            vec!["say \"Hi {name}, total {a + b}\""]
        );
    }

    #[test]
    fn calculations_have_spans_covering_them() {
        let program = parse_source("x = 10 / count").unwrap();
        let Statement::Let { value, .. } = &program.statements[0] else {
            panic!()
        };
        assert_eq!(value.span, span(1, 5, 10));
        let ExprKind::Arithmetic { op_span, .. } = &value.kind else {
            panic!()
        };
        assert_eq!(*op_span, span(1, 8, 1));
    }

    #[test]
    fn values_inside_text() {
        let program = parse_source("say \"Hi {name}!\"").unwrap();
        let Statement::Say { value, .. } = &program.statements[0] else {
            panic!()
        };
        let ExprKind::Template(parts) = &value.kind else {
            panic!()
        };
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], Part::Text("Hi ".to_string()));
        let Part::Value(name) = &parts[1] else {
            panic!()
        };
        assert_eq!(name.kind, ExprKind::Name("name".to_string()));
        assert_eq!(name.span, span(1, 10, 4));
    }

    #[test]
    fn only_one_value_fits_in_braces() {
        let err = error("say \"{a b}\"");
        assert_eq!((err.line, err.column), (1, 9));
        assert!(err.message.contains("Only one value can go inside {}"));
    }

    #[test]
    fn mistakes_inside_braces_are_reported() {
        let err = error("say \"{a +}\"");
        assert_eq!((err.line, err.column), (1, 9));
        assert!(err.message.contains("+ needs a value on its right"));
    }

    #[test]
    fn operators_need_values_on_both_sides() {
        let err = error("x = 5 +");
        assert_eq!((err.line, err.column), (1, 7));
        assert!(err.message.contains("+ needs a value on its right"));

        let err = error("x = 5 * * 2");
        assert_eq!((err.line, err.column), (1, 7));
        assert!(err.message.contains("found *"));

        let err = error("x = a and");
        assert!(err.message.contains("and needs a value on its right"));
    }

    #[test]
    fn brackets_must_close() {
        let err = error("x = (1 + 2");
        assert_eq!((err.line, err.column), (1, 5));
        assert!(err.message.contains("never closed"));
        assert_eq!(err.suggestion.as_deref(), Some("(1 + 2)"));

        let err = error("x = ()");
        assert_eq!((err.column, err.length), (5, 2));
        assert!(err.message.contains("empty"));
    }

    #[test]
    fn comparisons_cannot_be_chained() {
        let err = error("x = 1 < y < 10");
        assert_eq!((err.line, err.column), (1, 11));
        assert_eq!(err.suggestion.as_deref(), Some("1 < y and y < 10"));
    }

    #[test]
    fn a_bare_calculation_suggests_say() {
        let err = error("price * 2");
        assert_eq!(err.suggestion.as_deref(), Some("say price * 2"));
        let err = error("(1 + 2)");
        assert_eq!(err.suggestion.as_deref(), Some("say (1 + 2)"));
    }

    #[test]
    fn checks_can_compare_calculations_with_any_comparison() {
        assert_eq!(
            statements("check 1 + 2 == 3\ncheck total >= 10\ncheck a != b"),
            vec!["check 1 + 2 == 3", "check total >= 10", "check a != b"]
        );
    }

    #[test]
    fn var_and_set() {
        assert_eq!(
            statements("var visits = 0\nset visits = visits + 1"),
            vec!["var visits = 0", "set visits = visits + 1"]
        );
        let program = parse_source("var n = 1").unwrap();
        assert!(matches!(
            program.statements[0],
            Statement::Let { mutable: true, .. }
        ));
    }

    #[test]
    fn var_and_set_need_a_name_and_a_value() {
        let err = error("var");
        assert_eq!((err.line, err.column), (1, 1));
        assert_eq!(err.suggestion.as_deref(), Some("var count = 0"));

        let err = error("var count");
        assert_eq!(err.suggestion.as_deref(), Some("var count = 0"));

        let err = error("set visits");
        assert_eq!(err.suggestion.as_deref(), Some("set visits = visits + 1"));

        let err = error("set visits =");
        assert_eq!(err.suggestion.as_deref(), Some("set visits = visits + 1"));

        let err = error("var 5 = 1");
        assert_eq!((err.line, err.column), (1, 5));
    }

    #[test]
    fn misspelled_keywords_are_suggested() {
        assert_eq!(error("vr count = 1").suggestion.as_deref(), Some("var"));
        assert_eq!(error("sett count = 1").suggestion.as_deref(), Some("set"));
    }

    #[test]
    fn when_with_cases() {
        let program = parse_source(
            "when\n  n > 10 -> say \"big\"\n  n > 5 -> set x = 1\n  else -> say n\nsay 1",
        )
        .unwrap();
        assert_eq!(program.statements.len(), 2);
        let Statement::When { arms, span: when } = &program.statements[0] else {
            panic!()
        };
        assert_eq!(*when, span(1, 1, 4));
        assert_eq!(arms.len(), 3);
        assert_eq!(arms[0].condition.as_ref().unwrap().to_string(), "n > 10");
        assert_eq!(arms[0].body.to_string(), "say \"big\"");
        assert_eq!(arms[0].span, span(2, 3, 6));
        assert_eq!(arms[1].body.to_string(), "set x = 1");
        assert_eq!(arms[2].condition, None);
        assert_eq!(arms[2].span, span(4, 3, 4));
        assert_eq!(program.statements[1].to_string(), "say 1");
    }

    #[test]
    fn when_cases_can_hold_another_when() {
        let program = parse_source(
            "when\n  a -> when\n    b -> say 1\n    else -> say 2\n  else -> say 3\nsay 4",
        )
        .unwrap();
        assert_eq!(program.statements.len(), 2);
        let Statement::When { arms, .. } = &program.statements[0] else {
            panic!()
        };
        assert_eq!(arms.len(), 2);
        assert!(matches!(*arms[0].body, Statement::When { .. }));
    }

    #[test]
    fn when_at_the_end_of_the_file() {
        let program = parse_source("when\n  a -> say 1").unwrap();
        assert_eq!(program.statements.len(), 1);
    }

    #[test]
    fn when_needs_cases_below_it() {
        let err = error("when\nsay 1");
        assert_eq!((err.line, err.column), (1, 1));
        assert!(err.message.contains("has no cases"));

        let err = error("when x > 1 -> say 1");
        assert!(err.message.contains("go on the lines below it"));
    }

    #[test]
    fn when_cases_need_an_arrow_and_a_statement() {
        let err = error("when\n  n > 1 say 1");
        assert_eq!((err.line, err.column), (2, 9));
        assert!(err.message.contains("needs -> after n > 1"));
        assert_eq!(err.suggestion.as_deref(), Some("n > 1 -> say \"...\""));

        let err = error("when\n  n > 1");
        assert_eq!((err.line, err.column), (2, 3));

        let err = error("when\n  n > 1 ->");
        assert_eq!((err.line, err.column), (2, 9));
        assert!(err.message.contains("needs something to do"));
    }

    #[test]
    fn else_must_be_the_last_case() {
        let err = error("when\n  else -> say 1\n  a -> say 2");
        assert_eq!((err.line, err.column), (3, 3));
        assert!(err.message.contains("can never run"));
    }

    #[test]
    fn else_only_goes_inside_when() {
        let err = error("else -> say 1");
        assert!(err.message.contains("only goes inside a when"));
    }

    #[test]
    fn a_when_case_cannot_create_a_value() {
        let err = error("when\n  a -> total = 5");
        assert_eq!((err.line, err.column), (2, 8));
        assert_eq!(err.suggestion.as_deref(), Some("set total = 5"));

        let err = error("when\n  a -> var total = 5");
        assert_eq!(err.suggestion.as_deref(), Some("set total = 5"));
    }

    #[test]
    fn unexpected_indentation_is_explained() {
        let err = error("say 1\n  say 2");
        assert_eq!((err.line, err.column), (2, 3));
        assert!(err.message.contains("nothing above it starts a block"));
    }

    #[test]
    fn an_arrow_outside_when_is_explained() {
        let err = error("say 1 -> say 2");
        assert!(err.message.contains("-> only goes inside a when"));
    }

    #[test]
    fn a_for_loop_counts_through_a_range() {
        let program = parse_source("for i in 1..3\n  say i\n").unwrap();
        let Statement::For { name, body, .. } = &program.statements[0] else {
            panic!("expected a for");
        };
        assert_eq!(name, "i");
        assert_eq!(body.len(), 1);
        assert_eq!(program.statements[0].to_string(), "for i in 1..3");
    }

    #[test]
    fn a_for_needs_in_a_range_and_a_body() {
        let err = error("for i 1..3\n  say i");
        assert!(err.message.contains("for needs in after i"));
        let err = error("for i in 1 3\n  say i");
        assert!(err.message.contains("needs .."));
        let err = error("for i in 1..3");
        assert!(err.message.contains("nothing to repeat"));
        let err = error("for i in 1..3\n  total = i");
        assert!(err.message.contains("can't create a new value"));
        assert_eq!(err.suggestion.as_deref(), Some("set total = i"));
    }

    #[test]
    fn an_arrow_after_say_is_not_part_of_say() {
        let err = error("say -> \"Hello\"");
        assert!(err.message.contains("belongs inside a when"));
        assert_eq!(err.suggestion.as_deref(), Some("say \"Hello\""));
    }

    #[test]
    fn contracts_and_fails_come_first_in_any_order() {
        let program = parse_source(
            "fn withdraw(amount: USD) -> USD\n  require amount > 0 USD\n  fails Declined\n  ensure result >= 0 USD\n  return amount\n",
        )
        .unwrap();
        let Statement::Function {
            fails,
            contracts,
            body,
            ..
        } = &program.statements[0]
        else {
            panic!("expected a function");
        };
        assert_eq!(fails.len(), 1);
        assert_eq!(contracts.len(), 2);
        assert!(!contracts[0].ensure);
        assert!(contracts[1].ensure);
        assert_eq!(contracts[1].condition.to_string(), "result >= 0 USD");
        assert_eq!(body.len(), 1);
    }

    #[test]
    fn a_contract_after_the_first_line_of_work_is_a_mistake() {
        let err = error("fn f(n: number) -> number\n  say n\n  require n > 0\n  return n\n");
        assert!(err
            .message
            .contains("require goes at the top of a function"));
        let outside = error("require 1 > 0\n");
        assert!(outside.message.contains("require goes inside a function"));
    }

    #[test]
    fn a_contract_needs_one_condition() {
        let empty = error("fn f(n: number) -> number\n  ensure\n  return n\n");
        assert!(empty.message.starts_with("ensure needs a condition"));
        let extra = error("fn f(n: number) -> number\n  require n > 0 n\n  return n\n");
        assert!(extra.message.contains("Join two with and."));
        let twice = error("fn f(n: number) -> number\n  fails A\n  fails B\n  return n\n");
        assert!(twice.message.contains("already has a fails line"));
    }

    #[test]
    fn a_parameter_can_have_a_range() {
        let program =
            parse_source("fn book(guests: number where 1..500) -> text\n  return \"ok\"\n")
                .unwrap();
        let Statement::Function { params, .. } = &program.statements[0] else {
            panic!("expected a function");
        };
        let rule = params[0].rule.as_ref().unwrap();
        assert_eq!(rule.to_string(), "where 1..500");
        assert_eq!(rule.range(), "from 1 through 500");
        let money = parse_source("fn f(a: USD where -5 USD..0 USD) -> USD\n  return a\n").unwrap();
        let Statement::Function { params, .. } = &money.statements[0] else {
            panic!("expected a function");
        };
        assert_eq!(
            params[0].rule.as_ref().unwrap().to_string(),
            "where -5 USD..0 USD"
        );
    }

    #[test]
    fn a_range_is_two_written_values() {
        let empty = error("fn f(n: number where) -> number\n  return n\n");
        assert!(empty.message.starts_with("where needs a range after it."));
        let one = error("fn f(n: number where 5) -> number\n  return n\n");
        assert!(one.message.starts_with("This range needs .. after 5"));
        let open = error("fn f(n: number where 5..) -> number\n  return n\n");
        assert!(open
            .message
            .starts_with("This range is missing the largest value"));
        let worked = error("fn f(n: number, m: number where 0..n) -> number\n  return n\n");
        assert!(worked.message.contains("but n has to be worked out"));
        let result = error("fn f(n: number) -> number where 0..5\n  return n\n");
        assert!(result.message.starts_with("where goes on a parameter"));
    }

    #[test]
    fn where_filters_a_list_and_can_follow_get_and_sum() {
        let program = parse_source("say students where score >= 50 and paid\n").unwrap();
        let Statement::Say { value, .. } = &program.statements[0] else {
            panic!("expected say");
        };
        assert!(matches!(value.kind, ExprKind::Where { .. }));
        assert_eq!(value.to_string(), "students where score >= 50 and paid");
        let program = parse_source("say sum fee of roster where paid == false\n").unwrap();
        let Statement::Say { value, .. } = &program.statements[0] else {
            panic!("expected say");
        };
        let ExprKind::Sum { list, .. } = &value.kind else {
            panic!("expected sum");
        };
        assert!(matches!(list.kind, ExprKind::Where { .. }));
        let chained = parse_source("say s where a > 1 where b < 2\n").unwrap();
        let Statement::Say { value, .. } = &chained.statements[0] else {
            panic!("expected say");
        };
        let ExprKind::Where { list, .. } = &value.kind else {
            panic!("expected where");
        };
        assert!(matches!(list.kind, ExprKind::Where { .. }));
        let empty = error("say students where\n");
        assert!(empty
            .message
            .starts_with("where needs a condition after it."));
    }

    #[test]
    fn when_gives_a_value() {
        let program =
            parse_source("label = when\n  1 > 0 -> \"a\"\n  else -> \"b\"\nsay label\n").unwrap();
        let Statement::Let { value, .. } = &program.statements[0] else {
            panic!("expected a value");
        };
        let ExprKind::When(arms) = &value.kind else {
            panic!("expected a when");
        };
        assert_eq!(arms.len(), 2);
        assert!(arms[1].condition.is_none());
        assert_eq!(program.statements.len(), 2);
    }

    #[test]
    fn a_when_value_needs_else_last() {
        let err = error("x = when\n  1 > 0 -> \"a\"\n");
        assert!(err
            .message
            .starts_with("A when that gives a value needs else"));
        let after = error("x = when\n  else -> \"b\"\n  1 > 0 -> \"a\"\n");
        assert!(after.message.contains("comes after else"));
    }

    #[test]
    fn a_when_value_gives_values_not_statements() {
        let err = error("x = when\n  1 > 0 -> say \"a\"\n  else -> \"b\"\n");
        assert!(err
            .message
            .contains("each case gives a value after ->, not say"));
        let same_line = error("x = when 5\n");
        assert!(same_line.message.contains("cases on the lines below it"));
    }

    #[test]
    fn on_takes_a_statement_or_a_value() {
        let program = parse_source(
            "fn f() -> text\n  fails A, B\n  fail A\nx = f()\n  on A -> \"a\"\n  on B -> say \"b\"\n",
        )
        .unwrap();
        let Statement::Let { handlers, .. } = &program.statements[1] else {
            panic!("expected a value");
        };
        assert!(matches!(handlers[0].body, Recovery::Value(_)));
        assert!(matches!(handlers[1].body, Recovery::Stop(_)));
    }

    #[test]
    fn a_call_after_on_under_a_call_statement_runs_it() {
        let program =
            parse_source("fn f()\n  fails A\n  fail A\nfn log()\n  say 1\nf()\n  on A -> log()\n")
                .unwrap();
        let Statement::Call { handlers, .. } = &program.statements[2] else {
            panic!("expected a call");
        };
        assert!(matches!(handlers[0].body, Recovery::Stop(_)));
    }

    #[test]
    fn examples_above_a_function_run_after_it() {
        let program = parse_source(
            "--- Doubles n.\n--- example: double(2) == 4\n--- example: double(0) == 0\nfn double(n: number) -> number\n  return n * 2\nsay double(1)\n",
        )
        .unwrap();
        let lines: Vec<usize> = program.statements.iter().map(Statement::line).collect();
        assert_eq!(lines, vec![4, 2, 3, 6]);
    }

    #[test]
    fn examples_not_directly_above_a_function_stay_where_they_are() {
        let program = parse_source(
            "--- example: 1 + 1 == 2\nsay 1\nfn double(n: number) -> number\n  return n * 2\n",
        )
        .unwrap();
        let lines: Vec<usize> = program.statements.iter().map(Statement::line).collect();
        assert_eq!(lines, vec![1, 2, 3]);
    }
}
