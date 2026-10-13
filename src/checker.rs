//! The checker: finds mistakes before anything runs.
//!
//! - every name refers to a value created earlier
//! - a value is created once; only values marked `var` can change, with `set`
//! - every value has a kind (Text, Number, or true/false), and each operation
//!   gets the kinds it needs, so `price + "5"` is caught here, not while running

use crate::compiler::too_many_digits;
use crate::decimal::Decimal;
use crate::diagnostic::{closest_match, Diagnostic, Span};
use crate::parser::{Arithmetic, Comparison, Expr, ExprKind, Part, Program, Statement};

/// The kind of a value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Text,
    Number,
    Bool,
    /// Not known because of an earlier problem. Never reported again, so one
    /// mistake gives one error.
    Unknown,
}

impl Kind {
    /// The kind's name, as errors and `quill explain` show it.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Text => "Text",
            Kind::Number => "Number",
            Kind::Bool => "true/false",
            Kind::Unknown => "a value",
        }
    }

    /// The name with "a" where English needs one: "holds a Number".
    fn with_article(self) -> &'static str {
        match self {
            Kind::Number => "a Number",
            other => other.name(),
        }
    }

    fn known(self) -> bool {
        self != Kind::Unknown
    }
}

/// A value created by the program.
#[derive(Debug, Clone)]
pub struct KnownValue {
    pub name: String,
    /// Where its name was written when it was created.
    pub span: Span,
    pub kind: Kind,
    pub mutable: bool,
    /// The line that created it, as Quill code.
    pub created: String,
}

/// Checks the whole program and reports every problem it finds.
pub fn check(program: &Program) -> Result<(), Vec<Diagnostic>> {
    let mut checker = Checker::new(program);
    for statement in &program.statements {
        checker.statement(statement);
    }
    if checker.problems.is_empty() {
        Ok(())
    } else {
        Err(checker.problems)
    }
}

/// The values created before `line`, with their kinds.
pub fn values_before(program: &Program, line: usize) -> Vec<KnownValue> {
    let mut checker = Checker::new(program);
    for statement in &program.statements {
        if statement.line() >= line {
            break;
        }
        checker.statement(statement);
    }
    checker.defined
}

/// The kind of `expr`, given the values that exist.
pub fn kind_of(program: &Program, expr: &Expr, known: &[KnownValue]) -> Kind {
    let mut checker = Checker::new(program);
    checker.defined = known.to_vec();
    checker.value(expr, None, &|_| String::new())
}

/// Rebuilds the line around a corrected value, for a suggestion.
type Rewrite<'r> = &'r dyn Fn(&Expr) -> String;

struct Checker<'a> {
    program: &'a Program,
    /// Values created so far, in order.
    defined: Vec<KnownValue>,
    problems: Vec<Diagnostic>,
}

impl<'a> Checker<'a> {
    fn new(program: &'a Program) -> Self {
        Checker {
            program,
            defined: Vec::new(),
            problems: Vec::new(),
        }
    }

    fn statement(&mut self, statement: &Statement) {
        match statement {
            Statement::Say { value, .. } => {
                self.value(value, None, &|fixed| format!("say {fixed}"));
            }
            Statement::Let {
                name,
                name_span,
                value,
                mutable,
            } => {
                let prefix = if *mutable { "var " } else { "" };
                let kind = self.value(value, Some(name), &|fixed| {
                    format!("{prefix}{name} = {fixed}")
                });
                match self.lookup(name).cloned() {
                    Some(first) if first.mutable => self.problems.push(
                        Diagnostic::new(
                            *name_span,
                            format!(
                                "{name} already exists. It was created on line {} with var.\n\
                                 To change it, use set instead of creating it again.",
                                first.span.line
                            ),
                        )
                        .suggest(format!("set {name} = {value}")),
                    ),
                    Some(first) => self.problems.push(Diagnostic::new(
                        *name_span,
                        format!(
                            "{name} already exists. It was created on line {}.\n\
                             Values in Quill can't change once they're created, \
                             so give this one a different name.",
                            first.span.line
                        ),
                    )),
                    None => self.defined.push(KnownValue {
                        name: name.clone(),
                        span: *name_span,
                        kind,
                        mutable: *mutable,
                        created: statement.to_string(),
                    }),
                }
            }
            Statement::Set {
                name,
                name_span,
                value,
                ..
            } => self.set(name, *name_span, value),
            Statement::Check {
                left,
                op,
                right,
                span,
                example,
            } => {
                let keyword = if *example { "--- example:" } else { "check" };
                let symbol = op.symbol();
                let fix_left = |fixed: &Expr| format!("{keyword} {fixed} {symbol} {right}");
                let fix_right = |fixed: &Expr| format!("{keyword} {left} {symbol} {fixed}");
                let left_kind = self.value(left, None, &fix_left);
                let right_kind = self.value(right, None, &fix_right);
                let sides = Sides {
                    left,
                    left_kind,
                    right,
                    right_kind,
                };
                if let Some(problem) = comparison_problem(*op, &sides, *span, &fix_left, &fix_right)
                {
                    self.problems.push(problem);
                }
            }
            Statement::When { arms, .. } => {
                for arm in arms {
                    if let Some(condition) = &arm.condition {
                        let body = &arm.body;
                        let kind =
                            self.value(condition, None, &|fixed| format!("{fixed} -> {body}"));
                        if kind.known() && kind != Kind::Bool {
                            self.problems.push(Diagnostic::new(
                                condition.span,
                                format!(
                                    "A case of a when needs a condition that is true or false, \
                                     but {condition} is {}.\n\
                                     Compare it to something, like {}.",
                                    kind.with_article(),
                                    condition_example(condition, kind)
                                ),
                            ));
                        }
                    }
                    self.statement(&arm.body);
                }
            }
        }
    }

    fn set(&mut self, name: &str, name_span: Span, value: &Expr) {
        let kind = self.value(value, None, &|fixed| format!("set {name} = {fixed}"));
        let Some(target) = self.lookup(name).cloned() else {
            let problem = if let Some(later) = self.created_later(name, name_span.line) {
                Diagnostic::new(
                    name_span,
                    format!(
                        "{name} is changed here, but it isn't created until line {}.\n\
                         Quill reads from top to bottom, so create values before changing them.",
                        later.line
                    ),
                )
            } else {
                let problem = Diagnostic::new(
                    name_span,
                    format!(
                        "There's no value called {name}.\n\
                         set changes a value that was created with var, so create it first."
                    ),
                );
                let changeable = self
                    .defined
                    .iter()
                    .filter(|known| known.mutable)
                    .map(|known| known.name.as_str());
                match closest_match(name, changeable) {
                    Some(fixed) => problem.suggest(format!("set {fixed} = {value}")),
                    None => problem.suggest(format!("var {name} = {value}")),
                }
            };
            self.problems.push(problem);
            return;
        };

        if !target.mutable {
            self.problems.push(Diagnostic::new(
                name_span,
                format!(
                    "{name} can't change, because it isn't marked var.\n\
                     It was created on line {} as a value that stays the same. \
                     To let it change, write line {} as:\n\n    var {}",
                    target.span.line, target.span.line, target.created
                ),
            ));
            return;
        }

        if target.kind.known() && kind.known() && target.kind != kind {
            let problem = Diagnostic::new(
                value.span,
                format!(
                    "{name} holds {}, so it can't be set to {} ({value}).\n\
                     A value keeps the same kind for the whole program.",
                    target.kind.with_article(),
                    kind.name()
                ),
            );
            self.problems.push(match number_from_text(value) {
                Some(number) if target.kind == Kind::Number => {
                    problem.suggest(format!("set {name} = {number}"))
                }
                _ => problem,
            });
        }
    }

    /// Checks a whole value and returns its kind.
    /// `creating` is the name the surrounding line creates, if any.
    fn value(&mut self, top: &Expr, creating: Option<&str>, rewrite: Rewrite) -> Kind {
        self.infer(
            top,
            &Context {
                top,
                creating,
                rewrite,
            },
        )
    }

    fn infer(&mut self, expr: &Expr, context: &Context) -> Kind {
        match &expr.kind {
            ExprKind::Text(_) => Kind::Text,
            ExprKind::Template(parts) => {
                for part in parts {
                    if let Part::Value(value) = part {
                        self.infer(value, context);
                    }
                }
                Kind::Text
            }
            ExprKind::Number(number) => {
                if Decimal::parse(number).is_none() {
                    self.problems.push(too_many_digits(number, expr.span));
                }
                Kind::Number
            }
            ExprKind::Bool(_) => Kind::Bool,
            ExprKind::Name(name) => self.name(expr, name, context),
            ExprKind::Group(inner) => self.infer(inner, context),
            ExprKind::Negate(inner) => {
                let kind = self.infer(inner, context);
                if kind.known() && kind != Kind::Number {
                    self.problems.push(Diagnostic::new(
                        expr.span,
                        format!(
                            "- makes a number negative, but {inner} is {}.\n\
                             Only numbers can be negative.",
                            kind.name()
                        ),
                    ));
                    return Kind::Unknown;
                }
                Kind::Number
            }
            ExprKind::Not(inner) => {
                let kind = self.infer(inner, context);
                if kind.known() && kind != Kind::Bool {
                    self.problems.push(Diagnostic::new(
                        expr.span,
                        format!(
                            "not turns true into false and false into true, but {inner} is {}.\n\
                             To use it as a condition, compare it to something, like {}.",
                            kind.name(),
                            condition_example(inner, kind)
                        ),
                    ));
                }
                Kind::Bool
            }
            ExprKind::Arithmetic {
                op, left, right, ..
            } => self.arithmetic(*op, expr, left, right, context),
            ExprKind::Compare {
                op, left, right, ..
            } => {
                let left_kind = self.infer(left, context);
                let right_kind = self.infer(right, context);
                let sides = Sides {
                    left,
                    left_kind,
                    right,
                    right_kind,
                };
                let fix_left =
                    |fixed: &Expr| (context.rewrite)(&replace(context.top, left.span, fixed));
                let fix_right =
                    |fixed: &Expr| (context.rewrite)(&replace(context.top, right.span, fixed));
                if let Some(problem) =
                    comparison_problem(*op, &sides, expr.span, &fix_left, &fix_right)
                {
                    self.problems.push(problem);
                }
                Kind::Bool
            }
            ExprKind::Logic {
                op, left, right, ..
            } => {
                for side in [left, right] {
                    let kind = self.infer(side, context);
                    if kind.known() && kind != Kind::Bool {
                        self.problems.push(Diagnostic::new(
                            side.span,
                            format!(
                                "{} joins conditions that are true or false, but {side} is {}.\n\
                                 To use it as a condition, compare it to something, like {}.",
                                op.word(),
                                kind.with_article(),
                                condition_example(side, kind)
                            ),
                        ));
                    }
                }
                Kind::Bool
            }
        }
    }

    fn arithmetic(
        &mut self,
        op: Arithmetic,
        expr: &Expr,
        left: &Expr,
        right: &Expr,
        context: &Context,
    ) -> Kind {
        let left_kind = self.infer(left, context);
        let right_kind = self.infer(right, context);

        if matches!(op, Arithmetic::Divide | Arithmetic::Remainder) && is_zero(right) {
            self.problems.push(Diagnostic::new(
                right.span,
                format!(
                    "This {} by zero, which has no answer.\n\
                     Dividing by zero isn't allowed, so use a number other than 0.",
                    if op == Arithmetic::Divide {
                        "divides"
                    } else {
                        "takes the remainder of dividing"
                    }
                ),
            ));
            return Kind::Number;
        }
        if !left_kind.known() || !right_kind.known() {
            return Kind::Unknown;
        }
        if left_kind == Kind::Number && right_kind == Kind::Number {
            return Kind::Number;
        }

        let l = format!("{} ({left})", left_kind.name());
        let r = format!("{} ({right})", right_kind.name());
        let doing = match op {
            Arithmetic::Add => format!("adding {l} to {r}"),
            Arithmetic::Subtract => format!("subtracting {r} from {l}"),
            Arithmetic::Multiply => format!("multiplying {l} by {r}"),
            Arithmetic::Divide => format!("dividing {l} by {r}"),
            Arithmetic::Remainder => format!("taking the remainder of {l} divided by {r}"),
        };
        let has_text = left_kind == Kind::Text || right_kind == Kind::Text;
        let why = if op == Arithmetic::Add && has_text {
            "Quill won't guess which one you meant."
        } else {
            "Calculations only work on numbers."
        };
        // Underline the side that isn't a number, or both if neither is.
        let at = match (left_kind == Kind::Number, right_kind == Kind::Number) {
            (true, false) => right.span,
            (false, true) => left.span,
            _ => expr.span,
        };
        let problem = Diagnostic::new(at, format!("You're {doing}.\n{why}"));

        // A number written as text: "5" next to a number.
        let fixed_side = match (left_kind, right_kind) {
            (Kind::Number, Kind::Text) => number_from_text(right).map(|n| (right.span, n)),
            (Kind::Text, Kind::Number) => number_from_text(left).map(|n| (left.span, n)),
            _ => None,
        };
        let problem = if let Some((span, number)) = fixed_side {
            problem.suggest((context.rewrite)(&replace(context.top, span, &number)))
        } else if op == Arithmetic::Add && has_text {
            let joined = Expr {
                kind: ExprKind::Template(joined_text(left, right)),
                span: expr.span,
            };
            problem.suggest((context.rewrite)(&replace(context.top, expr.span, &joined)))
        } else {
            problem
        };
        self.problems.push(problem);
        Kind::Unknown
    }

    fn name(&mut self, expr: &Expr, name: &str, context: &Context) -> Kind {
        if let Some(known) = self.lookup(name) {
            return known.kind;
        }

        let problem = if context.creating == Some(name) {
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
            let known = self.defined.iter().map(|known| known.name.as_str());
            match closest_match(name, known) {
                Some(fixed) => {
                    let fixed = Expr {
                        kind: ExprKind::Name(fixed.to_string()),
                        span: expr.span,
                    };
                    problem.suggest((context.rewrite)(&replace(context.top, expr.span, &fixed)))
                }
                None => problem,
            }
        };
        self.problems.push(problem);
        Kind::Unknown
    }

    fn lookup(&self, name: &str) -> Option<&KnownValue> {
        self.defined.iter().find(|known| known.name == name)
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

/// What a value is part of, so a suggestion can rewrite the whole line.
struct Context<'c> {
    /// The whole value on the line.
    top: &'c Expr,
    creating: Option<&'c str>,
    rewrite: Rewrite<'c>,
}

/// The two sides of a comparison and their kinds.
struct Sides<'s> {
    left: &'s Expr,
    left_kind: Kind,
    right: &'s Expr,
    right_kind: Kind,
}

/// Whether a comparison's two sides can be compared. `fix_left` and
/// `fix_right` rebuild the line with one side replaced, for a suggestion.
fn comparison_problem(
    op: Comparison,
    sides: &Sides,
    span: Span,
    fix_left: &dyn Fn(&Expr) -> String,
    fix_right: &dyn Fn(&Expr) -> String,
) -> Option<Diagnostic> {
    let Sides {
        left,
        left_kind,
        right,
        right_kind,
    } = *sides;
    if !left_kind.known() || !right_kind.known() {
        return None;
    }

    if op.orders() {
        let (side, kind, fix) = if left_kind != Kind::Number {
            (left, left_kind, fix_left)
        } else if right_kind != Kind::Number {
            (right, right_kind, fix_right)
        } else {
            return None;
        };
        let problem = Diagnostic::new(
            side.span,
            format!(
                "{} compares numbers, but {side} is {}.\n\
                 Only numbers can be bigger or smaller than each other.",
                op.symbol(),
                kind.name()
            ),
        );
        return Some(match number_from_text(side) {
            Some(number) => problem.suggest(fix(&number)),
            None => problem,
        });
    }

    if left_kind == right_kind {
        return None;
    }
    let problem = Diagnostic::new(
        span,
        format!(
            "This compares {} ({left}) to {} ({right}).\n\
             Both sides have to be the same kind of value, and Quill won't turn one into the other.",
            left_kind.name(),
            right_kind.name()
        ),
    );
    let suggestion = match (left_kind, right_kind) {
        (Kind::Text, Kind::Number) => number_from_text(left).map(|n| fix_left(&n)),
        (Kind::Number, Kind::Text) => number_from_text(right).map(|n| fix_right(&n)),
        _ => None,
    };
    Some(match suggestion {
        Some(suggestion) => problem.suggest(suggestion),
        None => problem,
    })
}

/// A condition built from a value that isn't true/false, for an error message.
fn condition_example(expr: &Expr, kind: Kind) -> String {
    match kind {
        Kind::Text => format!("{expr} == \"yes\""),
        _ => format!("{expr} > 0"),
    }
}

/// The number inside a text like "5", as a number.
fn number_from_text(expr: &Expr) -> Option<Expr> {
    let ExprKind::Text(text) = &expr.ungrouped().kind else {
        return None;
    };
    let text = text.trim();
    Decimal::parse(text)?;
    Some(Expr {
        kind: ExprKind::Number(text.to_string()),
        span: expr.span,
    })
}

fn is_zero(expr: &Expr) -> bool {
    matches!(
        &expr.ungrouped().kind,
        ExprKind::Number(number) if Decimal::parse(number).is_some_and(|n| n.is_zero())
    )
}

/// `left` and `right` joined into one text, for the suggestion `"{a}{b}"`.
fn joined_text(left: &Expr, right: &Expr) -> Vec<Part> {
    let mut parts = Vec::new();
    for side in [left, right] {
        match &side.ungrouped().kind {
            ExprKind::Text(text) => parts.push(Part::Text(text.clone())),
            ExprKind::Template(inner) => parts.extend(inner.iter().cloned()),
            _ => parts.push(Part::Value(side.clone())),
        }
    }
    parts
}

/// A copy of `expr` with the part at `span` replaced by `with`.
fn replace(expr: &Expr, span: Span, with: &Expr) -> Expr {
    if expr.span == span {
        return with.clone();
    }
    let swap = |inner: &Expr| Box::new(replace(inner, span, with));
    let kind = match &expr.kind {
        ExprKind::Template(parts) => ExprKind::Template(
            parts
                .iter()
                .map(|part| match part {
                    Part::Value(value) => Part::Value(replace(value, span, with)),
                    text => text.clone(),
                })
                .collect(),
        ),
        ExprKind::Negate(inner) => ExprKind::Negate(swap(inner)),
        ExprKind::Not(inner) => ExprKind::Not(swap(inner)),
        ExprKind::Group(inner) => ExprKind::Group(swap(inner)),
        ExprKind::Arithmetic {
            op,
            left,
            right,
            op_span,
        } => ExprKind::Arithmetic {
            op: *op,
            left: swap(left),
            right: swap(right),
            op_span: *op_span,
        },
        ExprKind::Compare {
            op,
            left,
            right,
            op_span,
        } => ExprKind::Compare {
            op: *op,
            left: swap(left),
            right: swap(right),
            op_span: *op_span,
        },
        ExprKind::Logic {
            op,
            left,
            right,
            op_span,
        } => ExprKind::Logic {
            op: *op,
            left: swap(left),
            right: swap(right),
            op_span: *op_span,
        },
        other => other.clone(),
    };
    Expr {
        kind,
        span: expr.span,
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

    /// The single problem in `source`.
    fn problem(source: &str) -> Diagnostic {
        let mut found = problems(source);
        assert_eq!(found.len(), 1, "expected one problem, got {found:#?}");
        found.remove(0)
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
    fn check_rewrites_the_side_that_is_wrong() {
        let found = problems("greeting = \"Hi\"\ncheck greting == \"Hi\"");
        assert_eq!(
            found[0].suggestion.as_deref(),
            Some("check greeting == \"Hi\"")
        );
        let found = problems("greeting = \"Hi\"\ncheck greeting == greting");
        assert_eq!(
            found[0].suggestion.as_deref(),
            Some("check greeting == greeting")
        );
    }

    #[test]
    fn a_bad_value_still_creates_the_name() {
        // Only the first line is wrong; `say total` shouldn't be reported too.
        let found = problems("total = missing\nsay total");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn calculations_with_numbers_are_fine() {
        assert_eq!(
            problems(
                "price = 10\nqty = 3\ntotal = price * qty + 1.5\nsay -total % 4\n\
                 ok = total > 30 and not (qty == 0)\nsay \"Total: {total}, ok: {ok}\""
            ),
            vec![]
        );
    }

    #[test]
    fn adding_text_to_a_number_follows_the_readme() {
        let found = problem("price = 100\ntotal = price + \"5\"");
        assert_eq!((found.line, found.column, found.length), (2, 17, 3));
        assert_eq!(
            found.message,
            "You're adding Number (price) to Text (\"5\").\nQuill won't guess which one you meant."
        );
        assert_eq!(found.suggestion.as_deref(), Some("total = price + 5"));
    }

    #[test]
    fn adding_two_texts_suggests_joining_them() {
        let found = problem("first = \"Amina\"\nsay \"Hi \" + first");
        assert!(found
            .message
            .contains("You're adding Text (\"Hi \") to Text (first)"));
        assert_eq!(found.suggestion.as_deref(), Some("say \"Hi {first}\""));
    }

    #[test]
    fn other_calculations_need_numbers() {
        let found = problem("name = \"Amina\"\nsay name * 2");
        assert_eq!(found.column, 5);
        assert_eq!(
            found.message,
            "You're multiplying Text (name) by Number (2).\nCalculations only work on numbers."
        );
        let found = problem("say 10 - true");
        assert!(found
            .message
            .contains("subtracting true/false (true) from Number (10)"));
    }

    #[test]
    fn one_mistake_gives_one_error() {
        // `bad` is unknown, so the + and the comparison aren't reported too.
        let found = problems("x = bad + 1 > 2");
        assert_eq!(found.len(), 1);
        // A failed calculation isn't reported again where it is used.
        let found = problems("x = \"a\" * 2\nsay x + 1");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn dividing_by_zero_is_caught_before_running() {
        let found = problem("say 10 / 0");
        assert_eq!(found.column, 10);
        assert!(found.message.contains("divides by zero"));
        assert!(problem("say 10 % (0.0)").message.contains("remainder"));
    }

    #[test]
    fn numbers_with_too_many_digits() {
        let found = problem(&format!("x = {}", "9".repeat(40)));
        assert!(found
            .message
            .contains("more digits than a Quill number can hold"));
    }

    #[test]
    fn comparisons_need_matching_kinds() {
        let found = problem("say 5 == \"5\"");
        assert!(found
            .message
            .contains("compares Number (5) to Text (\"5\")"));
        assert_eq!(found.suggestion.as_deref(), Some("say 5 == 5"));

        let found = problem("name = \"Amina\"\nsay name > 3");
        assert_eq!(found.column, 5);
        assert!(found
            .message
            .contains("> compares numbers, but name is Text"));
    }

    #[test]
    fn checks_need_matching_kinds() {
        let found = problem("total = 3\ncheck total == \"3\"");
        assert_eq!((found.line, found.column), (2, 1));
        assert_eq!(found.suggestion.as_deref(), Some("check total == 3"));
        assert_eq!(
            problems("check 1 + 2 == 3\ncheck 2 < 3\ncheck \"a\" != \"b\""),
            vec![]
        );
    }

    #[test]
    fn conditions_must_be_true_or_false() {
        let found = problem("count = 3\nwhen\n  count -> say 1");
        assert_eq!((found.line, found.column), (3, 3));
        assert!(found.message.contains("count is a Number"));
        assert!(found.message.contains("like count > 0"));

        let found = problem("say 1 and true");
        assert!(found.message.contains("and joins conditions"));
        let found = problem("say not \"x\"");
        assert!(found.message.contains("not turns true into false"));
        let found = problem("say -\"x\"");
        assert!(found.message.contains("Only numbers can be negative"));
    }

    #[test]
    fn names_inside_text_are_checked() {
        let found = problem("name = \"Amina\"\nsay \"Hi {nme}!\"");
        assert_eq!((found.line, found.column), (2, 10));
        assert_eq!(found.suggestion.as_deref(), Some("say \"Hi {name}!\""));
    }

    #[test]
    fn typo_inside_a_calculation_rewrites_the_line() {
        let found = problem("price = 2\ntotal = prise * 3 + 1");
        assert_eq!(found.suggestion.as_deref(), Some("total = price * 3 + 1"));
    }

    #[test]
    fn var_values_can_be_set() {
        assert_eq!(
            problems("var visits = 0\nset visits = visits + 1\nsay visits"),
            vec![]
        );
    }

    #[test]
    fn set_needs_a_var_value_like_the_readme_says() {
        let found = problem("name = \"Amina\"\nset name = \"Omar\"");
        assert_eq!((found.line, found.column), (2, 5));
        assert!(found
            .message
            .contains("name can't change, because it isn't marked var"));
        assert!(found.message.contains("var name = \"Amina\""));
    }

    #[test]
    fn set_on_a_misspelled_name_suggests_the_var() {
        let found = problem("var visits = 0\nset vistis = 2");
        assert!(found.message.contains("There's no value called vistis"));
        assert_eq!(found.suggestion.as_deref(), Some("set visits = 2"));
    }

    #[test]
    fn set_on_an_unknown_name_suggests_var() {
        let found = problem("set total = 5");
        assert_eq!(found.suggestion.as_deref(), Some("var total = 5"));
    }

    #[test]
    fn set_before_the_var_is_created() {
        let found = problem("set count = 1\nvar count = 0");
        assert!(found.message.contains("isn't created until line 2"));
    }

    #[test]
    fn set_keeps_the_kind() {
        let found = problem("var total = 0\nset total = \"10\"");
        assert_eq!((found.line, found.column), (2, 13));
        assert!(found
            .message
            .contains("total holds a Number, so it can't be set to Text"));
        assert_eq!(found.suggestion.as_deref(), Some("set total = 10"));
    }

    #[test]
    fn creating_a_var_again_suggests_set() {
        let found = problem("var count = 0\ncount = 5");
        assert!(found.message.contains("To change it, use set"));
        assert_eq!(found.suggestion.as_deref(), Some("set count = 5"));
    }

    #[test]
    fn when_cases_are_checked() {
        let found =
            problem("var label = \"\"\nn = 5\nwhen\n  n > 3 -> set label = big\n  else -> say n");
        assert!(found.message.contains("There's no value called big"));
        assert_eq!(
            problems("n = 5\nwhen\n  n % 2 == 0 -> say \"even\"\n  else -> say \"odd\""),
            vec![]
        );
    }

    #[test]
    fn typo_in_a_when_condition_rewrites_the_case() {
        let found = problem("count = 1\nwhen\n  cout > 0 -> say 1");
        assert_eq!(found.suggestion.as_deref(), Some("count > 0 -> say 1"));
    }

    #[test]
    fn values_before_a_line_have_kinds() {
        let program = parse(&lex("a = 1\nvar b = \"x\"\nc = a > 0\nsay a").unwrap()).unwrap();
        let known = values_before(&program, 4);
        let summary: Vec<(String, Kind, bool)> = known
            .iter()
            .map(|k| (k.name.clone(), k.kind, k.mutable))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("a".to_string(), Kind::Number, false),
                ("b".to_string(), Kind::Text, true),
                ("c".to_string(), Kind::Bool, false),
            ]
        );
    }
}
