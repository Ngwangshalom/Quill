//! Quill's own functions, and the permits that guard them.
//!
//! A native function is called like any function, `read_file("config/rates.txt")`,
//! and can fail like one, so `on` and `try` work the same way. A function that
//! reaches outside the program (a file, a setting, the network) needs a `permit`
//! at the top of the program. A path, host or name written in the call is checked
//! before running; any other value is checked when the call runs.

use std::fs;
use std::path::{Path, PathBuf};

use crate::parser::Permission;
use crate::value::Value;

/// A kind a native function takes or gives back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Text,
    Bool,
    MaybeText,
    /// Any value at all, as `json` takes.
    Any,
    /// A list of text, like the headers of a request.
    TextList,
    /// The answer to a web request: a record with `status` (a number) and `body` (text).
    Answer,
    /// A number.
    Number,
    /// What a route sends back: `status`, `body`, and `kind`, the type of the body.
    Response,
}

/// One of Quill's own functions.
#[derive(Debug)]
pub struct Native {
    pub name: &'static str,
    pub params: &'static [(&'static str, Kind)],
    /// Absent when the function gives nothing back.
    pub result: Option<Kind>,
    /// Failures the caller has to handle with `on` or `try`.
    pub fails: &'static [&'static str],
    /// The permit it needs, and which argument names what it touches.
    pub guard: Option<(Permission, usize)>,
    /// The Quill module a program brings to use it, like `bring http`.
    pub module: Option<&'static str>,
}

pub const NATIVES: &[Native] = &[
    Native {
        name: "read_file",
        params: &[("path", Kind::Text)],
        result: Some(Kind::Text),
        fails: &["FileMissing"],
        guard: Some((Permission::Read, 0)),
        module: None,
    },
    Native {
        name: "write_file",
        params: &[("path", Kind::Text), ("text", Kind::Text)],
        result: None,
        fails: &["CantWrite"],
        guard: Some((Permission::Write, 0)),
        module: None,
    },
    Native {
        name: "file_exists",
        params: &[("path", Kind::Text)],
        result: Some(Kind::Bool),
        fails: &[],
        guard: Some((Permission::Read, 0)),
        module: None,
    },
    Native {
        name: "fetch",
        params: &[("url", Kind::Text)],
        result: Some(Kind::Answer),
        fails: &["NetworkDown"],
        guard: Some((Permission::Net, 0)),
        module: Some("http"),
    },
    Native {
        name: "post",
        params: &[("url", Kind::Text), ("body", Kind::Text)],
        result: Some(Kind::Answer),
        fails: &["NetworkDown"],
        guard: Some((Permission::Net, 0)),
        module: Some("http"),
    },
    Native {
        name: "send",
        params: &[
            ("method", Kind::Text),
            ("url", Kind::Text),
            ("body", Kind::Text),
            ("headers", Kind::TextList),
        ],
        result: Some(Kind::Answer),
        fails: &["NetworkDown"],
        guard: Some((Permission::Net, 1)),
        module: Some("http"),
    },
    Native {
        name: "respond",
        params: &[("status", Kind::Number), ("value", Kind::Any)],
        result: Some(Kind::Response),
        fails: &[],
        guard: None,
        module: Some("web"),
    },
    Native {
        name: "json",
        params: &[("value", Kind::Any)],
        result: Some(Kind::Text),
        fails: &[],
        guard: None,
        module: None,
    },
    Native {
        name: "env",
        params: &[("name", Kind::Text)],
        result: Some(Kind::MaybeText),
        fails: &[],
        guard: Some((Permission::Env, 0)),
        module: None,
    },
];

/// The native function with this name, and its number.
pub fn find(name: &str) -> Option<(usize, &'static Native)> {
    NATIVES
        .iter()
        .enumerate()
        .find(|(_, native)| native.name == name)
}

/// The permits a program wrote, and the folder its paths are counted from.
#[derive(Debug, Clone, Default)]
pub struct Permits {
    pub list: Vec<(Permission, String)>,
    pub folder: PathBuf,
}

impl Permits {
    /// Whether a permit covers `target`. A folder covers everything inside it.
    pub fn allows(&self, access: Permission, target: &str) -> bool {
        self.list
            .iter()
            .filter(|(kind, _)| *kind == access)
            .any(|(_, permitted)| covers(access, permitted, target))
    }
}

fn covers(access: Permission, permitted: &str, target: &str) -> bool {
    match access {
        Permission::Read | Permission::Write => {
            let permitted = normal(permitted);
            let target = normal(target);
            // `permit read "."` is the program's folder and everything below it.
            if permitted.is_empty() {
                return !target.starts_with("..") && !target.starts_with('/');
            }
            inside(&target, &permitted)
        }
        Permission::Env => permitted == target,
        Permission::Net => {
            let permitted = permitted.to_ascii_lowercase();
            let mut host = host_of(target).to_ascii_lowercase();
            // A permit without a port allows any port of that host.
            if !permitted.contains(':') {
                host = host.split(':').next().unwrap_or_default().to_string();
            }
            match permitted.strip_prefix("*.") {
                Some(domain) => host.ends_with(&format!(".{domain}")),
                None => host == permitted,
            }
        }
    }
}

fn inside(target: &str, folder: &str) -> bool {
    target == folder || target.starts_with(&format!("{folder}/"))
}

/// A path with `.` and `..` worked out, and `/` between folders: `./a/../b/` is `b`.
/// `..` that leaves the start is kept, so it can never pass as a folder inside.
pub fn normal(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|last| *last != "..") {
                    parts.pop();
                } else if !absolute {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    let joined = parts.join("/");
    if absolute {
        format!("/{joined}")
    } else {
        joined
    }
}

/// `https://api.stripe.com:443/v1` is `api.stripe.com:443`; a bare host stays as it is.
pub fn host_of(target: &str) -> &str {
    let rest = target.split_once("://").map_or(target, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.rsplit_once('@').map_or(host, |(_, host)| host)
}

/// What a missing permit should say to add: the folder of a file, or the name itself.
pub fn permit_for(access: Permission, target: &str) -> String {
    let shown = match access {
        Permission::Read | Permission::Write => {
            let path = normal(target);
            match path.rsplit_once('/') {
                Some((folder, _)) if !folder.is_empty() => folder.to_string(),
                _ => path,
            }
        }
        Permission::Net => host_of(target)
            .split(':')
            .next()
            .unwrap_or(target)
            .to_string(),
        Permission::Env => target.to_string(),
    };
    format!("permit {} \"{shown}\"", access.word())
}

/// What a native function does with what it touches, for a missing permit's message.
pub fn verb(access: Permission) -> &'static str {
    match access {
        Permission::Read => "read",
        Permission::Write => "write",
        Permission::Net => "reach",
        Permission::Env => "read the setting",
    }
}

/// What a call gave back: a value, nothing, or a failure the caller handles.
pub enum Outcome {
    Value(Value),
    Nothing,
}

/// Runs native function number `index`. An `Err` stops the program with that message.
pub fn call(index: usize, args: Vec<Value>, permits: &Permits) -> Result<Outcome, String> {
    let native = &NATIVES[index];
    let text = |at: usize| match args.get(at) {
        Some(Value::Text(text)) => Ok(text.clone()),
        _ => Err(format!(
            "Quill made an internal mistake calling {} (an argument isn't text).\n\
             This is a bug in Quill, not in your code. Please report it.",
            native.name
        )),
    };
    if let Some((access, at)) = native.guard {
        let target = text(at)?;
        if !permits.allows(access, &target) {
            return Err(format!(
                "{} can't {} {target}, because the program has no permit for it.\n\
                 A program touches only what its permits allow. If it should, add this \
                 at the top of the program: {}",
                native.name,
                verb(access),
                permit_for(access, &target)
            ));
        }
    }
    let failure = |name: &str| Ok(Outcome::Value(Value::Failure(name.to_string())));
    match native.name {
        "read_file" => match fs::read_to_string(place(permits, &text(0)?)) {
            Ok(contents) => Ok(Outcome::Value(Value::Text(contents))),
            Err(_) => failure("FileMissing"),
        },
        "write_file" => {
            let path = place(permits, &text(0)?);
            if let Some(folder) = path.parent() {
                if !folder.as_os_str().is_empty() && fs::create_dir_all(folder).is_err() {
                    return failure("CantWrite");
                }
            }
            match fs::write(&path, text(1)?) {
                Ok(()) => Ok(Outcome::Nothing),
                Err(_) => failure("CantWrite"),
            }
        }
        "respond" => {
            let status = match args.first() {
                Some(Value::Number(number)) => {
                    number.whole().filter(|code| (100..=599).contains(code))
                }
                _ => None,
            };
            let Some(status) = status else {
                return Err(format!(
                    "respond needs a status from 100 through 599, but got {}.\n\
                     200 means it worked, 201 that something was made, 400 that the request \
                     was wrong, 404 that it wasn't found.",
                    args.first().map(Value::written).unwrap_or_default()
                ));
            };
            let (body, kind) = match args.get(1) {
                Some(Value::Text(text)) => (text.clone(), "text/plain; charset=utf-8"),
                Some(value) => (crate::json::write(value), "application/json"),
                None => (String::new(), "text/plain; charset=utf-8"),
            };
            Ok(Outcome::Value(Value::Record(vec![
                (
                    "status".to_string(),
                    Value::Number(crate::decimal::Decimal::from_int(status)),
                ),
                ("body".to_string(), Value::Text(body)),
                ("kind".to_string(), Value::Text(kind.to_string())),
            ])))
        }
        "json" => Ok(Outcome::Value(Value::Text(crate::json::write(
            args.first().unwrap_or(&Value::Missing),
        )))),
        "fetch" => request("GET", &text(0)?, None, Vec::new()),
        "post" => request(
            "POST",
            &text(0)?,
            Some(text(1)?),
            vec!["Content-Type: application/json".to_string()],
        ),
        "send" => {
            let headers = match args.get(3) {
                Some(Value::List(items)) => items
                    .iter()
                    .map(|item| match item {
                        Value::Text(text) => text.clone(),
                        other => other.to_string(),
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let body = text(2)?;
            let body = (!body.is_empty()).then_some(body);
            request(&text(0)?.to_uppercase(), &text(1)?, body, headers)
        }
        "file_exists" => Ok(Outcome::Value(Value::Bool(
            place(permits, &text(0)?).is_file(),
        ))),
        "env" => Ok(Outcome::Value(match std::env::var(text(0)?) {
            Ok(value) => Value::Found(Box::new(Value::Text(value))),
            Err(_) => Value::Missing,
        })),
        other => Err(format!(
            "Quill made an internal mistake: it has no way to run {other}.\n\
             This is a bug in Quill, not in your code. Please report it."
        )),
    }
}

/// How long a web request may take before it counts as `NetworkDown`.
const WAIT: std::time::Duration = std::time::Duration::from_secs(30);

/// Sends a web request. Any answer, even 404 or 500, is an answer: its status says how
/// it went. Not reaching the host at all is the failure `NetworkDown`.
fn request(
    method: &str,
    url: &str,
    body: Option<String>,
    headers: Vec<String>,
) -> Result<Outcome, String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!(
            "{url} isn't a web address.\n\
             A web address starts with https:// or http://, like https://api.example.com/rates."
        ));
    }
    if !matches!(method, "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD") {
        return Err(format!(
            "{method} isn't a way to send a web request.\n\
             Use GET, POST, PUT, PATCH, DELETE or HEAD."
        ));
    }
    let agent = ureq::AgentBuilder::new().timeout(WAIT).build();
    let mut request = agent.request(method, url);
    for header in &headers {
        let Some((name, value)) = header.split_once(':') else {
            return Err(format!(
                "The header {header:?} needs a name, a colon, and a value.\n\
                 A header looks like \"Authorization: Bearer KEY\"."
            ));
        };
        request = request.set(name.trim(), value.trim());
    }
    let sent = match body {
        Some(body) => request.send_string(&body),
        None => request.call(),
    };
    let answer = match sent {
        Ok(answer) | Err(ureq::Error::Status(_, answer)) => answer,
        Err(ureq::Error::Transport(_)) => {
            return Ok(Outcome::Value(Value::Failure("NetworkDown".to_string())))
        }
    };
    let status = answer.status();
    let body = answer.into_string().unwrap_or_default();
    Ok(Outcome::Value(answer_record(status, body)))
}

/// The record a web request gives back.
pub fn answer_record(status: u16, body: String) -> Value {
    Value::Record(vec![
        (
            "status".to_string(),
            Value::Number(crate::decimal::Decimal::from_int(i128::from(status))),
        ),
        ("body".to_string(), Value::Text(body)),
    ])
}

/// A path counted from the program's folder.
fn place(permits: &Permits, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        permits.folder.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permits(list: &[(Permission, &str)]) -> Permits {
        Permits {
            list: list
                .iter()
                .map(|(access, target)| (*access, target.to_string()))
                .collect(),
            folder: PathBuf::new(),
        }
    }

    #[test]
    fn a_folder_permit_covers_what_is_inside_it() {
        let read = permits(&[(Permission::Read, "./config")]);
        assert!(read.allows(Permission::Read, "config/rates.txt"));
        assert!(read.allows(Permission::Read, "config"));
        assert!(!read.allows(Permission::Read, "configs/rates.txt"));
        assert!(!read.allows(Permission::Read, "config/../secrets.txt"));
        assert!(!read.allows(Permission::Write, "config/rates.txt"));
        let here = permits(&[(Permission::Read, ".")]);
        assert!(here.allows(Permission::Read, "a/b.txt"));
        assert!(!here.allows(Permission::Read, "../outside.txt"));
        assert!(!here.allows(Permission::Read, "/etc/passwd"));
    }

    #[test]
    fn hosts_and_settings_match_exactly() {
        let net = permits(&[
            (Permission::Net, "api.stripe.com"),
            (Permission::Net, "*.example.com"),
        ]);
        assert!(net.allows(Permission::Net, "https://api.stripe.com/v1/charges"));
        assert!(net.allows(Permission::Net, "https://API.stripe.com:443/v1"));
        assert!(!net.allows(Permission::Net, "https://evil.com/?api.stripe.com"));
        assert!(!net.allows(Permission::Net, "https://api.stripe.com.evil.com/"));
        assert!(net.allows(Permission::Net, "https://shop.example.com/"));
        assert!(!net.allows(Permission::Net, "https://example.com/"));
        let env = permits(&[(Permission::Env, "STRIPE_KEY")]);
        assert!(env.allows(Permission::Env, "STRIPE_KEY"));
        assert!(!env.allows(Permission::Env, "STRIPE_KEY_2"));
    }

    #[test]
    fn a_missing_permit_suggests_the_folder_or_the_name() {
        assert_eq!(
            permit_for(Permission::Read, "config/rates.txt"),
            "permit read \"config\""
        );
        assert_eq!(
            permit_for(Permission::Write, "today.txt"),
            "permit write \"today.txt\""
        );
        assert_eq!(
            permit_for(Permission::Net, "https://api.stripe.com:443/v1"),
            "permit net \"api.stripe.com\""
        );
        assert_eq!(permit_for(Permission::Env, "KEY"), "permit env \"KEY\"");
        assert_eq!(normal("./a/../b/"), "b");
        assert_eq!(normal("../x"), "../x");
    }
}
