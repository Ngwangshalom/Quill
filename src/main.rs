use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;

use quill::diagnostic::{closest_match, Diagnostic};
use quill::lexer;

const USAGE: &str = "\
Quill: write what you mean.

Usage:
  quill run FILE.ql      Run a program
  quill check FILE.ql    Find problems without running anything
  quill tokens FILE.ql   Show how Quill reads a file, token by token
  quill help             Show this message
  quill version          Show which version of Quill this is

A file on its own runs it:  quill hello.ql
";

const COMMANDS: [&str; 5] = ["run", "check", "tokens", "help", "version"];

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    match args.as_slice() {
        [] => {
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
        ["help" | "--help" | "-h"] => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        ["version" | "--version" | "-V"] => {
            println!("quill {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        // A lone argument that looks like a file is run, so `quill hello.ql` works.
        [path] if path.contains(['.', '/', '\\']) => run(path),
        ["run", path] => run(path),
        ["check", path] => check(path),
        ["tokens", path] => tokens(path),
        [command @ ("run" | "check" | "tokens")] => fail(
            format!("quill {command} needs a file to work on."),
            Some(format!("quill {command} hello.ql")),
        ),
        [command, rest @ ..] if !COMMANDS.contains(command) => {
            let suggestion = closest_match(command, COMMANDS).map(|fixed| {
                let mut line = vec!["quill", fixed];
                line.extend_from_slice(rest);
                line.join(" ")
            });
            fail(
                format!(
                    "Quill doesn't have a command called {command}.\n\
                     Run quill help to see what it can do."
                ),
                suggestion,
            )
        }
        [command, ..] => fail(
            format!(
                "quill {command} was given more than it needs.\n\
                 Each command works on one file at a time."
            ),
            None,
        ),
    }
}

fn run(path: &str) -> ExitCode {
    let Some(source) = read(path) else {
        return ExitCode::FAILURE;
    };
    let mut stdout = io::stdout().lock();
    let result = quill::run_source(&source, &mut stdout);
    let _ = stdout.flush();
    finish(path, &source, result)
}

fn check(path: &str) -> ExitCode {
    let Some(source) = read(path) else {
        return ExitCode::FAILURE;
    };
    let result = quill::compile_source(&source).map(|_| ());
    if result.is_ok() {
        println!("No problems found in {path}.");
    }
    finish(path, &source, result)
}

fn tokens(path: &str) -> ExitCode {
    let Some(source) = read(path) else {
        return ExitCode::FAILURE;
    };
    let result = lexer::lex(&source).map(|tokens| {
        for token in tokens {
            let position = format!("{}:{}", token.line, token.column);
            println!("{position:<8}{}", token.kind);
        }
    });
    finish(path, &source, result.map_err(|problem| vec![problem]))
}

/// Prints any problems and turns the result into an exit code.
fn finish(path: &str, source: &str, result: Result<(), Vec<Diagnostic>>) -> ExitCode {
    let Err(problems) = result else {
        return ExitCode::SUCCESS;
    };
    let mut report = String::new();
    for (i, problem) in problems.iter().enumerate() {
        if i > 0 {
            report.push('\n');
        }
        report.push_str(&problem.render(path, source));
    }
    if problems.len() > 1 {
        let _ = write!(report, "\nQuill found {} problems.\n", problems.len());
    }
    eprint!("{report}");
    ExitCode::FAILURE
}

fn read(path: &str) -> Option<String> {
    if !path.ends_with(".ql") {
        let stem = path.rsplit_once('.').map_or(path, |(stem, _)| stem);
        fail(
            format!(
                "{path} doesn't end in .ql, so it doesn't look like Quill code.\n\
                 Quill source files use the .ql extension."
            ),
            Some(format!("{stem}.ql")),
        );
        return None;
    }
    match fs::read_to_string(path) {
        Ok(source) => Some(source),
        Err(err) => {
            fail(
                format!(
                    "Quill couldn't read {path}: {err}.\n\
                     Check that the file exists and that the path is spelled correctly."
                ),
                None,
            );
            None
        }
    }
}

/// Reports a problem that isn't tied to a line of code.
fn fail(message: String, suggestion: Option<String>) -> ExitCode {
    eprintln!("Problem: {message}");
    if let Some(suggestion) = suggestion {
        eprintln!("\nDid you mean:  {suggestion}");
    }
    ExitCode::FAILURE
}
