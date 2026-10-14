//! The checker: finds mistakes before anything runs.
//!
//! - every name refers to a value created earlier
//! - a value is created once; only values marked `var` can change, with `set`
//! - every value has a kind (Text, Number, or true/false), and each operation
//!   gets the kinds it needs, so `price + "5"` is caught here, not while running

use crate::compiler::too_many_digits;
use crate::decimal::Decimal;
use crate::diagnostic::{closest_match, Diagnostic, Span};
use crate::parser::{
    Access, Arithmetic, Catch, Comparison, Expr, ExprKind, Handler, Param, Part, Program, SigKind,
    Statement,
};

/// The kind of a value.
#[derive(Debug, Clone)]
pub enum Kind {
    Text,
    Number,
    Bool,
    /// Every item has this kind.
    List(Box<Kind>),
    /// Named fields. The order here is the order they were written.
    Record(Vec<FieldKind>),
    /// Money in one currency. The text is the code, such as `AED`.
    Money(String),
    /// A value that might be missing.
    Maybe(Box<Kind>),
    /// Not known because of an earlier problem. Never reported again, so one
    /// mistake gives one error.
    Unknown,
}

/// One field of a record's kind.
#[derive(Debug, Clone)]
pub struct FieldKind {
    pub name: String,
    pub kind: Kind,
}

impl PartialEq for Kind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Kind::Text, Kind::Text) | (Kind::Number, Kind::Number) | (Kind::Bool, Kind::Bool) => {
                true
            }
            (Kind::Unknown, Kind::Unknown) => true,
            (Kind::List(left), Kind::List(right)) => left == right,
            (Kind::Money(left), Kind::Money(right)) => left == right,
            (Kind::Maybe(left), Kind::Maybe(right)) => left == right,
            (Kind::Record(left), Kind::Record(right)) => {
                left.len() == right.len()
                    && left.iter().all(|field| {
                        right
                            .iter()
                            .any(|other| other.name == field.name && other.kind == field.kind)
                    })
            }
            _ => false,
        }
    }
}

impl Kind {
    /// The kind's name, as errors and `quill explain` show it.
    pub fn name(&self) -> String {
        match self {
            Kind::Text => "Text".to_string(),
            Kind::Number => "Number".to_string(),
            Kind::Bool => "true/false".to_string(),
            Kind::List(item) => format!("a list of {}", item.name()),
            Kind::Record(fields) => {
                let names: Vec<&str> = fields.iter().map(|field| field.name.as_str()).collect();
                format!("a record with {}", names.join(", "))
            }
            Kind::Money(code) => format!("money in {code}"),
            Kind::Maybe(inner) => format!("maybe {}", inner.name()),
            Kind::Unknown => "a value".to_string(),
        }
    }

    /// The name with "a" where English needs one: "holds a Number".
    fn with_article(&self) -> String {
        match self {
            Kind::Number => "a Number".to_string(),
            other => other.name(),
        }
    }

    fn known(&self) -> bool {
        !matches!(self, Kind::Unknown)
    }

    /// The kind the way a function writes it: "a number", "text", "a list of number".
    fn written_kind(&self) -> String {
        match self {
            Kind::Text => "text".to_string(),
            Kind::Number => "a number".to_string(),
            Kind::Bool => "true or false".to_string(),
            Kind::List(item) => format!("a list of {}", item.written_item()),
            Kind::Money(code) => format!("money in {code}"),
            Kind::Maybe(inner) => format!("maybe {}", inner.written_item()),
            other => other.with_article(),
        }
    }

    fn written_item(&self) -> String {
        match self {
            Kind::Text => "text".to_string(),
            Kind::Number => "number".to_string(),
            Kind::Bool => "true/false".to_string(),
            Kind::List(item) => format!("list of {}", item.written_item()),
            Kind::Money(code) => code.clone(),
            Kind::Maybe(inner) => format!("maybe {}", inner.written_item()),
            other => other.name(),
        }
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
    /// `true` when a `for` owns this name and moves it to the next number itself.
    pub loop_counter: bool,
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
    checker.stop_before = Some(line);
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
    checker.functions = function_signatures(program);
    checker.value(expr, None, &|_| String::new())
}

/// Rebuilds the line around a corrected value, for a suggestion.
type Rewrite<'r> = &'r dyn Fn(&Expr) -> String;

/// A function the program has written.
#[derive(Debug, Clone)]
struct KnownFn {
    name: String,
    span: Span,
    params: Vec<(String, Kind)>,
    /// Absent when the function gives nothing back.
    result: Option<Kind>,
    /// Failure names this function can leave with.
    fails: Vec<String>,
    /// `double(n: number) -> number`, for errors.
    written: String,
}

struct Checker<'a> {
    program: &'a Program,
    /// Values created so far, in order.
    defined: Vec<KnownValue>,
    /// Names from outside a function, while its body is being checked.
    outer: Vec<KnownValue>,
    functions: Vec<KnownFn>,
    /// The function whose body is being checked.
    in_function: Option<String>,
    /// `true` while a call's failures are handled by `on` or passed on by `try`.
    handling: bool,
    problems: Vec<Diagnostic>,
    /// While explaining one line, stop before it and keep the names that cover it.
    stop_before: Option<usize>,
}

impl<'a> Checker<'a> {
    fn new(program: &'a Program) -> Self {
        Checker {
            program,
            defined: Vec::new(),
            outer: Vec::new(),
            functions: Vec::new(),
            in_function: None,
            handling: false,
            problems: Vec::new(),
            stop_before: None,
        }
    }

    /// Checks `statement`. Returns true when the walk reached `stop_before`.
    fn statement(&mut self, statement: &Statement) -> bool {
        if self
            .stop_before
            .is_some_and(|stop| statement.line() >= stop)
        {
            return true;
        }
        match statement {
            Statement::Say { value, span, .. } => {
                let kind = self.value(value, None, &|fixed| format!("say {fixed}"));
                if let Kind::Maybe(inner) = &kind {
                    self.problems.push(Diagnostic::new(
                        *span,
                        format!(
                            "say would hide that this might be missing.\n\
                             Open it with when, and handle found and missing. \
                             The value inside is {}.",
                            inner.written_kind()
                        ),
                    ));
                }
            }
            Statement::Let {
                name,
                name_span,
                value,
                mutable,
                handlers,
            } => {
                let prefix = if *mutable { "var " } else { "" };
                let bare_call = matches!(value.kind, ExprKind::Call { .. });
                if bare_call {
                    self.handling = true;
                }
                let kind = self.value(value, Some(name), &|fixed| {
                    format!("{prefix}{name} = {fixed}")
                });
                self.handling = false;
                if matches!(value.kind, ExprKind::Missing) {
                    self.problems.push(Diagnostic::new(
                        value.span,
                        "missing belongs in return, inside a function that gives a maybe back.\n\
                         return missing gives nothing back. found 1 gives a value back.",
                    ));
                }
                if bare_call {
                    self.check_handlers(value, handlers);
                } else if !handlers.is_empty() {
                    self.problems.push(Diagnostic::new(
                        handlers[0].span,
                        "on handles the failures of a call.\n\
                         The value after = has to be the call itself, like charge(amount).",
                    ));
                }
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
                    None if self.functions.iter().any(|func| &func.name == name) => {
                        let line = self
                            .functions
                            .iter()
                            .find(|func| &func.name == name)
                            .map(|func| func.span.line)
                            .unwrap_or(name_span.line);
                        self.problems.push(Diagnostic::new(
                            *name_span,
                            format!(
                                "{name} is already a function, written on line {line}.\n\
                                 A value needs its own name."
                            ),
                        ));
                    }
                    None => self.defined.push(KnownValue {
                        name: name.clone(),
                        span: *name_span,
                        kind,
                        mutable: *mutable,
                        loop_counter: false,
                        created: statement.to_string(),
                    }),
                }
            }
            Statement::Set {
                name,
                name_span,
                steps,
                value,
                handlers,
                ..
            } => {
                let bare_call = matches!(value.kind, ExprKind::Call { .. });
                if bare_call {
                    self.handling = true;
                }
                self.set(name, *name_span, steps, value);
                self.handling = false;
                if bare_call {
                    self.check_handlers(value, handlers);
                }
            }
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
                    if self.stop_before.is_some_and(|stop| arm.span.line >= stop) {
                        return true;
                    }
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
                                    condition_example(condition, &kind)
                                ),
                            ));
                        }
                    }
                    if self.statement(&arm.body) {
                        return true;
                    }
                }
            }
            Statement::For {
                name,
                name_span,
                start,
                end,
                body,
                ..
            } => {
                let item_kind = match end {
                    Some(end) => {
                        self.expect_number(start, "start");
                        self.expect_number(end, "end");
                        Kind::Number
                    }
                    None => self.list_item_kind(start),
                };
                let saved = self.hide(name);
                self.defined.push(KnownValue {
                    name: name.clone(),
                    span: *name_span,
                    kind: item_kind,
                    mutable: false,
                    loop_counter: true,
                    created: statement.to_string(),
                });
                for statement in body {
                    if self.statement(statement) {
                        return true;
                    }
                }
                self.defined.pop();
                self.restore(saved);
            }
            Statement::Function {
                name,
                name_span,
                params,
                result,
                fails,
                body,
                ..
            } => return self.function(name, *name_span, params, result, fails, body),
            Statement::Return { value, span } => self.return_value(value.as_ref(), *span),
            Statement::Call {
                name,
                name_span,
                args,
                handlers,
                ..
            } => {
                self.handling = true;
                self.call_statement(name, *name_span, args);
                self.handling = false;
                self.check_call_handlers(name, handlers);
            }
            Statement::Match {
                subject,
                arms,
                span,
            } => {
                self.open_maybe(subject, arms, *span);
            }
            Statement::Fail { name, span } => self.fail_name(name, *span),
        }
        false
    }

    /// The start or end of a `for` range must be a number.
    fn expect_number(&mut self, expr: &Expr, role: &str) {
        let kind = self.value(expr, None, &|fixed| fixed.to_string());
        if kind.known() && kind != Kind::Number {
            self.problems.push(Diagnostic::new(
                expr.span,
                format!(
                    "The {role} of a for loop has to be a number, but {expr} is {}.\n\
                     for counts from one number to another, like for i in 1..3.",
                    kind.name()
                ),
            ));
        }
    }

    /// Takes a value of this name out of scope, so a `for` can use the name itself.
    fn hide(&mut self, name: &str) -> Option<(usize, KnownValue)> {
        let index = self.defined.iter().position(|known| known.name == name)?;
        Some((index, self.defined.remove(index)))
    }

    fn restore(&mut self, saved: Option<(usize, KnownValue)>) {
        if let Some((index, saved)) = saved {
            let index = index.min(self.defined.len());
            self.defined.insert(index, saved);
        }
    }

    /// Checks a function, then keeps it for later calls.
    fn function(
        &mut self,
        name: &str,
        name_span: Span,
        params: &[Param],
        result: &Option<SigKind>,
        fails: &[(String, Span)],
        body: &[Statement],
    ) -> bool {
        if let Some(first) = self.functions.iter().find(|func| func.name == name) {
            self.problems.push(Diagnostic::new(
                name_span,
                format!(
                    "{name} is already a function, written on line {}.\n\
                     Each function needs its own name.",
                    first.span.line
                ),
            ));
            return false;
        }
        if self.lookup(name).is_some() {
            self.problems.push(Diagnostic::new(
                name_span,
                format!(
                    "{name} is already a value.\n\
                     A function needs its own name."
                ),
            ));
            return false;
        }
        let known = known_function(name, name_span, params, result, fails);
        self.functions.push(known.clone());
        self.outer = std::mem::take(&mut self.defined);
        self.in_function = Some(name.to_string());
        for (param_name, kind) in &known.params {
            let param = params.iter().find(|param| &param.name == param_name);
            self.defined.push(KnownValue {
                name: param_name.clone(),
                span: param.map(|param| param.name_span).unwrap_or(name_span),
                kind: kind.clone(),
                mutable: false,
                loop_counter: false,
                created: format!(
                    "{param_name}: {}",
                    param.map(|param| param.kind.written()).unwrap_or_default()
                ),
            });
        }
        let mut stopped = false;
        for statement in body {
            if self.statement(statement) {
                stopped = true;
                break;
            }
        }
        if !stopped {
            if let Some(result_kind) = &known.result {
                if !body_returns(body) {
                    self.problems.push(
                        Diagnostic::new(
                            name_span,
                            format!(
                                "{name} has to give back {}.\n\
                             End every path with return, like return 1.",
                                result_kind.with_article()
                            ),
                        )
                        .suggest("return 1"),
                    );
                }
            }
            self.report_dead_code(body);
            self.defined = std::mem::take(&mut self.outer);
            self.in_function = None;
        }
        stopped
    }

    fn return_value(&mut self, value: Option<&Expr>, span: Span) {
        let Some(name) = self.in_function.clone() else {
            self.problems.push(Diagnostic::new(
                span,
                "return only goes inside a function, to give a value back.\n\
                 On its own line, use say to show a value.",
            ));
            return;
        };
        let result = self
            .functions
            .iter()
            .find(|func| func.name == name)
            .and_then(|func| func.result.clone());
        match (result, value) {
            (Some(expected), Some(value)) => {
                let kind = self.value(value, None, &|fixed| format!("return {fixed}"));
                if matches!(value.kind, ExprKind::Missing) && matches!(expected, Kind::Maybe(_)) {
                    return;
                }
                if kind.known() && kind != expected {
                    self.problems.push(Diagnostic::new(
                        value.span,
                        format!(
                            "{name} gives back {}, but {value} is {}.\n\
                             return has to give back {}.",
                            expected.with_article(),
                            kind.with_article(),
                            expected.written_kind()
                        ),
                    ));
                }
            }
            (Some(expected), None) => {
                self.problems.push(
                    Diagnostic::new(
                        span,
                        format!(
                            "{name} gives back {}, so return needs a value after it.\n\
                         return 1 gives a number back.",
                            expected.with_article()
                        ),
                    )
                    .suggest("return 1"),
                );
            }
            (None, Some(value)) => {
                self.problems.push(Diagnostic::new(
                    value.span,
                    format!(
                        "{name} doesn't give a value back, so return stands on its own.\n\
                         To give {value} back, write what {name} gives back after ->."
                    ),
                ));
            }
            (None, None) => {}
        }
    }

    fn call_statement(&mut self, name: &str, name_span: Span, args: &[Expr]) {
        let Some(func) = self
            .functions
            .iter()
            .find(|func| func.name == name)
            .cloned()
        else {
            self.missing_function(name, name_span);
            return;
        };
        if let Some(result) = &func.result {
            let suggestion = format!("var result = {}", call_written(name, args));
            let message = if self.handling {
                format!(
                    "{name} gives back {}, and this call doesn't keep it.\n\
                     Write var result = {}, then handle each failure with on.",
                    result.with_article(),
                    call_written(name, args)
                )
            } else {
                format!(
                    "{name} gives back {}, so the call has to be used.\n\
                     say {} shows what it gives back.",
                    result.with_article(),
                    func.written
                )
            };
            self.problems.push(
                Diagnostic::new(name_span, message).suggest(if self.handling {
                    suggestion
                } else {
                    format!("say {}", call_written(name, args))
                }),
            );
            if !self.handling {
                return;
            }
        }
        self.check_args(name, name_span, args, &func);
        self.require_handled(&func, name_span);
    }

    /// Money arithmetic, when either side is money. `None` when neither side is.
    fn money_arithmetic(
        &mut self,
        op: Arithmetic,
        expr: &Expr,
        left: &Expr,
        right: &Expr,
        left_kind: &Kind,
        right_kind: &Kind,
    ) -> Option<Kind> {
        let left_code = match left_kind {
            Kind::Money(code) => Some(code.clone()),
            _ => None,
        };
        let right_code = match right_kind {
            Kind::Money(code) => Some(code.clone()),
            _ => None,
        };
        if left_code.is_none() && right_code.is_none() {
            return None;
        }
        if let (Some(left_money), Some(right_money)) = (&left_code, &right_code) {
            if left_money == right_money && matches!(op, Arithmetic::Add | Arithmetic::Subtract) {
                return Some(Kind::Money(left_money.clone()));
            }
        }
        if op == Arithmetic::Multiply {
            if let (Some(code), Kind::Number) = (&left_code, right_kind) {
                return Some(Kind::Money(code.clone()));
            }
            if let (Some(code), Kind::Number) = (&right_code, left_kind) {
                return Some(Kind::Money(code.clone()));
            }
        }
        if let (Some(code), Kind::Number) = (&left_code, right_kind) {
            if op == Arithmetic::Divide {
                return Some(Kind::Money(code.clone()));
            }
        }
        let why = if left_code.is_some() && right_code.is_some() && left_code != right_code {
            format!(
                "Quill won't mix currencies. {} and {} stay apart until one of them is converted.",
                left_code.unwrap_or_default(),
                right_code.unwrap_or_default()
            )
        } else if matches!(op, Arithmetic::Add | Arithmetic::Subtract) {
            "Add or subtract two amounts in the same currency, like 100 AED + 50 AED.".to_string()
        } else if op == Arithmetic::Divide {
            "Divide money by a number, as in 100 AED / 2.".to_string()
        } else if op == Arithmetic::Remainder {
            "A remainder needs two numbers. Money can be added, subtracted, multiplied by a number, or divided by a number.".to_string()
        } else {
            "Multiply money by a number, as in 100 AED * 0.90.".to_string()
        };
        self.problems.push(Diagnostic::new(
            expr.span,
            format!(
                "You're {} {} ({left}) and {} ({right}).\n{why}",
                op.verb(),
                left_kind.name(),
                right_kind.name()
            ),
        ));
        Some(Kind::Unknown)
    }

    fn require_handled(&mut self, func: &KnownFn, span: Span) {
        if func.fails.is_empty() || self.handling {
            return;
        }
        self.problems.push(Diagnostic::new(
            span,
            format!(
                "{} can fail with {}.\n\
                 Handle each failure with on under the call, or write try {}(...) \
                 inside a function that fails the same way.",
                func.name,
                func.fails.join(" or "),
                func.name
            ),
        ));
    }

    fn check_handlers(&mut self, value: &Expr, handlers: &[Handler]) {
        let ExprKind::Call {
            name, name_span, ..
        } = &value.kind
        else {
            return;
        };
        let Some(func) = self
            .functions
            .iter()
            .find(|func| &func.name == name)
            .cloned()
        else {
            return;
        };
        self.cover_failures(&func, *name_span, handlers);
    }

    fn check_call_handlers(&mut self, name: &str, handlers: &[Handler]) {
        let Some(func) = self
            .functions
            .iter()
            .find(|func| func.name == name)
            .cloned()
        else {
            return;
        };
        let span = handlers
            .first()
            .map(|handler| handler.span)
            .unwrap_or(func.span);
        self.cover_failures(&func, span, handlers);
    }

    fn cover_failures(&mut self, func: &KnownFn, span: Span, handlers: &[Handler]) {
        if func.fails.is_empty() {
            if !handlers.is_empty() {
                self.problems.push(Diagnostic::new(
                    handlers[0].span,
                    format!(
                        "{} doesn't fail, so on doesn't belong here.\n\
                         on handles a failure named in fails.",
                        func.name
                    ),
                ));
            }
            return;
        }
        let mut seen = Vec::new();
        for handler in handlers {
            if !func.fails.iter().any(|name| name == &handler.name) {
                self.problems.push(Diagnostic::new(
                    handler.name_span,
                    format!(
                        "{} doesn't fail with {}.\n\
                         It fails with {}.",
                        func.name,
                        handler.name,
                        func.fails.join(" or ")
                    ),
                ));
            } else if seen.contains(&handler.name) {
                self.problems.push(Diagnostic::new(
                    handler.name_span,
                    format!(
                        "{} is already handled.\n\
                         Each failure needs one on.",
                        handler.name
                    ),
                ));
            } else {
                seen.push(handler.name.clone());
            }
            if self.in_function.is_some() {
                if let Some(result) = self
                    .functions
                    .iter()
                    .find(|outer| self.in_function.as_deref() == Some(outer.name.as_str()))
                    .and_then(|outer| outer.result.clone())
                {
                    if !always_returns(&handler.body) {
                        self.problems.push(Diagnostic::new(
                            handler.span,
                            format!(
                                "This on has to give back {}, or the function stops without a value.\n\
                                 End it with return.",
                                result.with_article()
                            ),
                        ));
                    }
                }
            }
            let _ = self.statement(&handler.body);
        }
        for name in &func.fails {
            if !seen.contains(name) {
                self.problems.push(
                    Diagnostic::new(
                        span,
                        format!(
                            "{name} isn't handled.\n\
                             {} can fail with {name}, so the call needs on {name}.",
                            func.name
                        ),
                    )
                    .suggest(format!("on {name} -> say \"{name}\"")),
                );
            }
        }
    }

    fn open_maybe(&mut self, subject: &Expr, arms: &[Catch], span: Span) {
        let kind = self.value(subject, None, &|fixed| format!("when {fixed}"));
        let inner = match kind {
            Kind::Maybe(inner) => *inner,
            Kind::Unknown => Kind::Unknown,
            other => {
                self.problems.push(Diagnostic::new(
                    subject.span,
                    format!(
                        "when opens a maybe here, but {subject} is {}.\n\
                         A maybe comes back from a function written -> maybe, \
                         and the cases below are found and missing.",
                        other.with_article()
                    ),
                ));
                Kind::Unknown
            }
        };
        let mut saw_found = false;
        let mut saw_missing = false;
        for arm in arms {
            match &arm.found {
                Some((name, name_span)) => {
                    if saw_found {
                        self.problems.push(Diagnostic::new(
                            *name_span,
                            "found is already handled above.\n\
                             A maybe has one found case and one missing case.",
                        ));
                    }
                    saw_found = true;
                    let saved = self.hide(name);
                    self.defined.push(KnownValue {
                        name: name.clone(),
                        span: *name_span,
                        kind: inner.clone(),
                        mutable: false,
                        loop_counter: false,
                        created: format!("found {name}"),
                    });
                    let _ = self.statement(&arm.body);
                    self.defined.pop();
                    self.restore(saved);
                }
                None => {
                    if saw_missing {
                        self.problems.push(Diagnostic::new(
                            arm.span,
                            "missing is already handled above.\n\
                             A maybe has one found case and one missing case.",
                        ));
                    }
                    saw_missing = true;
                    let _ = self.statement(&arm.body);
                }
            }
        }
        if !saw_found {
            self.problems.push(
                Diagnostic::new(
                    span,
                    "This when doesn't handle found.\n\
                     found name -> ... uses the value when it is there.",
                )
                .suggest("found value -> say value"),
            );
        }
        if !saw_missing {
            self.problems.push(
                Diagnostic::new(
                    span,
                    "This when doesn't handle missing.\n\
                     A maybe can be missing, and that case has to be handled too.",
                )
                .suggest("missing -> say \"none\""),
            );
        }
    }

    fn fail_name(&mut self, name: &str, span: Span) {
        let Some(function) = self.in_function.clone() else {
            self.problems.push(Diagnostic::new(
                span,
                "fail only goes inside a function that declares fails.",
            ));
            return;
        };
        let declared = self
            .functions
            .iter()
            .find(|func| func.name == function)
            .map(|func| func.fails.clone())
            .unwrap_or_default();
        if !declared.iter().any(|failure| failure == name) {
            let listed = if declared.is_empty() {
                format!("{function} doesn't declare any failures. Add fails {name} under fn.")
            } else {
                format!("{function} fails with {}.", declared.join(" or "))
            };
            self.problems.push(Diagnostic::new(
                span,
                format!("{function} doesn't fail with {name}.\n{listed}"),
            ));
        }
    }

    fn try_call(&mut self, inner: &Expr) -> Kind {
        let ExprKind::Call { name, .. } = &inner.kind else {
            self.problems.push(Diagnostic::new(
                inner.span,
                "try needs a call after it.\n\
                 try charge(amount) runs charge and passes its failures upward.",
            ));
            return Kind::Unknown;
        };
        let Some(function) = self.in_function.clone() else {
            self.problems.push(Diagnostic::new(
                inner.span,
                "try goes inside a function that fails the same way.\n\
                 At the top of the program, handle each failure with on.",
            ));
            return Kind::Unknown;
        };
        let saved = self.handling;
        self.handling = true;
        let kind = self.value(inner, None, &|fixed| format!("try {fixed}"));
        self.handling = saved;
        let callee = self
            .functions
            .iter()
            .find(|func| &func.name == name)
            .cloned();
        let caller = self
            .functions
            .iter()
            .find(|func| func.name == function)
            .cloned();
        if let (Some(callee), Some(caller)) = (callee, caller) {
            if callee.fails.is_empty() {
                self.problems.push(Diagnostic::new(
                    inner.span,
                    format!(
                        "try is for a call that can fail.\n\
                         {name} doesn't fail, so call it without try."
                    ),
                ));
            }
            for failure in &callee.fails {
                if !caller.fails.iter().any(|name| name == failure) {
                    self.problems.push(Diagnostic::new(
                        inner.span,
                        format!(
                            "{function} doesn't fail with {failure}, so it can't pass that failure on.\n\
                             Add {failure} after fails, or handle it with on."
                        ),
                    ));
                }
            }
        }
        kind
    }

    fn missing_function(&mut self, name: &str, span: Span) {
        if let Some(later) = self.function_later(name, span.line) {
            self.problems.push(Diagnostic::new(
                span,
                format!(
                    "{name} is used here, but the function isn't written until line {}.\n\
                     Write the function above the line that calls it.",
                    later.line
                ),
            ));
            return;
        }
        let problem = Diagnostic::new(
            span,
            format!(
                "There's no function called {name}.\n\
                 Write it with fn {name}(...) above this line."
            ),
        );
        let names = self.functions.iter().map(|func| func.name.as_str());
        self.problems.push(match closest_match(name, names) {
            Some(fixed) => problem.suggest(fixed.to_string()),
            None => problem.suggest(format!("fn {name}() -> number\n  return 1")),
        });
    }

    fn check_args(&mut self, name: &str, span: Span, args: &[Expr], func: &KnownFn) {
        if args.len() != func.params.len() {
            let count = func.params.len();
            let word = if count == 1 { "value" } else { "values" };
            self.problems.push(Diagnostic::new(
                span,
                format!(
                    "{name} takes {count} {word}, but this call has {}.\n\
                     {}.",
                    args.len(),
                    func.written
                ),
            ));
        }
        for (index, arg) in args.iter().enumerate() {
            let kind = self.value(arg, None, &|fixed| {
                let mut args = args.to_vec();
                if let Some(slot) = args.get_mut(index) {
                    *slot = fixed.clone();
                }
                call_written(name, &args)
            });
            if let Some((param, expected)) = func.params.get(index) {
                if matches!(arg.kind, ExprKind::Missing) && matches!(expected, Kind::Maybe(_)) {
                    continue;
                }
                if kind.known() && kind != *expected {
                    self.problems.push(Diagnostic::new(
                        arg.span,
                        format!(
                            "{name} needs {} for {param}, but {arg} is {}.\n\
                             {}.",
                            expected.written_kind(),
                            kind.with_article(),
                            func.written
                        ),
                    ));
                }
            }
        }
    }

    fn function_later(&self, name: &str, after_line: usize) -> Option<Span> {
        self.program.statements.iter().find_map(|statement| {
            if let Statement::Function {
                name: created,
                name_span,
                ..
            } = statement
            {
                if created == name && name_span.line > after_line {
                    return Some(*name_span);
                }
            }
            None
        })
    }

    fn report_dead_code(&mut self, body: &[Statement]) {
        for pair in body.windows(2) {
            if always_returns(&pair[0]) {
                self.problems.push(Diagnostic::new(
                    span_of(&pair[1]),
                    "This line can never run.\n\
                     return already left the function.",
                ));
            }
        }
        for statement in body {
            match statement {
                Statement::When { arms, .. } => {
                    for arm in arms {
                        self.report_dead_code(std::slice::from_ref(arm.body.as_ref()));
                    }
                }
                Statement::For { body, .. } | Statement::Function { body, .. } => {
                    self.report_dead_code(body);
                }
                _ => {}
            }
        }
    }

    fn set(&mut self, name: &str, name_span: Span, steps: &[Access], value: &Expr) {
        let place = access_text(steps);
        let kind = self.value(value, None, &|fixed| format!("set {name}{place} = {fixed}"));
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

        if target.loop_counter {
            self.problems.push(Diagnostic::new(
                name_span,
                format!(
                    "{name} changes on its own, once each time through the for.\n\
                     The for on line {} moves {name} on to the next one. \
                     To keep a total, create a var before the for and change that with set.",
                    target.span.line
                ),
            ));
            return;
        }

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

        let mut slot_kind = target.kind.clone();
        for step in steps {
            slot_kind = self.follow(step, &slot_kind, name);
        }
        if slot_kind.known() && kind.known() && slot_kind != kind {
            let problem = Diagnostic::new(
                value.span,
                format!(
                    "{name}{place} holds {}, so it can't be set to {} ({value}).\n\
                     A value keeps the same kind for the whole program.",
                    slot_kind.with_article(),
                    kind.name()
                ),
            );
            self.problems.push(match number_from_text(value) {
                Some(number) if slot_kind == Kind::Number => {
                    problem.suggest(format!("set {name}{place} = {number}"))
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
                if let Kind::Money(code) = &kind {
                    return Kind::Money(code.clone());
                }
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
                            condition_example(inner, &kind)
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
                                condition_example(side, &kind)
                            ),
                        ));
                    }
                }
                Kind::Bool
            }
            ExprKind::List { items, empty } => self.list_kind(items, empty.as_ref()),
            ExprKind::Record { fields } => {
                let mut kinds = Vec::new();
                for field in fields {
                    kinds.push(FieldKind {
                        name: field.name.clone(),
                        kind: self.infer(&field.value, context),
                    });
                }
                Kind::Record(kinds)
            }
            ExprKind::At {
                list,
                index,
                at_span,
            } => {
                let kind = self.infer(list, context);
                self.expect_position(index);
                match &kind {
                    Kind::List(item) => {
                        self.position_in_range(list, index);
                        (**item).clone()
                    }
                    Kind::Unknown => Kind::Unknown,
                    _ => {
                        self.problems.push(Diagnostic::new(
                            *at_span,
                            format!(
                                "{list} is {}, so it has no item at a position.\n\
                                 at picks an item from a list, like hobbies at 1. Lists count from 1.",
                                kind.name()
                            ),
                        ));
                        Kind::Unknown
                    }
                }
            }
            ExprKind::Money { amount, currency } => {
                if Decimal::parse(amount).is_none() {
                    self.problems.push(too_many_digits(amount, expr.span));
                }
                Kind::Money(currency.clone())
            }
            ExprKind::Found(inner) => Kind::Maybe(Box::new(self.infer(inner, context))),
            ExprKind::Missing => Kind::Maybe(Box::new(Kind::Unknown)),
            ExprKind::Try(inner) => self.try_call(inner),
            ExprKind::Call {
                name,
                name_span,
                args,
            } => {
                let Some(func) = self
                    .functions
                    .iter()
                    .find(|func| &func.name == name)
                    .cloned()
                else {
                    self.missing_function(name, *name_span);
                    return Kind::Unknown;
                };
                self.check_args(name, *name_span, args, &func);
                self.require_handled(&func, *name_span);
                match func.result.clone() {
                    Some(kind) => kind,
                    None => {
                        self.problems.push(Diagnostic::new(
                            *name_span,
                            format!(
                                "{name} doesn't give a value back.\n\
                                 Write {name}(...) on its own line. It runs, and nothing comes back."
                            ),
                        ));
                        Kind::Unknown
                    }
                }
            }
            ExprKind::Field {
                record,
                name,
                name_span,
            } => {
                let kind = self.infer(record, context);
                match &kind {
                    Kind::Record(fields) => self.field_kind(fields, name, *name_span),
                    Kind::Unknown => Kind::Unknown,
                    _ => {
                        self.problems.push(Diagnostic::new(
                            *name_span,
                            format!(
                                "{record} is {}, so it has no fields.\n\
                                 A dot reads a field of a record, like address.city.",
                                kind.name()
                            ),
                        ));
                        Kind::Unknown
                    }
                }
            }
        }
    }

    /// The kind of the items when `for` walks a list.
    fn list_item_kind(&mut self, list: &Expr) -> Kind {
        let kind = self.value(list, None, &|fixed| fixed.to_string());
        match kind {
            Kind::List(item) => *item,
            Kind::Unknown => Kind::Unknown,
            Kind::Number => {
                self.problems.push(Diagnostic::new(
                    list.span,
                    format!(
                        "for walks a list here, but {list} is a Number.\n\
                         To count from one number through another, write .. between them, like for i in 1..3."
                    ),
                ));
                Kind::Unknown
            }
            other => {
                self.problems.push(Diagnostic::new(
                    list.span,
                    format!(
                        "for walks a list here, but {list} is {}.\n\
                         A list holds values of one kind. To count, use a range: for i in 1..3.",
                        other.name()
                    ),
                ));
                Kind::Unknown
            }
        }
    }

    fn list_kind(&mut self, items: &[Expr], empty: Option<&SigKind>) -> Kind {
        let Some(first) = items.first() else {
            return match empty {
                Some(kind) => Kind::List(Box::new(sig_kind(kind))),
                None => Kind::Unknown,
            };
        };
        let item_kind = self.value(first, None, &|fixed| fixed.to_string());
        for item in items.iter().skip(1) {
            let kind = self.value(item, None, &|fixed| fixed.to_string());
            if item_kind.known() && kind.known() && kind != item_kind {
                self.problems.push(Diagnostic::new(
                    item.span,
                    format!(
                        "Every item in a list has to be the same kind.\n\
                         This list holds {}, but {item} is {}.",
                        item_kind.name(),
                        kind.name()
                    ),
                ));
            }
        }
        if item_kind.known() {
            Kind::List(Box::new(item_kind))
        } else {
            Kind::Unknown
        }
    }

    /// Walk one `set` step and return the kind that step reaches.
    fn follow(&mut self, step: &Access, kind: &Kind, root: &str) -> Kind {
        if !kind.known() {
            return Kind::Unknown;
        }
        match step {
            Access::Field { name, span } => match kind {
                Kind::Record(fields) => self.field_kind(fields, name, *span),
                _ => {
                    self.problems.push(Diagnostic::new(
                        *span,
                        format!(
                            "{root} is {}, so it has no field called {name}.\n\
                             A dot reads a field of a record, like address.city.",
                            kind.name()
                        ),
                    ));
                    Kind::Unknown
                }
            },
            Access::At { index, span } => {
                self.expect_position(index);
                match kind {
                    Kind::List(item) => (**item).clone(),
                    _ => {
                        self.problems.push(Diagnostic::new(
                            *span,
                            format!(
                                "{root} is {}, so it has no item at a position.\n\
                                 at picks an item from a list, like hobbies at 1.",
                                kind.name()
                            ),
                        ));
                        Kind::Unknown
                    }
                }
            }
        }
    }

    fn field_kind(&mut self, fields: &[FieldKind], name: &str, span: Span) -> Kind {
        if let Some(field) = fields.iter().find(|field| field.name == name) {
            return field.kind.clone();
        }
        let known = fields.iter().map(|field| field.name.as_str());
        let problem = Diagnostic::new(
            span,
            format!(
                "This record has no field called {name}.\n\
                 Its fields are {}.",
                fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
        self.problems.push(match closest_match(name, known) {
            Some(fixed) => problem.suggest(fixed),
            None => problem,
        });
        Kind::Unknown
    }

    /// A list position is a whole number, and 1 is the first item.
    fn expect_position(&mut self, index: &Expr) {
        let kind = self.value(index, None, &|fixed| fixed.to_string());
        if kind.known() && kind != Kind::Number {
            self.problems.push(Diagnostic::new(
                index.span,
                format!(
                    "A list position has to be a number, but {index} is {}.\n\
                     Lists count from 1: hobbies at 1 is the first hobby.",
                    kind.name()
                ),
            ));
            return;
        }
        let ExprKind::Number(written) = &index.ungrouped().kind else {
            return;
        };
        let Some(number) = Decimal::parse(written) else {
            return;
        };
        match number.whole() {
            Some(position) if position >= 1 => {}
            Some(position) => self.problems.push(Diagnostic::new(
                index.span,
                format!(
                    "Lists count from 1, so {position} isn't a position.\n\
                     The first item is at 1."
                ),
            )),
            None => self.problems.push(Diagnostic::new(
                index.span,
                format!(
                    "A list position has to be a whole number, but {written} isn't.\n\
                     Lists count from 1: hobbies at 1 is the first hobby."
                ),
            )),
        }
    }

    /// When both the list and the position are written out, reject a position past the end.
    fn position_in_range(&mut self, list: &Expr, index: &Expr) {
        let ExprKind::List { items, empty: None } = &list.ungrouped().kind else {
            return;
        };
        let ExprKind::Number(written) = &index.ungrouped().kind else {
            return;
        };
        let Some(number) = Decimal::parse(written) else {
            return;
        };
        let Some(position) = number.whole() else {
            return;
        };
        let Some(position) = usize::try_from(position).ok() else {
            return;
        };
        if position > items.len() {
            let last = items.len();
            self.problems.push(Diagnostic::new(
                index.span,
                format!(
                    "This list has {last} {}, so {position} is past the end.\n\
                     The last item is at {last}.",
                    if last == 1 { "item" } else { "items" }
                ),
            ));
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
        if let Some(kind) = self.money_arithmetic(op, expr, left, right, &left_kind, &right_kind) {
            return kind;
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
            return known.kind.clone();
        }
        if let Some(func) = self.functions.iter().find(|func| func.name == name) {
            self.problems.push(
                Diagnostic::new(
                    expr.span,
                    format!(
                        "{name} is a function, so it has to be called.\n\
                         {}.",
                        func.written
                    ),
                )
                .suggest(func.written.clone()),
            );
            return Kind::Unknown;
        }
        if self.in_function.is_some() && self.outer.iter().any(|known| known.name == name) {
            let function = self.in_function.clone().unwrap_or_default();
            self.problems.push(Diagnostic::new(
                expr.span,
                format!(
                    "{name} is outside {function}.\n\
                     A function uses its parameters and the names it creates inside it. \
                     Pass {name} in as a parameter."
                ),
            ));
            return Kind::Unknown;
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
                Statement::Function {
                    name: created,
                    name_span,
                    ..
                } if created == name && name_span.line > after_line => Some(*name_span),
                _ => None,
            })
    }
}

fn known_function(
    name: &str,
    span: Span,
    params: &[Param],
    result: &Option<SigKind>,
    fails: &[(String, Span)],
) -> KnownFn {
    let written_params: Vec<String> = params
        .iter()
        .map(|param| format!("{}: {}", param.name, param.kind.written()))
        .collect();
    KnownFn {
        name: name.to_string(),
        span,
        params: params
            .iter()
            .map(|param| (param.name.clone(), sig_kind(&param.kind)))
            .collect(),
        result: result.as_ref().map(sig_kind),
        fails: fails.iter().map(|(name, _)| name.clone()).collect(),
        written: format!("{name}({})", written_params.join(", ")),
    }
}

fn function_signatures(program: &Program) -> Vec<KnownFn> {
    program
        .statements
        .iter()
        .filter_map(|statement| {
            let Statement::Function {
                name,
                name_span,
                params,
                result,
                fails,
                ..
            } = statement
            else {
                return None;
            };
            Some(known_function(name, *name_span, params, result, fails))
        })
        .collect()
}

fn sig_kind(kind: &SigKind) -> Kind {
    match kind {
        SigKind::Text => Kind::Text,
        SigKind::Number => Kind::Number,
        SigKind::Bool => Kind::Bool,
        SigKind::List(item) => Kind::List(Box::new(sig_kind(item))),
        SigKind::Money(code) => Kind::Money(code.clone()),
        SigKind::Maybe(inner) => Kind::Maybe(Box::new(sig_kind(inner))),
    }
}

fn call_written(name: &str, args: &[Expr]) -> String {
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    format!("{name}({})", args.join(", "))
}

fn body_returns(body: &[Statement]) -> bool {
    body.last().is_some_and(always_returns)
}

fn always_returns(statement: &Statement) -> bool {
    match statement {
        Statement::Return { .. } | Statement::Fail { .. } => true,
        Statement::Match { arms, .. } => arms.iter().all(|arm| always_returns(&arm.body)),
        Statement::When { arms, .. } => {
            arms.iter().any(|arm| arm.condition.is_none())
                && arms.iter().all(|arm| always_returns(&arm.body))
        }
        _ => false,
    }
}

fn span_of(statement: &Statement) -> Span {
    match statement {
        Statement::Say { span, .. }
        | Statement::Set { span, .. }
        | Statement::Check { span, .. }
        | Statement::When { span, .. }
        | Statement::For { span, .. }
        | Statement::Function { span, .. }
        | Statement::Return { span, .. }
        | Statement::Call { span, .. } => *span,
        Statement::Let { name_span, .. } => *name_span,
        Statement::Match { span, .. } | Statement::Fail { span, .. } => *span,
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
    let left = sides.left;
    let right = sides.right;
    let left_kind = sides.left_kind.clone();
    let right_kind = sides.right_kind.clone();
    if !left_kind.known() || !right_kind.known() {
        return None;
    }

    if op.orders() {
        if let (Kind::Money(left_code), Kind::Money(right_code)) = (&left_kind, &right_kind) {
            if left_code == right_code {
                return None;
            }
            return Some(Diagnostic::new(
                span,
                format!(
                    "{} compares money in {left_code} ({left}) to money in {right_code} ({right}).\n\
                     Quill won't mix currencies. Compare two amounts in the same currency.",
                    op.symbol()
                ),
            ));
        }
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
fn condition_example(expr: &Expr, kind: &Kind) -> String {
    match kind {
        Kind::Text => format!("{expr} == \"yes\""),
        _ => format!("{expr} > 0"),
    }
}

fn access_text(steps: &[Access]) -> String {
    let mut out = String::new();
    for step in steps {
        match step {
            Access::Field { name, .. } => out.push_str(&format!(".{name}")),
            Access::At { index, .. } => out.push_str(&format!(" at {index}")),
        }
    }
    out
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
        ExprKind::List { items, empty } => ExprKind::List {
            items: items.iter().map(|item| replace(item, span, with)).collect(),
            empty: empty.clone(),
        },
        ExprKind::Record { fields } => ExprKind::Record {
            fields: fields
                .iter()
                .map(|field| crate::parser::Field {
                    name: field.name.clone(),
                    name_span: field.name_span,
                    value: replace(&field.value, span, with),
                })
                .collect(),
        },
        ExprKind::At {
            list,
            index,
            at_span,
        } => ExprKind::At {
            list: swap(list),
            index: swap(index),
            at_span: *at_span,
        },
        ExprKind::Field {
            record,
            name,
            name_span,
        } => ExprKind::Field {
            record: swap(record),
            name: name.clone(),
            name_span: *name_span,
        },
        ExprKind::Call {
            name,
            name_span,
            args,
        } => ExprKind::Call {
            name: name.clone(),
            name_span: *name_span,
            args: args.iter().map(|arg| replace(arg, span, with)).collect(),
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
        let found = problems("total = nowhere\nsay total");
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
            .map(|k| (k.name.clone(), k.kind.clone(), k.mutable))
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

    #[test]
    fn a_for_loop_gives_its_name_only_to_the_body() {
        assert_eq!(problems("for i in 1..3\n  say i\nsay \"done\""), vec![]);
        let found = problem("for i in 1..3\n  say i\nsay i");
        assert!(found.message.contains("There's no value called i"));
        let found = problem("for i in 1..\"3\"\n  say i");
        assert!(found
            .message
            .contains("The end of a for loop has to be a number"));
        let found = problem("var total = 0\nfor i in 1..3\n  set i = i + 1");
        assert!(found.message.contains("changes on its own"));
    }

    #[test]
    fn a_for_loop_can_reuse_an_outer_name() {
        assert_eq!(problems("i = 10\nfor i in i..12\n  say i\nsay i"), vec![]);
    }

    #[test]
    fn a_list_holds_one_kind_and_a_record_names_its_fields() {
        assert_eq!(
            problems("hobbies = list \"coding\", \"reading\"\nsay hobbies at 1"),
            vec![]
        );
        let found = problem("hobbies = list \"coding\", 2");
        assert!(found.message.contains("same kind"));
        let found = problem("hobbies = list \"coding\"\nsay hobbies.city");
        assert!(found.message.contains("has no fields"));
        let found = problem("address = record city = \"Anytown\"\nsay address.street");
        assert!(found.message.contains("no field called street"));
        let found = problem("hobbies = list \"coding\", \"reading\"\nsay hobbies at 0");
        assert!(found.message.contains("count from 1"));
        assert_eq!(
            problems("var hobbies = list of text\nset hobbies = list \"coding\""),
            vec![]
        );
        let found =
            problem("address = record city = \"Anytown\"\nset address.city = \"Springfield\"");
        assert!(found.message.contains("isn't marked var"));
        assert_eq!(
            problems("var address = record city = \"Anytown\"\nset address.city = \"Springfield\""),
            vec![]
        );
        let found = problem("for hobby in 1\n  say hobby");
        assert!(found.message.contains("for i in 1..3"));
    }

    #[test]
    fn a_function_gives_back_the_kind_it_names() {
        assert_eq!(
            problems("fn double(n: number) -> number\n  return n * 2\nsay double(4)"),
            vec![]
        );
        let found = problem("fn double(n: number) -> number\n  return n * 2\nsay double(\"4\")");
        assert!(found.message.contains("needs a number"));
        let found = problem("fn double(n: number) -> number\n  say n");
        assert!(found.message.contains("has to give back"));
    }
}
