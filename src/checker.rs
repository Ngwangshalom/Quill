//! The checker: finds mistakes before anything runs.
//!
//! So far it makes sure every name refers to a value created earlier,
//! and that no value is created twice (values can't change).

use crate::diagnostic::{closest_match, Diagnostic, Span};
use crate::parser::{Expr, ExprKind, Program, Statement};

/// Checks the whole program and reports every problem it finds.
pub fn check(program: &Program) -> Result<(), Vec<Diagnostic>> {
    let mut checker = Checker {
        program,
        defined: Vec::new(),
        problems: Vec::new(),
    };
    for statement in &program.statements {
        checker.statement(statement);
    }
    if checker.problems.is_empty() {
        Ok(())
    } else {
        Err(checker.problems)
    }
}

struct Checker<'a> {
    program: &'a Program,
    /// Values created so far, in order, with where they were created.
    defined: Vec<(&'a str, Span)>,
    problems: Vec<Diagnostic>,
}

impl<'a> Checker<'a> {
    fn statement(&mut self, statement: &'a Statement) {
        match statement {
            Statement::Say { value, .. } => {
                self.expression(value, None, |fixed| format!("say {fixed}"));
            }
            Statement::Let {
                name,
                name_span,
                value,
            } => {
                self.expression(value, Some(name), |fixed| format!("{name} = {fixed}"));

                if let Some(first) = self.lookup(name) {
                    self.problems.push(Diagnostic::new(
                        *name_span,
                        format!(
                            "{name} already exists. It was created on line {}.\n\
                             Values in Quill can't change once they're created, \
                             so give this one a different name.",
                            first.line
                        ),
                    ));
                } else {
                    self.defined.push((name, *name_span));
                }
            }
        }
    }

    /// Checks one value. `creating` is the name the surrounding line creates, if any.
    /// `rewrite` rebuilds the line with a corrected value, for suggestions.
    fn expression(
        &mut self,
        expr: &Expr,
        creating: Option<&str>,
        rewrite: impl Fn(&str) -> String,
    ) {
        let ExprKind::Name(name) = &expr.kind else {
            return;
        };
        if self.lookup(name).is_some() {
            return;
        }

        let problem = if creating == Some(name.as_str()) {
            Diagnostic::new(
                expr.span,
                format!(
                    "{name} is being created on this line, so it doesn't have a value yet.\n\
                     A value can't be made from itself."
                ),
            )
        } else if let Some(later) = self.created_later(name, expr.span.line) {
            Diagnostic::new(
                expr.span,
                format!(
                    "{name} is used here, but it isn't created until line {}.\n\
                     Quill reads from top to bottom, so create values before using them.",
                    later.line
                ),
            )
        } else {
            let problem = Diagnostic::new(
                expr.span,
                format!(
                    "There's no value called {name}.\n\
                     Values must be created with {name} = ... before they can be used."
                ),
            );
            let known = self.defined.iter().map(|(known, _)| *known);
            match closest_match(name, known) {
                Some(fixed) => problem.suggest(rewrite(fixed)),
                None => problem,
            }
        };
        self.problems.push(problem);
    }

    fn lookup(&self, name: &str) -> Option<Span> {
        self.defined
            .iter()
            .find(|(known, _)| *known == name)
            .map(|(_, span)| *span)
    }

    fn created_later(&self, name: &str, after_line: usize) -> Option<Span> {
        self.program
            .statements
            .iter()
            .find_map(|statement| match statement {
                Statement::Let {
                    name: created,
                    name_span,
                    ..
                } if created == name && name_span.line > after_line => Some(*name_span),
                _ => None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;

    fn problems(source: &str) -> Vec<Diagnostic> {
        let program = parse(&lex(source).unwrap()).unwrap();
        match check(&program) {
            Ok(()) => Vec::new(),
            Err(problems) => problems,
        }
    }

    #[test]
    fn valid_program_has_no_problems() {
        assert_eq!(
            problems("greeting = \"Hi\"\ncopy = greeting\nsay copy\nsay 5\nsay \"x\""),
            vec![]
        );
    }

    #[test]
    fn unknown_name_suggests_close_match() {
        let found = problems("greeting = \"Hi\"\nsay greting");
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].line, found[0].column, found[0].length), (2, 5, 7));
        assert!(found[0].message.contains("There's no value called greting"));
        assert_eq!(found[0].suggestion.as_deref(), Some("say greeting"));
    }

    #[test]
    fn unknown_name_in_let_rewrites_the_whole_line() {
        let found = problems("visits = 1\ntotal = vistis");
        assert_eq!(found[0].suggestion.as_deref(), Some("total = visits"));
    }

    #[test]
    fn unknown_name_without_close_match_has_no_suggestion() {
        let found = problems("say price");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].suggestion, None);
    }

    #[test]
    fn using_a_value_before_it_is_created() {
        let found = problems("say name\nname = \"Amina\"");
        assert_eq!(found.len(), 1);
        assert!(found[0].message.contains("it isn't created until line 2"));
    }

    #[test]
    fn a_value_cannot_be_made_from_itself() {
        let found = problems("x = x");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].column, 5);
        assert!(found[0].message.contains("being created on this line"));
    }

    #[test]
    fn values_cannot_be_created_twice() {
        let found = problems("name = \"Amina\"\nname = \"Omar\"");
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].line, found[0].column), (2, 1));
        assert!(found[0].message.contains("created on line 1"));
    }

    #[test]
    fn reports_every_problem_not_just_the_first() {
        let found = problems("say a\nsay b\nx = 1\nx = 2");
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn a_bad_value_still_creates_the_name() {
        // Only the first line is wrong; `say total` shouldn't be reported too.
        let found = problems("total = missing\nsay total");
        assert_eq!(found.len(), 1);
    }
}
