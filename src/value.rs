//! Values that Quill programs work with at runtime.

use std::fmt;

use crate::decimal::Decimal;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    /// An exact decimal, so money and prices never round by surprise.
    Number(Decimal),
    /// The result of a comparison: `true` or `false`.
    Bool(bool),
}

/// How `say` shows a value.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Text(text) => write!(f, "{text}"),
            Value::Number(number) => write!(f, "{number}"),
            Value::Bool(value) => write!(f, "{value}"),
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
        }
    }

    /// A short name for the kind of value, for use in a sentence.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Value::Text(_) => "text",
            Value::Number(_) => "a number",
            Value::Bool(_) => "true or false",
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
