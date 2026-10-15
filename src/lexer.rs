//! The lexer: turns Quill source text into a flat list of tokens.
//!
//! It reads keywords (`say`, `check`, `var`, `set`, `when`, `else`, `for`, `in`,
//! `and`, `or`, `not`, `true`, `false`), text (with `{value}` inside), numbers, names,
//! operators, comments (`--` and `---`) and new lines.
//!
//! Quill uses indentation for blocks, like the cases under `when`. Each block
//! moves in by exactly 2 spaces. The lexer turns that indentation into `Indent`
//! and `Dedent` tokens, so the parser can treat a block like a pair of brackets.
//!
//! Every token remembers the line and column it started on (both 1-based,
//! counted in characters) so later stages can point at the exact spot.

use std::fmt;

use crate::diagnostic::{Diagnostic, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Say,
    Check,
    Var,
    Set,
    When,
    Else,
    For,
    In,
    List,
    Record,
    Of,
    At,
    /// `get fee of roster`, or `get bookings with acts in roster`.
    Get,
    /// `sum fee of roster`.
    Sum,
    /// The embedded field in `get bookings with acts in roster`.
    With,
    /// `fn`, a function.
    Fn,
    /// `return`, the value a function gives back.
    Return,
    /// `maybe`, a value that might be missing.
    Maybe,
    /// `found`, the value inside a maybe.
    Found,
    /// `missing`, a maybe with no value.
    Missing,
    /// `fails`, the failures a function declares.
    Fails,
    /// `fail`, leaving a function with one of those failures.
    Fail,
    /// `on`, what to do when a call fails.
    On,
    /// `try`, passing a call's failures up to the function that declared them.
    Try,
    True,
    False,
    And,
    Or,
    Not,
    /// A text literal, with escapes already resolved.
    Text(String),
    /// Text with values inside, like `"Hello, {name}"`.
    Template(Vec<Piece>),
    /// A number exactly as written, so `Decimal` and `Money` can stay exact.
    Number(String),
    Identifier(String),
    Equals,
    /// `==`, used by `check`, doc examples and `when`.
    EqualEqual,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LeftParen,
    RightParen,
    /// `:`, between a parameter and its kind.
    Colon,
    /// `->`, between a `when` case and what to do, or a function and the kind it gives back.
    Arrow,
    /// `..`, the range in `for i in 1..3`.
    DotDot,
    /// `.`, the field of a record: `address.city`.
    Dot,
    /// `,`, between the items of a list or the fields of a one-line record.
    Comma,
    /// The `--- example:` marker. The comparison itself follows on the same line.
    Example,
    /// The start of a more indented block.
    Indent,
    /// The end of an indented block.
    Dedent,
    Newline,
    EndOfFile,
}

/// One part of a text with values inside.
#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    Text(String),
    /// The code between `{` and `}`: as written, and as tokens ending with `EndOfFile`.
    Code {
        source: String,
        tokens: Vec<Token>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
    /// How many characters of source the token covers.
    pub length: usize,
}

impl Token {
    pub fn span(&self) -> Span {
        Span {
            line: self.line,
            column: self.column,
            length: self.length,
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let symbol = match self {
            TokenKind::Text(text) => return write!(f, "Text {text:?}"),
            TokenKind::Template(pieces) => {
                write!(f, "Text \"")?;
                for piece in pieces {
                    match piece {
                        Piece::Text(text) => {
                            let escaped = format!("{text:?}");
                            let inner = &escaped[1..escaped.len() - 1];
                            write!(f, "{}", inner.replace('{', "\\{").replace('}', "\\}"))?;
                        }
                        Piece::Code { source, .. } => write!(f, "{{{source}}}")?,
                    }
                }
                return write!(f, "\"");
            }
            TokenKind::Number(number) => return write!(f, "Number {number}"),
            TokenKind::Identifier(name) => return write!(f, "Identifier {name}"),
            TokenKind::Say => "say",
            TokenKind::Check => "check",
            TokenKind::Var => "var",
            TokenKind::Set => "set",
            TokenKind::When => "when",
            TokenKind::Else => "else",
            TokenKind::For => "for",
            TokenKind::In => "in",
            TokenKind::List => "list",
            TokenKind::Record => "record",
            TokenKind::Of => "of",
            TokenKind::At => "at",
            TokenKind::Get => "get",
            TokenKind::Sum => "sum",
            TokenKind::With => "with",
            TokenKind::Fn => "fn",
            TokenKind::Return => "return",
            TokenKind::Maybe => "maybe",
            TokenKind::Found => "found",
            TokenKind::Missing => "missing",
            TokenKind::Fails => "fails",
            TokenKind::Fail => "fail",
            TokenKind::On => "on",
            TokenKind::Try => "try",
            TokenKind::True => "true",
            TokenKind::False => "false",
            TokenKind::And => "and",
            TokenKind::Or => "or",
            TokenKind::Not => "not",
            TokenKind::Equals => "=",
            TokenKind::EqualEqual => "==",
            TokenKind::NotEqual => "!=",
            TokenKind::Less => "<",
            TokenKind::LessEqual => "<=",
            TokenKind::Greater => ">",
            TokenKind::GreaterEqual => ">=",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::LeftParen => "(",
            TokenKind::RightParen => ")",
            TokenKind::Colon => ":",
            TokenKind::Arrow => "->",
            TokenKind::DotDot => "..",
            TokenKind::Dot => ".",
            TokenKind::Comma => ",",
            TokenKind::Example => "example",
            TokenKind::Indent => "Indent",
            TokenKind::Dedent => "Dedent",
            TokenKind::Newline => "Newline",
            TokenKind::EndOfFile => "EndOfFile",
        };
        write!(f, "{symbol}")
    }
}

pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    Lexer::new(source.chars().collect(), 1, 1, false).run()
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
    /// Widths of the open indented blocks. Starts with 0 for the top level.
    indents: Vec<usize>,
    at_line_start: bool,
    /// Lexing the code inside `{}` in a text: no lines, no indentation.
    in_code: bool,
}

impl Lexer {
    fn new(chars: Vec<char>, line: usize, column: usize, in_code: bool) -> Self {
        Lexer {
            chars,
            pos: 0,
            line,
            column,
            indents: vec![0],
            at_line_start: !in_code,
            in_code,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    fn run(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();

        while let Some(c) = self.peek() {
            if self.at_line_start {
                self.at_line_start = false;
                self.indentation(&mut tokens)?;
                continue;
            }

            let line = self.line;
            let column = self.column;

            let kind = match c {
                ' ' | '\t' => {
                    self.advance();
                    continue;
                }
                // Windows line ending: \r\n is one Newline, placed where the line ends.
                '\r' if self.peek_next() == Some('\n') => {
                    self.advance();
                    self.advance();
                    self.at_line_start = !self.in_code;
                    TokenKind::Newline
                }
                '\n' => {
                    self.advance();
                    self.at_line_start = !self.in_code;
                    TokenKind::Newline
                }
                '=' => self.one_or_two('=', TokenKind::Equals, TokenKind::EqualEqual),
                '<' => self.one_or_two('=', TokenKind::Less, TokenKind::LessEqual),
                '>' => self.one_or_two('=', TokenKind::Greater, TokenKind::GreaterEqual),
                '!' if self.peek_next() == Some('=') => {
                    self.advance();
                    self.advance();
                    TokenKind::NotEqual
                }
                '-' if self.peek_next() == Some('-') => match self.comment()? {
                    Some(kind) => kind,
                    None => continue,
                },
                '-' => self.one_or_two('>', TokenKind::Minus, TokenKind::Arrow),
                '.' if self.peek_next() == Some('.') => {
                    self.advance();
                    self.advance();
                    if self.peek() == Some('.') {
                        self.advance();
                        return Err(three_dots(line, column, &tokens, &mut self));
                    }
                    TokenKind::DotDot
                }
                '.' => self.single(TokenKind::Dot),
                ',' => self.single(TokenKind::Comma),
                '+' => self.single(TokenKind::Plus),
                '*' => self.single(TokenKind::Star),
                '/' => self.single(TokenKind::Slash),
                '%' => self.single(TokenKind::Percent),
                '(' => self.single(TokenKind::LeftParen),
                ')' => self.single(TokenKind::RightParen),
                ':' => self.single(TokenKind::Colon),
                '"' => self.text(line, column)?,
                c if c.is_ascii_digit() => self.number(line, column)?,
                c if c.is_alphabetic() || c == '_' => self.word(),
                other => return Err(unexpected(other, line, column)),
            };

            // Tokens never span lines; a new line counts as one character.
            let length = if kind == TokenKind::Newline {
                1
            } else {
                self.column - column
            };
            tokens.push(Token {
                kind,
                line,
                column,
                length,
            });
        }

        // Close every block that is still open.
        while self.indents.len() > 1 {
            self.indents.pop();
            tokens.push(self.marker(TokenKind::Dedent));
        }
        tokens.push(self.marker(TokenKind::EndOfFile));
        Ok(tokens)
    }

    fn single(&mut self, kind: TokenKind) -> TokenKind {
        self.advance();
        kind
    }

    /// A one-character token, or a two-character one when `second` follows.
    fn one_or_two(&mut self, second: char, one: TokenKind, two: TokenKind) -> TokenKind {
        self.advance();
        if self.peek() == Some(second) {
            self.advance();
            two
        } else {
            one
        }
    }

    /// A zero-width token at the current position.
    fn marker(&self, kind: TokenKind) -> Token {
        Token {
            kind,
            line: self.line,
            column: self.column,
            length: 0,
        }
    }

    /// Reads the indentation at the start of a line and opens or closes blocks.
    /// Blank lines and comment lines don't change the indentation.
    fn indentation(&mut self, tokens: &mut Vec<Token>) -> Result<(), Diagnostic> {
        let line = self.line;
        let mut width = 0;
        let (mut tabs, mut spaces) = (false, false);
        while let Some(c @ (' ' | '\t')) = self.peek() {
            tabs |= c == '\t';
            spaces |= c == ' ';
            width += 1;
            self.advance();
        }
        if self.rest_of_line_is_empty() {
            return Ok(());
        }
        if tabs {
            let message = if spaces {
                "This line is indented with a mix of tabs and spaces.\n\
                 A block moves in by exactly 2 spaces. Tabs are wider in some editors than others, \
                 so Quill doesn't use them."
            } else {
                "This line is indented with a tab.\n\
                 A block moves in by exactly 2 spaces. Tabs are wider in some editors than others, \
                 so Quill doesn't use them."
            };
            return Err(Diagnostic::new(
                Span {
                    line,
                    column: 1,
                    length: width,
                },
                message,
            ));
        }

        let current = self.indents.last().copied().unwrap_or(0);
        if width > current {
            if width != current + 2 {
                let unit = if width == 1 { "space" } else { "spaces" };
                return Err(Diagnostic::new(
                    Span {
                        line,
                        column: 1,
                        length: width,
                    },
                    format!(
                        "This line is indented by {width} {unit}.\n\
                         A block moves in by exactly 2 spaces from the line above it, \
                         so this line should be indented by {} spaces.",
                        current + 2
                    ),
                ));
            }
            self.indents.push(width);
            tokens.push(self.marker(TokenKind::Indent));
        } else if width < current {
            while self.indents.last().is_some_and(|&open| open > width) {
                self.indents.pop();
                tokens.push(self.marker(TokenKind::Dedent));
            }
            if self.indents.last() != Some(&width) {
                return Err(Diagnostic::new(
                    Span {
                        line,
                        column: 1,
                        length: width.max(1),
                    },
                    "This line's indentation doesn't line up with any line above it.\n\
                     A line that ends a block comes back by 2 spaces at a time, \
                     to the same column as the line that opened the block.",
                ));
            }
        }
        Ok(())
    }

    /// True when the rest of this line is empty or only a comment.
    fn rest_of_line_is_empty(&self) -> bool {
        match self.peek() {
            None | Some('\n') => true,
            Some('\r') => self.peek_next() == Some('\n') || self.peek_next().is_none(),
            Some('-') => self.starts_with("--") && !self.at_example(),
            _ => false,
        }
    }

    /// True at `--- example:` (with any spaces after the dashes).
    fn at_example(&self) -> bool {
        if !self.starts_with("---") {
            return false;
        }
        let mut rest = self.chars[self.pos + 3..]
            .iter()
            .copied()
            .skip_while(|c| *c == ' ' || *c == '\t');
        "example:"
            .chars()
            .all(|expected| rest.next() == Some(expected))
    }

    fn text(&mut self, line: usize, column: usize) -> Result<TokenKind, Diagnostic> {
        let start = self.pos;
        self.advance(); // opening quote
                        // """...""" holds quotes as themselves, so "hi" needs no backslash.
        if self.peek() == Some('"') && self.peek_at(1) == Some('"') {
            self.advance();
            self.advance();
            return self.plain_text(start, line, column);
        }

        let mut builder = TextBuilder::default();
        loop {
            match self.peek() {
                None | Some('\n') => {
                    return Err(self.unclosed_text(
                        start,
                        line,
                        column,
                        "\"",
                        "This text starts with a quote but never ends.\n\
                         Text must open and close with \" on the same line.",
                    ));
                }
                Some('"') => {
                    self.advance();
                    return Ok(builder.finish());
                }
                Some('\\') => {
                    if let Some(resolved) = self.escape()? {
                        builder.push(resolved);
                    }
                }
                Some('{') => builder.code(self.code_piece()?),
                Some('}') => return Err(self.stray_close_brace()),
                Some(c) => {
                    self.advance();
                    builder.push(c);
                }
            }
        }
    }

    /// Text wrapped in `"""`. A `"` inside is a quote. Escapes and `{value}` still work.
    fn plain_text(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
    ) -> Result<TokenKind, Diagnostic> {
        let mut builder = TextBuilder::default();
        loop {
            if self.starts_with("\"\"\"") {
                self.advance();
                self.advance();
                self.advance();
                return Ok(builder.finish());
            }
            match self.peek() {
                None | Some('\n') => {
                    return Err(self.unclosed_text(
                        start,
                        line,
                        column,
                        "\"\"\"",
                        "This text starts with three quotes but never ends.\n\
                         Write \"\"\" again after the text. A quote inside needs no backslash.",
                    ));
                }
                Some('\\') => {
                    if let Some(resolved) = self.escape()? {
                        builder.push(resolved);
                    }
                }
                Some('{') => builder.code(self.code_piece()?),
                Some('}') => return Err(self.stray_close_brace()),
                Some(c) => {
                    self.advance();
                    builder.push(c);
                }
            }
        }
    }

    fn unclosed_text(
        &self,
        start: usize,
        line: usize,
        column: usize,
        closing: &str,
        message: &str,
    ) -> Diagnostic {
        let raw: String = self.chars[start..self.pos].iter().collect();
        let raw = raw.trim_end_matches('\r');
        Diagnostic {
            line,
            column,
            length: raw.chars().count(),
            message: message.to_string(),
            suggestion: Some(format!("{raw}{closing}")),
        }
    }

    /// A `{value}` inside text. The `{` has not been consumed yet.
    fn code_piece(&mut self) -> Result<Piece, Diagnostic> {
        let open = Span {
            line: self.line,
            column: self.column,
            length: 1,
        };
        self.advance(); // {
        let start = self.pos;
        let code_column = self.column;
        loop {
            match self.peek() {
                Some('}') => break,
                // A quote here almost always means the } was forgotten.
                None | Some('\n') | Some('\r') | Some('"') => {
                    let inner: String = self.chars[start..self.pos].iter().collect();
                    return Err(Diagnostic::new(
                        open,
                        "This { inside text is never closed.\n\
                         A value inside text goes between { and }, and text can't go inside the braces.",
                    )
                    .suggest(format!("{{{}}}", inner.trim())));
                }
                Some('{') => {
                    return Err(Diagnostic::new(
                        Span {
                            line: self.line,
                            column: self.column,
                            length: 1,
                        },
                        "This { is inside another {}.\n\
                         A value inside text can't hold another {. To show a { itself, write \\{.",
                    )
                    .suggest("\\{"))
                }
                Some(_) => {
                    self.advance();
                }
            }
        }
        let source: String = self.chars[start..self.pos].iter().collect();
        self.advance(); // }
        if source.trim().is_empty() {
            return Err(Diagnostic::new(
                Span { length: 2, ..open },
                "These braces are empty.\n\
                 Put the value to show between them, like {name}. To show { itself, write \\{.",
            )
            .suggest("{name}"));
        }
        let code = Lexer::new(source.chars().collect(), open.line, code_column, true);
        let tokens = code.run()?;
        Ok(Piece::Code {
            source: source.trim().to_string(),
            tokens,
        })
    }

    fn stray_close_brace(&self) -> Diagnostic {
        Diagnostic::new(
            Span {
                line: self.line,
                column: self.column,
                length: 1,
            },
            "This } doesn't close a {.\n\
             Braces in text hold a value, like {name}. To show a } itself, write \\}.",
        )
        .suggest("\\}")
    }

    /// One escape after a backslash that has not been consumed yet.
    /// `None` means the escape ran off the end of the line; the caller reports that.
    fn escape(&mut self) -> Result<Option<char>, Diagnostic> {
        let escape_column = self.column;
        self.advance();
        let resolved = match self.peek() {
            Some('n') => '\n',
            Some('t') => '\t',
            Some('"') => '"',
            Some('\\') => '\\',
            Some('{') => '{',
            Some('}') => '}',
            None | Some('\n') | Some('\r') => return Ok(None),
            Some(other) => {
                return Err(Diagnostic {
                    line: self.line,
                    column: escape_column,
                    length: 2,
                    message: format!(
                        "`\\{other}` isn't an escape Quill knows.\n\
                         Inside text, a backslash must be followed by n (new line), \
                         t (tab), \" (a quote), \\ (a backslash), or {{ or }} (a brace)."
                    ),
                    suggestion: Some(format!(
                        "\\\\{other}   (to write a backslash followed by {other})"
                    )),
                });
            }
        };
        self.advance();
        Ok(Some(resolved))
    }

    fn number(&mut self, line: usize, column: usize) -> Result<TokenKind, Diagnostic> {
        let mut number = self.take_while(|c| c.is_ascii_digit());

        // Only treat `.` as a decimal point when a digit follows, so `1..15`
        // is a range, not a number.
        if self.peek() == Some('.') && self.peek_next().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
            number.push('.');
            number.push_str(&self.take_while(|c| c.is_ascii_digit()));
        }

        if self.peek().is_some_and(|c| c.is_alphabetic() || c == '_') {
            let rest = self.take_while(|c| c.is_alphanumeric() || c == '_');
            return Err(Diagnostic {
                line,
                column,
                length: number.chars().count() + rest.chars().count(),
                message: format!(
                    "`{number}{rest}` starts like a number but continues with letters.\n\
                     Names can't start with a digit, and a number needs a space before the next word."
                ),
                suggestion: Some(format!("{number} {rest}")),
            });
        }

        Ok(TokenKind::Number(number))
    }

    fn word(&mut self) -> TokenKind {
        let word = self.take_while(|c| c.is_alphanumeric() || c == '_');
        match word.as_str() {
            "say" => TokenKind::Say,
            "check" => TokenKind::Check,
            "var" => TokenKind::Var,
            "set" => TokenKind::Set,
            "when" => TokenKind::When,
            "else" => TokenKind::Else,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "list" => TokenKind::List,
            "record" => TokenKind::Record,
            "of" => TokenKind::Of,
            "at" => TokenKind::At,
            "get" => TokenKind::Get,
            "sum" => TokenKind::Sum,
            "with" => TokenKind::With,
            "fn" => TokenKind::Fn,
            "return" => TokenKind::Return,
            "maybe" => TokenKind::Maybe,
            "found" => TokenKind::Found,
            "missing" => TokenKind::Missing,
            "fails" => TokenKind::Fails,
            "fail" => TokenKind::Fail,
            "on" => TokenKind::On,
            "try" => TokenKind::Try,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            _ => TokenKind::Identifier(word),
        }
    }

    /// `--` is a comment to the end of the line. `--- example:` is a doc example,
    /// and the comparison after it is tokenized normally. Anything else after
    /// `---` is a doc comment and is skipped.
    fn comment(&mut self) -> Result<Option<TokenKind>, Diagnostic> {
        if self.at_example() {
            for _ in 0.."---".len() {
                self.advance();
            }
            while matches!(self.peek(), Some(' ' | '\t')) {
                self.advance();
            }
            for _ in 0.."example:".len() {
                self.advance();
            }
            while matches!(self.peek(), Some(' ' | '\t')) {
                self.advance();
            }
            return Ok(Some(TokenKind::Example));
        }
        while let Some(c) = self.peek() {
            if c == '\n' || c == '\r' {
                break;
            }
            self.advance();
        }
        Ok(None)
    }

    fn starts_with(&self, text: &str) -> bool {
        self.chars[self.pos..]
            .iter()
            .copied()
            .take(text.chars().count())
            .eq(text.chars())
    }

    fn take_while(&mut self, keep: impl Fn(char) -> bool) -> String {
        let mut taken = String::new();
        while let Some(c) = self.peek() {
            if !keep(c) {
                break;
            }
            self.advance();
            taken.push(c);
        }
        taken
    }
}

/// Collects the pieces of a text, so plain text stays a simple `Text` token.
#[derive(Default)]
struct TextBuilder {
    pieces: Vec<Piece>,
    current: String,
}

impl TextBuilder {
    fn push(&mut self, c: char) {
        self.current.push(c);
    }

    fn code(&mut self, piece: Piece) {
        if !self.current.is_empty() {
            self.pieces
                .push(Piece::Text(std::mem::take(&mut self.current)));
        }
        self.pieces.push(piece);
    }

    fn finish(mut self) -> TokenKind {
        if self.pieces.is_empty() {
            return TokenKind::Text(self.current);
        }
        if !self.current.is_empty() {
            self.pieces.push(Piece::Text(self.current));
        }
        TokenKind::Template(self.pieces)
    }
}

/// `18...25` is a common way to write a range. Quill's range is two dots.
fn three_dots(line: usize, column: usize, tokens: &[Token], lexer: &mut Lexer) -> Diagnostic {
    let end = lexer.take_while(|c| c.is_ascii_digit());
    let start = match tokens.last() {
        Some(Token {
            kind: TokenKind::Number(number),
            ..
        }) => number.clone(),
        _ => String::new(),
    };
    let suggestion = if !start.is_empty() && !end.is_empty() {
        format!("{start}..{end}")
    } else {
        "1..3".to_string()
    };
    Diagnostic::new(
        Span {
            line,
            column,
            length: 3,
        },
        format!(
            "A range uses two dots, not three, as in {suggestion}.\n\
             {suggestion} counts from the first number through the last, including both ends."
        ),
    )
    .suggest(suggestion)
}

fn unexpected(c: char, line: usize, column: usize) -> Diagnostic {
    let span = Span {
        line,
        column,
        length: 1,
    };
    match c {
        '\'' => Diagnostic::new(
            span,
            "Quill doesn't use single quotes.\nText is always written in double quotes.",
        )
        .suggest("\"like this\""),
        '!' => Diagnostic::new(
            span,
            "Quill writes not instead of !.\n\
             Words read more clearly than symbols, so use not, or != for \"is not equal to\".",
        )
        .suggest("not"),
        '&' => Diagnostic::new(
            span,
            "Quill writes and instead of &.\nUse and between two conditions that must both be true.",
        )
        .suggest("and"),
        '|' => Diagnostic::new(
            span,
            "Quill writes or instead of |.\nUse or between two conditions when either one is enough.",
        )
        .suggest("or"),
        ';' => Diagnostic::new(
            span,
            "Quill doesn't use semicolons.\nEach statement goes on its own line instead.",
        ),
        '[' | ']' => Diagnostic::new(
            span,
            "Quill writes a list with the word list, not with square brackets.\n\
             A list is values of one kind, in order: list \"coding\", \"reading\".",
        )
        .suggest("list \"coding\", \"reading\""),
        '{' | '}' => Diagnostic::new(
            span,
            "Quill writes a record with the word record, not with braces.\n\
             A record keeps named fields together. Put the fields on the following lines, indented by 2 spaces.\n\
             Braces inside text still show a value: \"Hello, {name}\".",
        )
        .suggest("record\n  city = \"Anytown\""),
        _ => Diagnostic::new(
            span,
            format!(
                "Quill doesn't understand the character {c:?} here.\n\
                 It isn't part of any Quill word, number or symbol."
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;

    /// Lexes `source` and returns (kind, line, column) for each token.
    fn tokens(source: &str) -> Vec<(TokenKind, usize, usize)> {
        lex(source)
            .expect("source should lex")
            .into_iter()
            .map(|t| (t.kind, t.line, t.column))
            .collect()
    }

    fn text(s: &str) -> TokenKind {
        Text(s.to_string())
    }

    fn number(s: &str) -> TokenKind {
        Number(s.to_string())
    }

    fn ident(s: &str) -> TokenKind {
        Identifier(s.to_string())
    }

    #[test]
    fn empty_source_is_just_end_of_file() {
        assert_eq!(tokens(""), vec![(EndOfFile, 1, 1)]);
    }

    #[test]
    fn tokens_know_their_length() {
        let lengths: Vec<usize> = lex("say \"héllo\" 3.14 name =\r\n")
            .unwrap()
            .iter()
            .map(|t| t.length)
            .collect();
        assert_eq!(lengths, vec![3, 7, 4, 4, 1, 1, 0]);
    }

    #[test]
    fn hello_world() {
        assert_eq!(
            tokens("say \"Hello, World!\""),
            vec![
                (Say, 1, 1),
                (text("Hello, World!"), 1, 5),
                (EndOfFile, 1, 20)
            ]
        );
    }

    #[test]
    fn values_across_lines_have_positions() {
        assert_eq!(
            tokens("name = \"Amina\"\nage = 30\n"),
            vec![
                (ident("name"), 1, 1),
                (Equals, 1, 6),
                (text("Amina"), 1, 8),
                (Newline, 1, 15),
                (ident("age"), 2, 1),
                (Equals, 2, 5),
                (number("30"), 2, 7),
                (Newline, 2, 9),
                (EndOfFile, 3, 1),
            ]
        );
    }

    #[test]
    fn say_with_identifier() {
        assert_eq!(
            tokens("say greeting"),
            vec![(Say, 1, 1), (ident("greeting"), 1, 5), (EndOfFile, 1, 13)]
        );
    }

    #[test]
    fn check_and_equals_are_their_own_tokens() {
        assert_eq!(
            tokens("check greeting == \"Hi\""),
            vec![
                (Check, 1, 1),
                (ident("greeting"), 1, 7),
                (EqualEqual, 1, 16),
                (text("Hi"), 1, 19),
                (EndOfFile, 1, 23),
            ]
        );
    }

    #[test]
    fn comments_are_skipped_and_examples_stay() {
        assert_eq!(
            tokens("say \"hi\" -- note\n--- just a doc\n--- example: greeting == \"Hi\""),
            vec![
                (Say, 1, 1),
                (text("hi"), 1, 5),
                (Newline, 1, 17),
                (Newline, 2, 15),
                (Example, 3, 1),
                (ident("greeting"), 3, 14),
                (EqualEqual, 3, 23),
                (text("Hi"), 3, 26),
                (EndOfFile, 3, 30),
            ]
        );
    }

    #[test]
    fn one_dash_is_minus_two_are_a_comment() {
        assert_eq!(
            tokens("-5 -> x --note"),
            vec![
                (Minus, 1, 1),
                (number("5"), 1, 2),
                (Arrow, 1, 4),
                (ident("x"), 1, 7),
                (EndOfFile, 1, 15),
            ]
        );
    }

    #[test]
    fn operators() {
        let kinds: Vec<TokenKind> = tokens("+ - * / % ( ) < <= > >= == != = ->")
            .into_iter()
            .map(|(kind, _, _)| kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                Plus,
                Minus,
                Star,
                Slash,
                Percent,
                LeftParen,
                RightParen,
                Less,
                LessEqual,
                Greater,
                GreaterEqual,
                EqualEqual,
                NotEqual,
                Equals,
                Arrow,
                EndOfFile,
            ]
        );
    }

    #[test]
    fn keywords() {
        let kinds: Vec<TokenKind> = tokens(
            "var set when else for in list record of at get sum with fn return maybe found missing fails fail on try true false and or not variable",
        )
        .into_iter()
        .map(|(kind, _, _)| kind)
        .collect();
        assert_eq!(
            kinds,
            vec![
                Var,
                Set,
                When,
                Else,
                For,
                In,
                List,
                Record,
                Of,
                At,
                Get,
                Sum,
                With,
                Fn,
                Return,
                Maybe,
                Found,
                Missing,
                Fails,
                Fail,
                On,
                Try,
                True,
                False,
                And,
                Or,
                Not,
                ident("variable"),
                EndOfFile
            ]
        );
    }

    #[test]
    fn symbols_from_other_languages_suggest_the_quill_word() {
        assert_eq!(
            lex("x = !y").unwrap_err().suggestion.as_deref(),
            Some("not")
        );
        assert_eq!(lex("a & b").unwrap_err().suggestion.as_deref(), Some("and"));
        assert_eq!(lex("a | b").unwrap_err().suggestion.as_deref(), Some("or"));
        assert!(lex("say 1;").unwrap_err().message.contains("semicolons"));
        assert!(lex("x = {").unwrap_err().message.contains("word record"));
        assert!(lex("x = [").unwrap_err().message.contains("word list"));
    }

    #[test]
    fn say_is_only_a_keyword_as_a_whole_word() {
        assert_eq!(
            tokens("sayings say_hi Say _say"),
            vec![
                (ident("sayings"), 1, 1),
                (ident("say_hi"), 1, 9),
                (ident("Say"), 1, 16),
                (ident("_say"), 1, 20),
                (EndOfFile, 1, 24),
            ]
        );
    }

    #[test]
    fn identifiers_may_contain_digits_and_unicode() {
        assert_eq!(
            tokens("total2 = café"),
            vec![
                (ident("total2"), 1, 1),
                (Equals, 1, 8),
                (ident("café"), 1, 10),
                (EndOfFile, 1, 14),
            ]
        );
    }

    #[test]
    fn decimals_are_kept_exactly_as_written() {
        assert_eq!(
            tokens("0.10 1.05 42"),
            vec![
                (number("0.10"), 1, 1),
                (number("1.05"), 1, 6),
                (number("42"), 1, 11),
                (EndOfFile, 1, 13),
            ]
        );
    }

    #[test]
    fn two_dots_are_a_range() {
        assert_eq!(
            tokens("for i in 1..15"),
            vec![
                (For, 1, 1),
                (ident("i"), 1, 5),
                (In, 1, 7),
                (number("1"), 1, 10),
                (DotDot, 1, 11),
                (number("15"), 1, 13),
                (EndOfFile, 1, 15),
            ]
        );
    }

    #[test]
    fn three_dots_are_not_a_range() {
        let err = lex("for age in 18...25").unwrap_err();
        assert_eq!((err.line, err.column, err.length), (1, 14, 3));
        assert_eq!(err.suggestion.as_deref(), Some("18..25"));
        assert!(err.message.contains("two dots"));
    }

    #[test]
    fn dot_without_following_digit_is_a_field_dot() {
        assert_eq!(
            tokens("x = 3."),
            vec![
                (ident("x"), 1, 1),
                (Equals, 1, 3),
                (number("3"), 1, 5),
                (Dot, 1, 6),
                (EndOfFile, 1, 7),
            ]
        );
    }

    #[test]
    fn three_quotes_hold_a_quote_without_a_backslash() {
        assert_eq!(
            tokens("\"\"\"She said \"hi\".\"\"\""),
            vec![(text("She said \"hi\"."), 1, 1), (EndOfFile, 1, 21)]
        );
    }

    #[test]
    fn three_quotes_must_close() {
        let err = lex("\"\"\"She said \"hi\"").unwrap_err();
        assert_eq!((err.line, err.column), (1, 1));
        assert!(err.message.contains("three quotes"));
        assert_eq!(
            err.suggestion.as_deref(),
            Some("\"\"\"She said \"hi\"\"\"\"")
        );
    }

    #[test]
    fn escapes_in_text_are_resolved() {
        assert_eq!(
            tokens(r#""a\"b\\c\nd\te""#),
            vec![(text("a\"b\\c\nd\te"), 1, 1), (EndOfFile, 1, 16)]
        );
    }

    #[test]
    fn columns_count_characters_not_bytes() {
        assert_eq!(
            tokens("say \"héllo\" x"),
            vec![
                (Say, 1, 1),
                (text("héllo"), 1, 5),
                (ident("x"), 1, 13),
                (EndOfFile, 1, 14)
            ]
        );
    }

    #[test]
    fn windows_line_endings_make_one_newline() {
        assert_eq!(
            tokens("say 1\r\nsay 2"),
            vec![
                (Say, 1, 1),
                (number("1"), 1, 5),
                (Newline, 1, 6),
                (Say, 2, 1),
                (number("2"), 2, 5),
                (EndOfFile, 2, 6),
            ]
        );
    }

    #[test]
    fn blank_lines_dont_end_a_block() {
        assert_eq!(
            tokens("  say x\n\n"),
            vec![
                (Indent, 1, 3),
                (Say, 1, 3),
                (ident("x"), 1, 7),
                (Newline, 1, 8),
                (Newline, 2, 1),
                (Dedent, 3, 1),
                (EndOfFile, 3, 1),
            ]
        );
    }

    #[test]
    fn a_block_moves_in_by_exactly_two_spaces() {
        let err = lex("when\n    ready -> say 1").unwrap_err();
        assert_eq!((err.line, err.column), (2, 1));
        assert!(err.message.contains("indented by 4 spaces"));
        assert!(err.message.contains("exactly 2 spaces"));

        let err = lex("when\n ready -> say 1").unwrap_err();
        assert!(err.message.contains("indented by 1 space"));

        let err = lex("when\n  ready -> say 1\n      say 2").unwrap_err();
        assert_eq!(err.line, 3);
        assert!(err.message.contains("indented by 4 spaces"));
    }

    #[test]
    fn a_tab_is_not_an_indent() {
        let err = lex("when\n\tready -> say 1").unwrap_err();
        assert_eq!((err.line, err.column, err.length), (2, 1, 1));
        assert!(err.message.contains("indented with a tab"));
        assert!(err.message.contains("exactly 2 spaces"));
    }

    #[test]
    fn indentation_opens_and_closes_blocks() {
        assert_eq!(
            tokens("when\n  a -> b\n  c\nsay 1"),
            vec![
                (When, 1, 1),
                (Newline, 1, 5),
                (Indent, 2, 3),
                (ident("a"), 2, 3),
                (Arrow, 2, 5),
                (ident("b"), 2, 8),
                (Newline, 2, 9),
                (ident("c"), 3, 3),
                (Newline, 3, 4),
                (Dedent, 4, 1),
                (Say, 4, 1),
                (number("1"), 4, 5),
                (EndOfFile, 4, 6),
            ]
        );
    }

    #[test]
    fn several_blocks_can_close_at_once() {
        let kinds: Vec<TokenKind> = tokens("a\n  b\n    c\nd")
            .into_iter()
            .map(|(kind, _, _)| kind)
            .filter(|kind| matches!(kind, Indent | Dedent))
            .collect();
        assert_eq!(kinds, vec![Indent, Indent, Dedent, Dedent]);
    }

    #[test]
    fn blank_and_comment_lines_dont_change_indentation() {
        let kinds: Vec<TokenKind> = tokens("a\n  b\n\n-- note\n     \n  c\n")
            .into_iter()
            .map(|(kind, _, _)| kind)
            .filter(|kind| matches!(kind, Indent | Dedent))
            .collect();
        assert_eq!(kinds, vec![Indent, Dedent]);
    }

    #[test]
    fn an_indented_example_is_a_statement_line() {
        let kinds: Vec<TokenKind> = tokens("a\n  --- example: x == 1\n")
            .into_iter()
            .map(|(kind, _, _)| kind)
            .collect();
        assert_eq!(kinds[2], Indent);
        assert_eq!(kinds[3], Example);
    }

    #[test]
    fn indentation_must_line_up() {
        let err = lex("when\n  a\n b").unwrap_err();
        assert_eq!((err.line, err.column), (3, 1));
        assert!(err.message.contains("doesn't line up"));
        assert!(err.message.contains("2 spaces"));
    }

    #[test]
    fn tabs_and_spaces_cannot_mix_in_one_indent() {
        let err = lex("when\n \ta").unwrap_err();
        assert_eq!((err.line, err.column, err.length), (2, 1, 2));
        assert!(err.message.contains("mix of tabs and spaces"));
    }

    #[test]
    fn values_inside_text() {
        let found = lex("say \"Hi, {name}!\"").unwrap();
        assert_eq!(
            found[1].kind,
            Template(vec![
                Piece::Text("Hi, ".to_string()),
                Piece::Code {
                    source: "name".to_string(),
                    tokens: vec![
                        Token {
                            kind: ident("name"),
                            line: 1,
                            column: 11,
                            length: 4
                        },
                        Token {
                            kind: EndOfFile,
                            line: 1,
                            column: 15,
                            length: 0
                        },
                    ],
                },
                Piece::Text("!".to_string()),
            ])
        );
        assert_eq!((found[1].column, found[1].length), (5, 13));
    }

    #[test]
    fn calculations_inside_text_show_as_written() {
        let found = lex("\"Total: { price * 2 }\"").unwrap();
        assert_eq!(found[0].kind.to_string(), "Text \"Total: {price * 2}\"");
        assert!(matches!(found[0].kind, Template(_)));
    }

    #[test]
    fn escaped_braces_are_plain_text() {
        assert_eq!(tokens(r#""\{x\}""#)[0].0, text("{x}"));
        assert_eq!(
            tokens("\"\"\"a {b}\"\"\"")[0].0.to_string(),
            "Text \"a {b}\""
        );
    }

    #[test]
    fn braces_in_text_must_close() {
        let err = lex("say \"Hi {name\"").unwrap_err();
        assert_eq!((err.line, err.column), (1, 9));
        assert!(err.message.contains("never closed"));
        assert_eq!(err.suggestion.as_deref(), Some("{name}"));
    }

    #[test]
    fn braces_in_text_cannot_be_empty() {
        let err = lex("\"a {} b\"").unwrap_err();
        assert_eq!((err.column, err.length), (4, 2));
        assert!(err.message.contains("empty"));
    }

    #[test]
    fn a_lone_closing_brace_needs_a_backslash() {
        let err = lex("\"a } b\"").unwrap_err();
        assert_eq!(err.column, 4);
        assert_eq!(err.suggestion.as_deref(), Some("\\}"));
    }

    #[test]
    fn mistakes_inside_braces_point_at_the_right_column() {
        let err = lex("say \"{price @ 2}\"").unwrap_err();
        assert_eq!((err.line, err.column), (1, 13));
    }

    #[test]
    fn unterminated_text_points_at_the_opening_quote() {
        let err = lex("say \"Hello").unwrap_err();
        assert_eq!((err.line, err.column, err.length), (1, 5, 6));
        assert!(err.message.contains("never ends"));
        assert_eq!(err.suggestion.as_deref(), Some("\"Hello\""));
    }

    #[test]
    fn text_cannot_span_lines() {
        let err = lex("x = 1\nsay \"Hi\nsay 2").unwrap_err();
        assert_eq!((err.line, err.column), (2, 5));
        assert_eq!(err.suggestion.as_deref(), Some("\"Hi\""));
    }

    #[test]
    fn backslash_at_end_of_text_reports_missing_quote() {
        let err = lex("say \"oops\\").unwrap_err();
        assert!(err.message.contains("never ends"));
    }

    #[test]
    fn unknown_escape_is_reported() {
        let err = lex("say \"a\\qb\"").unwrap_err();
        assert_eq!((err.line, err.column, err.length), (1, 7, 2));
        assert!(err.message.contains("\\q"));
    }

    #[test]
    fn unexpected_character_is_reported_with_position() {
        let err = lex("x = 5 @ 1").unwrap_err();
        assert_eq!((err.line, err.column), (1, 7));
        assert!(err.message.contains("'@'"));
    }

    #[test]
    fn single_quotes_suggest_double_quotes() {
        let err = lex("say 'hi'").unwrap_err();
        assert_eq!((err.line, err.column), (1, 5));
        assert!(err.message.contains("double quotes"));
    }

    #[test]
    fn names_cannot_start_with_a_digit() {
        let err = lex("2x = 1").unwrap_err();
        assert_eq!((err.line, err.column, err.length), (1, 1, 2));
        assert_eq!(err.suggestion.as_deref(), Some("2 x"));
    }

    #[test]
    fn render_follows_the_readme_style() {
        let source = "name = \"Amina\"\nsay \"Hello";
        let err = lex(source).unwrap_err();
        let rendered = err.render("test.ql", source);
        assert_eq!(
            rendered,
            "Problem on line 2 of test.ql:\n\
             \n    say \"Hello\
             \n        ^^^^^^\
             \nThis text starts with a quote but never ends.\
             \nText must open and close with \" on the same line.\
             \n\
             \nDid you mean:  \"Hello\"\n"
        );
    }
}
