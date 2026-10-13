//! Values that Quill programs work with at runtime.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    /// Kept exactly as written until `Decimal` arrives, so nothing is rounded.
    Number(String),
}

/// How `say` shows a value.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Text(text) => write!(f, "{text}"),
            Value::Number(number) => write!(f, "{number}"),
        }
    }
}

impl Value {
    /// The value written the way it would appear in Quill source.
    pub fn written(&self) -> String {
        match self {
            Value::Text(text) => format!("\"{}\"", escape(text)),
            Value::Number(number) => number.clone(),
        }
    }

    /// A short name for the kind of value, for use in a sentence.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Value::Text(_) => "text",
            Value::Number(_) => "a number",
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
