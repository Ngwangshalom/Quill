//! JSON: reading text as a Quill kind (`body as Booking`), and writing a value out
//! (`json(booking)`).
//!
//! Reading checks everything the kind says: each field is there and of its kind,
//! money is exact, and every `where` range holds. Text that doesn't fit is `Invalid`,
//! a failure the program handles with `on Invalid`. Numbers are read digit for digit,
//! never through floating point, so `0.10` stays `0.10`.

use serde_json::{Map, Number, Value as Json};

use crate::checker::KnownType;
use crate::decimal::Decimal;
use crate::parser::SigKind;
use crate::value::Value;

/// What JSON has to look like to be read as a kind.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Text,
    Number,
    Bool,
    Money(String),
    Maybe(Box<Shape>),
    List(Box<Shape>),
    Record(Vec<FieldShape>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldShape {
    pub name: String,
    pub shape: Shape,
    /// From a `where 1..500` on the field.
    pub range: Option<(Decimal, Decimal)>,
}

/// The shape of `kind`. A type becomes a record of its fields.
pub fn shape_of(kind: &SigKind, types: &[KnownType]) -> Shape {
    shape_inside(kind, types, &mut Vec::new())
}

fn shape_inside(kind: &SigKind, types: &[KnownType], inside: &mut Vec<String>) -> Shape {
    match kind {
        SigKind::Text => Shape::Text,
        SigKind::Number => Shape::Number,
        SigKind::Bool => Shape::Bool,
        SigKind::Money(code) => Shape::Money(code.clone()),
        SigKind::Maybe(inner) => Shape::Maybe(Box::new(shape_inside(inner, types, inside))),
        SigKind::List(inner) => Shape::List(Box::new(shape_inside(inner, types, inside))),
        SigKind::Named(name) => {
            // The checker has refused a type that holds itself; stop here all the same.
            if inside.contains(name) {
                return Shape::Record(Vec::new());
            }
            let Some(known) = types.iter().find(|known| &known.name == name) else {
                return Shape::Record(Vec::new());
            };
            inside.push(name.clone());
            let fields = known
                .fields
                .iter()
                .map(|field| FieldShape {
                    name: field.name.clone(),
                    shape: shape_inside(&field.kind, types, inside),
                    range: field.rule.as_ref().and_then(|rule| {
                        Some((rule.low.written_amount()?, rule.high.written_amount()?))
                    }),
                })
                .collect();
            inside.pop();
            Shape::Record(fields)
        }
    }
}

/// Reads `text` as `shape`. The `Err` says why it doesn't fit.
pub fn read(text: &str, shape: &Shape) -> Result<Value, String> {
    let json: Json =
        serde_json::from_str(text).map_err(|err| format!("the text isn't JSON ({err})"))?;
    convert(&json, shape, "the value")
}

fn convert(json: &Json, shape: &Shape, place: &str) -> Result<Value, String> {
    match (shape, json) {
        (Shape::Maybe(_), Json::Null) => Ok(Value::Missing),
        (Shape::Maybe(inner), json) => Ok(Value::Found(Box::new(convert(json, inner, place)?))),
        (Shape::Text, Json::String(text)) => Ok(Value::Text(text.clone())),
        (Shape::Bool, Json::Bool(value)) => Ok(Value::Bool(*value)),
        (Shape::Number, Json::Number(number)) => {
            Ok(Value::Number(exact(&number.to_string(), place)?))
        }
        (Shape::Money(code), Json::Number(number)) => Ok(Value::Money {
            amount: exact(&number.to_string(), place)?,
            currency: code.clone(),
        }),
        // Money is often sent as text, so no digit is lost on the way: "500.00".
        (Shape::Money(code), Json::String(text)) => Ok(Value::Money {
            amount: exact(text.trim(), place)?,
            currency: code.clone(),
        }),
        (Shape::List(item), Json::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(index, json)| convert(json, item, &format!("item {} of {place}", index + 1)))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::List),
        (Shape::Record(fields), Json::Object(object)) => {
            let mut values = Vec::with_capacity(fields.len());
            for field in fields {
                let value = match (object.get(&field.name), &field.shape) {
                    (None, Shape::Maybe(_)) => Value::Missing,
                    (None, _) => return Err(format!("{} is missing", field.name)),
                    (Some(json), shape) => convert(json, shape, &field.name)?,
                };
                if let Some((low, high)) = field.range {
                    let amount = match &value {
                        Value::Number(number) => Some(*number),
                        Value::Money { amount, .. } => Some(*amount),
                        _ => None,
                    };
                    if amount.is_some_and(|amount| amount < low || amount > high) {
                        return Err(format!(
                            "{} is {}, but it has to be from {low} through {high}",
                            field.name,
                            value.written()
                        ));
                    }
                }
                values.push((field.name.clone(), value));
            }
            Ok(Value::Record(values))
        }
        (shape, json) => Err(format!(
            "{place} should be {}, but it is {}",
            describe(shape),
            what(json)
        )),
    }
}

/// A JSON number as an exact Quill number: `-12.50` stays `-12.50`.
fn exact(written: &str, place: &str) -> Result<Decimal, String> {
    let (negative, digits) = match written.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, written),
    };
    let number = Decimal::parse(digits).ok_or_else(|| {
        format!("{place} is {written}, which isn't an exact number Quill can hold")
    })?;
    if negative {
        number
            .checked_neg()
            .map_err(|_| format!("{place} is too large"))
    } else {
        Ok(number)
    }
}

fn describe(shape: &Shape) -> String {
    match shape {
        Shape::Text => "text".to_string(),
        Shape::Number => "a number".to_string(),
        Shape::Bool => "true or false".to_string(),
        Shape::Money(code) => format!("an amount in {code}"),
        Shape::Maybe(inner) => format!("{} or null", describe(inner)),
        Shape::List(_) => "a list".to_string(),
        Shape::Record(_) => "an object with fields".to_string(),
    }
}

fn what(json: &Json) -> &'static str {
    match json {
        Json::Null => "null",
        Json::Bool(_) => "true or false",
        Json::Number(_) => "a number",
        Json::String(_) => "text",
        Json::Array(_) => "a list",
        Json::Object(_) => "an object",
    }
}

/// `value` as JSON text. Money is written as an exact number in its currency.
pub fn write(value: &Value) -> String {
    to_json(value).to_string()
}

fn to_json(value: &Value) -> Json {
    match value {
        Value::Text(text) => Json::String(text.clone()),
        Value::Bool(value) => Json::Bool(*value),
        Value::Number(number) => number_json(&number.to_string()),
        Value::Money { amount, .. } => number_json(&amount.to_string()),
        Value::List(items) => Json::Array(items.iter().map(to_json).collect()),
        Value::Record(fields) => {
            let mut object = Map::new();
            for (name, value) in fields {
                object.insert(name.clone(), to_json(value));
            }
            Json::Object(object)
        }
        Value::Found(inner) => to_json(inner),
        Value::Missing => Json::Null,
        Value::Failure(name) => Json::String(name.clone()),
    }
}

fn number_json(written: &str) -> Json {
    written
        .parse::<Number>()
        .map(Json::Number)
        .unwrap_or_else(|_| Json::String(written.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn booking() -> Shape {
        Shape::Record(vec![
            FieldShape {
                name: "venue".to_string(),
                shape: Shape::Text,
                range: None,
            },
            FieldShape {
                name: "guests".to_string(),
                shape: Shape::Number,
                range: Some((Decimal::from_int(1), Decimal::from_int(500))),
            },
            FieldShape {
                name: "fee".to_string(),
                shape: Shape::Money("USD".to_string()),
                range: None,
            },
            FieldShape {
                name: "note".to_string(),
                shape: Shape::Maybe(Box::new(Shape::Text)),
                range: None,
            },
        ])
    }

    #[test]
    fn json_is_read_exactly_as_the_kind_says() {
        let value = read(
            r#"{"venue": "Hall", "guests": 120, "fee": "500.10", "extra": 1}"#,
            &booking(),
        )
        .unwrap();
        assert_eq!(
            value.written(),
            "record venue = \"Hall\", guests = 120, fee = 500.10 USD, note = missing"
        );
        let numbers = read("[0.10, -2, 3.5]", &Shape::List(Box::new(Shape::Number))).unwrap();
        assert_eq!(numbers.to_string(), "0.10, -2, 3.5");
    }

    #[test]
    fn json_that_does_not_fit_says_why() {
        let shape = booking();
        assert_eq!(
            read(r#"{"venue": "Hall"}"#, &shape).unwrap_err(),
            "guests is missing"
        );
        assert_eq!(
            read(r#"{"venue": "Hall", "guests": 900, "fee": 1}"#, &shape).unwrap_err(),
            "guests is 900, but it has to be from 1 through 500"
        );
        assert_eq!(
            read(r#"{"venue": 5, "guests": 9, "fee": 1}"#, &shape).unwrap_err(),
            "venue should be text, but it is a number"
        );
        assert!(read("not json", &shape)
            .unwrap_err()
            .starts_with("the text isn't JSON"));
        assert!(read("1e3", &Shape::Number).is_err());
    }

    #[test]
    fn values_are_written_as_json() {
        let value = Value::Record(vec![
            (
                "fee".to_string(),
                Value::Money {
                    amount: Decimal::parse("500.10").unwrap(),
                    currency: "USD".to_string(),
                },
            ),
            ("note".to_string(), Value::Missing),
            (
                "tags".to_string(),
                Value::List(vec![Value::Text("a".to_string())]),
            ),
        ]);
        assert_eq!(write(&value), r#"{"fee":500.10,"note":null,"tags":["a"]}"#);
    }
}
