//! Runs every Quill example in the book (`book/src`), so the book can't drift
//! out of date as the language changes.
//!
//! In the book's Markdown:
//!
//! - ```` ```quill ```` must run without problems.
//! - ```` ```quill,error ```` must stop with a problem.
//! - ```` ```quill,ignore ```` isn't run (planned features, fragments).
//! - Add `file=NAME.ql` to choose the file name errors mention (default `example.ql`).
//! - An ```` ```output ```` block right after an example is exactly what it
//!   prints: what `say` shows, then any problem report.
//!
//! After an intended change to Quill's output, refresh the output blocks with
//!
//!     QUILL_BLESS=1 cargo test --test book
//!
//! and read the diff before committing: the book should still say what you mean.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Example {
    /// The line of the opening ```` ```quill ```` fence.
    line: usize,
    code: String,
    file_name: String,
    expect_error: bool,
    /// The expected output, and the lines of the output block's contents.
    output: Option<(String, std::ops::Range<usize>)>,
}

#[test]
fn every_example_in_the_book_works() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut chapters = Vec::new();
    collect_markdown(&root.join("book/src"), &mut chapters);
    chapters.sort();
    assert!(!chapters.is_empty(), "the book has no chapters");

    let bless = std::env::var_os("QUILL_BLESS").is_some();
    let scratch = std::env::temp_dir().join(format!("quill-book-{}", std::process::id()));
    let mut failures = Vec::new();
    let mut count = 0;

    for chapter in &chapters {
        let text = fs::read_to_string(chapter).unwrap();
        let examples = examples_in(&text);
        let mut blessed: Vec<(std::ops::Range<usize>, String)> = Vec::new();

        for example in &examples {
            count += 1;
            let dir = scratch.join(count.to_string());
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join(&example.file_name), &example.code).unwrap();
            let run = Command::new(env!("CARGO_BIN_EXE_quill"))
                .args(["run", &example.file_name])
                .current_dir(&dir)
                .output()
                .expect("quill should start");
            let printed = format!(
                "{}{}",
                String::from_utf8_lossy(&run.stdout),
                String::from_utf8_lossy(&run.stderr)
            );
            let place = format!(
                "{}:{}",
                chapter.strip_prefix(root).unwrap_or(chapter).display(),
                example.line
            );

            let wanted_code = if example.expect_error { 1 } else { 0 };
            if run.status.code() != Some(wanted_code) {
                let wanted = if example.expect_error {
                    "to stop with a problem"
                } else {
                    "to run without problems"
                };
                failures.push(format!(
                    "{place}: expected this example {wanted}, but it printed:\n{printed}"
                ));
                continue;
            }
            if let Some((expected, lines)) = &example.output {
                if bless {
                    blessed.push((lines.clone(), printed));
                } else if *expected != printed {
                    failures.push(format!(
                        "{place}: the output doesn't match the book.\n\
                         --- the book says:\n{expected}--- Quill printed:\n{printed}"
                    ));
                }
            }
        }

        if bless && !blessed.is_empty() {
            fs::write(chapter, replace_lines(&text, &blessed)).unwrap();
        }
    }
    let _ = fs::remove_dir_all(&scratch);

    assert!(count > 0, "the book has no Quill examples");
    assert!(
        failures.is_empty(),
        "{} of {count} book examples failed:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

fn collect_markdown(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_markdown(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }
}

/// The Quill examples in one chapter.
fn examples_in(text: &str) -> Vec<Example> {
    let lines: Vec<&str> = text.lines().collect();
    let mut examples = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some(info) = lines[i].trim_start().strip_prefix("```") else {
            i += 1;
            continue;
        };
        let start = i;
        let (body, end) = fenced_body(&lines, i);
        let mut words = info.split(',').map(str::trim);
        if words.next() != Some("quill") {
            i = end + 1;
            continue;
        }
        let attributes: Vec<&str> = words.collect();
        if attributes.contains(&"ignore") {
            i = end + 1;
            continue;
        }
        let file_name = attributes
            .iter()
            .find_map(|a| a.strip_prefix("file="))
            .unwrap_or("example.ql")
            .to_string();

        // An output block may follow, after blank lines.
        let mut next = end + 1;
        while next < lines.len() && lines[next].trim().is_empty() {
            next += 1;
        }
        let output = if lines.get(next).map(|l| l.trim()) == Some("```output") {
            let (expected, output_end) = fenced_body(&lines, next);
            i = output_end + 1;
            Some((expected, next + 1..output_end))
        } else {
            i = end + 1;
            None
        };

        examples.push(Example {
            line: start + 1,
            code: body,
            file_name,
            expect_error: attributes.contains(&"error"),
            output,
        });
    }
    examples
}

/// The text between the fence at `start` and its closing fence, and the
/// index of the closing fence.
fn fenced_body(lines: &[&str], start: usize) -> (String, usize) {
    let mut body = String::new();
    let mut i = start + 1;
    while i < lines.len() && lines[i].trim_start() != "```" {
        body.push_str(lines[i]);
        body.push('\n');
        i += 1;
    }
    (body, i)
}

/// `text` with each range of lines replaced by new contents.
/// The ranges must be in order and not overlap.
fn replace_lines(text: &str, replacements: &[(std::ops::Range<usize>, String)]) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::new();
    let mut next = replacements.iter().peekable();
    let mut i = 0;
    while i <= lines.len() {
        if let Some((range, new)) = next.next_if(|(range, _)| range.start == i) {
            out.push_str(new);
            i = range.end;
            continue;
        }
        let Some(line) = lines.get(i) else { break };
        out.push_str(line);
        out.push('\n');
        i += 1;
    }
    out
}
