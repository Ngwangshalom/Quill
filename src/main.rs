mod editor;
mod lsp;
mod update;

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use quill::compiler::Op;
use quill::diagnostic::{closest_match, Diagnostic};
use quill::explain;
use quill::lexer;

const USAGE: &str = "\
Quill: write what you mean.

Usage:
  quill run FILE.ql         Run a program
  quill FILE.ql             Same thing: a file on its own runs
  quill check FILE.ql       Find problems without running anything
  quill lsp                 Report those problems to an editor
  quill test                Run every check and doc example in this folder
  quill explain FILE.ql:N   Show how Quill read line N
  quill new NAME            Create a folder NAME with hello.ql inside
  quill tokens FILE.ql      Show how Quill reads a file, token by token
  quill update              Choose a version of Quill to install (also: upgrade)
  quill update VERSION      Install that version, or latest
  quill help                Show this message
  quill version             Show which version of Quill this is

A program is one statement per line:

  price = 12.50                  a value that can't change
  var total = price * 3          a value that can change
  set total = total + 1          change it
  say \"Total: {total}\"           show a value; {} puts a value inside text
  check total == 38.50           a test; quill test runs these
  --- example: price > 10        a doc example, also run by quill test
  -- a comment

Numbers are exact decimals, so 0.1 + 0.2 is 0.3. Calculate with + - * / %,
compare with == != < <= > >=, and combine conditions with and, or, not.

when runs the first case whose condition is true:

  when
    total > 100 -> say \"Free delivery\"
    else -> say \"Delivery: 5\"

A block is indented by exactly 2 spaces. A tab is a mistake.

for repeats the lines under it for each number from the first through the last,
or once for each item in a list:

  for i in 1..3
    say i

  hobbies = list \"coding\", \"reading\"
  for hobby in hobbies
    say hobby

get and sum ask a record, or a list of records, for data. for stays when each record
needs its own step. A missing amount is left out:

  sisters = record name = \"Nchang Sisters\", fee = 80000 USD
  brass = record name = \"Bamenda Brass\", fee = 150000 USD
  roster = list sisters, brass
  say sum fee of roster
  say get fee of roster

get place of desk.bookings reads a field of a list on a record.
get gpa of school.acceptances reads a field of a nested record.
Name the record before the dot: first = desk.bookings at 1, then first.place.
get bookings with acts in desk is the longer form.

A record keeps named fields together. A dot reads one field:

  address = record
    city = \"Anytown\"
  say address.city

A function names a piece of work. -> names the kind it gives back, and return
gives that value back. With no ->, call the function on its own line:

  fn double(n: number) -> number
    return n * 2
  say double(21)

  fn greet(name: text)
    say \"Hello, {name}\"
  greet(\"Amina\")

Money names its currency. 100 USD + 50 USD is 150 USD. 100 USD + 20 EUR
is a mistake. maybe means the answer might be missing:

  fn find_price(name: text) -> maybe number
    when
      name == \"bread\" -> return found 2.50
      else -> return missing

A function can fail, and the caller has to handle it:

  fn charge(amount: USD) -> text
    fails Declined
    when
      amount <= 0 USD -> fail Declined
      else -> return \"paid\"
  var receipt = charge(250 USD)
    on Declined -> say \"Card declined\"

A quote inside text needs no backslash when the text uses three quotes:

  \"\"\"She said \"hi\".\"\"\"

Quill checks for a new version at most once a day and tells you when one is
out. It never updates by itself. Set QUILL_NO_UPDATE_CHECK=1 to turn this off.

The first time Quill runs, it adds Quill's own colors and the feather icon
for .ql files to Cursor and VS Code, and underlines mistakes as you type.
The feather is pale on a dark editor and deep blue on a light one.
Reload the editor window to see them.
Set QUILL_NO_EDITOR_SETUP=1 to leave your editor alone.

Examples:
  quill new shop
  quill run shop/hello.ql
  quill explain shop/hello.ql:1
  quill test
";

const COMMANDS: [&str; 11] = [
    "run", "check", "lsp", "tokens", "test", "explain", "new", "update", "upgrade", "help",
    "version",
];

const STARTER: &str = "\
greeting = \"Hello, World!\"
say greeting

check greeting == \"Hello, World!\"
";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    match args.as_slice() {
        [update::CHECK_COMMAND] => return update::background_check(),
        ["lsp"] => return lsp::serve(),
        ["update" | "upgrade"] => {
            let code = update::update(None);
            editor::install();
            return code;
        }
        ["update" | "upgrade", version] => {
            let code = update::update(Some(version));
            editor::install();
            return code;
        }
        _ => {}
    }
    let code = command(&args);
    editor::install();
    update::notice();
    code
}

fn command(args: &[&str]) -> ExitCode {
    match args {
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
        ["test"] => test(),
        ["explain", target] => explain_file(target),
        ["new", name] => new_project(name),
        ["test", ..] => fail(
            "quill test looks through the project for checks and examples.\n\
             It doesn't take a file. Run it from the project folder."
                .to_string(),
            Some("quill test".to_string()),
        ),
        ["explain"] => fail(
            "quill explain needs a file and a line number, written as file.ql:LINE.".to_string(),
            Some("quill explain hello.ql:1".to_string()),
        ),
        ["new"] => fail(
            "quill new needs a name for the project.".to_string(),
            Some("quill new hello".to_string()),
        ),
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

fn test() -> ExitCode {
    let root = match env::current_dir() {
        Ok(root) => root,
        Err(err) => {
            return fail(
                format!(
                    "Quill couldn't see the current folder: {err}.\n\
                     Run quill test from the project folder."
                ),
                None,
            );
        }
    };
    let mut files = Vec::new();
    if let Err(message) = collect_tests(&root, &mut files) {
        return fail(message, None);
    }
    files.sort();
    if files.is_empty() {
        println!("Quill found no checks or examples.");
        println!("A check looks like:  check greeting == \"Hello\"");
        println!("A doc example looks like:  --- example: greeting == \"Hello\"");
        return ExitCode::SUCCESS;
    }

    let mut passed = 0;
    let mut failed = 0;
    for path in &files {
        let shown = relative(path, &root);
        let Some(source) = read(path.to_str().unwrap_or(&shown)) else {
            failed += 1;
            continue;
        };
        match quill::compile_source(&source) {
            Err(problems) => {
                eprintln!("{shown}");
                report(&shown, &source, &problems);
                failed += 1;
            }
            Ok(chunk) => {
                let checks = chunk
                    .code
                    .iter()
                    .filter(|op| matches!(op, Op::Check { .. }))
                    .count();
                let mut sink = Vec::new();
                match quill::vm::run(&chunk, &mut sink) {
                    Ok(()) => {
                        let word = if checks == 1 { "check" } else { "checks" };
                        println!("{shown}: {checks} {word} passed");
                        passed += checks;
                    }
                    Err(problem) => {
                        eprintln!("{shown}");
                        report(&shown, &source, &[problem]);
                        failed += 1;
                    }
                }
            }
        }
    }

    println!();
    if failed == 0 {
        let word = if passed == 1 { "check" } else { "checks" };
        println!("{passed} {word} passed.");
        ExitCode::SUCCESS
    } else {
        let passed_word = if passed == 1 { "check" } else { "checks" };
        let failed_word = if failed == 1 { "file" } else { "files" };
        println!("{passed} {passed_word} passed, {failed} {failed_word} failed.");
        ExitCode::FAILURE
    }
}

fn explain_file(target: &str) -> ExitCode {
    let Some((path, line_text)) = target.rsplit_once(':') else {
        return fail(
            "quill explain needs a file and a line number, written as file.ql:LINE.".to_string(),
            Some(format!("quill explain {target}:1")),
        );
    };
    if path.is_empty() {
        return fail(
            "quill explain needs a file before the line number.".to_string(),
            Some("quill explain hello.ql:1".to_string()),
        );
    }
    let Ok(line) = line_text.parse::<usize>() else {
        return fail(
            format!(
                "{line_text} isn't a line number.\n\
                 A line number is a whole number, like 1."
            ),
            Some(format!("quill explain {path}:1")),
        );
    };
    if line == 0 {
        return fail(
            "Line numbers start at 1.\n\
             There is no line 0."
                .to_string(),
            Some(format!("quill explain {path}:1")),
        );
    }
    let Some(source) = read(path) else {
        return ExitCode::FAILURE;
    };
    let count = if source.is_empty() {
        0
    } else {
        source.lines().count()
    };
    if line > count {
        let word = if count == 1 { "line" } else { "lines" };
        return fail(
            format!(
                "{path} has {count} {word}, so line {line} isn't there.\n\
                 Pick a line that exists."
            ),
            if count == 0 {
                None
            } else {
                Some(format!("quill explain {path}:1"))
            },
        );
    }
    print!("{}", explain::explain(&source, line));
    ExitCode::SUCCESS
}

fn new_project(name: &str) -> ExitCode {
    if let Some((message, suggestion)) = invalid_project_name(name) {
        return fail(message, suggestion);
    }
    let dir = Path::new(name);
    if dir.exists() {
        return fail(
            format!(
                "{name} already exists.\n\
                 Quill won't replace it. Pick another name, or delete that folder if you mean to start over."
            ),
            None,
        );
    }
    if let Err(err) = fs::create_dir(dir) {
        return fail(
            format!(
                "Quill couldn't create {name}: {err}.\n\
                 Check that you're allowed to create a folder here."
            ),
            None,
        );
    }
    let file = dir.join("hello.ql");
    if let Err(err) = fs::write(&file, STARTER) {
        let _ = fs::remove_dir_all(dir);
        return fail(
            format!(
                "Quill created {name} but couldn't write hello.ql: {err}.\n\
                 Check that you're allowed to create files in that folder."
            ),
            None,
        );
    }
    println!("Created {name}/hello.ql.");
    println!("Run it with:  quill run {name}/hello.ql");
    println!("Test it with:  quill test");
    ExitCode::SUCCESS
}

fn invalid_project_name(name: &str) -> Option<(String, Option<String>)> {
    if let Some(stem) = name.strip_suffix(".ql") {
        return Some((
            format!(
                "{name} looks like a source file, and quill new starts a project folder.\n\
                 The name is the folder, not a .ql file."
            ),
            Some(format!("quill new {stem}")),
        ));
    }
    let ok = !name.is_empty()
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-');
    if ok {
        return None;
    }
    Some((
        format!(
            "{name} isn't a project name Quill can make a folder from.\n\
             Use letters, digits, _ and -, and start with a letter."
        ),
        Some("quill new hello".to_string()),
    ))
}

/// Files under `dir` that contain a `check` or a `--- example:` line.
fn collect_tests(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|err| {
        format!(
            "Quill couldn't look through {}: {err}.\n\
             Check that the folder exists and that you're allowed to read it.",
            dir.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| {
            format!(
                "Quill couldn't read an entry in {}: {err}.\n\
                 Check that you're allowed to read that folder.",
                dir.display()
            )
        })?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "target" {
            continue;
        }
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|err| format!("Quill couldn't tell what {} is: {err}.", path.display()))?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            collect_tests(&path, files)?;
        } else if name.ends_with(".ql") && file_has_tests(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn file_has_tests(path: &Path) -> bool {
    let Ok(source) = fs::read_to_string(path) else {
        return true;
    };
    source.lines().any(is_test_line)
}

fn is_test_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed == "check" || trimmed.starts_with("check ") || trimmed.starts_with("check\t") {
        return true;
    }
    let Some(rest) = trimmed.strip_prefix("---") else {
        return false;
    };
    rest.trim_start().starts_with("example:")
}

/// Shows `path` relative to `root`, with `/` between folders on every system,
/// so `quill test` prints the same thing on Windows as on macOS and Linux.
fn relative(path: &Path, root: &Path) -> String {
    let shown = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
    if cfg!(windows) {
        shown.replace('\\', "/")
    } else {
        shown.into_owned()
    }
}

fn report(path: &str, source: &str, problems: &[Diagnostic]) {
    let mut text = String::new();
    for (i, problem) in problems.iter().enumerate() {
        if i > 0 {
            text.push('\n');
        }
        text.push_str(&problem.render(path, source));
    }
    if problems.len() > 1 {
        let _ = write!(text, "\nQuill found {} problems.\n", problems.len());
    }
    eprint!("{text}");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_use_forward_slashes_everywhere() {
        let root = Path::new("project");
        let file = root.join("tests").join("programs").join("checks.ql");
        assert_eq!(relative(&file, root), "tests/programs/checks.ql");
    }
}
