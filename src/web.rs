//! `bring web`: serving the program's routes.
//!
//! After the top of the program runs, `quill run` answers web requests with its
//! routes. Each request runs the route whose method and path match, with `request`
//! and each `{name}` of its path. A route that stops with a problem answers 500, and
//! the problem is shown where the server was started; the server keeps running.

use std::io::Write;

use crate::bring::{self, File};
use crate::compiler::Chunk;
use crate::parser::ROUTE;
use crate::value::Value;
use crate::vm;

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

/// The `request` a route gets.
pub fn request_record(method: &str, path: &str, query: &str, body: String) -> Value {
    Value::Record(vec![
        ("method".to_string(), Value::Text(method.to_string())),
        ("path".to_string(), Value::Text(path.to_string())),
        ("query".to_string(), Value::Text(query.to_string())),
        ("body".to_string(), Value::Text(body)),
    ])
}

/// What a route's answer says: status, body, and the type of the body.
fn response_parts(value: Option<Value>) -> Option<(u16, String, String)> {
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
    Some((status, text("body")?, text("kind")?))
}

/// Runs the route that fits a request, and gives back its status, body and type.
/// A problem while it runs is shown on `log` and answered with 500.
pub fn answer(
    chunk: &Chunk,
    routes: &[Route],
    method: &str,
    url: &str,
    body: String,
    files: &[File],
    log: &mut impl Write,
) -> (u16, String, String) {
    let text = "text/plain; charset=utf-8".to_string();
    let (path, query) = url.split_once('?').unwrap_or((url, ""));
    let (route, values) = match find(routes, method, path) {
        Found::Route(route, values) => (route, values),
        Found::WrongMethod => {
            return (405, format!("{path} doesn't answer {method}."), text);
        }
        Found::NotFound => return (404, format!("There's nothing at {path}."), text),
    };
    let mut args = vec![request_record(method, path, query, body)];
    args.extend(values.into_iter().map(Value::Text));
    let mut printed = Vec::new();
    let result = vm::call_function(chunk, route.function, args, &mut printed);
    let _ = log.write_all(&printed);
    match result.map(response_parts) {
        Ok(Some(parts)) => parts,
        Ok(None) => (500, "The route didn't give an answer.".to_string(), text),
        Err(problem) => {
            let _ = write!(log, "{}", bring::render(&[problem], files));
            (
                500,
                format!("The server stopped while answering {method} {path}."),
                text,
            )
        }
    }
}

/// Serves `chunk`'s routes on `port` until the program is stopped.
pub fn serve(chunk: &Chunk, port: u16, files: &[File]) -> Result<(), String> {
    let routes = routes(chunk);
    let server = tiny_http::Server::http(("0.0.0.0", port)).map_err(|err| {
        format!(
            "Quill couldn't serve on port {port}: {err}.\n\
             Another program may be using it. Set PORT to another number, like PORT=8081."
        )
    })?;
    println!(
        "Serving {} routes on http://localhost:{port}. Press Ctrl+C to stop.",
        routes.len()
    );
    let _ = std::io::stdout().flush();
    for mut request in server.incoming_requests() {
        let mut body = String::new();
        let _ = request.as_reader().read_to_string(&mut body);
        let method = request.method().as_str().to_uppercase();
        let url = request.url().to_string();
        let mut out = std::io::stdout();
        let (status, text, kind) = answer(chunk, &routes, &method, &url, body, files, &mut out);
        let _ = out.flush();
        let mut response = tiny_http::Response::from_string(text).with_status_code(status);
        if let Ok(header) = tiny_http::Header::from_bytes("Content-Type", kind.as_bytes()) {
            response = response.with_header(header);
        }
        let _ = request.respond(response);
    }
    Ok(())
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
        route GET \"/zero\"\n  n = 0\n  respond 200, 10 / n\n";

    #[test]
    fn a_request_finds_its_route() {
        let chunk = compiled(DESK);
        let routes = routes(&chunk);
        assert_eq!(routes.len(), 3);
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
        let mut ask = |method: &str, url: &str, body: &str| {
            answer(
                &chunk,
                &routes,
                method,
                url,
                body.to_string(),
                &[],
                &mut log,
            )
        };
        assert_eq!(ask("GET", "/hello/Omar?x=1", "").1, "Hello, Omar!");
        let made = ask("POST", "/bookings", r#"{"venue": "Hall", "guests": 120}"#);
        assert_eq!(
            (made.0, made.1.as_str(), made.2.as_str()),
            (201, r#"{"venue":"Hall","guests":120}"#, "application/json")
        );
        assert_eq!(
            ask("POST", "/bookings", r#"{"venue": "Hall", "guests": 900}"#).0,
            400
        );
        assert_eq!(ask("GET", "/nothing", "").0, 404);
        assert_eq!(ask("PUT", "/hello/x", "").0, 405);
        assert_eq!(ask("GET", "/zero", "").0, 500);
        assert!(String::from_utf8_lossy(&log).contains("This divides by zero"));
    }
}
