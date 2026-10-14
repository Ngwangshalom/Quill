//! Values that Quill programs work with at runtime.

use std::fmt;

use crate::decimal::Decimal;

#[derive(Debug, Clone)]
pub enum Value {
    Text(String),
    /// An exact decimal, so money and prices never round by surprise.
    Number(Decimal),
    /// The result of a comparison: `true` or `false`.
    Bool(bool),
    /// Values in order, all of one kind. The first item is at position 1.
    List(Vec<Value>),
    /// Named fields. Order on the page doesn't change which field is which.
    Record(Vec<(String, Value)>),
    /// An amount in one currency, such as `100 AED`.
    Money {
        amount: Decimal,
        currency: String,
    },
    /// A maybe that holds a value.
    Found(Box<Value>),
    /// A maybe that holds nothing.
    Missing,
    /// A failure raised by `fail`, caught by `on` or passed on by `try`.
    Failure(String),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Text(left), Value::Text(right)) => left == right,
            (Value::Number(left), Value::Number(right)) => left == right,
            (Value::Bool(left), Value::Bool(right)) => left == right,
            (Value::List(left), Value::List(right)) => left == right,
            (Value::Record(left), Value::Record(right)) => {
                left.len() == right.len()
                    && left.iter().all(|(name, value)| {
                        right.iter().any(|(other_name, other_value)| {
                            other_name == name && other_value == value
                        })
                    })
            }
            (
                Value::Money {
                    amount: left,
                    currency: left_code,
                },
                Value::Money {
                    amount: right,
                    currency: right_code,
                },
            ) => left == right && left_code == right_code,
            (Value::Found(left), Value::Found(right)) => left == right,
            (Value::Missing, Value::Missing) => true,
            (Value::Failure(left), Value::Failure(right)) => left == right,
            _ => false,
        }
    }
}

/// How `say` shows a value.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Text(text) => write!(f, "{text}"),
            Value::Number(number) => write!(f, "{number}"),
            Value::Bool(value) => write!(f, "{value}"),
            Value::List(items) if items.is_empty() => write!(f, "an empty list"),
            Value::List(items) => {
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                Ok(())
            }
            Value::Record(fields) => {
                for (index, (name, value)) in fields.iter().enumerate() {
                    if index > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{name}: {value}")?;
                }
                Ok(())
            }
            Value::Money { amount, currency } => write!(f, "{amount} {currency}"),
            Value::Found(value) => write!(f, "found {value}"),
            Value::Missing => write!(f, "missing"),
            Value::Failure(name) => write!(f, "{name}"),
        }
    }
}

impl Value {
    /// The value written the way it would appear in Quill source.
    pub fn written(&self) -> String {
        match self {
            Value::Text(text) => format!("\"{}\"", escape(text)),
            Value::Number(number) => number.to_string(),
            Value::Bool(value) => value.to_string(),
            Value::List(items) if items.is_empty() => "an empty list".to_string(),
            Value::List(items) => {
                let shown: Vec<String> = items.iter().map(Value::written).collect();
                format!("list {}", shown.join(", "))
            }
            Value::Record(fields) => {
                let shown: Vec<String> = fields
                    .iter()
                    .map(|(name, value)| format!("{name} = {}", value.written()))
                    .collect();
                format!("record {}", shown.join(", "))
            }
            Value::Money { amount, currency } => format!("{amount} {currency}"),
            Value::Found(value) => format!("found {}", value.written()),
            Value::Missing => "missing".to_string(),
            Value::Failure(name) => name.clone(),
        }
    }

    /// A short name for the kind of value, for use in a sentence.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Value::Text(_) => "text",
            Value::Number(_) => "a number",
            Value::Bool(_) => "true or false",
            Value::List(_) => "a list",
            Value::Record(_) => "a record",
            Value::Money { .. } => "money",
            Value::Found(_) | Value::Missing => "a maybe",
            Value::Failure(_) => "a failure",
        }
    }
}

fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}
