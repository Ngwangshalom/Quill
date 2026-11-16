//! `bring web`: serving the program's routes.
//!
//! After the top of the program runs, `quill run` answers web requests with its
//! routes. Each request runs the route whose method and path match, with `request`
//! and each `{name}` of its path. A route that stops with a problem answers 500, and
//! the problem is shown where the server was started; the server keeps running.
//!
//! Many workers answer at once. A body larger than 1 MB is refused with 413. Each
//! request gets one log line. `allow origin` adds CORS headers on every answer.

use std::io::{Read, Write};
use std::sync::Mutex;
use std::time::Instant;

use crate::bring::{self, File};
use crate::compiler::Chunk;
use crate::parser::ROUTE;
use crate::value::Value;
use crate::vm;

/// How many requests may be answered at the same time.
const WORKERS: usize = 32;

/// The largest body a request may send, in bytes.
pub const MAX_BODY: usize = 1_048_576;

/// One route: its method, the parts of its path, and the function that answers it.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub method: String,
    pub path: String,
    parts: Vec<Part>,
    function: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum Part {
    /// A part that has to match exactly: `bookings`.
    Fixed(String),
    /// `{id}`: any part, given to the route as text.
    Named,
}

/// The answer a route, or the server itself, sends.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub status: u16,
    pub body: String,
    pub kind: String,
    /// Extra headers the route named, as `"Name: value"`.
    pub headers: Vec<String>,
}

/// The routes a compiled program has, in the order they were written.
pub fn routes(chunk: &Chunk) -> Vec<Route> {
    chunk
        .functions
        .iter()
        .enumerate()
        .filter_map(|(function, compiled)| {
            let (method, path) = compiled.name.strip_prefix(ROUTE)?.split_once(' ')?;
            Some(Route {
                method: method.to_string(),
                path: path.to_string(),
                parts: parts_of(path)
                    .map(|part| {
                        if part.starts_with('{') && part.ends_with('}') {
                            Part::Named
                        } else {
                            Part::Fixed(part.to_string())
                        }
                    })
                    .collect(),
                function,
            })
        })
        .collect()
}

fn parts_of(path: &str) -> impl Iterator<Item = &str> {
    path.split('/').filter(|part| !part.is_empty())
}

/// What a request gets: the route to run with the values of its path, or the status
/// to answer when no route fits.
#[derive(Debug, PartialEq)]
pub enum Found<'r> {
    Route(&'r Route, Vec<String>),
    /// A route has this path, but not for this method.
    WrongMethod,
    NotFound,
}

/// The route that answers `method` on `path` (the path without `?query`).
pub fn find<'r>(routes: &'r [Route], method: &str, path: &str) -> Found<'r> {
    let asked: Vec<&str> = parts_of(path).collect();
    let mut path_fits = false;
    for route in routes {
        if route.parts.len() != asked.len() {
            continue;
        }
        let mut values = Vec::new();
        let fits = route
            .parts
            .iter()
            .zip(&asked)
            .all(|(part, asked)| match part {
                Part::Fixed(fixed) => fixed == asked,
                Part::Named => {
                    values.push(decode(asked));
                    true
                }
            });
        if !fits {
            continue;
        }
        if route.method == method {
            return Found::Route(route, values);
        }
        path_fits = true;
    }
    if path_fits {
        Found::WrongMethod
    } else {
        Found::NotFound
    }
}

/// `%20` back to a space, and so on, in one part of a path.
fn decode(part: &str) -> String {
    let bytes = part.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&part[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A header name as a Quill field: lowercase, with `-` written as `_`.
pub fn header_field(name: &str) -> String {
    name.to_ascii_lowercase().replace('-', "_")
}

/// The headers of a request, as a record of text fields.
pub fn headers_record<'a>(headers: impl IntoIterator<Item = (&'a str, &'a str)>) -> Value {
    let mut fields = Vec::new();
    for (name, value) in headers {
        let field = header_field(name);
        if let Some((_, held)) = fields.iter_mut().find(|(held, _)| *held == field) {
            *held = Value::Text(value.to_string());
        } else {
            fields.push((field, Value::Text(value.to_string())));
        }
    }
    Value::Record(fields)
}

/// The `request` a route gets.
pub fn request_record(
    method: &str,
    path: &str,
    query: &str,
    body: String,
    headers: Value,
) -> Value {
    Value::Record(vec![
        ("method".to_string(), Value::Text(method.to_string())),
        ("path".to_string(), Value::Text(path.to_string())),
        ("query".to_string(), Value::Text(query.to_string())),
        ("body".to_string(), Value::Text(body)),
        ("headers".to_string(), headers),
    ])
}

fn plain(status: u16, body: String) -> Answer {
    Answer {
        status,
        body,
        kind: "text/plain; charset=utf-8".to_string(),
        headers: Vec::new(),
    }
}

/// What a route's answer says: status, body, type, and any headers it named.
fn response_parts(value: Option<Value>) -> Option<Answer> {
    let Some(Value::Record(fields)) = value else {
        return None;
    };
    let field = |name: &str| {
        fields
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value)
    };
    let status = match field("status") {
        Some(Value::Number(number)) => u16::try_from(number.whole()?).ok()?,
        _ => return None,
    };
    let text = |name: &str| match field(name) {
        Some(Value::Text(text)) => Some(text.clone()),
        _ => None,
    };
    let headers = match field("headers") {
        Some(Value::List(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::Text(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    Some(Answer {
        status,
        body: text("body")?,
        kind: text("kind")?,
        headers,
    })
}

/// Runs the route that fits a request, and gives back its answer.
/// A problem while it runs is shown on `log` and answered with 500.
pub fn answer(
    chunk: &Chunk,
    routes: &[Route],
    method: &str,
    url: &str,
    body: String,
    headers: Value,
    files: &[File],
    log: &mut impl Write,
) -> Answer {
    let (path, query) = url.split_once('?').unwrap_or((url, ""));
    let (route, values) = match find(routes, method, path) {
        Found::Route(route, values) => (route, values),
        Found::WrongMethod => {
            return plain(405, format!("{path} doesn't answer {method}."));
        }
        Found::NotFound => return plain(404, format!("There's nothing at {path}.")),
    };
    let mut args = vec![request_record(method, path, query, body, headers)];
    args.extend(values.into_iter().map(Value::Text));
    let mut printed = Vec::new();
    let result = vm::call_function(chunk, route.function, args, &mut printed);
    let _ = log.write_all(&printed);
    match result.map(response_parts) {
        Ok(Some(parts)) => parts,
        Ok(None) => plain(500, "The route didn't give an answer.".to_string()),
        Err(problem) => {
            let _ = write!(log, "{}", bring::render(&[problem], files));
            plain(
                500,
                format!("The server stopped while answering {method} {path}."),
            )
        }
    }
}

/// A port nothing is listening on yet, or `None` when Quill couldn't find one.
pub fn free_port() -> Option<u16> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|listener| listener.local_addr().ok())
        .map(|addr| addr.port())
}

/// Serves `chunk`'s routes on `port` until the program is stopped.
pub fn serve(chunk: &Chunk, port: u16, files: &[File]) -> Result<(), String> {
    let routes = routes(chunk);
    let server = tiny_http::Server::http(("0.0.0.0", port)).map_err(|err| {
        let hint = match free_port() {
            Some(open) if open != port => {
                format!("Another program is using it. Try a free one: PORT={open}.")
            }
            _ => "Another program may be using it. Set PORT to another number, like PORT=8081."
                .to_string(),
        };
        format!("Quill couldn't serve on port {port}: {err}.\n{hint}")
    })?;
    println!(
        "Serving {} routes on http://localhost:{port}. Press Ctrl+C to stop.",
        routes.len()
    );
    let _ = std::io::stdout().flush();
    let log = Mutex::new(std::io::stdout());
    let origins = &chunk.origins;
    std::thread::scope(|scope| {
        for _ in 0..WORKERS {
            scope.spawn(|| {
                while let Ok(request) = server.recv() {
                    handle(chunk, &routes, files, origins, &log, request);
                }
            });
        }
    });
    Ok(())
}

fn handle(
    chunk: &Chunk,
    routes: &[Route],
    files: &[File],
    origins: &[String],
    log: &Mutex<impl Write>,
    mut request: tiny_http::Request,
) {
    let started = Instant::now();
    let method = request.method().as_str().to_uppercase();
    let url = request.url().to_string();
    let (path, _) = url.split_once('?').unwrap_or((url.as_str(), ""));
    let request_headers: Vec<(String, String)> = request
        .headers()
        .iter()
        .map(|header| {
            (
                header.field.as_str().to_string(),
                header.value.as_str().to_string(),
            )
        })
        .collect();
    let origin = request_headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("origin"))
        .map(|(_, value)| value.as_str());
    let cors = cors_origin(origins, origin);

    if method == "OPTIONS" && !origins.is_empty() {
        let mut notes = Vec::new();
        reply(
            request,
            plain(204, String::new()),
            cors.as_deref(),
            &request_headers,
            true,
        );
        write_log(log, &mut notes, "OPTIONS", path, 204, started);
        return;
    }

    let body = match read_body(&mut request) {
        Ok(body) => body,
        Err(BodyError::TooLarge) => {
            let mut notes = Vec::new();
            let answered = plain(
                413,
                "The request is larger than 1 MB, so it wasn't read.\n\
                 Send less at a time."
                    .to_string(),
            );
            let status = answered.status;
            reply(request, answered, cors.as_deref(), &request_headers, false);
            write_log(log, &mut notes, &method, path, status, started);
            return;
        }
        Err(BodyError::Io) => {
            let mut notes = Vec::new();
            let answered = plain(400, "Quill couldn't read the request body.".to_string());
            let status = answered.status;
            reply(request, answered, cors.as_deref(), &request_headers, false);
            write_log(log, &mut notes, &method, path, status, started);
            return;
        }
    };

    let headers = headers_record(
        request_headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str())),
    );
    let mut notes = Vec::new();
    let answered = answer(
        chunk, routes, &method, &url, body, headers, files, &mut notes,
    );
    let status = answered.status;
    reply(request, answered, cors.as_deref(), &request_headers, false);
    write_log(log, &mut notes, &method, path, status, started);
}

enum BodyError {
    TooLarge,
    Io,
}

fn read_body(request: &mut tiny_http::Request) -> Result<String, BodyError> {
    if request.body_length().is_some_and(|len| len > MAX_BODY) {
        return Err(BodyError::TooLarge);
    }
    let mut limited = request.as_reader().take((MAX_BODY as u64) + 1);
    let mut bytes = Vec::new();
    limited.read_to_end(&mut bytes).map_err(|_| BodyError::Io)?;
    if bytes.len() > MAX_BODY {
        return Err(BodyError::TooLarge);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Which `Access-Control-Allow-Origin` value to send, if any.
pub fn cors_origin(allowed: &[String], origin: Option<&str>) -> Option<String> {
    let origin = origin?;
    if allowed.iter().any(|item| item == "*") {
        return Some("*".to_string());
    }
    allowed.iter().find(|item| item.as_str() == origin).cloned()
}

fn reply(
    request: tiny_http::Request,
    answer: Answer,
    cors: Option<&str>,
    request_headers: &[(String, String)],
    preflight: bool,
) {
    let mut response =
        tiny_http::Response::from_string(answer.body).with_status_code(answer.status);
    if let Ok(header) = tiny_http::Header::from_bytes(b"Content-Type", answer.kind.as_bytes()) {
        response = response.with_header(header);
    }
    for header in &answer.headers {
        if let Some((name, value)) = header.split_once(':') {
            if let Ok(header) =
                tiny_http::Header::from_bytes(name.trim().as_bytes(), value.trim().as_bytes())
            {
                response = response.with_header(header);
            }
        }
    }
    if let Some(origin) = cors {
        if let Ok(header) =
            tiny_http::Header::from_bytes(b"Access-Control-Allow-Origin", origin.as_bytes())
        {
            response = response.with_header(header);
        }
        if preflight {
            if let Ok(header) = tiny_http::Header::from_bytes(
                b"Access-Control-Allow-Methods",
                b"GET, POST, PUT, PATCH, DELETE, OPTIONS",
            ) {
                response = response.with_header(header);
            }
            let asked = request_headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("access-control-request-headers"))
                .map(|(_, value)| value.as_str())
                .unwrap_or("authorization, content-type");
            if let Ok(header) =
                tiny_http::Header::from_bytes(b"Access-Control-Allow-Headers", asked.as_bytes())
            {
                response = response.with_header(header);
            }
            if let Ok(header) = tiny_http::Header::from_bytes(b"Access-Control-Max-Age", b"86400") {
                response = response.with_header(header);
            }
        }
    }
    let _ = request.respond(response);
}

fn write_log(
    log: &Mutex<impl Write>,
    notes: &mut Vec<u8>,
    method: &str,
    path: &str,
    status: u16,
    started: Instant,
) {
    let ms = started.elapsed().as_millis();
    let line = format!("{method} {path} {status} {ms}ms\n");
    if let Ok(mut out) = log.lock() {
        let _ = out.write_all(notes);
        let _ = out.write_all(line.as_bytes());
        let _ = out.flush();
    }
}

/// The port to serve on: the setting PORT, or 8080.
pub fn port() -> Result<u16, String> {
    match std::env::var("PORT") {
        Err(_) => Ok(8080),
        Ok(text) => text.trim().parse().map_err(|_| {
            format!("PORT is {text:?}, which isn't a port number.\nA port is a whole number, like 8080.")
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compiled(source: &str) -> Chunk {
        crate::compile_source(source).unwrap()
    }

    const DESK: &str = "bring web\ntype Booking\n  venue: text\n  guests: number where 1..500\n\
        route GET \"/hello/{name}\"\n  respond 200, \"Hello, {name}!\"\n\
        route POST \"/bookings\"\n  booking = request.body as Booking\n    on Invalid -> respond 400, \"That isn't a booking.\"\n  respond 201, booking\n\
        route GET \"/zero\"\n  n = 0\n  respond 200, 10 / n\n\
        route GET \"/token\"\n  respond 200, request.headers.authorization\n\
        route GET \"/secret\"\n  respond 200, \"ok\", list \"X-Token: secret\"\n";

    #[test]
    fn a_request_finds_its_route() {
        let chunk = compiled(DESK);
        let routes = routes(&chunk);
        assert_eq!(routes.len(), 5);
        assert!(
            matches!(find(&routes, "GET", "/hello/Amina"), Found::Route(_, values) if values == ["Amina"])
        );
        assert!(
            matches!(find(&routes, "GET", "/hello/Amina%20N/"), Found::Route(_, values) if values == ["Amina N"])
        );
        assert_eq!(find(&routes, "DELETE", "/bookings"), Found::WrongMethod);
        assert_eq!(find(&routes, "GET", "/nothing"), Found::NotFound);
    }

    #[test]
    fn routes_answer_with_status_body_and_kind() {
        let chunk = compiled(DESK);
        let routes = routes(&chunk);
        let mut log = Vec::new();
        let empty = headers_record([]);
        let mut ask = |method: &str, url: &str, body: &str| {
            answer(
                &chunk,
                &routes,
                method,
                url,
                body.to_string(),
                empty.clone(),
                &[],
                &mut log,
            )
        };
        assert_eq!(ask("GET", "/hello/Omar?x=1", "").body, "Hello, Omar!");
        let made = ask("POST", "/bookings", r#"{"venue": "Hall", "guests": 120}"#);
        assert_eq!(made.status, 201);
        assert_eq!(made.body, r#"{"venue":"Hall","guests":120}"#);
        assert_eq!(made.kind, "application/json");
        assert_eq!(
            ask("POST", "/bookings", r#"{"venue": "Hall", "guests": 900}"#).status,
            400
        );
        assert_eq!(ask("GET", "/nothing", "").status, 404);
        assert_eq!(ask("PUT", "/hello/x", "").status, 405);
        assert_eq!(ask("GET", "/zero", "").status, 500);
        assert!(String::from_utf8_lossy(&log).contains("This divides by zero"));
    }

    #[test]
    fn a_route_reads_and_sends_headers() {
        let chunk = compiled(DESK);
        let routes = routes(&chunk);
        let mut log = Vec::new();
        let headers = headers_record([("Authorization", "Bearer secret")]);
        let got = answer(
            &chunk,
            &routes,
            "GET",
            "/token",
            String::new(),
            headers,
            &[],
            &mut log,
        );
        assert_eq!(got.body, "Bearer secret");
        let sent = answer(
            &chunk,
            &routes,
            "GET",
            "/secret",
            String::new(),
            headers_record([]),
            &[],
            &mut log,
        );
        assert_eq!(sent.headers, vec!["X-Token: secret".to_string()]);
    }

    #[test]
    fn header_names_become_quill_fields() {
        assert_eq!(header_field("Authorization"), "authorization");
        assert_eq!(header_field("Content-Type"), "content_type");
    }

    #[test]
    fn cors_picks_a_matching_origin() {
        assert_eq!(
            cors_origin(&["https://kopsala.app".into()], Some("https://kopsala.app")),
            Some("https://kopsala.app".into())
        );
        assert_eq!(
            cors_origin(&["https://kopsala.app".into()], Some("https://evil.test")),
            None
        );
        assert_eq!(
            cors_origin(&["*".into()], Some("https://anywhere.test")),
            Some("*".into())
        );
        assert_eq!(cors_origin(&["https://kopsala.app".into()], None), None);
    }

    #[test]
    fn a_busy_port_names_a_free_one() {
        // Hold the same kind of address the server binds, so the port is really taken.
        let held = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
        let busy = held.local_addr().unwrap().port();
        let chunk = compiled("bring web\nroute GET \"/hi\"\n  respond 200, \"hi\"\n");
        let problem = serve(&chunk, busy, &[]).unwrap_err();
        assert!(
            problem.contains(&format!("couldn't serve on port {busy}")),
            "{problem}"
        );
        let suggested = problem
            .split("PORT=")
            .nth(1)
            .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
            .and_then(|digits| digits.parse::<u16>().ok());
        let open = suggested.expect(&format!("no free PORT in: {problem}"));
        assert_ne!(open, busy);
        assert!(
            std::net::TcpListener::bind(("127.0.0.1", open)).is_ok(),
            "suggested PORT={open} isn't free"
        );
    }

    #[test]
    fn free_port_finds_one_that_binds() {
        let open = free_port().expect("a free port");
        assert!(std::net::TcpListener::bind(("127.0.0.1", open)).is_ok());
    }
}
