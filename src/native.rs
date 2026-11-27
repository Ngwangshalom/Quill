//! Quill's own functions, and the permits that guard them.
//!
//! A native function is called like any function and can fail like one, so `on` and
//! `try` work the same way. Paths, settings, hosts and database files need permits.

use std::fs;
use std::path::{Path, PathBuf};

use crate::parser::Permission;
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Text,
    Bool,
    MaybeText,
    Any,
    TextList,
    Answer,
    Number,
    Response,
}

#[derive(Debug)]
pub struct Native {
    pub name: &'static str,
    pub params: &'static [(&'static str, Kind)],
    pub result: Option<Kind>,
    pub fails: &'static [&'static str],
    pub guard: Option<(Permission, usize)>,
    pub module: Option<&'static str>,
}

pub const NATIVES: &[Native] = &[
    Native { name: "read_file", params: &[("path", Kind::Text)], result: Some(Kind::Text), fails: &["FileMissing"], guard: Some((Permission::Read, 0)), module: None },
    Native { name: "write_file", params: &[("path", Kind::Text), ("text", Kind::Text)], result: None, fails: &["CantWrite"], guard: Some((Permission::Write, 0)), module: None },
    Native { name: "file_exists", params: &[("path", Kind::Text)], result: Some(Kind::Bool), fails: &[], guard: Some((Permission::Read, 0)), module: None },
    Native { name: "fetch", params: &[("url", Kind::Text)], result: Some(Kind::Answer), fails: &["NetworkDown"], guard: Some((Permission::Net, 0)), module: Some("http") },
    Native { name: "post", params: &[("url", Kind::Text), ("body", Kind::Text)], result: Some(Kind::Answer), fails: &["NetworkDown"], guard: Some((Permission::Net, 0)), module: Some("http") },
    Native { name: "send", params: &[("method", Kind::Text), ("url", Kind::Text), ("body", Kind::Text), ("headers", Kind::TextList)], result: Some(Kind::Answer), fails: &["NetworkDown"], guard: Some((Permission::Net, 1)), module: Some("http") },
    Native { name: "respond", params: &[("status", Kind::Number), ("value", Kind::Any), ("headers", Kind::TextList)], result: Some(Kind::Response), fails: &[], guard: None, module: Some("web") },
    Native { name: "json", params: &[("value", Kind::Any)], result: Some(Kind::Text), fails: &[], guard: None, module: None },
    Native { name: "env", params: &[("name", Kind::Text)], result: Some(Kind::MaybeText), fails: &[], guard: Some((Permission::Env, 0)), module: None },
    Native { name: "db_execute", params: &[("path", Kind::Text), ("sql", Kind::Text), ("parameters", Kind::Text)], result: Some(Kind::Number), fails: &["DatabaseDown"], guard: Some((Permission::Write, 0)), module: Some("db") },
    Native { name: "db_query", params: &[("path", Kind::Text), ("sql", Kind::Text), ("parameters", Kind::Text)], result: Some(Kind::Text), fails: &["DatabaseDown"], guard: Some((Permission::Read, 0)), module: Some("db") },
];

pub fn find(name: &str) -> Option<(usize, &'static Native)> {
    NATIVES.iter().enumerate().find(|(_, native)| native.name == name)
}

#[derive(Debug, Clone, Default)]
pub struct Permits {
    pub list: Vec<(Permission, String)>,
    pub folder: PathBuf,
}

impl Permits {
    pub fn allows(&self, access: Permission, target: &str) -> bool {
        self.list.iter().filter(|(kind, _)| *kind == access)
            .any(|(_, permitted)| covers(access, permitted, target))
    }
}

fn covers(access: Permission, permitted: &str, target: &str) -> bool {
    match access {
        Permission::Read | Permission::Write => {
            let permitted = normal(permitted);
            let target = normal(target);
            if permitted.is_empty() { return !target.starts_with("..") && !target.starts_with('/'); }
            target == permitted || target.starts_with(&format!("{permitted}/"))
        }
        Permission::Env => permitted == target,
        Permission::Net => {
            let permitted = permitted.to_ascii_lowercase();
            let mut host = host_of(target).to_ascii_lowercase();
            if !permitted.contains(':') { host = host.split(':').next().unwrap_or_default().to_string(); }
            match permitted.strip_prefix("*.") {
                Some(domain) => host.ends_with(&format!(".{domain}")),
                None => host == permitted,
            }
        }
    }
}

pub fn normal(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => if parts.last().is_some_and(|last| *last != "..") { parts.pop(); } else if !absolute { parts.push(".."); },
            other => parts.push(other),
        }
    }
    let joined = parts.join("/");
    if absolute { format!("/{joined}") } else { joined }
}

pub fn host_of(target: &str) -> &str {
    let rest = target.split_once("://").map_or(target, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.rsplit_once('@').map_or(host, |(_, host)| host)
}

pub fn permit_for(access: Permission, target: &str) -> String {
    let shown = match access {
        Permission::Read | Permission::Write => {
            let path = normal(target);
            match path.rsplit_once('/') { Some((folder, _)) if !folder.is_empty() => folder.to_string(), _ => path }
        }
        Permission::Net => host_of(target).split(':').next().unwrap_or(target).to_string(),
        Permission::Env => target.to_string(),
    };
    format!("permit {} \"{shown}\"", access.word())
}

pub fn verb(access: Permission) -> &'static str {
    match access { Permission::Read => "read", Permission::Write => "write", Permission::Net => "reach", Permission::Env => "read the setting" }
}

pub enum Outcome { Value(Value), Nothing }

pub fn call(index: usize, args: Vec<Value>, permits: &Permits) -> Result<Outcome, String> {
    let native = &NATIVES[index];
    let text = |at: usize| match args.get(at) {
        Some(Value::Text(text)) => Ok(text.clone()),
        _ => Err(format!("Quill made an internal mistake calling {} (an argument isn't text).\nThis is a bug in Quill, not in your code. Please report it.", native.name)),
    };
    if let Some((access, at)) = native.guard {
        let target = text(at)?;
        if !permits.allows(access, &target) {
            return Err(format!("{} can't {} {target}, because the program has no permit for it.\nA program touches only what its permits allow. If it should, add this at the top of the program: {}", native.name, verb(access), permit_for(access, &target)));
        }
    }
    let failure = |name: &str| Ok(Outcome::Value(Value::Failure(name.to_string())));
    match native.name {
        "read_file" => match fs::read_to_string(place(permits, &text(0)?)) { Ok(contents) => Ok(Outcome::Value(Value::Text(contents))), Err(_) => failure("FileMissing") },
        "write_file" => {
            let path = place(permits, &text(0)?);
            if let Some(folder) = path.parent() { if !folder.as_os_str().is_empty() && fs::create_dir_all(folder).is_err() { return failure("CantWrite"); } }
            match fs::write(path, text(1)?) { Ok(()) => Ok(Outcome::Nothing), Err(_) => failure("CantWrite") }
        }
        "file_exists" => Ok(Outcome::Value(Value::Bool(place(permits, &text(0)?).is_file()))),
        "env" => Ok(Outcome::Value(match std::env::var(text(0)?) { Ok(value) => Value::Found(Box::new(Value::Text(value))), Err(_) => Value::Missing })),
        "json" => Ok(Outcome::Value(Value::Text(crate::json::write(args.first().unwrap_or(&Value::Missing))))),
        "db_execute" => {
            let result = crate::database::execute(&permits.folder, &text(0)?, &text(1)?, &text(2)?);
            match result {
                Ok(changed) => Ok(Outcome::Value(Value::Number(crate::decimal::Decimal::from_int(changed as i128)))),
                Err(_) => failure("DatabaseDown"),
            }
        }
        "db_query" => match crate::database::query(&permits.folder, &text(0)?, &text(1)?, &text(2)?) {
            Ok(rows) => Ok(Outcome::Value(Value::Text(rows))),
            Err(_) => failure("DatabaseDown"),
        },
        "respond" => respond(&args),
        "fetch" => request("GET", &text(0)?, None, Vec::new()),
        "post" => request("POST", &text(0)?, Some(text(1)?), vec!["Content-Type: application/json".into()]),
        "send" => {
            let headers = match args.get(3) { Some(Value::List(items)) => items.iter().map(|item| match item { Value::Text(text) => text.clone(), other => other.to_string() }).collect(), _ => Vec::new() };
            let body = text(2)?;
            request(&text(0)?.to_uppercase(), &text(1)?, (!body.is_empty()).then_some(body), headers)
        }
        other => Err(format!("Quill made an internal mistake: it has no way to run {other}.\nThis is a bug in Quill, not in your code. Please report it.")),
    }
}

fn respond(args: &[Value]) -> Result<Outcome, String> {
    let status = match args.first() { Some(Value::Number(number)) => number.whole().filter(|code| (100..=599).contains(code)), _ => None };
    let Some(status) = status else { return Err(format!("respond needs a status from 100 through 599, but got {}.\n200 means it worked, 201 that something was made, 400 that the request was wrong, 404 that it wasn't found.", args.first().map(Value::written).unwrap_or_default())); };
    let (body, kind) = match args.get(1) { Some(Value::Text(text)) => (text.clone(), "text/plain; charset=utf-8"), Some(value) => (crate::json::write(value), "application/json"), None => (String::new(), "text/plain; charset=utf-8") };
    let headers = match args.get(2) {
        Some(Value::List(items)) => { let mut out = Vec::new(); for item in items { match item { Value::Text(text) if text.contains(':') => out.push(Value::Text(text.clone())), Value::Text(text) => return Err(format!("The header {text:?} needs a name, a colon, and a value.\nA header looks like \"Authorization: Bearer KEY\".")), other => return Err(format!("respond's headers are a list of text, but one item is {}.\nEach header looks like \"Authorization: Bearer KEY\".", other.written())) } } Value::List(out) },
        Some(other) => return Err(format!("respond's headers are a list of text, but got {}.\nEach header looks like \"Authorization: Bearer KEY\".", other.written())),
        None => Value::List(Vec::new()),
    };
    Ok(Outcome::Value(Value::Record(vec![
        ("status".into(), Value::Number(crate::decimal::Decimal::from_int(status))),
        ("body".into(), Value::Text(body)), ("kind".into(), Value::Text(kind.into())), ("headers".into(), headers),
    ])))
}

const WAIT: std::time::Duration = std::time::Duration::from_secs(30);

fn request(method: &str, url: &str, body: Option<String>, headers: Vec<String>) -> Result<Outcome, String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) { return Err(format!("{url} isn't a web address.\nA web address starts with https:// or http://, like https://api.example.com/rates.")); }
    if !matches!(method, "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD") { return Err(format!("{method} isn't a way to send a web request.\nUse GET, POST, PUT, PATCH, DELETE or HEAD.")); }
    let agent = ureq::AgentBuilder::new().timeout(WAIT).build();
    let mut request = agent.request(method, url);
    for header in &headers { let Some((name, value)) = header.split_once(':') else { return Err(format!("The header {header:?} needs a name, a colon, and a value.\nA header looks like \"Authorization: Bearer KEY\".")); }; request = request.set(name.trim(), value.trim()); }
    let sent = match body { Some(body) => request.send_string(&body), None => request.call() };
    let answer = match sent { Ok(answer) | Err(ureq::Error::Status(_, answer)) => answer, Err(ureq::Error::Transport(_)) => return Ok(Outcome::Value(Value::Failure("NetworkDown".into()))) };
    let status = answer.status();
    let body = answer.into_string().unwrap_or_default();
    Ok(Outcome::Value(answer_record(status, body)))
}

pub fn answer_record(status: u16, body: String) -> Value {
    Value::Record(vec![("status".into(), Value::Number(crate::decimal::Decimal::from_int(i128::from(status)))), ("body".into(), Value::Text(body))])
}

fn place(permits: &Permits, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() { path.to_path_buf() } else { permits.folder.join(path) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn permits(list: &[(Permission, &str)]) -> Permits { Permits { list: list.iter().map(|(access, target)| (*access, target.to_string())).collect(), folder: PathBuf::new() } }
    #[test]
    fn a_folder_permit_covers_only_what_is_inside_it() {
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
    fn module_functions_need_database_permits() {
        let write = permits(&[(Permission::Write, "data")]);
        assert!(write.allows(Permission::Write, "data/app.sqlite"));
        assert!(!write.allows(Permission::Read, "data/app.sqlite"));
    }
}
