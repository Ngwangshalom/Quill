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
