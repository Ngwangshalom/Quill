//! The lexer: turns Quill source text into a flat list of tokens.
//!
//! Supported so far: `say`, `check`, text literals, numbers, names, `=`, `==`,
//! comments (`--` and `---`) and new lines.
//! Every token remembers the line and column it started on (both 1-based,
//! counted in characters) so later stages can point at the exact spot.

use std::fmt;

use crate::diagnostic::{Diagnostic, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Say,
    Check,
    /// A text literal, with escapes already resolved.
    Text(String),
    /// A number exactly as written, so `Decimal` and `Money` can stay exact.
    Number(String),
    Identifier(String),
    Equals,
    /// `==`, used by `check` and doc examples.
    EqualEqual,
    /// The `--- example:` marker. The comparison itself follows on the same line.
    Example,
    Newline,
    EndOfFile,
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
        match self {
            TokenKind::Say => write!(f, "say"),
            TokenKind::Check => write!(f, "check"),
            TokenKind::Text(text) => write!(f, "Text {text:?}"),
            TokenKind::Number(number) => write!(f, "Number {number}"),
            TokenKind::Identifier(name) => write!(f, "Identifier {name}"),
            TokenKind::Equals => write!(f, "="),
            TokenKind::EqualEqual => write!(f, "=="),
            TokenKind::Example => write!(f, "example"),
            TokenKind::Newline => write!(f, "Newline"),
            TokenKind::EndOfFile => write!(f, "EndOfFile"),
        }
    }
}

pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    Lexer::new(source).run()
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    fn new(source: &str) -> Self {
        Lexer {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
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
                    TokenKind::Newline
                }
                '\n' => {
                    self.advance();
                    TokenKind::Newline
                }
                '=' => {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::EqualEqual
                    } else {
                        TokenKind::Equals
                    }
                }
                '-' => match self.comment(line, column)? {
                    Some(kind) => kind,
                    None => continue,
                },
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

        tokens.push(Token {
            kind: TokenKind::EndOfFile,
            line: self.line,
            column: self.column,
            length: 0,
        });
        Ok(tokens)
    }

    fn text(&mut self, line: usize, column: usize) -> Result<TokenKind, Diagnostic> {
        let start = self.pos;
        self.advance(); // opening quote
        let mut value = String::new();

        loop {
            match self.peek() {
                None | Some('\n') => {
                    let raw: String = self.chars[start..self.pos].iter().collect();
                    let raw = raw.trim_end_matches('\r');
                    return Err(Diagnostic {
                        line,
                        column,
                        length: raw.chars().count(),
                        message: "This text starts with a quote but never ends.\n\
                                  Text must open and close with \" on the same line."
                            .to_string(),
                        suggestion: Some(format!("{raw}\"")),
                    });
                }
                Some('"') => {
                    self.advance();
                    return Ok(TokenKind::Text(value));
                }
                Some('\\') => {
                    let escape_column = self.column;
                    self.advance();
                    let resolved = match self.peek() {
                        Some('n') => '\n',
                        Some('t') => '\t',
                        Some('"') => '"',
                        Some('\\') => '\\',
                        // Let the loop report the missing closing quote.
                        None | Some('\n') | Some('\r') => continue,
                        Some(other) => {
                            return Err(Diagnostic {
                                line: self.line,
                                column: escape_column,
                                length: 2,
                                message: format!(
                                    "`\\{other}` isn't an escape Quill knows.\n\
                                     Inside text, a backslash must be followed by n (new line), \
                                     t (tab), \" (a quote) or \\ (a backslash)."
                                ),
                                suggestion: Some(format!(
                                    "\\\\{other}   (to write a backslash followed by {other})"
                                )),
                            });
                        }
                    };
                    self.advance();
                    value.push(resolved);
                }
                Some(c) => {
                    self.advance();
                    value.push(c);
                }
            }
        }
    }

    fn number(&mut self, line: usize, column: usize) -> Result<TokenKind, Diagnostic> {
        let mut number = self.take_while(|c| c.is_ascii_digit());

        // Only treat `.` as a decimal point when a digit follows, so `1..15`
        // stays available for ranges later.
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
            _ => TokenKind::Identifier(word),
        }
    }

    /// `--` is a comment to the end of the line. `--- example:` is a doc example,
    /// and the comparison after it is tokenized normally. Anything else after
    /// `---` is a doc comment and is skipped.
    fn comment(&mut self, line: usize, column: usize) -> Result<Option<TokenKind>, Diagnostic> {
        self.advance(); // first '-'
        if self.peek() != Some('-') {
            return Err(unexpected('-', line, column));
        }
        self.advance(); // second '-'
        if self.peek() == Some('-') {
            self.advance();
            while matches!(self.peek(), Some(' ' | '\t')) {
                self.advance();
            }
            if self.starts_with("example:") {
                for _ in 0.."example:".len() {
                    self.advance();
                }
                while matches!(self.peek(), Some(' ' | '\t')) {
                    self.advance();
                }
                return Ok(Some(TokenKind::Example));
            }
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

fn unexpected(c: char, line: usize, column: usize) -> Diagnostic {
    if c == '\'' {
        return Diagnostic {
            line,
            column,
            length: 1,
            message: "Quill doesn't use single quotes.\nText is always written in double quotes."
                .to_string(),
            suggestion: Some("\"like this\"".to_string()),
        };
    }

    if c == '-' {
        return Diagnostic {
            line,
            column,
            length: 1,
            message: "Quill doesn't use a single - here.\n\
                      A comment starts with --, and a doc comment starts with ---."
                .to_string(),
            suggestion: Some("--".to_string()),
        };
    }

    Diagnostic {
        line,
        column,
        length: 1,
        message: format!(
            "Quill doesn't understand the character {c:?} here.\n\
             So far Quill reads say, check, text in \"quotes\", numbers, names, \
             =, ==, comments starting with -- and new lines."
        ),
        suggestion: None,
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
    fn a_single_dash_is_not_a_comment() {
        let err = lex("say -").unwrap_err();
        assert_eq!((err.line, err.column), (1, 5));
        assert!(err.message.contains("single -"));
        assert_eq!(err.suggestion.as_deref(), Some("--"));
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
    fn dot_without_following_digit_is_not_part_of_the_number() {
        let err = lex("x = 3.").unwrap_err();
        assert_eq!((err.line, err.column), (1, 6));
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
    fn tabs_and_blank_lines() {
        assert_eq!(
            tokens("\tsay x\n\n"),
            vec![
                (Say, 1, 2),
                (ident("x"), 1, 6),
                (Newline, 1, 7),
                (Newline, 2, 1),
                (EndOfFile, 3, 1),
            ]
        );
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
        let err = lex("x = 5 + 1").unwrap_err();
        assert_eq!((err.line, err.column), (1, 7));
        assert!(err.message.contains("'+'"));
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
