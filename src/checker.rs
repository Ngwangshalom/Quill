//! The checker: finds mistakes before anything runs.
//!
//! - every name refers to a value created earlier
//! - a value is created once; only values marked `var` can change, with `set`
//! - every value has a kind (Text, Number, or true/false), and each operation
//!   gets the kinds it needs, so `price + "5"` is caught here, not while running

use crate::compiler::too_many_digits;
use crate::decimal::Decimal;
use crate::diagnostic::{closest_match, Diagnostic, Span};
use crate::native;
use crate::parser;
use crate::parser::{
    Access, Arithmetic, Catch, Comparison, Contract, Expr, ExprKind, Handler, Param, Part,
    Permission, Program, Recovery, RoundTo, Rule, SigKind, Statement, ValueArm,
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
    /// Money in one currency. The text is the code, such as `USD`.
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
        if statement.file() == 0 && statement.line() >= line {
            break;
        }
        checker.statement(statement);
    }
    checker.defined
}

/// For every `where` that filters records: the fields its condition reads. The span
/// covers the whole `where`.
pub fn where_fields(program: &Program) -> Vec<(Span, Vec<String>)> {
    let mut checker = Checker::new(program);
    for statement in &program.statements {
        checker.statement(statement);
    }
    checker.where_fields
}

/// The currency of every `sum` whose field is money. The span covers the whole `sum`.
pub fn sum_currencies(program: &Program) -> Vec<(Span, String)> {
    let mut checker = Checker::new(program);
    for statement in &program.statements {
        checker.statement(statement);
    }
    checker.sum_currencies
}

/// The kind of `expr`, given the values that exist.
pub fn kind_of(program: &Program, expr: &Expr, known: &[KnownValue]) -> Kind {
    let mut checker = Checker::new(program);
    checker.defined = known.to_vec();
    checker.functions = function_signatures(program, &checker.types);
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
    /// Each parameter's `where` range, in the same order as `params`.
    rules: Vec<Option<Rule>>,
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
    /// Currency of each `sum` whose field is money, so an empty list still has a currency.
    sum_currencies: Vec<(Span, String)>,
    /// See `where_fields`.
    where_fields: Vec<(Span, Vec<String>)>,
    /// Every `type` in the program, so a type can be used above the line that writes it.
    types: Vec<KnownType>,
    /// Every `permit`, so a call that names a file or a setting can be checked before running.
    permits: native::Permits,
    /// The Quill modules the program brings, like `http`.
    modules: Vec<String>,
}

/// The Quill modules a program brings: `bring http` brings `http`.
fn program_modules(program: &Program) -> Vec<String> {
    program
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Bring {
                target,
                quoted: false,
                ..
            } if crate::bring::MODULES.contains(&target.as_str()) => Some(target.clone()),
            _ => None,
        })
        .collect()
}

/// Every `permit` the program writes.
pub fn program_permits(program: &Program) -> native::Permits {
    native::Permits {
        list: program
            .statements
            .iter()
            .filter_map(|statement| match statement {
                Statement::Permit { access, target, .. } => Some((*access, target.clone())),
                _ => None,
            })
            .collect(),
        folder: Default::default(),
    }
}

/// Quill's own functions, as the checker knows them.
fn native_signatures() -> Vec<KnownFn> {
    native::NATIVES
        .iter()
        .map(|native| {
            let kind = |kind: native::Kind| match kind {
                native::Kind::Text => Kind::Text,
                native::Kind::Bool => Kind::Bool,
                native::Kind::MaybeText => Kind::Maybe(Box::new(Kind::Text)),
                // Unknown matches every kind an argument has.
                native::Kind::Any => Kind::Unknown,
                native::Kind::TextList => Kind::List(Box::new(Kind::Text)),
                native::Kind::Number => Kind::Number,
                native::Kind::Response => Kind::Record(vec![
                    FieldKind {
                        name: "status".to_string(),
                        kind: Kind::Number,
                    },
                    FieldKind {
                        name: "body".to_string(),
                        kind: Kind::Text,
                    },
                    FieldKind {
                        name: "kind".to_string(),
                        kind: Kind::Text,
                    },
                ]),
                native::Kind::Answer => Kind::Record(vec![
                    FieldKind {
                        name: "status".to_string(),
                        kind: Kind::Number,
                    },
                    FieldKind {
                        name: "body".to_string(),
                        kind: Kind::Text,
                    },
                ]),
            };
            let params: Vec<(String, Kind)> = native
                .params
                .iter()
                .map(|(name, param)| (name.to_string(), kind(*param)))
                .collect();
            let written: Vec<String> = params
                .iter()
                .map(|(name, param)| format!("{name}: {}", param.written_item()))
                .collect();
            KnownFn {
                name: native.name.to_string(),
                span: Span::default(),
                rules: vec![None; params.len()],
                params,
                result: native.result.map(kind),
                fails: native.fails.iter().map(|fail| fail.to_string()).collect(),
                written: format!("{}({})", native.name, written.join(", ")),
            }
        })
        .collect()
}

/// A `type` and its fields.
#[derive(Debug, Clone)]
pub struct KnownType {
    pub name: String,
    pub span: Span,
    pub fields: Vec<Param>,
}

/// Every `type` written at the top of the program, and the types a brought module gives.
pub fn program_types(program: &Program) -> Vec<KnownType> {
    let mut types = written_types(program);
    if program_modules(program)
        .iter()
        .any(|module| module == "web")
    {
        let field = |name: &str, kind: SigKind| Param {
            name: name.to_string(),
            name_span: Span::default(),
            kind,
            kind_span: Span::default(),
            rule: None,
        };
        types.push(KnownType {
            name: "Request".to_string(),
            span: Span::default(),
            fields: vec![
                field("method", SigKind::Text),
                field("path", SigKind::Text),
                field("query", SigKind::Text),
                field("body", SigKind::Text),
            ],
        });
        types.push(KnownType {
            name: "Response".to_string(),
            span: Span::default(),
            fields: vec![
                field("status", SigKind::Number),
                field("body", SigKind::Text),
                field("kind", SigKind::Text),
            ],
        });
    }
    types
}

fn written_types(program: &Program) -> Vec<KnownType> {
    program
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Type {
                name,
                name_span,
                fields,
                ..
            } => Some(KnownType {
                name: name.clone(),
                span: *name_span,
                fields: fields.clone(),
            }),
            _ => None,
        })
        .collect()
}

impl<'a> Checker<'a> {
    fn new(program: &'a Program) -> Self {
        Checker {
            program,
            defined: Vec::new(),
            outer: Vec::new(),
            functions: native_signatures(),
            in_function: None,
            handling: false,
            problems: Vec::new(),
            stop_before: None,
            sum_currencies: Vec::new(),
            where_fields: Vec::new(),
            types: program_types(program),
            permits: program_permits(program),
            modules: program_modules(program),
        }
    }

    /// Checks `statement`. Returns true when the walk reached `stop_before`.
    fn statement(&mut self, statement: &Statement) -> bool {
        if self
            .stop_before
            .is_some_and(|stop| statement.file() == 0 && statement.line() >= stop)
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
                            .func(name)
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
            Statement::Function { .. } => return self.function(statement),
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
                self.check_call_handlers(name, *name_span, handlers);
            }
            Statement::Match {
                subject,
                arms,
                span,
            } => {
                self.open_maybe(subject, arms, *span);
            }
            Statement::Fail { name, span } => self.fail_name(name, *span),
            Statement::Type {
                name,
                name_span,
                fields,
                ..
            } => self.type_statement(name, *name_span, fields),
            // The loader has already brought in the file's functions and types.
            Statement::Bring { .. } => {}
            Statement::Permit {
                access,
                target,
                target_span,
                ..
            } => self.permit_statement(*access, target, *target_span),
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
    fn function(&mut self, function: &Statement) -> bool {
        let Statement::Function {
            name,
            name_span,
            params,
            result,
            fails,
            contracts,
            body,
            ..
        } = function
        else {
            return false;
        };
        let (name, name_span) = (name.as_str(), *name_span);
        if name.starts_with(parser::ROUTE) && !self.modules.iter().any(|module| module == "web") {
            self.problems.push(
                Diagnostic::new(
                    name_span,
                    "route is part of Quill's web module, which this program doesn't bring.\n\
                     Add bring web at the top, so a reader sees that the program serves requests.",
                )
                .suggest("bring web"),
            );
            return false;
        }
        if native::find(name).is_some() {
            self.problems.push(Diagnostic::new(
                name_span,
                format!(
                    "{name} is one of Quill's own functions, so a function you write \
                     can't use that name.\n\
                     Give yours a name that says what it does differently."
                ),
            ));
            return false;
        }
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
        for param in params {
            self.known_types(&param.kind, param.kind_span);
        }
        if let Some(result) = result {
            self.known_types(result, name_span);
        }
        let known = known_function(name, name_span, params, result, fails, &self.types);
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
        for param in params {
            if let Some(rule) = &param.rule {
                self.rule(param, rule);
            }
        }
        for contract in contracts {
            self.contract(name, contract, known.result.as_ref(), params);
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
                if !body_returns(body) && name.starts_with(parser::ROUTE) {
                    self.problems.push(
                        Diagnostic::new(
                            name_span,
                            format!(
                                "{} has to respond on every path.\n\
                                 End each path with respond, like respond 200, \"OK\".",
                                name.trim_start_matches(parser::ROUTE)
                            ),
                        )
                        .suggest("respond 200, \"OK\""),
                    );
                } else if !body_returns(body) {
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

    /// Reports a type name in `kind` that no `type` writes.
    fn known_types(&mut self, kind: &SigKind, span: Span) {
        let Some(name) = named_in(kind) else {
            return;
        };
        if self.types.iter().any(|known| known.name == name) {
            return;
        }
        let problem = Diagnostic::new(
            span,
            format!(
                "There's no type called {name}.\n\
                 Write it with type {name}, then its fields on the lines below, indented."
            ),
        );
        let names = self.types.iter().map(|known| known.name.as_str());
        self.problems.push(match closest_match(name, names) {
            Some(fixed) => problem.suggest(fixed.to_string()),
            None => problem.suggest(format!("type {name}\n  name: text")),
        });
    }

    /// `type Booking` and its fields: each kind exists, each `where` fits, and the type
    /// doesn't hold itself.
    fn type_statement(&mut self, name: &str, name_span: Span, fields: &[Param]) {
        if let Some(first) = self
            .types
            .iter()
            .find(|known| known.name == name && known.span != name_span)
        {
            if first.span.line == 0 {
                self.problems.push(Diagnostic::new(
                    name_span,
                    format!(
                        "{name} is a type Quill's web module gives every route.\n\
                         Give your type another name."
                    ),
                ));
                return;
            }
            if first.span.line < name_span.line {
                self.problems.push(Diagnostic::new(
                    name_span,
                    format!(
                        "{name} is already a type, written on line {}.\n\
                         Each type needs its own name.",
                        first.span.line
                    ),
                ));
                return;
            }
        }
        for field in fields {
            self.known_types(&field.kind, field.kind_span);
            if let Some(rule) = &field.rule {
                self.rule(field, rule);
            }
            if holds(&field.kind, name, &self.types, &mut Vec::new()) {
                self.problems.push(Diagnostic::new(
                    field.kind_span,
                    format!(
                        "A {name} can't hold a {name} inside it, \
                         because that one would hold another, forever.\n\
                         Hold something that names the other {name} instead, like its id."
                    ),
                ));
            }
        }
    }

    /// `Booking venue = "Town Hall", guests = 120`: every field of the type, each of its kind.
    fn typed_kind(
        &mut self,
        name: &str,
        name_span: Span,
        record: &Expr,
        context: &Context,
    ) -> Kind {
        let ExprKind::Record { fields } = &record.kind else {
            return Kind::Unknown;
        };
        let Some(known) = self.types.iter().find(|known| known.name == name).cloned() else {
            for field in fields {
                self.infer(&field.value, context);
            }
            self.known_types(&SigKind::Named(name.to_string()), name_span);
            return Kind::Unknown;
        };
        let wanted: Vec<&str> = known
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect();
        let mut ok = true;
        // A misspelled field is reported once, not again as a missing one.
        let mut meant = Vec::new();
        for field in fields {
            let Some(expected) = known.fields.iter().find(|wanted| wanted.name == field.name)
            else {
                ok = false;
                let problem = Diagnostic::new(
                    field.name_span,
                    format!(
                        "A {name} has no field called {}.\nIts fields are {}.",
                        field.name,
                        wanted.join(", ")
                    ),
                );
                self.problems
                    .push(match closest_match(&field.name, wanted.iter().copied()) {
                        Some(fixed) => {
                            meant.push(fixed.to_string());
                            problem.suggest(fixed.to_string())
                        }
                        None => problem,
                    });
                continue;
            };
            let expected_kind = sig_kind(&expected.kind, &self.types);
            let kind = self.infer(&field.value, context);
            let fits = matches!(field.value.kind, ExprKind::Missing)
                && matches!(expected_kind, Kind::Maybe(_));
            if !fits && kind.known() && expected_kind.known() && kind != expected_kind {
                ok = false;
                self.problems.push(Diagnostic::new(
                    field.value.span,
                    format!(
                        "A {name}'s {} is {}, but {} is {}.\n\
                         Line {} says {}: {}.",
                        field.name,
                        expected_kind.written_kind(),
                        field.value,
                        kind.with_article(),
                        expected.name_span.line,
                        expected.name,
                        expected.kind.written()
                    ),
                ));
                continue;
            }
            if let (Some(rule), Some(given)) = (&expected.rule, field.value.written_amount()) {
                let below = rule.low.written_amount().is_some_and(|low| given < low);
                let above = rule.high.written_amount().is_some_and(|high| given > high);
                if below || above {
                    ok = false;
                    self.problems.push(Diagnostic::new(
                        field.value.span,
                        format!(
                            "A {name} takes {} {}, but this one gives {}.\n\
                             Line {} says {} {rule}.",
                            field.name,
                            rule.range(),
                            field.value,
                            expected.name_span.line,
                            field.name
                        ),
                    ));
                }
            }
        }
        let missing: Vec<&str> = wanted
            .iter()
            .copied()
            .filter(|wanted| !fields.iter().any(|field| field.name == *wanted))
            .filter(|wanted| !meant.iter().any(|name| name == wanted))
            .collect();
        if !missing.is_empty() {
            ok = false;
            let (word, them) = if missing.len() == 1 {
                ("field", "it")
            } else {
                ("fields", "them")
            };
            self.problems.push(Diagnostic::new(
                name_span,
                format!(
                    "This {name} is missing the {word} {}.\n\
                     Every {name} has all of its fields. Add {them}, or use missing for a maybe field.",
                    missing.join(", ")
                ),
            ));
        }
        if !ok {
            return Kind::Unknown;
        }
        sig_kind(&SigKind::Named(name.to_string()), &self.types)
    }

    /// `round x`, `round x to cents`, `round x to 2 places`: the same kind as `x`.
    fn round_kind(&mut self, value: &Expr, to: &RoundTo, context: &Context) -> Kind {
        let kind = self.infer(value, context);
        let money = matches!(kind, Kind::Money(_));
        if kind.known() && kind != Kind::Number && !money {
            self.problems.push(Diagnostic::new(
                value.span,
                format!(
                    "round works on a number or an amount of money, but {value} is {}.",
                    kind.with_article()
                ),
            ));
            return Kind::Unknown;
        }
        match to {
            RoundTo::Cents if kind == Kind::Number => {
                self.problems.push(
                    Diagnostic::new(
                        value.span,
                        format!(
                            "{value} is a number, and cents belong to money.\n\
                             Round a number to a count of places instead."
                        ),
                    )
                    .suggest(format!("round {value} to 2 places")),
                );
                return Kind::Unknown;
            }
            RoundTo::Places(places) => {
                let places_kind = self.infer(places, context);
                if places_kind.known() && places_kind != Kind::Number {
                    self.problems.push(Diagnostic::new(
                        places.span,
                        format!(
                            "The places to round to are a number, but {places} is {}.",
                            places_kind.with_article()
                        ),
                    ));
                    return Kind::Unknown;
                }
                if let Some(count) = places.written_amount() {
                    if count.whole().is_none_or(|count| !(0..=20).contains(&count)) {
                        self.problems.push(Diagnostic::new(
                            places.span,
                            format!(
                                "round keeps a whole number of places, from 0 to 20, \
                                 but this asks for {places}.\n\
                                 round x to 2 places keeps 2 digits after the point."
                            ),
                        ));
                        return Kind::Unknown;
                    }
                }
            }
            _ => {}
        }
        kind
    }

    /// `split 100 USD into 3` or `split fee by list 70, 30`: a list of money.
    fn split_kind(
        &mut self,
        amount: &Expr,
        parts: &Expr,
        by_shares: bool,
        context: &Context,
    ) -> Kind {
        let kind = self.infer(amount, context);
        let parts_kind = self.infer(parts, context);
        let Kind::Money(code) = &kind else {
            if kind.known() {
                self.problems.push(Diagnostic::new(
                    amount.span,
                    format!(
                        "split shares out money in whole cents, but {amount} is {}.\n\
                         To divide a number, use /.",
                        kind.with_article()
                    ),
                ));
            }
            return Kind::Unknown;
        };
        let wanted = if by_shares {
            Kind::List(Box::new(Kind::Number))
        } else {
            Kind::Number
        };
        if parts_kind.known() && parts_kind != wanted {
            let message = if by_shares {
                format!(
                    "split by needs a list of numbers, the shares, but {parts} is {}.\n\
                     split {amount} by list 70, 30 gives 70 parts to the first and 30 to the second.",
                    parts_kind.with_article()
                )
            } else {
                format!(
                    "split into needs how many parts, but {parts} is {}.\n\
                     split {amount} into 3 gives three parts.",
                    parts_kind.with_article()
                )
            };
            self.problems.push(Diagnostic::new(parts.span, message));
            return Kind::Unknown;
        }
        if !by_shares {
            if let Some(count) = parts.written_amount() {
                if count.whole().is_none_or(|count| count < 1) {
                    self.problems.push(Diagnostic::new(
                        parts.span,
                        format!(
                            "split makes a whole number of parts, at least 1, \
                             but this asks for {parts}."
                        ),
                    ));
                    return Kind::Unknown;
                }
            }
        }
        Kind::List(Box::new(Kind::Money(code.clone())))
    }

    /// `20% of fee`: the kind of `fee`.
    fn percent_kind(&mut self, percent: &Expr, of: &Expr, span: Span, context: &Context) -> Kind {
        let percent_kind = self.infer(percent, context);
        let kind = self.infer(of, context);
        if percent_kind.known() && percent_kind != Kind::Number {
            self.problems.push(Diagnostic::new(
                percent.span,
                format!(
                    "A percent is a number, but {percent} is {}.\n\
                     20% of fee is a fifth of the fee.",
                    percent_kind.with_article()
                ),
            ));
            return Kind::Unknown;
        }
        if kind.known() && kind != Kind::Number && !matches!(kind, Kind::Money(_)) {
            self.problems.push(Diagnostic::new(
                span,
                format!(
                    "% of takes part of a number or an amount of money, but {of} is {}.",
                    kind.with_article()
                ),
            ));
            return Kind::Unknown;
        }
        kind
    }

    /// `convert 100 USD to EUR at 0.92`: money in the new currency.
    fn convert_kind(
        &mut self,
        amount: &Expr,
        currency: &str,
        currency_span: Span,
        rate: &Expr,
        context: &Context,
    ) -> Kind {
        let kind = self.infer(amount, context);
        let rate_kind = self.infer(rate, context);
        let Kind::Money(from) = &kind else {
            if kind.known() {
                self.problems.push(Diagnostic::new(
                    amount.span,
                    format!(
                        "convert changes money from one currency to another, \
                         but {amount} is {}.",
                        kind.with_article()
                    ),
                ));
            }
            return Kind::Unknown;
        };
        if from == currency {
            self.problems.push(Diagnostic::new(
                currency_span,
                format!(
                    "{amount} is already in {currency}, so there's nothing to convert.\n\
                     Use the amount as it is."
                ),
            ));
            return Kind::Unknown;
        }
        if rate_kind.known() && rate_kind != Kind::Number {
            self.problems.push(Diagnostic::new(
                rate.span,
                format!(
                    "A rate is a plain number, how many {currency} one {from} is worth, \
                     but {rate} is {}.",
                    rate_kind.with_article()
                ),
            ));
            return Kind::Unknown;
        }
        if rate
            .written_amount()
            .is_some_and(|rate| rate <= Decimal::ZERO)
        {
            self.problems.push(Diagnostic::new(
                rate.span,
                format!(
                    "A rate is more than zero, but this one is {rate}.\n\
                     at 0.92 means one {from} is worth 0.92 {currency}."
                ),
            ));
            return Kind::Unknown;
        }
        Kind::Money(currency.to_string())
    }

    /// `where 1..500` on a parameter: both ends match its kind, and the smaller comes first.
    fn rule(&mut self, param: &Param, rule: &Rule) {
        let name = &param.name;
        let kind = sig_kind(&param.kind, &self.types);
        if !matches!(kind, Kind::Number | Kind::Money(_)) {
            self.problems.push(Diagnostic::new(
                rule.span,
                format!(
                    "where gives the smallest and largest a number or an amount of money can be, \
                     but {name} is {}.\n\
                     For other rules, write require under the fn line.",
                    kind.written_kind()
                ),
            ));
            return;
        }
        let mut fits = true;
        for bound in [&rule.low, &rule.high] {
            let bound_kind = amount_kind(bound);
            if bound_kind == kind {
                continue;
            }
            fits = false;
            let problem = Diagnostic::new(
                bound.span,
                format!(
                    "{name} is {}, so each end of its range is too, but {bound} is {}.\n\
                     A rule can only compare values of the same kind.",
                    kind.written_kind(),
                    bound_kind.written_kind()
                ),
            );
            let amount = bound.written_amount().map(|amount| amount.to_string());
            self.problems.push(match (&kind, amount) {
                (Kind::Money(code), Some(amount)) => problem.suggest(format!("{amount} {code}")),
                (Kind::Number, Some(amount)) => problem.suggest(amount),
                _ => problem,
            });
        }
        let (Some(low), Some(high)) = (rule.low.written_amount(), rule.high.written_amount())
        else {
            return;
        };
        if fits && low > high {
            self.problems.push(
                Diagnostic::new(
                    rule.span,
                    format!(
                        "{name} {rule} starts above where it ends, so no value fits it.\n\
                         Write the smallest value first, then .., then the largest."
                    ),
                )
                .suggest(format!("where {}..{}", rule.high, rule.low)),
            );
        }
    }

    /// A `require` or `ensure` line: a condition about the parameters, and for
    /// `ensure`, about `result`, the value the function gives back.
    fn contract(
        &mut self,
        name: &str,
        contract: &Contract,
        result: Option<&Kind>,
        params: &[Param],
    ) {
        let word = contract.keyword();
        let condition = &contract.condition;
        let mut added_result = false;
        if contract.ensure {
            let Some(result_kind) = result else {
                self.problems.push(Diagnostic::new(
                    contract.span,
                    format!(
                        "ensure promises something about the value {name} gives back, \
                         but {name} gives nothing back.\n\
                         Add -> and a kind to the fn line, or use require for what has to be \
                         true when {name} is called."
                    ),
                ));
                return;
            };
            if let Some(param) = params.iter().find(|param| param.name == "result") {
                self.problems.push(Diagnostic::new(
                    param.name_span,
                    format!(
                        "In an ensure, result is the value {name} gives back, \
                         so a parameter can't be called result.\n\
                         Give the parameter another name."
                    ),
                ));
                return;
            }
            self.defined.push(KnownValue {
                name: "result".to_string(),
                span: contract.span,
                kind: result_kind.clone(),
                mutable: false,
                loop_counter: false,
                created: format!("the value {name} gives back"),
            });
            added_result = true;
        } else if condition.names().iter().any(|used| used == "result")
            && !params.iter().any(|param| param.name == "result")
        {
            self.problems.push(Diagnostic::new(
                contract.span,
                format!(
                    "result is the value {name} gives back, and it doesn't exist yet when \
                     {name} is called.\n\
                     Check result with ensure instead of require."
                ),
            ));
            return;
        }
        let kind = self.value(condition, None, &|fixed| format!("{word} {fixed}"));
        if added_result {
            self.defined.pop();
        }
        if kind.known() && kind != Kind::Bool {
            self.problems.push(Diagnostic::new(
                condition.span,
                format!(
                    "{word} needs a condition that is true or false, but {condition} is {}.\n\
                     Compare it to something, like {}.",
                    kind.with_article(),
                    condition_example(condition, &kind)
                ),
            ));
        }
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
        let result = self.func(&name).and_then(|func| func.result.clone());
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
        let Some(func) = self.func(name) else {
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
                "Quill won't mix currencies. Change one with convert, at the rate you were given: \
                 convert {right} to {} at 0.92.",
                left_code.unwrap_or_default()
            )
        } else if matches!(op, Arithmetic::Add | Arithmetic::Subtract) {
            "Add or subtract two amounts in the same currency, like 100 USD + 50 USD.".to_string()
        } else if op == Arithmetic::Divide {
            "Divide money by a number, as in 100 USD / 2.".to_string()
        } else if op == Arithmetic::Remainder {
            "A remainder needs two numbers. Money can be added, subtracted, multiplied by a number, or divided by a number.".to_string()
        } else {
            "Multiply money by a number, as in 100 USD * 0.90.".to_string()
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

    /// The function called `name`: one the program writes, one of Quill's own, or the
    /// reading that `as KIND` makes.
    fn func(&self, name: &str) -> Option<KnownFn> {
        if let Some(found) = self.functions.iter().find(|func| func.name == name) {
            return Some(found.clone());
        }
        if name == parser::ASK || parser::kind_asked(name).is_some() {
            let (result, fails) = match parser::kind_asked(name) {
                Some(kind) => (sig_kind(&kind, &self.types), vec!["ModelFailed", "Invalid"]),
                None => (Kind::Text, vec!["ModelFailed"]),
            };
            return Some(KnownFn {
                name: name.to_string(),
                span: Span::default(),
                params: vec![("the question".to_string(), Kind::Text)],
                rules: vec![None],
                result: Some(result),
                fails: fails.into_iter().map(str::to_string).collect(),
                written: "ask \"a question\", or ask \"a question\" as Review".to_string(),
            });
        }
        let kind = parser::kind_read_as(name)?;
        Some(KnownFn {
            name: name.to_string(),
            span: Span::default(),
            params: vec![("the JSON".to_string(), Kind::Text)],
            rules: vec![None],
            result: Some(sig_kind(&kind, &self.types)),
            fails: vec!["Invalid".to_string()],
            written: format!("as reads JSON text, like body {name}"),
        })
    }

    fn require_handled(&mut self, func: &KnownFn, span: Span) {
        if func.fails.is_empty() || self.handling {
            return;
        }
        if func.name.starts_with(parser::ASK) {
            self.problems.push(Diagnostic::new(
                span,
                format!(
                    "Asking a model can fail with {}: the model may be out of reach, \
                     or say no.\n\
                     Handle each with on under this line, or write try in front, \
                     inside a function that fails the same way.",
                    func.fails.join(" or ")
                ),
            ));
            return;
        }
        if let Some(kind) = func.name.strip_prefix(parser::AS) {
            self.problems.push(Diagnostic::new(
                span,
                format!(
                    "Reading text as {kind} fails with Invalid when the text isn't JSON \
                     of that shape.\n\
                     Handle it with on Invalid under this line, or write try in front, \
                     inside a function that fails with Invalid."
                ),
            ));
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
        let Some(func) = self.func(name) else {
            return;
        };
        let gives = func.result.clone();
        self.cover_failures(&func, *name_span, handlers, gives.as_ref());
    }

    /// `on Declined -> "declined"`: the value takes the place of the call's answer.
    fn recovery_value(
        &mut self,
        func: &KnownFn,
        handler: &Handler,
        value: &Expr,
        gives: Option<&Kind>,
    ) {
        let failure = &handler.name;
        let Some(expected) = gives else {
            self.problems.push(
                Diagnostic::new(
                    value.span,
                    format!(
                        "{} gives nothing back, so on {failure} can't give a value in its place.\n\
                         Write what to do instead, like say.",
                        func.name
                    ),
                )
                .suggest(format!("on {failure} -> say \"{failure}\"")),
            );
            return;
        };
        let kind = self.value(value, None, &|fixed| format!("on {failure} -> {fixed}"));
        if matches!(value.kind, ExprKind::Missing) && matches!(expected, Kind::Maybe(_)) {
            return;
        }
        if kind.known() && &kind != expected {
            self.problems.push(Diagnostic::new(
                value.span,
                format!(
                    "{} gives back {}, but {value} is {}.\n\
                     The value after -> takes the place of the call's answer, \
                     so it has to be {} too.",
                    func.name,
                    expected.with_article(),
                    kind.with_article(),
                    expected.written_kind()
                ),
            ));
        }
    }

    /// A `when` that gives a value: every condition is true/false, and every case
    /// gives the same kind of value.
    fn when_kind(&mut self, arms: &[ValueArm], context: &Context) -> Kind {
        let mut first: Option<(Kind, &Expr)> = None;
        let mut saw_missing = false;
        for arm in arms {
            if let Some(condition) = &arm.condition {
                let kind = self.infer(condition, context);
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
            let kind = self.infer(&arm.value, context);
            if matches!(arm.value.kind, ExprKind::Missing) {
                saw_missing = true;
                continue;
            }
            if !kind.known() {
                continue;
            }
            match &first {
                None => first = Some((kind, &arm.value)),
                Some((expected, earlier)) if &kind != expected => {
                    self.problems.push(Diagnostic::new(
                        arm.value.span,
                        format!(
                            "Every case of this when has to give the same kind of value.\n\
                             {earlier} on line {} is {}, but {} is {}.",
                            earlier.span.line,
                            expected.with_article(),
                            arm.value,
                            kind.with_article()
                        ),
                    ));
                }
                Some(_) => {}
            }
        }
        match first {
            Some((Kind::Maybe(inner), _)) => Kind::Maybe(inner),
            Some((_, value)) if saw_missing => {
                let span = arms
                    .iter()
                    .find(|arm| matches!(arm.value.kind, ExprKind::Missing))
                    .map_or(value.span, |arm| arm.value.span);
                self.problems.push(Diagnostic::new(
                    span,
                    format!(
                        "missing only fits a when whose other cases are maybe values.\n\
                         Write found before the other values, like found {value}."
                    ),
                ));
                Kind::Unknown
            }
            Some((kind, _)) => kind,
            None => Kind::Unknown,
        }
    }

    fn check_call_handlers(&mut self, name: &str, call_span: Span, handlers: &[Handler]) {
        let Some(func) = self.func(name) else {
            return;
        };
        let span = handlers
            .first()
            .map(|handler| handler.span)
            .unwrap_or(call_span);
        self.cover_failures(&func, span, handlers, None);
    }

    /// `gives` is the kind the call gives back, when its answer is kept with `=` or `set`.
    fn cover_failures(
        &mut self,
        func: &KnownFn,
        span: Span,
        handlers: &[Handler],
        gives: Option<&Kind>,
    ) {
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
            let body = match &handler.body {
                Recovery::Stop(body) => body,
                Recovery::Value(value) => {
                    self.recovery_value(func, handler, value, gives);
                    continue;
                }
            };
            if self.in_function.is_some() {
                if let Some(result) = self
                    .functions
                    .iter()
                    .find(|outer| self.in_function.as_deref() == Some(outer.name.as_str()))
                    .and_then(|outer| outer.result.clone())
                {
                    if !always_returns(body) {
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
            let _ = self.statement(body);
        }
        for name in &func.fails {
            if !seen.contains(name) {
                self.problems.push(
                    Diagnostic::new(
                        span,
                        format!(
                            "{name} isn't handled.\n\
                             {} can fail with {name}, so the call needs on {name}.",
                            match func.name.strip_prefix(parser::AS) {
                                Some(kind) => format!("Reading text as {kind}"),
                                None => func.name.clone(),
                            }
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
            .func(&function)
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
        let callee = self.func(name);
        let caller = self.func(&function);
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
            let in_example = self.program.statements.iter().any(|statement| {
                matches!(statement, Statement::Check { example: true, span: at, .. } if at.line == span.line)
            });
            let fix = if in_example {
                format!(
                    "An example runs after the fn right below it. Move this example \
                     directly above fn {name}, with no other lines between them."
                )
            } else {
                "Write the function above the line that calls it.".to_string()
            };
            self.problems.push(Diagnostic::new(
                span,
                format!(
                    "{name} is used here, but the function isn't written until line {}.\n{fix}",
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
                if kind.known() && expected.known() && kind != *expected {
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
                    continue;
                }
                let rule = func.rules.get(index).and_then(Option::as_ref);
                if let (Some(rule), Some(given)) = (rule, arg.written_amount()) {
                    let low = rule.low.written_amount();
                    let high = rule.high.written_amount();
                    let outside =
                        low.is_some_and(|low| given < low) || high.is_some_and(|high| given > high);
                    if outside {
                        self.problems.push(Diagnostic::new(
                            arg.span,
                            format!(
                                "{name} takes {param} {}, but this call gives {arg}.\n\
                                 Line {} says {param} {rule}. \
                                 Give {name} a value in that range.",
                                rule.range(),
                                func.span.line
                            ),
                        ));
                    }
                }
            }
        }
        self.permitted(name, span, args);
    }

    /// `ask` needs `bring ai`, and a permit for a model's key (and, for Claude, its host).
    fn can_ask(&mut self, span: Span) {
        if !self.modules.iter().any(|module| module == "ai") {
            self.problems.push(
                Diagnostic::new(
                    span,
                    "ask is part of Quill's ai module, which this program doesn't bring.\n\
                     Add bring ai at the top, so a reader sees that the program asks a model.",
                )
                .suggest("bring ai"),
            );
            return;
        }
        let claude = self.permits.allows(Permission::Env, crate::ai::CLAUDE_KEY);
        let openai = self.permits.allows(Permission::Env, crate::ai::OPENAI_KEY);
        if !claude && !openai {
            self.problems.push(
                Diagnostic::new(
                    span,
                    format!(
                        "ask needs a key for a model, and the program permits none.\n\
                         Add permit env \"{}\" to ask Claude, or permit env \"{}\" for an \
                         OpenAI-compatible service. The key itself stays in the setting, out of the code.",
                        crate::ai::CLAUDE_KEY,
                        crate::ai::OPENAI_KEY
                    ),
                )
                .suggest(format!("permit env \"{}\"", crate::ai::CLAUDE_KEY)),
            );
            return;
        }
        let any_host = self
            .permits
            .list
            .iter()
            .any(|(access, _)| *access == Permission::Net);
        if claude && !any_host {
            self.problems.push(
                Diagnostic::new(
                    span,
                    "ask sends the question to Claude at api.anthropic.com, \
                     and the program has no permit to reach it.\n\
                     Add a permit for the host at the top of the program.",
                )
                .suggest("permit net \"api.anthropic.com\""),
            );
        }
    }

    /// A native call that names a file, a host or a setting in the call itself needs
    /// a permit for it, found before the program runs.
    fn permitted(&mut self, name: &str, span: Span, args: &[Expr]) {
        let Some((_, native)) = native::find(name) else {
            return;
        };
        if let Some(module) = native.module {
            if !self.modules.iter().any(|brought| brought == module) {
                self.problems.push(
                    Diagnostic::new(
                        span,
                        format!(
                            "{name} is part of Quill's {module} module, \
                             which this program doesn't bring.\n\
                             Add bring {module} at the top, so a reader sees what the program uses."
                        ),
                    )
                    .suggest(format!("bring {module}")),
                );
                return;
            }
        }
        if name == "respond" {
            if let Some(status) = args.first() {
                let fits = status
                    .written_amount()
                    .map(|code| code.whole().is_some_and(|code| (100..=599).contains(&code)));
                if fits == Some(false) {
                    self.problems.push(Diagnostic::new(
                        status.span,
                        format!(
                            "respond needs a status from 100 through 599, but this one is {status}.\n\
                             200 means it worked, 201 that something was made, 400 that the \
                             request was wrong, 404 that it wasn't found."
                        ),
                    ));
                }
            }
            return;
        }
        let Some((access, at)) = native.guard else {
            return;
        };
        let Some(arg) = args.get(at) else {
            return;
        };
        let ExprKind::Text(target) = &arg.kind else {
            return;
        };
        if access == Permission::Net
            && !(target.starts_with("https://") || target.starts_with("http://"))
        {
            self.problems.push(Diagnostic::new(
                arg.span,
                format!(
                    "{target} isn't a web address.\n\
                     A web address starts with https:// or http://, like https://api.example.com/rates."
                ),
            ));
            return;
        }
        if self.permits.allows(access, target) {
            return;
        }
        self.problems.push(
            Diagnostic::new(
                arg.span,
                format!(
                    "{name} can't {} {target}, because the program has no permit for it.\n\
                     A program touches only what its permits allow. If it should, add this \
                     at the top of the program.",
                    native::verb(access)
                ),
            )
            .suggest(native::permit_for(access, target)),
        );
    }

    /// `permit net "api.stripe.com"` names a host; `permit env "STRIPE_KEY"` names a setting.
    fn permit_statement(&mut self, access: Permission, target: &str, span: Span) {
        match access {
            Permission::Net if target.contains("://") || target.contains('/') => {
                let host = native::host_of(target);
                self.problems.push(
                    Diagnostic::new(
                        span,
                        "permit net names a host, like api.stripe.com, \
                         without https:// or a path.\n\
                         Every path on that host is allowed.",
                    )
                    .suggest(format!("permit net \"{host}\"")),
                );
            }
            Permission::Env
                if !target
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_') =>
            {
                self.problems.push(Diagnostic::new(
                    span,
                    format!(
                        "permit env names one setting, like STRIPE_KEY, \
                         but {target:?} has other characters.\n\
                         A setting's name has letters, digits and _."
                    ),
                ));
            }
            _ => {}
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
            ExprKind::List { items, empty } => self.list_kind(items, empty.as_ref(), expr.span),
            ExprKind::Typed {
                name,
                name_span,
                record,
            } => self.typed_kind(name, *name_span, record, context),
            ExprKind::Round { value, to, .. } => self.round_kind(value, to, context),
            ExprKind::Split {
                amount,
                parts,
                by_shares,
            } => self.split_kind(amount, parts, *by_shares, context),
            ExprKind::Percent {
                percent,
                of,
                percent_span,
            } => self.percent_kind(percent, of, *percent_span, context),
            ExprKind::Convert {
                amount,
                currency,
                currency_span,
                rate,
            } => self.convert_kind(amount, currency, *currency_span, rate, context),
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
                if let Some(kind) = parser::kind_read_as(name) {
                    self.known_types(&kind, *name_span);
                }
                if name == parser::ASK || parser::kind_asked(name).is_some() {
                    if let Some(kind) = parser::kind_asked(name) {
                        self.known_types(&kind, *name_span);
                    }
                    self.can_ask(*name_span);
                }
                let Some(func) = self.func(name) else {
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
            ExprKind::Sum {
                field,
                field_span,
                list,
                ..
            } => self.sum_kind(expr.span, field, *field_span, list, context),
            ExprKind::Get { fields, list, .. } => self.get_kind(fields, list, context),
            ExprKind::When(arms) => self.when_kind(arms, context),
            ExprKind::Where {
                list,
                condition,
                where_span,
            } => self.where_kind(expr.span, list, condition, *where_span, context),
            ExprKind::GetWith {
                field,
                field_span,
                nested,
                nested_span,
                source,
            } => self.gather_kind(field, *field_span, nested, *nested_span, source, context),
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

    fn list_kind(&mut self, items: &[Expr], empty: Option<&SigKind>, span: Span) -> Kind {
        let Some(first) = items.first() else {
            return match empty {
                Some(kind) => {
                    self.known_types(kind, span);
                    Kind::List(Box::new(sig_kind(kind, &self.types)))
                }
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

    /// The fields of each record in a list. `word` is `get` or `sum`, for the error.
    fn records_of(&mut self, list: &Expr, word: &str, context: &Context) -> Option<Vec<FieldKind>> {
        let kind = self.infer(list, context);
        match kind {
            Kind::List(item) => match *item {
                Kind::Record(fields) => Some(fields),
                Kind::Unknown => None,
                other => {
                    self.problems.push(Diagnostic::new(
                        list.span,
                        format!(
                            "{list} holds {}, and {word} needs a record or a list of records.\n\
                             {word} fees of school reads that one amount. \
                             {word} fee of roster reads the fee of each record.",
                            other.name()
                        ),
                    ));
                    None
                }
            },
            Kind::Record(fields) => Some(fields),
            Kind::Unknown => None,
            other => {
                self.problems.push(Diagnostic::new(
                    list.span,
                    format!(
                        "{list} is {}, and {word} needs a record or a list of records.\n\
                         {word} fees of school reads that one amount. \
                         {word} fee of roster reads the fee of each record.",
                        other.name()
                    ),
                ));
                None
            }
        }
    }

    /// `get fee of roster` is a list of that field. `get name, fee of roster` is a list of
    /// records that keep only those fields.
    fn get_kind(&mut self, asked: &[crate::parser::Asked], list: &Expr, context: &Context) -> Kind {
        let source = self.infer(list, context);
        let (fields, one_record) = match source {
            Kind::List(item) => match *item {
                Kind::Record(fields) => (fields, false),
                Kind::Unknown => return Kind::Unknown,
                other => {
                    self.problems.push(Diagnostic::new(
                        list.span,
                        format!(
                            "{list} holds {}, and get needs a record or a list of records.\n\
                             get name, fee of roster reads those fields.",
                            other.name()
                        ),
                    ));
                    return Kind::Unknown;
                }
            },
            Kind::Record(fields) => (fields, true),
            Kind::Unknown => return Kind::Unknown,
            other => {
                self.problems.push(Diagnostic::new(
                    list.span,
                    format!(
                        "{list} is {}, and get needs a record or a list of records.\n\
                         get name, fee of school reads those fields of one record. \
                         get name, fee of roster reads them from each record in a list.",
                        other.name()
                    ),
                ));
                return Kind::Unknown;
            }
        };
        if asked.len() == 1 {
            let field = &asked[0];
            let kind = self.field_kind(&fields, &field.name, field.span);
            if one_record {
                return kind;
            }
            return match kind {
                Kind::Maybe(inner) => Kind::List(inner),
                Kind::Unknown => Kind::Unknown,
                kind => Kind::List(Box::new(kind)),
            };
        }
        let mut kinds = Vec::new();
        let mut seen = Vec::new();
        for field in asked {
            if seen.iter().any(|name| name == &field.name) {
                self.problems.push(Diagnostic::new(
                    field.span,
                    format!(
                        "{name} is already in this get.\n\
                         Name each field once: get name, fee of roster.",
                        name = field.name
                    ),
                ));
                return Kind::Unknown;
            }
            seen.push(field.name.clone());
            kinds.push(FieldKind {
                name: field.name.clone(),
                kind: self.field_kind(&fields, &field.name, field.span),
            });
        }
        if kinds.iter().any(|field| !field.kind.known()) {
            return Kind::Unknown;
        }
        let record = Kind::Record(kinds);
        if one_record {
            record
        } else {
            Kind::List(Box::new(record))
        }
    }

    /// `students where score >= 50`: the same list, keeping the records whose condition is
    /// true. Each field of the records is a name inside the condition.
    fn where_kind(
        &mut self,
        span: Span,
        list: &Expr,
        condition: &Expr,
        where_span: Span,
        context: &Context,
    ) -> Kind {
        let list_kind = self.infer(list, context);
        let fields = match &list_kind {
            Kind::List(item) => match item.as_ref() {
                Kind::Record(fields) => fields.clone(),
                Kind::Unknown => return Kind::Unknown,
                other => {
                    self.problems.push(Diagnostic::new(
                        where_span,
                        format!(
                            "where keeps the records whose fields match, but {list} holds {}.\n\
                             Use where on a list of records, like students where score >= 50. \
                             To pick from other lists, use for and when.",
                            other.name()
                        ),
                    ));
                    return Kind::Unknown;
                }
            },
            Kind::Unknown => return Kind::Unknown,
            other => {
                self.problems.push(Diagnostic::new(
                    where_span,
                    format!(
                        "where keeps the records whose fields match, but {list} is {}.\n\
                         Use where on a list of records, like students where score >= 50.",
                        other.name()
                    ),
                ));
                return Kind::Unknown;
            }
        };
        let used = condition.names();
        let field_names = || fields.iter().map(|field| field.name.as_str());
        if let Some(unknown) = used
            .iter()
            .find(|name| !field_names().any(|field| field == *name) && self.lookup(name).is_none())
        {
            let problem = Diagnostic::new(
                where_span,
                format!(
                    "The records in {list} have no field called {unknown}, \
                     and there's no value called {unknown}.\n\
                     Their fields are {}.",
                    field_names().collect::<Vec<_>>().join(", ")
                ),
            );
            self.problems
                .push(match closest_match(unknown, field_names()) {
                    Some(field) => problem.suggest(field.to_string()),
                    None => problem,
                });
            return Kind::Unknown;
        }
        let mut hidden = Vec::new();
        let mut read = Vec::new();
        for field in &fields {
            hidden.push(self.hide(&field.name));
            self.defined.push(KnownValue {
                name: field.name.clone(),
                span: where_span,
                kind: field.kind.clone(),
                mutable: false,
                loop_counter: false,
                created: format!("a field of each record in {list}"),
            });
            if used.contains(&field.name) {
                read.push(field.name.clone());
            }
        }
        let kind = self.infer(condition, context);
        for _ in &fields {
            self.defined.pop();
        }
        for saved in hidden.into_iter().rev() {
            self.restore(saved);
        }
        self.where_fields.push((span, read));
        if kind.known() && kind != Kind::Bool {
            self.problems.push(Diagnostic::new(
                condition.span,
                format!(
                    "where needs a condition that is true or false, but {condition} is {}.\n\
                     Compare a field to something, like {}.",
                    kind.with_article(),
                    condition_example(condition, &kind)
                ),
            ));
            return Kind::Unknown;
        }
        list_kind
    }

    /// `sum fee of roster`: a number, or money in one currency. One record gives that one amount.
    /// A maybe amount counts when it is found.
    fn sum_kind(
        &mut self,
        span: Span,
        field: &str,
        field_span: Span,
        list: &Expr,
        context: &Context,
    ) -> Kind {
        let Some(fields) = self.records_of(list, "sum", context) else {
            return Kind::Unknown;
        };
        let amount = match self.field_kind(&fields, field, field_span) {
            Kind::Maybe(inner) => *inner,
            other => other,
        };
        match amount {
            Kind::Number => Kind::Number,
            Kind::Money(code) => {
                self.sum_currencies.push((span, code.clone()));
                Kind::Money(code)
            }
            Kind::Unknown => Kind::Unknown,
            other => {
                self.problems.push(Diagnostic::new(
                    field_span,
                    format!(
                        "sum adds amounts, but {field} is {}.\n\
                         Name a number or an amount of money: sum fee of roster.",
                        other.name()
                    ),
                ));
                Kind::Unknown
            }
        }
    }

    /// `get bookings with acts in roster`. The result is the list of bookings.
    fn gather_kind(
        &mut self,
        field: &str,
        field_span: Span,
        nested: &str,
        nested_span: Span,
        source: &Expr,
        context: &Context,
    ) -> Kind {
        let source_kind = self.infer(source, context);
        let outer = match &source_kind {
            Kind::Record(fields) => self.field_kind(fields, field, field_span),
            Kind::List(item) => match item.as_ref() {
                Kind::Record(fields) => self.field_kind(fields, field, field_span),
                Kind::Unknown => return Kind::Unknown,
                other => {
                    self.problems.push(Diagnostic::new(
                        source.span,
                        format!(
                            "{source} holds {}, and get needs a record or a list of records.\n\
                             get bookings with acts in roster brings the bookings and their acts.",
                            other.name()
                        ),
                    ));
                    return Kind::Unknown;
                }
            },
            Kind::Unknown => return Kind::Unknown,
            other => {
                self.problems.push(Diagnostic::new(
                    source.span,
                    format!(
                        "{source} is {}, and get needs a record or a list of records.\n\
                         get bookings with acts in roster brings the bookings and their acts.",
                        other.name()
                    ),
                ));
                return Kind::Unknown;
            }
        };
        let Kind::List(item) = outer else {
            if outer.known() {
                self.problems.push(Diagnostic::new(
                    field_span,
                    format!(
                        "{field} is {}, and get with needs a list of records.\n\
                         get bookings with acts in roster brings each booking and its acts.",
                        outer.name()
                    ),
                ));
            }
            return Kind::Unknown;
        };
        let Kind::Record(fields) = item.as_ref() else {
            if item.known() {
                self.problems.push(Diagnostic::new(
                    field_span,
                    format!(
                        "{field} holds {}, and each one has to be a record.\n\
                         get bookings with acts in roster brings each booking and its acts.",
                        item.name()
                    ),
                ));
            }
            return Kind::Unknown;
        };
        let nested_kind = self.field_kind(fields, nested, nested_span);
        if !nested_kind.known() {
            return Kind::Unknown;
        }
        Kind::List(item)
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
    types: &[KnownType],
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
            .map(|param| (param.name.clone(), sig_kind(&param.kind, types)))
            .collect(),
        rules: params.iter().map(|param| param.rule.clone()).collect(),
        result: result.as_ref().map(|kind| sig_kind(kind, types)),
        fails: fails.iter().map(|(name, _)| name.clone()).collect(),
        written: format!("{name}({})", written_params.join(", ")),
    }
}

fn function_signatures(program: &Program, types: &[KnownType]) -> Vec<KnownFn> {
    let mut natives = native_signatures();
    let written = program.statements.iter().filter_map(|statement| {
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
        Some(known_function(
            name, *name_span, params, result, fails, types,
        ))
    });
    natives.extend(written);
    natives
}

/// The kind of a written number or amount of money, like `500` or `0 USD`.
fn amount_kind(expr: &Expr) -> Kind {
    match &expr.kind {
        ExprKind::Number(_) => Kind::Number,
        ExprKind::Money { currency, .. } => Kind::Money(currency.clone()),
        ExprKind::Negate(inner) | ExprKind::Group(inner) => amount_kind(inner),
        _ => Kind::Unknown,
    }
}

/// The kind a function or type writes. A type's name becomes a record with its fields.
pub fn sig_kind(kind: &SigKind, types: &[KnownType]) -> Kind {
    sig_kind_inside(kind, types, &mut Vec::new())
}

/// `inside` holds the types being opened, so a type that holds itself stops.
fn sig_kind_inside(kind: &SigKind, types: &[KnownType], inside: &mut Vec<String>) -> Kind {
    match kind {
        SigKind::Text => Kind::Text,
        SigKind::Number => Kind::Number,
        SigKind::Bool => Kind::Bool,
        SigKind::List(item) => Kind::List(Box::new(sig_kind_inside(item, types, inside))),
        SigKind::Money(code) => Kind::Money(code.clone()),
        SigKind::Maybe(inner) => Kind::Maybe(Box::new(sig_kind_inside(inner, types, inside))),
        SigKind::Named(name) => {
            let Some(known) = types.iter().find(|known| &known.name == name) else {
                return Kind::Unknown;
            };
            if inside.contains(name) {
                return Kind::Unknown;
            }
            inside.push(name.clone());
            let fields = known
                .fields
                .iter()
                .map(|field| FieldKind {
                    name: field.name.clone(),
                    kind: sig_kind_inside(&field.kind, types, inside),
                })
                .collect();
            inside.pop();
            Kind::Record(fields)
        }
    }
}

/// Whether `kind` holds the type `name`, directly, in a list or maybe, or through other
/// types' fields.
fn holds(kind: &SigKind, name: &str, types: &[KnownType], seen: &mut Vec<String>) -> bool {
    match kind {
        SigKind::Named(inner) => {
            if inner == name {
                return true;
            }
            if seen.contains(inner) {
                return false;
            }
            seen.push(inner.clone());
            types
                .iter()
                .filter(|known| &known.name == inner)
                .flat_map(|known| known.fields.iter())
                .any(|field| holds(&field.kind, name, types, seen))
        }
        SigKind::Maybe(inner) | SigKind::List(inner) => holds(inner, name, types, seen),
        _ => false,
    }
}

/// The type names a kind refers to, such as `Act` in `list of Act`.
fn named_in(kind: &SigKind) -> Option<&str> {
    match kind {
        SigKind::Named(name) => Some(name),
        SigKind::List(inner) | SigKind::Maybe(inner) => named_in(inner),
        _ => None,
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
        Statement::Type { span, .. }
        | Statement::Bring { span, .. }
        | Statement::Permit { span, .. } => *span,
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
        ExprKind::Sum {
            field,
            field_span,
            of_span,
            list,
        } => ExprKind::Sum {
            field: field.clone(),
            field_span: *field_span,
            of_span: *of_span,
            list: swap(list),
        },
        ExprKind::Get {
            fields,
            of_span,
            list,
        } => ExprKind::Get {
            fields: fields.clone(),
            of_span: *of_span,
            list: swap(list),
        },
        ExprKind::GetWith {
            field,
            field_span,
            nested,
            nested_span,
            source,
        } => ExprKind::GetWith {
            field: field.clone(),
            field_span: *field_span,
            nested: nested.clone(),
            nested_span: *nested_span,
            source: swap(source),
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
        ExprKind::Where {
            list,
            condition,
            where_span,
        } => ExprKind::Where {
            list: swap(list),
            condition: swap(condition),
            where_span: *where_span,
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

    #[test]
    fn sum_and_get_need_a_real_field() {
        let roster = "\
roster = list record name = \"Nchang Sisters\", fee = 80000 USD
";
        assert_eq!(problems(&format!("{roster}say sum fee of roster")), vec![]);
        assert_eq!(problems(&format!("{roster}say get fee of roster")), vec![]);
        let found = problem(&format!("{roster}say sum name of roster"));
        assert!(found.message.contains("sum adds amounts"));
        assert_eq!(
            problems(&format!("{roster}say get name, fee of roster")),
            vec![]
        );
        let found = problem(&format!("{roster}say get name, name of roster"));
        assert!(found.message.contains("already in this get"));
        let found = problem("say sum fee of 1");
        assert!(found.message.contains("record or a list of records"));
        assert_eq!(
            problems("school = record fees = 10000\nsay sum fees of school"),
            vec![]
        );
        let found = problem(
            "\
up = record place = \"Up Station\", acts = list \"Bamenda Brass\"
roster = record bookings = list up
say get bookings with guest in roster
",
        );
        assert!(found.message.contains("no field called guest"));
    }

    #[test]
    fn contracts_read_the_parameters_and_result() {
        assert_eq!(
            problems(
                "fn withdraw(balance: USD, amount: USD) -> USD\n  require amount > 0 USD and amount <= balance\n  ensure result >= 0 USD\n  return balance - amount\n"
            ),
            vec![]
        );
    }

    #[test]
    fn a_contract_must_be_true_or_false() {
        let found = problem("fn f(n: number) -> number\n  require n\n  return n\n");
        assert!(found
            .message
            .starts_with("require needs a condition that is true or false"));
    }

    #[test]
    fn ensure_needs_a_value_given_back() {
        let found = problem("fn greet(name: text)\n  ensure name != \"\"\n  say name\n");
        assert!(found.message.contains("but greet gives nothing back"));
    }

    #[test]
    fn require_cant_read_result() {
        let found = problem("fn f(n: number) -> number\n  require result > 0\n  return n\n");
        assert!(found
            .message
            .contains("Check result with ensure instead of require."));
    }

    #[test]
    fn a_parameter_called_result_clashes_with_ensure() {
        let found = problem("fn f(result: number) -> number\n  ensure result > 0\n  return 1\n");
        assert!(found.message.contains("a parameter can't be called result"));
    }

    #[test]
    fn result_only_exists_inside_ensure() {
        let found = problem("fn f(n: number) -> number\n  ensure result > 0\n  return result\n");
        assert!(
            found.message.contains("no value called result"),
            "{}",
            found.message
        );
    }

    #[test]
    fn a_contract_checks_its_kinds() {
        let found = problem("fn f(amount: USD) -> USD\n  require amount > 0\n  return amount\n");
        assert_eq!(found.line, 2);
    }

    #[test]
    fn a_range_fits_its_parameter() {
        assert!(problems(
            "fn f(a: USD where 0 USD..10 USD, n: number where -3..3) -> USD\n  return a\n"
        )
        .is_empty());
        let text = problem("fn f(name: text where 1..5) -> text\n  return name\n");
        assert!(text.message.contains("but name is text"));
        let kinds = problem("fn f(a: USD where 0..10 USD) -> USD\n  return a\n");
        assert!(kinds.message.contains("but 0 is a number"));
        assert_eq!(kinds.suggestion.as_deref(), Some("0 USD"));
        let backwards = problem("fn f(n: number where 500..1) -> number\n  return n\n");
        assert!(backwards.message.contains("starts above where it ends"));
        assert_eq!(backwards.suggestion.as_deref(), Some("where 1..500"));
    }

    #[test]
    fn a_written_value_outside_the_range_is_caught_before_running() {
        let source = "fn book(guests: number where 1..500) -> text\n  return \"ok\"\nsay book(500)\nsay book(600)\n";
        let found = problem(source);
        assert_eq!(found.line, 4);
        assert_eq!(
            found.message,
            "book takes guests from 1 through 500, but this call gives 600.\n\
             Line 1 says guests where 1..500. Give book a value in that range."
        );
        let below = problem("fn t(c: number where -40..60) -> number\n  return c\nsay t(-41)\n");
        assert!(below.message.contains("but this call gives -41"));
    }

    #[test]
    fn where_reads_the_fields_of_records() {
        let roster = "a = record name = \"Amina\", score = 80, fee = 500 USD, paid = true\no = record name = \"Omar\", score = 64, fee = 300 USD, paid = false\nj = record name = \"John\", score = 45, fee = 200 USD, paid = false\nstudents = list a, o, j\n";
        assert!(problems(&format!(
            "{roster}say students where score >= 50 and paid\n"
        ))
        .is_empty());
        let numbers = problem("n = list 1, 2\nsay n where n > 1\n");
        assert!(numbers.message.contains("but n holds Number"));
        let typo = problem(&format!("{roster}say students where scor > 1\n"));
        assert!(typo.message.contains("have no field called scor"));
        assert!(typo
            .message
            .contains("Their fields are name, score, fee, paid."));
        assert_eq!(typo.suggestion.as_deref(), Some("score"));
        let not_a_condition = problem(&format!("{roster}say students where score\n"));
        assert!(not_a_condition
            .message
            .starts_with("where needs a condition that is true or false"));
        let outer = problems(&format!(
            "{roster}mark = 60\nsay students where score >= mark\n"
        ));
        assert!(outer.is_empty());
    }

    #[test]
    fn a_typed_record_has_every_field_of_its_kind() {
        let types =
            "type Booking\n  venue: text\n  guests: number where 1..500\n  note: maybe text\n";
        assert!(problems(&format!("{types}fn f(b: Booking) -> text\n  return b.venue\nb = Booking venue = \"Hall\", guests = 9, note = missing\nsay f(b)\n")).is_empty());
        let extra = problem(&format!(
            "{types}b = Booking venue = \"x\", guests = 1, note = missing, vneue = 1\n"
        ));
        assert!(extra
            .message
            .starts_with("A Booking has no field called vneue."));
        let short = problem(&format!("{types}b = Booking venue = \"x\"\n"));
        assert!(short
            .message
            .starts_with("This Booking is missing the fields guests, note."));
        let wrong = problem(&format!(
            "{types}b = Booking venue = 5, guests = 1, note = missing\n"
        ));
        assert!(wrong
            .message
            .starts_with("A Booking's venue is text, but 5 is a Number."));
        let range = problem(&format!(
            "{types}b = Booking venue = \"x\", guests = 900, note = missing\n"
        ));
        assert!(range
            .message
            .starts_with("A Booking takes guests from 1 through 500, but this one gives 900."));
    }

    #[test]
    fn type_names_are_checked() {
        let unknown =
            problem("type Booking\n  venue: text\nfn f(b: Bookng) -> number\n  return 1\n");
        assert_eq!(unknown.suggestion.as_deref(), Some("Booking"));
        let unknown_record = problem("b = Bookng venue = \"x\"\n");
        assert!(unknown_record
            .message
            .starts_with("There's no type called Bookng."));
        let itself = problem("type Node\n  next: maybe Node\n");
        assert!(itself
            .message
            .starts_with("A Node can't hold a Node inside it"));
        let later =
            problems("fn f(b: Booking) -> text\n  return b.venue\ntype Booking\n  venue: text\n");
        assert!(later.is_empty());
    }

    #[test]
    fn money_words_check_their_kinds() {
        assert!(problems("say round 1.5 USD to cents\nsay split 9 USD into 2\nsay 5% of 3\nsay convert 1 USD to EUR at 0.9\n").is_empty());
        let cents = problem("say round 2.5 to cents\n");
        assert_eq!(cents.suggestion.as_deref(), Some("round 2.5 to 2 places"));
        assert!(problem("say split 50 into 2\n")
            .message
            .starts_with("split shares out money"));
        assert!(problem("say split 10 USD into 0\n")
            .message
            .contains("at least 1"));
        assert!(problem("say convert 1 USD to USD at 1\n")
            .message
            .contains("already in USD"));
        assert!(problem("say convert 1 USD to EUR at -1\n")
            .message
            .starts_with("A rate is more than zero"));
        assert!(problem("say 20% of \"x\"\n")
            .message
            .starts_with("% of takes part"));
        assert!(problem("say round 2 to 2.5 places\n")
            .message
            .contains("whole number of places"));
    }

    #[test]
    fn a_named_file_needs_a_permit_before_running() {
        let ok = problems("permit read \"config\"\nx = read_file(\"config/rates.txt\")\n  on FileMissing -> \"none\"\nsay x\n");
        assert!(ok.is_empty(), "{ok:#?}");
        let missing =
            problem("x = read_file(\"secrets/key.txt\")\n  on FileMissing -> \"none\"\nsay x\n");
        assert!(missing
            .message
            .starts_with("read_file can't read secrets/key.txt"));
        assert_eq!(
            missing.suggestion.as_deref(),
            Some("permit read \"secrets\"")
        );
        let unhandled = problem("permit read \"config\"\nsay read_file(\"config/x.txt\")\n");
        assert!(unhandled
            .message
            .starts_with("read_file can fail with FileMissing."));
        let own = problem("fn env(n: text) -> text\n  return n\n");
        assert!(own
            .message
            .starts_with("env is one of Quill's own functions"));
        let host = problem("permit net \"https://api.stripe.com/v1\"\n");
        assert_eq!(
            host.suggestion.as_deref(),
            Some("permit net \"api.stripe.com\"")
        );
        let setting = problem("permit env \"MY KEY\"\n");
        assert!(setting.message.starts_with("permit env names one setting"));
    }

    #[test]
    fn reading_json_must_handle_invalid() {
        let types = "type B\n  a: text\n";
        let ok = problems(&format!(
            "{types}x = \"[]\" as B\n  on Invalid -> B a = \"none\"\nsay x.a\n"
        ));
        assert!(ok.is_empty(), "{ok:#?}");
        let unhandled = problem(&format!("{types}say \"[]\" as B\n"));
        assert!(unhandled
            .message
            .starts_with("Reading text as B fails with Invalid"));
        let kind = problem(&format!(
            "{types}x = 5 as B\n  on Invalid -> B a = \"none\"\nsay x.a\n"
        ));
        assert!(kind
            .message
            .contains("as B needs text for the JSON, but 5 is a Number."));
    }

    #[test]
    fn web_requests_need_the_module_and_a_permit() {
        let ok = problems("bring http\npermit net \"api.example.com\"\na = fetch(\"https://api.example.com/x\")\n  on NetworkDown -> say \"off\"\nsay a.status + 1\n");
        assert!(ok.is_empty(), "{ok:#?}");
        let module = problem("permit net \"api.example.com\"\na = fetch(\"https://api.example.com/x\")\n  on NetworkDown -> say \"off\"\n");
        assert_eq!(module.suggestion.as_deref(), Some("bring http"));
        let host = problem("bring http\na = fetch(\"https://api.example.com/x\")\n  on NetworkDown -> say \"off\"\n");
        assert_eq!(
            host.suggestion.as_deref(),
            Some("permit net \"api.example.com\"")
        );
        let address = problem("bring http\npermit net \"x\"\na = fetch(\"api.example.com\")\n  on NetworkDown -> say \"off\"\n");
        assert!(address
            .message
            .starts_with("api.example.com isn't a web address."));
    }

    #[test]
    fn asking_a_model_needs_the_module_a_key_and_handlers() {
        let ready =
            "bring ai\npermit env \"ANTHROPIC_API_KEY\"\npermit net \"api.anthropic.com\"\n";
        let ok = problems(&format!("{ready}type R\n  stars: number where 1..5\nr = ask \"Rate it\" as R\n  on ModelFailed -> R stars = 1\n  on Invalid -> R stars = 1\nsay r.stars\n"));
        assert!(ok.is_empty(), "{ok:#?}");
        let module = problem(
            "permit env \"ANTHROPIC_API_KEY\"\na = ask \"Hi\"\n  on ModelFailed -> \"no\"\nsay a\n",
        );
        assert_eq!(module.suggestion.as_deref(), Some("bring ai"));
        let key = problem("bring ai\na = ask \"Hi\"\n  on ModelFailed -> \"no\"\nsay a\n");
        assert_eq!(
            key.suggestion.as_deref(),
            Some("permit env \"ANTHROPIC_API_KEY\"")
        );
        let host = problem("bring ai\npermit env \"ANTHROPIC_API_KEY\"\na = ask \"Hi\"\n  on ModelFailed -> \"no\"\nsay a\n");
        assert_eq!(
            host.suggestion.as_deref(),
            Some("permit net \"api.anthropic.com\"")
        );
        let unhandled = problem(&format!("{ready}say ask \"Hi\"\n"));
        assert!(unhandled
            .message
            .starts_with("Asking a model can fail with ModelFailed"));
    }

    #[test]
    fn routes_need_the_module_and_respond_on_every_path() {
        let ok = problems("bring web\nroute GET \"/hi/{name}\"\n  respond 200, \"Hi {name} from {request.path}\"\n");
        assert!(ok.is_empty(), "{ok:#?}");
        let module = problem("route GET \"/hi\"\n  respond 200, \"Hi\"\n");
        assert_eq!(module.suggestion.as_deref(), Some("bring web"));
        let silent = problem("bring web\nroute GET \"/hi\"\n  say \"hi\"\n");
        assert!(silent
            .message
            .starts_with("GET /hi has to respond on every path."));
        let status = problem("bring web\nroute GET \"/hi\"\n  respond 700, \"Hi\"\n");
        assert!(status
            .message
            .starts_with("respond needs a status from 100 through 599"));
        let taken = problem("bring web\ntype Request\n  a: text\n");
        assert!(taken
            .message
            .starts_with("Request is a type Quill's web module gives"));
    }

    const CHARGE: &str = "fn charge(amount: USD) -> text
  fails Declined, NetworkDown
  when
    amount <= 0 USD -> fail Declined
    amount > 1000 USD -> fail NetworkDown
  return \"paid\"
";

    #[test]
    fn a_value_after_on_matches_the_call() {
        assert_eq!(
            problems(&format!(
                "{CHARGE}r = charge(1 USD)\n  on Declined -> \"declined\"\n  on NetworkDown -> say \"later\"\n"
            )),
            vec![]
        );
        let found = problem(&format!(
            "{CHARGE}r = charge(1 USD)\n  on Declined -> 0\n  on NetworkDown -> \"later\"\n"
        ));
        assert!(found
            .message
            .starts_with("charge gives back Text, but 0 is a Number."));
    }

    #[test]
    fn a_call_that_gives_nothing_back_cant_take_a_value() {
        let found = problem(
            "fn greet(n: text)\n  fails Oops\n  fail Oops\ngreet(\"a\")\n  on Oops -> \"x\"\n",
        );
        assert!(found.message.contains("greet gives nothing back"));
    }

    #[test]
    fn an_on_with_a_value_needs_no_return_in_a_function() {
        assert_eq!(
            problems(&format!(
                "{CHARGE}fn safe(amount: USD) -> text\n  r = charge(amount)\n    on Declined -> \"no\"\n    on NetworkDown -> \"later\"\n  return r\n"
            )),
            vec![]
        );
    }

    #[test]
    fn every_case_of_a_when_value_gives_the_same_kind() {
        let found = problem("x = when\n  1 > 0 -> \"a\"\n  else -> 2\n");
        assert!(found
            .message
            .contains("\"a\" on line 2 is Text, but 2 is a Number."));
    }

    #[test]
    fn a_when_value_needs_true_or_false_conditions() {
        let found = problem("x = when\n  1 -> \"a\"\n  else -> \"b\"\n");
        assert!(found
            .message
            .contains("needs a condition that is true or false"));
    }

    #[test]
    fn missing_needs_found_in_the_other_cases() {
        let found = problem("x = when\n  1 > 0 -> 1\n  else -> missing\n");
        assert!(found.message.contains("like found 1"));
    }

    #[test]
    fn a_when_value_has_the_kind_of_its_cases() {
        let found = problem("x = when\n  1 > 0 -> \"a\"\n  else -> \"b\"\ny = x + 1\n");
        assert!(found.message.contains("Text"), "{}", found.message);
    }

    #[test]
    fn an_example_far_from_its_function_says_to_move_it() {
        let found = problem(
            "--- example: double(2) == 4\nsay 1\nfn double(n: number) -> number\n  return n * 2\n",
        );
        assert!(found
            .message
            .contains("Move this example directly above fn double"));
    }
}
