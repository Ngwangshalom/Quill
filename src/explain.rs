//! `quill explain`: says how Quill read one line.

use crate::checker;
use crate::lexer;
use crate::parser::{self, Expr, ExprKind, Program, Statement};

/// Describes line `line` (1-based) of `source`.
///
/// The caller checks that the line exists. The report says what each piece
/// means, and, when this line has a problem, why Quill stopped.
pub fn explain(source: &str, line: usize) -> String {
    let line_text = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
    let header = format!("Line {line}:  {line_text}\n");

    let tokens = match lexer::lex(source) {
        Ok(tokens) => tokens,
        Err(problem) => {
            return if problem.line == line {
                stopped(
                    &header,
                    &problem_text(&problem.message, &problem.suggestion),
                )
            } else {
                blocked(&header, problem.line)
            };
        }
    };

    let program = match parser::parse(&tokens) {
        Ok(program) => program,
        Err(problem) => {
            return if problem.line == line {
                stopped(
                    &header,
                    &problem_text(&problem.message, &problem.suggestion),
                )
            } else {
                blocked(&header, problem.line)
            };
        }
    };

    let Some(statement) = program.statements.iter().find(|s| s.line() == line) else {
        let note = if line_text.trim().is_empty() {
            "  (blank)   an empty line, skipped\n".to_string()
        } else if let Some(note) = comment_note(line_text) {
            format!("{note}\n")
        } else {
            "  Quill didn't see a statement on this line.\n\
             A line can start with say, check, or a name and =.\n"
                .to_string()
        };
        return format!("{header}\n{note}");
    };

    let bindings = bindings_before(&program, line);
    let rows = describe(statement, &bindings);
    let mut body = header;
    body.push('\n');
    body.push_str(&format_rows(&rows));

    if let Err(problems) = checker::check(&program) {
        if let Some(problem) = problems.iter().find(|problem| problem.line == line) {
            return stopped(&body, &problem_text(&problem.message, &problem.suggestion));
        }
    }
    body
}

struct Binding {
    name: String,
    line: usize,
    kind: String,
}

fn bindings_before(program: &Program, line: usize) -> Vec<Binding> {
    let mut bindings = Vec::new();
    for statement in &program.statements {
        if statement.line() >= line {
            break;
        }
        let Statement::Let { name, value, .. } = statement else {
            continue;
        };
        if bindings
            .iter()
            .any(|binding: &Binding| binding.name == *name)
        {
            continue;
        }
        bindings.push(Binding {
            name: name.clone(),
            line: statement.line(),
            kind: kind_of(value, &bindings),
        });
    }
    bindings
}

fn kind_of(expr: &Expr, bindings: &[Binding]) -> String {
    match &expr.kind {
        ExprKind::Text(_) => "Text".to_string(),
        ExprKind::Number(_) => "Number".to_string(),
        ExprKind::Name(name) => bindings
            .iter()
            .find(|binding| binding.name == *name)
            .map(|binding| binding.kind.clone())
            .unwrap_or_else(|| "a value".to_string()),
    }
}

fn describe(statement: &Statement, bindings: &[Binding]) -> Vec<(String, String)> {
    match statement {
        Statement::Say { value, .. } => vec![
            ("say".to_string(), "show a value".to_string()),
            expr_row(value, bindings),
        ],
        Statement::Let { name, value, .. } => vec![
            (name.clone(), "a new value (can't change)".to_string()),
            (
                "=".to_string(),
                format!("store the value on the right in {name}"),
            ),
            expr_row(value, bindings),
        ],
        Statement::Check {
            left,
            right,
            example,
            ..
        } => {
            let head = if *example {
                (
                    "--- example".to_string(),
                    "a doc example, run as a test".to_string(),
                )
            } else {
                ("check".to_string(), "a test".to_string())
            };
            vec![
                head,
                expr_row(left, bindings),
                ("==".to_string(), "compare the two sides".to_string()),
                expr_row(right, bindings),
            ]
        }
    }
}

fn expr_row(expr: &Expr, bindings: &[Binding]) -> (String, String) {
    let label = expr.to_string();
    let detail = match &expr.kind {
        ExprKind::Text(_) => "Text".to_string(),
        ExprKind::Number(_) => "Number".to_string(),
        ExprKind::Name(name) => match bindings.iter().find(|binding| binding.name == *name) {
            Some(binding) => format!("{}, created on line {}", binding.kind, binding.line),
            None => format!("no value called {name}"),
        },
    };
    (label, detail)
}

fn format_rows(rows: &[(String, String)]) -> String {
    let width = rows
        .iter()
        .map(|(label, _)| label.chars().count())
        .max()
        .unwrap_or(0)
        + 3;
    let mut out = String::new();
    for (label, detail) in rows {
        out.push_str("  ");
        out.push_str(label);
        let pad = width.saturating_sub(label.chars().count());
        out.extend(std::iter::repeat_n(' ', pad));
        out.push_str(detail);
        out.push('\n');
    }
    out
}

fn comment_note(line_text: &str) -> Option<String> {
    let trimmed = line_text.trim();
    if let Some(rest) = trimmed.strip_prefix("---") {
        if rest.trim_start().starts_with("example:") {
            return None;
        }
        return Some(format!(
            "  {trimmed}   a doc comment, skipped when the program runs"
        ));
    }
    trimmed
        .strip_prefix("--")
        .map(|_| format!("  {trimmed}   a comment, skipped when the program runs"))
}

fn problem_text(message: &str, suggestion: &Option<String>) -> String {
    match suggestion {
        Some(suggestion) => format!("{message}\n\nDid you mean:  {suggestion}"),
        None => message.to_string(),
    }
}

fn stopped(body: &str, message: &str) -> String {
    let mut text = body.to_string();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push('\n');
    text.push_str(message);
    if !message.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("\nQuill stopped here before running anything.\n");
    text
}

fn blocked(body: &str, earlier: usize) -> String {
    format!(
        "{body}\nQuill found a problem on line {earlier}, so it stopped before reading this line.\n\
         \nQuill stopped here before running anything.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_creating_a_value() {
        assert_eq!(
            explain("year = 2026\n", 1),
            "Line 1:  year = 2026\n\n  year   a new value (can't change)\n  =      store the value on the right in year\n  2026   Number\n"
        );
    }

    #[test]
    fn explains_a_name_by_where_it_was_created() {
        assert_eq!(
            explain("greeting = \"Hello\"\nsay greeting\n", 2),
            "Line 2:  say greeting\n\n  say        show a value\n  greeting   Text, created on line 1\n"
        );
    }

    #[test]
    fn explains_a_check() {
        let text = explain("greeting = \"Hello\"\ncheck greeting == \"Hello\"\n", 2);
        assert!(text.contains("check      a test"));
        assert!(text.contains("Text, created on line 1"));
        assert!(text.contains("compare the two sides"));
        assert!(!text.contains("stopped"));
    }

    #[test]
    fn a_problem_on_the_line_stops_before_running() {
        let text = explain("say greting\n", 1);
        assert!(text.contains("no value called greting"));
        assert!(text.contains("There's no value called greting"));
        assert!(text.contains("Quill stopped here before running anything."));
    }

    #[test]
    fn a_blank_line_is_skipped() {
        let text = explain("say \"hi\"\n\nsay \"there\"\n", 2);
        assert!(text.contains("(blank)"));
        assert!(text.contains("empty line, skipped"));
    }

    #[test]
    fn a_comment_is_skipped() {
        let text = explain("say \"hi\" -- note\n", 1);
        assert!(text.contains("show a value"));
        assert!(!text.contains("stopped"));
        let text = explain("-- just a note\n", 1);
        assert!(text.contains("a comment, skipped"));
    }

    #[test]
    fn an_earlier_problem_blocks_a_later_line() {
        let text = explain("say \"oops\nsay 1\n", 2);
        assert!(text.contains("problem on line 1"));
        assert!(text.contains("Quill stopped here before running anything."));
    }
}
