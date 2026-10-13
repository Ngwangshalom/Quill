//! End-to-end tests: run the real `quill` binary the way a user would.

use std::process::{Command, Output};

fn quill(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_quill"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("quill binary should start")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Exit code 1 means Quill reported a problem; 101 would mean it panicked.
fn assert_reported_problem(output: &Output) {
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(output));
}

#[test]
fn runs_hello_world() {
    let output = quill(&["run", "tests/programs/hello.ql"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "Hello, World!\n");
}

#[test]
fn a_file_on_its_own_runs_it() {
    let output = quill(&["tests/programs/hello.ql"]);
    assert_eq!(stdout(&output), "Hello, World!\n");
}

#[test]
fn runs_values_and_escapes() {
    let output = quill(&["run", "tests/programs/values.ql"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "Hello from Quill\nHello from Quill\n0.10\nShe said \"hi\"\tand left.\n"
    );
}

#[test]
fn runs_the_example_in_the_project_root() {
    let output = quill(&["run", "hello.ql"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
}

#[test]
fn typo_gets_a_readme_style_error() {
    let output = quill(&["run", "tests/programs/typo.ql"]);
    assert_reported_problem(&output);
    assert_eq!(
        stdout(&output),
        "",
        "nothing should run when there are problems"
    );
    assert_eq!(
        stderr(&output),
        "Problem on line 2 of tests/programs/typo.ql:\n\
         \n    say greting\
         \n        ^^^^^^^\
         \nThere's no value called greting.\
         \nValues must be created with greting = ... before they can be used.\
         \n\
         \nDid you mean:  say greeting\n"
    );
}

#[test]
fn reports_every_problem() {
    let output = quill(&["check", "tests/programs/many_problems.ql"]);
    assert_reported_problem(&output);
    let err = stderr(&output);
    assert!(err.contains("Problem on line 1"));
    assert!(err.contains("Problem on line 3"));
    assert!(err.ends_with("Quill found 2 problems.\n"));
}

#[test]
fn lexer_errors_stop_before_running() {
    let output = quill(&["run", "tests/programs/unclosed.ql"]);
    assert_reported_problem(&output);
    assert!(stderr(&output).contains("never ends"));
}

#[test]
fn check_reports_success() {
    let output = quill(&["check", "tests/programs/values.ql"]);
    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        "No problems found in tests/programs/values.ql.\n"
    );
}

#[test]
fn tokens_shows_positions() {
    let output = quill(&["tokens", "tests/programs/hello.ql"]);
    assert_eq!(
        stdout(&output),
        "1:1     say\n1:5     Text \"Hello, World!\"\n1:20    Newline\n2:1     EndOfFile\n"
    );
}

#[test]
fn missing_file_is_explained() {
    let output = quill(&["run", "nowhere.ql"]);
    assert_reported_problem(&output);
    assert!(stderr(&output).contains("Quill couldn't read nowhere.ql"));
}

#[test]
fn wrong_extension_is_explained() {
    let output = quill(&["hello.txt"]);
    assert_reported_problem(&output);
    assert!(stderr(&output).contains("Did you mean:  hello.ql"));
}

#[test]
fn misspelled_command_gets_a_suggestion() {
    let output = quill(&["rnu", "hello.ql"]);
    assert_reported_problem(&output);
    assert!(stderr(&output).contains("Did you mean:  quill run hello.ql"));
}

#[test]
fn command_without_file_is_explained() {
    let output = quill(&["run"]);
    assert_reported_problem(&output);
    assert!(stderr(&output).contains("quill run needs a file"));
}

#[test]
fn help_and_version() {
    assert!(stdout(&quill(&["help"])).contains("quill run FILE.ql"));
    assert_eq!(
        stdout(&quill(&["--version"])),
        format!("quill {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_reported_problem(&quill(&[]));
}
