//! `bring ai`: asking a model, with `ask "..."` or `ask "..." as Review`.
//!
//! The program's permits choose the provider, so the code says which one it uses:
//! `permit env "ANTHROPIC_API_KEY"` asks Claude, and `permit env "OPENAI_API_KEY"` asks a
//! service that speaks the OpenAI chat format (OpenAI itself, or a local server such
//! as Ollama). The host needs a `permit net` like every other request.
//!
//! `ask ... as Review` sends the shape of `Review` as a JSON schema, so the model
//! answers in that shape, and then reads the answer with every rule of the type, the
//! same as `as Review` reads any JSON. A model that can't be reached, refuses, or
//! answers with an error fails with `ModelFailed`; an answer that breaks the type's
//! rules fails with `Invalid`.

use std::time::Duration;

use serde_json::{json, Map, Value as Json};

use crate::json::{self, Shape};
use crate::native::{self, Permits};
use crate::parser::Permission;
use crate::value::Value;

/// The model Quill asks when the program uses Claude and doesn't choose one.
pub const CLAUDE_MODEL: &str = "claude-opus-5-5";
const CLAUDE_URL: &str = "https://api.anthropic.com";
const OPENAI_URL: &str = "https://api.openai.com/v1";
/// Models think before they answer, so they get longer than a web page to reply.
const WAIT: Duration = Duration::from_secs(600);

/// Where a question goes.
#[derive(Debug, PartialEq)]
pub enum Provider {
    Claude {
        key: String,
        base: String,
        model: String,
    },
    OpenAi {
        key: String,
        base: String,
        model: String,
    },
}

/// The setting that holds the key for each kind of provider.
pub const CLAUDE_KEY: &str = "ANTHROPIC_API_KEY";
pub const OPENAI_KEY: &str = "OPENAI_API_KEY";

/// The provider the program's permits choose, with its settings. An `Err` stops the
/// program: the program can't fix a missing key by handling a failure.
pub fn provider(
    permits: &Permits,
    setting: impl Fn(&str) -> Option<String>,
) -> Result<Provider, String> {
    let model = setting("QUILL_AI_MODEL").filter(|model| !model.trim().is_empty());
    if permits.allows(Permission::Env, CLAUDE_KEY) {
        let key = setting(CLAUDE_KEY).ok_or_else(|| unset(CLAUDE_KEY, "your Claude API key"))?;
        return Ok(Provider::Claude {
            key,
            base: setting("ANTHROPIC_BASE_URL").unwrap_or_else(|| CLAUDE_URL.to_string()),
            model: model.unwrap_or_else(|| CLAUDE_MODEL.to_string()),
        });
    }
    if permits.allows(Permission::Env, OPENAI_KEY) {
        let key = setting(OPENAI_KEY).ok_or_else(|| unset(OPENAI_KEY, "the service's API key"))?;
        let model = model.ok_or_else(|| {
            "ask uses an OpenAI-compatible service, but QUILL_AI_MODEL isn't set.\n\
             Set QUILL_AI_MODEL to the name of the model the service should use."
                .to_string()
        })?;
        return Ok(Provider::OpenAi {
            key,
            base: setting("OPENAI_BASE_URL").unwrap_or_else(|| OPENAI_URL.to_string()),
            model,
        });
    }
    Err(format!(
        "ask needs a key for a model, and the program permits none.\n\
         Add permit env \"{CLAUDE_KEY}\" to ask Claude, or permit env \"{OPENAI_KEY}\" \
         for an OpenAI-compatible service."
    ))
}

fn unset(name: &str, what: &str) -> String {
    format!(
        "ask needs the setting {name}, but it isn't set.\n\
         Set {name} to {what} before running the program. Keep it out of the code."
    )
}

/// Asks the question. The answer is text, or `shape` when the program asked for a kind.
/// Failures come back as values (`ModelFailed`, `Invalid`); an `Err` stops the program.
pub fn ask(question: &str, shape: Option<&Shape>, permits: &Permits) -> Result<Value, String> {
    let provider = provider(permits, |name| std::env::var(name).ok())?;
    let url = match &provider {
        Provider::Claude { base, .. } => format!("{}/v1/messages", base.trim_end_matches('/')),
        Provider::OpenAi { base, .. } => format!("{}/chat/completions", base.trim_end_matches('/')),
    };
    if !permits.allows(Permission::Net, &url) {
        return Err(format!(
            "ask can't reach {}, because the program has no permit for it.\n\
             A program touches only what its permits allow. If it should, add this at the top \
             of the program: {}",
            native::host_of(&url),
            native::permit_for(Permission::Net, &url)
        ));
    }
    let schema = shape.map(schema_for);
    let failed = || Ok(Value::Failure("ModelFailed".to_string()));
    let agent = ureq::AgentBuilder::new().timeout(WAIT).build();
    let (request, body) = match &provider {
        Provider::Claude { key, model, .. } => {
            let mut body = json!({
                "model": model,
                "max_tokens": 16000,
                "messages": [{"role": "user", "content": question}],
            });
            if let Some(schema) = &schema {
                body["output_config"] =
                    json!({"format": {"type": "json_schema", "schema": schema}});
            }
            let mut request = agent
                .post(&url)
                .set("x-api-key", key)
                .set("anthropic-version", "2023-06-01")
                .set("content-type", "application/json");
            // When a model's safeguards decline, Anthropic's recommended model answers instead.
            if falls_back(model, &provider) {
                body["fallbacks"] = json!("default");
                request = request.set("anthropic-beta", "server-side-fallback-2026-07-01");
            }
            (request, body)
        }
        Provider::OpenAi { key, model, .. } => {
            let mut body = json!({
                "model": model,
                "messages": [{"role": "user", "content": question}],
            });
            if let Some(schema) = &schema {
                body["response_format"] = json!({
                    "type": "json_schema",
                    "json_schema": {"name": "answer", "strict": true, "schema": schema},
                });
            }
            let request = agent
                .post(&url)
                .set("authorization", &format!("Bearer {key}"))
                .set("content-type", "application/json");
            (request, body)
        }
    };
    let Ok(answer) = request.send_string(&body.to_string()) else {
        return failed();
    };
    let Ok(reply) = answer
        .into_string()
        .map_err(|_| ())
        .and_then(|text| serde_json::from_str::<Json>(&text).map_err(|_| ()))
    else {
        return failed();
    };
    let Some(text) = (match provider {
        Provider::Claude { .. } => claude_text(&reply),
        Provider::OpenAi { .. } => openai_text(&reply),
    }) else {
        return failed();
    };
    let Some(shape) = shape else {
        return Ok(Value::Text(text.trim().to_string()));
    };
    let wanted = if matches!(shape, Shape::Record(_)) {
        text
    } else {
        // A kind that isn't a record was asked for inside {"value": ...}.
        match serde_json::from_str::<Json>(&text) {
            Ok(Json::Object(mut object)) => match object.remove("value") {
                Some(value) => value.to_string(),
                None => return Ok(Value::Failure("Invalid".to_string())),
            },
            _ => return Ok(Value::Failure("Invalid".to_string())),
        }
    };
    Ok(json::read(&wanted, shape).unwrap_or_else(|_| Value::Failure("Invalid".to_string())))
}

/// Server-side fallbacks are offered for these models on Anthropic's own API.
fn falls_back(model: &str, provider: &Provider) -> bool {
    let Provider::Claude { base, .. } = provider else {
        return false;
    };
    base.trim_end_matches('/') == CLAUDE_URL
        && matches!(
            model,
            "claude-opus-5-5" | "claude-opus-5" | "claude-fable-5-1" | "claude-sonnet-5-5"
        )
}

/// The answer's text. `None` when the model declined or the reply has no text.
fn claude_text(reply: &Json) -> Option<String> {
    if reply.get("type").and_then(Json::as_str) == Some("error") {
        return None;
    }
    if reply.get("stop_reason").and_then(Json::as_str) == Some("refusal") {
        return None;
    }
    let text: String = reply
        .get("content")?
        .as_array()?
        .iter()
        .filter(|block| block.get("type").and_then(Json::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Json::as_str))
        .collect();
    (!text.is_empty()).then_some(text)
}

fn openai_text(reply: &Json) -> Option<String> {
    let message = reply.get("choices")?.get(0)?.get("message")?;
    if message
        .get("refusal")
        .is_some_and(|refusal| !refusal.is_null())
    {
        return None;
    }
    message
        .get("content")
        .and_then(Json::as_str)
        .map(str::to_string)
        .filter(|text| !text.is_empty())
}

/// The JSON schema for an answer of `shape`. The outside is always an object, so a kind
/// that isn't a record is asked for as `{"value": ...}`.
pub fn schema_for(shape: &Shape) -> Json {
    match shape {
        Shape::Record(_) => schema(shape),
        other => json!({
            "type": "object",
            "properties": {"value": schema(other)},
            "required": ["value"],
            "additionalProperties": false,
        }),
    }
}

fn schema(shape: &Shape) -> Json {
    match shape {
        Shape::Text => json!({"type": "string"}),
        Shape::Number => json!({"type": "number"}),
        Shape::Bool => json!({"type": "boolean"}),
        Shape::Money(code) => {
            json!({"type": "number", "description": format!("an amount in {code}")})
        }
        Shape::Maybe(inner) => json!({"anyOf": [schema(inner), {"type": "null"}]}),
        Shape::List(item) => json!({"type": "array", "items": schema(item)}),
        Shape::Record(fields) => {
            let mut properties = Map::new();
            for field in fields {
                let mut field_schema = schema(&field.shape);
                // Ranges aren't part of the schema; they are said in words, and checked
                // when the answer is read.
                if let (Some((low, high)), Json::Object(object)) = (field.range, &mut field_schema)
                {
                    object.insert(
                        "description".to_string(),
                        json!(format!("from {low} through {high}")),
                    );
                }
                properties.insert(field.name.clone(), field_schema);
            }
            let required: Vec<&str> = fields.iter().map(|field| field.name.as_str()).collect();
            json!({
                "type": "object",
                "properties": properties,
                "required": required,
                "additionalProperties": false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decimal::Decimal;
    use crate::json::FieldShape;

    fn permits(env: &[&str]) -> Permits {
        Permits {
            list: env
                .iter()
                .map(|name| (Permission::Env, name.to_string()))
                .collect(),
            folder: Default::default(),
        }
    }

    #[test]
    fn the_permits_choose_the_provider() {
        let set = |name: &str| match name {
            "ANTHROPIC_API_KEY" | "OPENAI_API_KEY" => Some("key".to_string()),
            _ => None,
        };
        assert_eq!(
            provider(&permits(&["ANTHROPIC_API_KEY"]), set).unwrap(),
            Provider::Claude {
                key: "key".to_string(),
                base: CLAUDE_URL.to_string(),
                model: CLAUDE_MODEL.to_string(),
            }
        );
        let open = provider(&permits(&["OPENAI_API_KEY"]), set).unwrap_err();
        assert!(open.contains("QUILL_AI_MODEL isn't set"));
        let none = provider(&permits(&[]), set).unwrap_err();
        assert!(none.starts_with("ask needs a key for a model"));
        let unset_key = provider(&permits(&["ANTHROPIC_API_KEY"]), |_| None).unwrap_err();
        assert!(unset_key.starts_with("ask needs the setting ANTHROPIC_API_KEY"));
    }

    #[test]
    fn a_kind_becomes_a_strict_schema() {
        let review = Shape::Record(vec![
            FieldShape {
                name: "stars".to_string(),
                shape: Shape::Number,
                range: Some((Decimal::from_int(1), Decimal::from_int(5))),
            },
            FieldShape {
                name: "note".to_string(),
                shape: Shape::Maybe(Box::new(Shape::Text)),
                range: None,
            },
        ]);
        assert_eq!(
            schema_for(&review).to_string(),
            r#"{"type":"object","properties":{"stars":{"type":"number","description":"from 1 through 5"},"note":{"anyOf":[{"type":"string"},{"type":"null"}]}},"required":["stars","note"],"additionalProperties":false}"#
        );
        assert_eq!(
            schema_for(&Shape::List(Box::new(Shape::Text))).to_string(),
            r#"{"type":"object","properties":{"value":{"type":"array","items":{"type":"string"}}},"required":["value"],"additionalProperties":false}"#
        );
    }

    #[test]
    fn refusals_and_errors_give_no_text() {
        assert_eq!(
            claude_text(
                &json!({"stop_reason": "end_turn", "content": [{"type": "thinking", "thinking": ""}, {"type": "text", "text": "Hi"}]})
            ),
            Some("Hi".to_string())
        );
        assert_eq!(
            claude_text(&json!({"stop_reason": "refusal", "content": []})),
            None
        );
        assert_eq!(
            claude_text(&json!({"type": "error", "error": {"type": "overloaded_error"}})),
            None
        );
        assert_eq!(
            openai_text(&json!({"choices": [{"message": {"content": "Hi", "refusal": null}}]})),
            Some("Hi".to_string())
        );
    }
}
