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
    let help = stdout(&quill(&["help"]));
    assert!(help.contains("quill run FILE.ql"));
    assert!(help.contains("quill test"));
    assert!(help.contains("quill explain FILE.ql:N"));
    assert!(help.contains("quill new NAME"));
    assert!(help.contains("check total == 38.50"));
    assert!(help.contains("--- example: price > 10"));
    assert!(help.contains("var total = price * 3"));
    assert!(help.contains("say \"Total: {total}\""));
    assert!(help.contains("total > 100 -> say \"Free delivery\""));
    assert!(help.contains("quill run shop/hello.ql"));
    assert!(help.contains("\"\"\"She said \"hi\".\"\"\""));
    assert_eq!(
        stdout(&quill(&["--version"])),
        format!("quill {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_reported_problem(&quill(&[]));
}

#[test]
fn test_runs_checks_and_examples() {
    // Run inside tests/programs, so files in the project folder don't change the count.
    let programs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs");
    let output = quill_in(&programs, &["test"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("checks.ql: 3 checks passed"));
    assert!(out.contains("receipt.ql: 3 checks passed"));
    assert!(out.ends_with("\n6 checks passed.\n"), "{out}");
}

#[test]
fn explain_describes_a_line() {
    let output = quill(&["explain", "tests/programs/checks.ql:1"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Line 1:  greeting = \"Hello\""));
    assert!(out.contains("a new value (can't change)"));
    assert!(out.contains("Text"));
}

#[test]
fn explain_needs_a_line() {
    let output = quill(&["explain"]);
    assert_reported_problem(&output);
    assert!(stderr(&output).contains("file.ql:LINE"));
    assert!(stderr(&output).contains("Did you mean:  quill explain hello.ql:1"));
}

#[test]
fn a_missing_line_is_explained() {
    let output = quill(&["explain", "tests/programs/hello.ql:9"]);
    assert_reported_problem(&output);
    assert!(stderr(&output).contains("isn't there"));
}

#[test]
fn new_starts_a_project_that_runs_and_tests() {
    let root = scratch("new-project");
    let output = quill_in(&root, &["new", "shop"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("Created shop/hello.ql."));

    let output = quill_in(&root, &["run", "shop/hello.ql"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "Hello, World!\n");

    let output = quill_in(&root, &["test"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("shop/hello.ql: 1 check passed"));
}

#[test]
fn new_wont_replace_a_folder() {
    let root = scratch("new-exists");
    assert!(quill_in(&root, &["new", "shop"]).status.success());
    let output = quill_in(&root, &["new", "shop"]);
    assert_reported_problem(&output);
    assert!(stderr(&output).contains("already exists"));
}

#[test]
fn a_failed_check_fails_the_test_run() {
    let root = scratch("failed-check");
    let file = root.join("shop.ql");
    std::fs::write(&file, "greeting = \"Hello\"\ncheck greeting == \"Hi\"\n").unwrap();
    let output = quill_in(&root, &["test"]);
    assert_reported_problem(&output);
    let err = stderr(&output);
    assert!(err.contains("This check failed"));
    assert!(err.contains("\"Hello\""));
    assert!(err.contains("\"Hi\""));
    assert!(stdout(&output).contains("0 checks passed, 1 file failed."));
}

#[test]
fn runs_arithmetic_text_values_var_set_and_when() {
    let output = quill(&["run", "tests/programs/receipt.ql"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "Items: 3 x 12.50\nTotal: 37.80\nVisits: 1\nDelivery: 5\n"
    );
}

#[test]
fn adding_text_to_a_number_matches_the_readme() {
    let output = quill(&["run", "tests/programs/kind_mistake.ql"]);
    assert_reported_problem(&output);
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        "Problem on line 2 of tests/programs/kind_mistake.ql:\n\
         \n    total = price + \"5\"\
         \n                    ^^^\
         \nYou're adding Number (price) to Text (\"5\").\
         \nQuill won't guess which one you meant.\
         \n\
         \nDid you mean:  total = price + 5\n"
    );
}

#[test]
fn set_needs_var() {
    let output = quill(&["check", "tests/programs/not_var.ql"]);
    assert_reported_problem(&output);
    let err = stderr(&output);
    assert!(err.contains("name can't change, because it isn't marked var."));
    assert!(err.contains("    var name = \"Amina\""));
}

#[test]
fn dividing_by_zero_stops_the_program_with_an_explanation() {
    let output = quill(&["run", "tests/programs/divide_by_zero.ql"]);
    assert_reported_problem(&output);
    assert_eq!(stdout(&output), "Before\n");
    let err = stderr(&output);
    assert!(err.contains("Problem on line 3"));
    assert!(err.contains("This divides by zero, which has no answer."));
}

#[test]
fn update_explains_how_to_update_a_copy_built_from_source() {
    let output = quill(&["update"]);
    assert_reported_problem(&output);
    let err = stderr(&output);
    assert!(err.contains("built from source with cargo, so quill update can't replace it"));
    assert!(err.contains("cargo install --path . --force"));
}

#[test]
fn update_is_suggested_for_a_typo() {
    let output = quill(&["upate"]);
    assert!(stderr(&output).contains("Did you mean:  quill update"));
}

fn quill_in(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_quill"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("quill binary should start")
}

fn scratch(name: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}
