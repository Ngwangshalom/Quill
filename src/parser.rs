//! The parser: turns the lexer's tokens into a tree of statements.
//!
//! Supported so far, one statement per line:
//!   say VALUE
//!   NAME = VALUE
//!   check VALUE == VALUE
//!   --- example: VALUE == VALUE
//! where VALUE is text, a number, or the name of an existing value.
//! A `--` comment is skipped. A `---` line is a doc comment, and
//! `--- example:` is a test.

use std::fmt;

use crate::diagnostic::{closest_match, Diagnostic, Span};
use crate::lexer::{Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// `say value`. `span` covers the `say` keyword.
    Say { value: Expr, span: Span },
    /// `name = value`
    Let {
        name: String,
        name_span: Span,
        value: Expr,
    },
    /// `check left == right`, or a doc example that means the same thing.
    Check {
        left: Expr,
        right: Expr,
        /// Covers the whole statement, so a failure can underline it.
        span: Span,
        /// `true` when this came from `--- example:`.
        example: bool,
    },
}

impl Statement {
    /// The line this statement starts on.
    pub fn line(&self) -> usize {
        match self {
            Statement::Say { span, .. } => span.line,
            Statement::Let { name_span, .. } => name_span.line,
            Statement::Check { span, .. } => span.line,
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
    Number(String),
    Name(String),
}

/// Writes an expression back out as Quill source, for suggestions.
impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            ExprKind::Text(text) => {
                write!(f, "\"")?;
                for c in text.chars() {
                    match c {
                        '"' => write!(f, "\\\"")?,
                        '\\' => write!(f, "\\\\")?,
                        '\n' => write!(f, "\\n")?,
                        '\t' => write!(f, "\\t")?,
                        c => write!(f, "{c}")?,
                    }
                }
                write!(f, "\"")
            }
            ExprKind::Number(number) => write!(f, "{number}"),
            ExprKind::Name(name) => write!(f, "{name}"),
        }
    }
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Statement::Say { value, .. } => write!(f, "say {value}"),
            Statement::Let { name, value, .. } => write!(f, "{name} = {value}"),
            Statement::Check {
                left,
                right,
                example,
                ..
            } => {
                if *example {
                    write!(f, "--- example: {left} == {right}")
                } else {
                    write!(f, "check {left} == {right}")
                }
            }
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
            TokenKind::Newline => {
                parser.advance();
            }
            _ => {
                let statement = parser.statement()?;
                parser.end_of_line(&statement)?;
                statements.push(statement);
            }
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

    fn statement(&mut self) -> Result<Statement, Diagnostic> {
        let token = self.peek();
        match &token.kind {
            TokenKind::Say => self.say(),
            TokenKind::Check => self.check_statement(false),
            TokenKind::Example => self.check_statement(true),
            TokenKind::Identifier(name) if self.peek_at(1).kind == TokenKind::Equals => {
                self.let_statement(name)
            }
            TokenKind::Identifier(name) => Err(self.stray_name(name, token)),
            TokenKind::EqualEqual => Err(Diagnostic::new(
                token.span(),
                "This line starts with ==, but == belongs inside a check.\n\
                 A check names two values and passes when they are the same.",
            )
            .suggest("check name == \"Hello\"")),
            TokenKind::Equals => Err(Diagnostic::new(
                token.span(),
                "This line starts with =, but = needs a name on its left.\n\
                 To create a value, write its name, then =, then the value.",
            )
            .suggest("name = \"Amina\"")),
            TokenKind::Text(_) | TokenKind::Number(_) => {
                let value = self.expression()?;
                Err(Diagnostic::new(
                    value.span,
                    "This value is on a line by itself, so nothing happens to it.\n\
                     Use say to show it, or give it a name to keep it.",
                )
                .suggest(format!("say {value}")))
            }
            // The main loop handles these before calling `statement`.
            TokenKind::Newline | TokenKind::EndOfFile => Err(Diagnostic::new(
                token.span(),
                "Quill expected a statement here.",
            )),
        }
    }

    fn check_statement(&mut self, example: bool) -> Result<Statement, Diagnostic> {
        let keyword = self.advance().clone();
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
        let left = self.expression()?;
        if !matches!(self.peek().kind, TokenKind::EqualEqual) {
            let found = self.peek();
            return Err(Diagnostic::new(
                found.span(),
                format!(
                    "{word} compares with ==, but found {}.\n\
                     Write the value you have, then ==, then the value you expect.",
                    describe(&found.kind)
                ),
            )
            .suggest(format!("{written} {left} == \"expected\"")));
        }
        let eq = self.advance().clone();
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                eq.span(),
                "== needs a value on its right.\n\
                 A check compares two values, so write the one you expect."
                    .to_string(),
            )
            .suggest(format!("{written} {left} == \"expected\"")));
        }
        let right = self.expression()?;
        Ok(Statement::Check {
            span: cover(keyword.span(), right.span),
            left,
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

    fn let_statement(&mut self, name: &str) -> Result<Statement, Diagnostic> {
        let name_token = self.advance();
        let equals = self.advance();
        if self.at_end_of_line() {
            return Err(Diagnostic::new(
                equals.span(),
                format!(
                    "{name} = needs a value after the =.\n\
                     A value can be text, a number or the name of another value."
                ),
            )
            .suggest(format!("{name} = \"some text\"")));
        }
        let value = self.expression()?;
        Ok(Statement::Let {
            name: name.to_string(),
            name_span: name_token.span(),
            value,
        })
    }

    fn expression(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek();
        let kind = match &token.kind {
            TokenKind::Text(text) => ExprKind::Text(text.clone()),
            TokenKind::Number(number) => ExprKind::Number(number.clone()),
            TokenKind::Identifier(name) => ExprKind::Name(name.clone()),
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
                         A value can be text, a number or the name of another value.",
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

    /// A line that starts with a name but isn't `name = value`.
    fn stray_name(&mut self, name: &str, token: &Token) -> Diagnostic {
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

        if matches!(self.peek().kind, TokenKind::EqualEqual) {
            self.advance();
            let suggestion = if self.at_end_of_line() {
                format!("check {name} == \"expected\"")
            } else {
                match self.expression() {
                    Ok(value) => format!("check {name} == {value}"),
                    Err(_) => format!("check {name} == \"expected\""),
                }
            };
            return Diagnostic::new(
                token.span(),
                format!(
                    "{name} is being compared, but a comparison belongs in a check.\n\
                     check runs as a test and passes when both sides are the same value."
                ),
            )
            .suggest(suggestion);
        }

        let diagnostic = Diagnostic::new(
            token.span(),
            format!(
                "Quill doesn't know what to do with {name} here.\n\
                 A line can start with say, check, or a name and = to create a value."
            ),
        );
        match self.expression() {
            Ok(value) if closest_match(name, ["say"]).is_some() => {
                diagnostic.suggest(format!("say {value}"))
            }
            Ok(value) => diagnostic.suggest(format!("{name} = {value}")),
            Err(_) => diagnostic,
        }
    }

    fn at_end_of_line(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Newline | TokenKind::EndOfFile)
    }

    fn end_of_line(&mut self, statement: &Statement) -> Result<(), Diagnostic> {
        if self.at_end_of_line() {
            return Ok(());
        }
        let extra = self.peek();
        Err(Diagnostic::new(
            extra.span(),
            format!(
                "Quill expected the line to end after `{statement}`, but found {}.\n\
                 Each line holds one statement.",
                describe(&extra.kind)
            ),
        ))
    }
}

fn describe(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Say => "say".to_string(),
        TokenKind::Check => "check".to_string(),
        TokenKind::Text(text) => format!("the text {text:?}"),
        TokenKind::Number(number) => format!("the number {number}"),
        TokenKind::Identifier(name) => format!("the name {name}"),
        TokenKind::Equals => "=".to_string(),
        TokenKind::EqualEqual => "==".to_string(),
        TokenKind::Example => "a doc example".to_string(),
        TokenKind::Newline => "the end of the line".to_string(),
        TokenKind::EndOfFile => "the end of the file".to_string(),
    }
}

/// A span from the start of `start` through the end of `end`, on the same line.
fn cover(start: Span, end: Span) -> Span {
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
        let program = parse_source("x = \"hi\"\n  say x").unwrap();
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
                },
                Statement::Say {
                    value: Expr {
                        kind: ExprKind::Name("x".to_string()),
                        span: span(2, 7, 1),
                    },
                    span: span(2, 3, 3),
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
}
