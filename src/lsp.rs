//! `quill lsp` speaks the Language Server Protocol on standard input and output.
//!
//! Cursor, VS Code, Vim, Neovim, Emacs, Helix, Zed, and Sublime can all use it.
//! Quill checks the file the same way `quill check` does, and sends each problem
//! back so the editor can underline it. The program is not run.

use std::io::{BufRead, Write};
use std::process::ExitCode;

use serde_json::{json, Value};

use quill::compile_file;
use quill::diagnostic::Diagnostic;

const MAX_MESSAGE: usize = 2_000_000;

/// Serve one editor session until the editor closes the input or sends `exit`.
pub fn serve() -> ExitCode {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    serve_with(&mut input, &mut output);
    ExitCode::SUCCESS
}

fn serve_with(input: &mut impl BufRead, output: &mut impl Write) {
    while let Some(body) = read_message(input) {
        let Ok(message) = serde_json::from_slice::<Value>(&body) else {
            continue;
        };
        if dispatch(&message, output) {
            break;
        }
    }
}

/// Handle one message. Returns true when the editor asked Quill to exit.
fn dispatch(message: &Value, output: &mut impl Write) -> bool {
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    match method {
        "initialize" => {
            reply(
                output,
                message.get("id").cloned(),
                json!({ "capabilities": { "textDocumentSync": 1 } }),
            );
        }
        "shutdown" => {
            reply(output, message.get("id").cloned(), Value::Null);
        }
        "exit" => return true,
        "textDocument/didOpen" => {
            publish(output, &message["params"]["textDocument"]);
        }
        "textDocument/didChange" => {
            let uri = message["params"]["textDocument"]["uri"]
                .as_str()
                .unwrap_or("");
            let text = message["params"]["contentChanges"]
                .as_array()
                .and_then(|changes| changes.last())
                .and_then(|change| change.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("");
            write_diagnostics(output, uri, text);
        }
        "textDocument/didClose" => {
            let uri = message["params"]["textDocument"]["uri"]
                .as_str()
                .unwrap_or("");
            if !uri.is_empty() {
                send(
                    output,
                    json!({
                        "jsonrpc": "2.0",
                        "method": "textDocument/publishDiagnostics",
                        "params": { "uri": uri, "diagnostics": [] }
                    }),
                );
            }
        }
        _ if message.get("id").is_some() => {
            send(
                output,
                json!({
                    "jsonrpc": "2.0",
                    "id": message.get("id"),
                    "error": { "code": -32601, "message": "Quill doesn't handle that request." }
                }),
            );
        }
        _ => {}
    }
    false
}

fn publish(output: &mut impl Write, document: &Value) {
    let uri = document["uri"].as_str().unwrap_or("");
    let text = document["text"].as_str().unwrap_or("");
    write_diagnostics(output, uri, text);
}

fn write_diagnostics(output: &mut impl Write, uri: &str, source: &str) {
    if uri.is_empty() {
        return;
    }
    send(
        output,
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": uri,
                "diagnostics": diagnostics_for(uri, source)
            }
        }),
    );
}

fn diagnostics_for(uri: &str, source: &str) -> Vec<Value> {
    // The document's own path, so `bring billing` finds billing.ql next to it.
    let path = path_of(uri);
    let (result, files) = compile_file(&path, source);
    let problems = match result {
        Ok(_) => Vec::new(),
        Err(problems) => problems,
    };
    problems
        .iter()
        .map(|problem| {
            if problem.file == 0 {
                return diagnostic_json(source, problem);
            }
            // A problem in a brought file is shown on this file's first bring line.
            let brought = files
                .get(problem.file)
                .map_or("", |file| file.path.as_str());
            let line = source
                .lines()
                .position(|line| line.trim_start().starts_with("bring"))
                .map_or(1, |index| index + 1);
            let moved = Diagnostic {
                file: 0,
                line,
                column: 1,
                length: 5,
                message: format!("In {brought}, line {}: {}", problem.line, problem.message),
                suggestion: problem.suggestion.clone(),
            };
            diagnostic_json(source, &moved)
        })
        .collect()
}

/// The file path in a `file://` URI, with `%20` and similar escapes undone.
fn path_of(uri: &str) -> String {
    let Some(path) = uri.strip_prefix("file://") else {
        return "example.ql".to_string();
    };
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&path[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    let decoded = String::from_utf8_lossy(&out).into_owned();
    // On Windows, file:///C:/x arrives as /C:/x.
    if cfg!(windows) {
        decoded.trim_start_matches('/').to_string()
    } else {
        decoded
    }
}

fn diagnostic_json(source: &str, problem: &Diagnostic) -> Value {
    let mut message = problem.message.clone();
    if let Some(suggestion) = &problem.suggestion {
        message.push_str("\n\nDid you mean:  ");
        message.push_str(suggestion);
    }
    let start_chars = problem.column.saturating_sub(1);
    let end_chars = start_chars.saturating_add(problem.length.max(1));
    json!({
        "range": {
            "start": position(source, problem.line, start_chars),
            "end": position(source, problem.line, end_chars)
        },
        "severity": 1,
        "source": "quill",
        "message": message
    })
}

/// LSP positions are 0-based, and the column counts UTF-16 code units.
fn position(source: &str, line: usize, char_index: usize) -> Value {
    let line_text = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
    let character: usize = line_text
        .chars()
        .take(char_index)
        .map(char::len_utf16)
        .sum();
    json!({
        "line": line.saturating_sub(1),
        "character": character
    })
}

fn reply(output: &mut impl Write, id: Option<Value>, result: Value) {
    let Some(id) = id else {
        return;
    };
    send(
        output,
        json!({ "jsonrpc": "2.0", "id": id, "result": result }),
    );
}

fn send(output: &mut impl Write, message: Value) {
    let Ok(body) = serde_json::to_vec(&message) else {
        return;
    };
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    if output.write_all(header.as_bytes()).is_ok() && output.write_all(&body).is_ok() {
        let _ = output.flush();
    }
}

fn read_message(input: &mut impl BufRead) -> Option<Vec<u8>> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            length = rest.trim().parse::<usize>().ok();
        }
    }
    let length = length.filter(|length| *length <= MAX_MESSAGE)?;
    let mut body = vec![0; length];
    input.read_exact(&mut body).ok()?;
    Some(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn frame(value: Value) -> Vec<u8> {
        let body = serde_json::to_vec(&value).unwrap();
        let mut bytes = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        bytes.extend(body);
        bytes
    }

    fn messages_from(bytes: &[u8]) -> Vec<Value> {
        let mut input = Cursor::new(bytes);
        let mut messages = Vec::new();
        while let Some(body) = read_message(&mut input) {
            messages.push(serde_json::from_slice(&body).unwrap());
        }
        messages
    }

    #[test]
    fn an_unknown_name_is_underlined() {
        let mut input = frame(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }));
        input.extend(frame(json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": "file:///shop.ql", "text": "say missing" } }
        })));
        let mut output = Vec::new();
        serve_with(&mut Cursor::new(input), &mut output);
        let messages = messages_from(&output);

        assert!(messages.iter().any(|message| {
            message["id"] == 1 && message["result"]["capabilities"]["textDocumentSync"] == 1
        }));
        let published = messages
            .iter()
            .find(|message| message["method"] == "textDocument/publishDiagnostics")
            .expect("an underline");
        assert_eq!(published["params"]["uri"], "file:///shop.ql");
        let diagnostic = &published["params"]["diagnostics"][0];
        assert_eq!(diagnostic["range"]["start"]["line"], 0);
        assert!(diagnostic["message"].as_str().unwrap().contains("missing"));
    }

    #[test]
    fn a_correct_program_clears_underlines() {
        assert!(diagnostics_for("file:///tmp/hello.ql", "say \"Hello\"\n").is_empty());
    }

    #[test]
    fn a_character_outside_the_basic_plane_counts_as_two_utf16_units() {
        let start = position("🪶x", 1, 1);
        assert_eq!(start["line"], 0);
        assert_eq!(start["character"], 2);
    }
}
