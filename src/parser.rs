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
    },
    /// `set name = value`. `span` covers the `set` keyword.
    Set {
        name: String,
        name_span: Span,
        value: Expr,
        span: Span,
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
}

/// One part of a text with values inside.
#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Text(String),
    Value(Expr),
}

impl Expr {
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
            Statement::Set { name, value, .. } => write!(f, "set {name} = {value}"),
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
        }
    }
}

pub fn parse(tokens: &[Token]) -> Result<Program, Diagnostic> {
    let mut statements = Vec::new();
    if tokens.is_empty() {
        return Ok(Program { statements });
    }

    let mut parser = Parser { tokens, pos: 0 };
    loop {
        match parser.peek().kind {
            TokenKind::EndOfFile => break,
            TokenKind::Newline | TokenKind::Dedent => {
                parser.advance();
            }
            _ => statements.push(parser.full_statement()?),
        }
    }
    Ok(Program { statements })
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
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
        let statement = self.statement()?;
        // A `when` ends with its indented block, so its line has already ended.
        if !matches!(statement, Statement::When { .. }) {
            self.end_of_line(&statement)?;
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
                     Only the cases under a when are indented, so move this line back to the left.",
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
                     A line can start with say, check, var, set, when, or a name and =.",
                    describe(other)
                ),
            )),
        }
    }

    fn check_statement(&mut self, example: bool) -> Result<Statement, Diagnostic> {
        let keyword = self.advance();
        let word = if example { "example" } else { "check" };
        let written = if example { "--- example:" } else { "check" };
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                keyword.span(),
                format!(
                    "{word} needs two values with == between them.\n\
                     A {word} passes when both sides are the same value."
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
        })
    }

    fn var_statement(&mut self) -> Result<Statement, Diagnostic> {
        let var = self.advance();
        let (name, _) = self.assignment_target(var, "var", "count", "0")?;
        self.let_statement(&name, true)
    }

    fn set_statement(&mut self) -> Result<Statement, Diagnostic> {
        let set = self.advance();
        let (name, name_span) = self.assignment_target(set, "set", "count", "count + 1")?;
        self.advance(); // name
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
            value,
            span: set.span(),
        })
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
        let when = self.advance();
        // Lined up under "Did you mean:  when", with the cases indented.
        let example = "when\n                 count > 10 -> say \"big\"\n                 else -> say \"small\"";
        if !self.at_end_of_line() {
            return Err(Diagnostic::new(
                cover(when.span(), self.peek().span()),
                "The cases of a when go on the lines below it, one per line, indented.\n\
                 Each case is a condition, then ->, then what to do.",
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
        self.or()
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
            return self.primary();
        }
        let minus = self.advance();
        self.expect_operand(minus)?;
        let inner = self.unary()?;
        Ok(Expr {
            span: cover(minus.span(), inner.span),
            kind: ExprKind::Negate(Box::new(inner)),
        })
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek();
        let kind = match &token.kind {
            TokenKind::Text(text) => ExprKind::Text(text.clone()),
            TokenKind::Template(pieces) => ExprKind::Template(template(pieces)?),
            TokenKind::Number(number) => ExprKind::Number(number.clone()),
            TokenKind::True => ExprKind::Bool(true),
            TokenKind::False => ExprKind::Bool(false),
            TokenKind::Identifier(name) => ExprKind::Name(name.clone()),
            TokenKind::LeftParen => return self.group(),
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
                         A value can be text, a number, true, false, a name or a calculation.",
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
                 Write a value after it: text, a number, true, false, a name or a calculation.",
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
                     A line can start with say, check, var, set, when, or a name and = to create a value."
                ),
            );
            let keyword = closest_match(name, ["say", "set", "var", "check", "when"]);
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
                let mut parser = Parser { tokens, pos: 0 };
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
}
