//! `quill explain`: says how Quill read one line.

use crate::checker::{self, KnownValue};
use crate::lexer;
use crate::parser::{self, Arm, Expr, ExprKind, Part, Program, Statement};

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

    let Some(found) = find(&program.statements, line) else {
        let note = if line_text.trim().is_empty() {
            "  (blank)   an empty line, skipped\n".to_string()
        } else if let Some(note) = comment_note(line_text) {
            format!("{note}\n")
        } else {
            "  Quill didn't see a statement on this line.\n\
             A line can start with say, check, var, set, when, for, or a name and =.\n"
                .to_string()
        };
        return format!("{header}\n{note}");
    };

    let known = checker::values_before(&program, line);
    let describer = Describer {
        program: &program,
        known: &known,
    };
    let rows = match found {
        Found::Statement(statement) => describer.statement(statement),
        Found::Arm(arm) => describer.arm(arm),
    };
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

/// What starts on the line being explained.
enum Found<'a> {
    Statement(&'a Statement),
    /// A case of a `when`, which has its own line.
    Arm(&'a Arm),
}

fn find(statements: &[Statement], line: usize) -> Option<Found<'_>> {
    for statement in statements {
        if statement.line() == line {
            return Some(Found::Statement(statement));
        }
        if let Some(found) = find_in(statement, line) {
            return Some(found);
        }
    }
    None
}

fn find_in(statement: &Statement, line: usize) -> Option<Found<'_>> {
    match statement {
        Statement::When { arms, .. } => {
            for arm in arms {
                if arm.span.line == line {
                    return Some(Found::Arm(arm));
                }
                if let Some(found) = find_in(&arm.body, line) {
                    return Some(found);
                }
            }
            None
        }
        Statement::Function { body, .. } => {
            for statement in body {
                if statement.line() == line {
                    return Some(Found::Statement(statement));
                }
                if let Some(found) = find_in(statement, line) {
                    return Some(found);
                }
            }
            None
        }
        Statement::For { body, .. } => {
            for statement in body {
                if statement.line() == line {
                    return Some(Found::Statement(statement));
                }
                if let Some(found) = find_in(statement, line) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

struct Describer<'a> {
    program: &'a Program,
    known: &'a [KnownValue],
}

impl Describer<'_> {
    fn statement(&self, statement: &Statement) -> Vec<(String, String)> {
        match statement {
            Statement::Say { value, .. } => {
                let mut rows = vec![("say".to_string(), "show a value".to_string())];
                rows.extend(self.value(value));
                rows
            }
            Statement::Let {
                name,
                value,
                mutable,
                ..
            } => {
                let (label, meaning) = if *mutable {
                    (format!("var {name}"), "a new value (can change with set)")
                } else {
                    (name.clone(), "a new value (can't change)")
                };
                let mut rows = vec![
                    (label, meaning.to_string()),
                    (
                        "=".to_string(),
                        format!("store the value on the right in {name}"),
                    ),
                ];
                rows.extend(self.value(value));
                rows
            }
            Statement::Set { name, value, .. } => {
                let meaning = match self.lookup(name) {
                    Some(known) => format!("change {name}, created on line {}", known.span.line),
                    None => format!("change {name}"),
                };
                let mut rows = vec![
                    (format!("set {name}"), meaning),
                    ("=".to_string(), format!("store the new value in {name}")),
                ];
                rows.extend(self.value(value));
                rows
            }
            Statement::Check {
                left,
                op,
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
                let mut rows = vec![head];
                rows.extend(self.value(left));
                rows.push((op.symbol().to_string(), "compare the two sides".to_string()));
                rows.extend(self.value(right));
                rows
            }
            Statement::When { arms, .. } => {
                let cases = if arms.len() == 1 { "case" } else { "cases" };
                vec![(
                    "when".to_string(),
                    format!(
                        "run the first of the {} {cases} below whose condition is true",
                        arms.len()
                    ),
                )]
            }
            Statement::Function {
                name,
                params,
                result,
                ..
            } => {
                let gives = match result {
                    Some(kind) => format!("gives back {}", kind.written()),
                    None => "does its work and gives nothing back".to_string(),
                };
                let mut rows = vec![(format!("fn {name}"), format!("a function that {gives}"))];
                for param in params {
                    rows.push((
                        format!("{}: {}", param.name, param.kind.written()),
                        "a parameter".to_string(),
                    ));
                }
                rows
            }
            Statement::Return { value, .. } => {
                let mut rows = vec![(
                    "return".to_string(),
                    "leave the function, giving a value back".to_string(),
                )];
                if let Some(value) = value {
                    rows.extend(self.value(value));
                }
                rows
            }
            Statement::Match { subject, arms, .. } => {
                let mut rows = vec![(
                    "when".to_string(),
                    "open a maybe, then handle found and missing".to_string(),
                )];
                rows.extend(self.value(subject));
                for arm in arms {
                    rows.extend(self.statement(&arm.body));
                }
                rows
            }
            Statement::Fail { name, .. } => {
                vec![(
                    format!("fail {name}"),
                    "leave the function with this failure".to_string(),
                )]
            }
            Statement::Call { name, args, .. } => {
                let mut rows = vec![(
                    format!("{name}()"),
                    "call the function, which gives nothing back".to_string(),
                )];
                for arg in args {
                    rows.extend(self.value(arg));
                }
                rows
            }
            Statement::For {
                name,
                start,
                end,
                body,
                ..
            } => {
                let lines = if body.len() == 1 { "line" } else { "lines" };
                match end {
                    Some(end) => {
                        let mut rows = vec![
                            (
                                "for".to_string(),
                                format!(
                                    "repeat the {} {lines} below, once for each number",
                                    body.len()
                                ),
                            ),
                            (
                                name.clone(),
                                format!("the number this pass, from {start} through {end}"),
                            ),
                            ("in".to_string(), "counts through this range".to_string()),
                        ];
                        rows.extend(self.value(start));
                        rows.push((
                            "..".to_string(),
                            "through, including the first number and the last".to_string(),
                        ));
                        rows.extend(self.value(end));
                        rows
                    }
                    None => {
                        let mut rows = vec![
                            (
                                "for".to_string(),
                                format!(
                                    "repeat the {} {lines} below, once for each item",
                                    body.len()
                                ),
                            ),
                            (name.clone(), "the item this pass".to_string()),
                            ("in".to_string(), "walks this list".to_string()),
                        ];
                        rows.extend(self.value(start));
                        rows
                    }
                }
            }
        }
    }

    fn arm(&self, arm: &Arm) -> Vec<(String, String)> {
        let mut rows = match &arm.condition {
            Some(condition) => {
                let mut rows = self.value(condition);
                rows.push(("->".to_string(), "when that is true, run:".to_string()));
                rows
            }
            None => vec![
                (
                    "else".to_string(),
                    "when none of the cases above are true".to_string(),
                ),
                ("->".to_string(), "run:".to_string()),
            ],
        };
        rows.extend(self.statement(&arm.body));
        rows
    }

    /// A row for a value, then a row for each name used inside it.
    fn value(&self, expr: &Expr) -> Vec<(String, String)> {
        let mut rows = vec![(expr.to_string(), self.meaning(expr))];
        if !matches!(expr.kind, ExprKind::Name(_)) {
            let mut names = Vec::new();
            names_in(expr, &mut names);
            for name in names {
                rows.push((name.clone(), self.name_meaning(&name)));
            }
        }
        rows
    }

    fn meaning(&self, expr: &Expr) -> String {
        let kind = checker::kind_of(self.program, expr, self.known).name();
        match &expr.kind {
            ExprKind::Text(_) | ExprKind::Number(_) => kind.to_string(),
            ExprKind::Bool(_) => "true/false".to_string(),
            ExprKind::Name(name) => self.name_meaning(name),
            ExprKind::Template(_) => "Text with values inside".to_string(),
            ExprKind::Group(inner) => self.meaning(inner),
            ExprKind::Arithmetic { .. } | ExprKind::Negate(_) => format!("a calculation, {kind}"),
            ExprKind::Compare { .. } => format!("a comparison, {kind}"),
            ExprKind::Logic { .. } | ExprKind::Not(_) => format!("a condition, {kind}"),
            ExprKind::List { .. } => kind,
            ExprKind::Record { .. } => kind,
            ExprKind::At { .. } => format!("an item from a list, {kind}"),
            ExprKind::Field { .. } => format!("a field of a record, {kind}"),
            ExprKind::Call { name, .. } => format!("a call of {name}, {kind}"),
            ExprKind::Money { .. } => kind,
            ExprKind::Found(_) => format!("a maybe that holds a value, {kind}"),
            ExprKind::Missing => "a maybe that holds nothing".to_string(),
            ExprKind::Try(inner) => format!("{}, passing failures upward", self.meaning(inner)),
        }
    }

    fn name_meaning(&self, name: &str) -> String {
        match self.lookup(name) {
            Some(known) => format!("{}, created on line {}", known.kind.name(), known.span.line),
            None => format!("no value called {name}"),
        }
    }

    fn lookup(&self, name: &str) -> Option<&KnownValue> {
        self.known.iter().find(|known| known.name == name)
    }
}

/// The distinct names used inside `expr`, in order.
fn names_in(expr: &Expr, names: &mut Vec<String>) {
    match &expr.kind {
        ExprKind::Name(name) => {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        ExprKind::Template(parts) => {
            for part in parts {
                if let Part::Value(value) = part {
                    names_in(value, names);
                }
            }
        }
        ExprKind::Negate(inner) | ExprKind::Not(inner) | ExprKind::Group(inner) => {
            names_in(inner, names)
        }
        ExprKind::Arithmetic { left, right, .. }
        | ExprKind::Compare { left, right, .. }
        | ExprKind::Logic { left, right, .. } => {
            names_in(left, names);
            names_in(right, names);
        }
        ExprKind::List { items, .. } => {
            for item in items {
                names_in(item, names);
            }
        }
        ExprKind::Record { fields } => {
            for field in fields {
                names_in(&field.value, names);
            }
        }
        ExprKind::At { list, index, .. } => {
            names_in(list, names);
            names_in(index, names);
        }
        ExprKind::Field { record, .. } => names_in(record, names),
        ExprKind::Call { args, .. } => {
            for arg in args {
                names_in(arg, names);
            }
        }
        ExprKind::Found(inner) | ExprKind::Try(inner) => names_in(inner, names),
        ExprKind::Text(_)
        | ExprKind::Number(_)
        | ExprKind::Money { .. }
        | ExprKind::Bool(_)
        | ExprKind::Missing => {}
    }
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

    #[test]
    fn explains_a_calculation_and_the_names_in_it() {
        assert_eq!(
            explain("price = 10\ntotal = price * 3 + 1\n", 2),
            "Line 2:  total = price * 3 + 1\n\n\
             \x20 total           a new value (can't change)\n\
             \x20 =               store the value on the right in total\n\
             \x20 price * 3 + 1   a calculation, Number\n\
             \x20 price           Number, created on line 1\n"
        );
    }

    #[test]
    fn explains_var_and_set() {
        let text = explain("var visits = 0\nset visits = visits + 1\n", 1);
        assert!(text.contains("var visits   a new value (can change with set)"));
        let text = explain("var visits = 0\nset visits = visits + 1\n", 2);
        assert!(text.contains("set visits   change visits, created on line 1"));
        assert!(text.contains("visits + 1   a calculation, Number"));
    }

    #[test]
    fn explains_a_when_and_its_cases() {
        let source = "n = 4\nwhen\n  n % 2 == 0 -> say \"even\"\n  else -> say \"odd\"\n";
        assert!(explain(source, 2).contains("run the first of the 2 cases below"));
        let case = explain(source, 3);
        assert!(case.contains("n % 2 == 0   a comparison, true/false"));
        assert!(case.contains("when that is true, run:"));
        assert!(case.contains("say          show a value"));
        let other = explain(source, 4);
        assert!(other.contains("else    when none of the cases above are true"));
    }

    #[test]
    fn explains_values_inside_text() {
        let text = explain("name = \"Amina\"\nsay \"Hi {name}\"\n", 2);
        assert!(text.contains("\"Hi {name}\"   Text with values inside"));
        assert!(text.contains("name          Text, created on line 1"));
    }

    #[test]
    fn a_kind_mistake_stops_before_running() {
        let text = explain("price = 100\ntotal = price + \"5\"\n", 2);
        assert!(text.contains("You're adding Number (price) to Text (\"5\")."));
        assert!(text.contains("Did you mean:  total = price + 5"));
        assert!(text.contains("Quill stopped here before running anything."));
    }

    #[test]
    fn explains_a_for_loop() {
        let text = explain("for i in 1..3\n  say i\n", 1);
        assert!(text.contains("repeat the 1 line below"));
        assert!(text.contains("the number this pass, from 1 through 3"));
        assert!(text.contains("including the first number and the last"));
        let body = explain("for i in 1..3\n  say i\n", 2);
        assert!(body.contains("Number, created on line 1"));
    }
}
