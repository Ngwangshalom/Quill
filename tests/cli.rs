//! End-to-end tests: run the real `quill` binary the way a user would.

use std::process::{Command, Output};

fn quill(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_quill"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("QUILL_NO_EDITOR_SETUP", "1")
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
    let output = quill(&["run", "tests/programs/hello.quill"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "Hello, World!\n");
}

#[test]
fn a_file_on_its_own_runs_it() {
    let output = quill(&["tests/programs/hello.quill"]);
    assert_eq!(stdout(&output), "Hello, World!\n");
}

#[test]
fn runs_values_and_escapes() {
    let output = quill(&["run", "tests/programs/values.quill"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "Hello from Quill\nHello from Quill\n0.10\nShe said \"hi\"\tand left.\n"
    );
}

#[test]
fn typo_gets_a_readme_style_error() {
    let output = quill(&["run", "tests/programs/typo.quill"]);
    assert_reported_problem(&output);
    assert_eq!(
        stdout(&output),
        "",
        "nothing should run when there are problems"
    );
    assert_eq!(
        stderr(&output),
        "Problem on line 2 of tests/programs/typo.quill:\n\
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
    let output = quill(&["check", "tests/programs/many_problems.quill"]);
    assert_reported_problem(&output);
    let err = stderr(&output);
    assert!(err.contains("Problem on line 1"));
    assert!(err.contains("Problem on line 3"));
    assert!(err.ends_with("Quill found 2 problems.\n"));
}

#[test]
fn lexer_errors_stop_before_running() {
    let output = quill(&["run", "tests/programs/unclosed.quill"]);
    assert_reported_problem(&output);
    assert!(stderr(&output).contains("never ends"));
}

#[test]
fn check_reports_success() {
    let output = quill(&["check", "tests/programs/values.quill"]);
    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        "No problems found in tests/programs/values.quill.\n"
    );
}

#[test]
fn tokens_shows_positions() {
    let output = quill(&["tokens", "tests/programs/hello.quill"]);
    assert_eq!(
        stdout(&output),
        "1:1     say\n1:5     Text \"Hello, World!\"\n1:20    Newline\n2:1     EndOfFile\n"
    );
}

#[test]
fn missing_file_is_explained() {
    let output = quill(&["run", "nowhere.quill"]);
    assert_reported_problem(&output);
    assert!(stderr(&output).contains("Quill couldn't read nowhere.quill"));
}

#[test]
fn wrong_extension_is_explained() {
    let output = quill(&["hello.txt"]);
    assert_reported_problem(&output);
    assert!(stderr(&output).contains("Did you mean:  hello.quill"));
}

#[test]
fn misspelled_command_gets_a_suggestion() {
    let output = quill(&["rnu", "hello.quill"]);
    assert_reported_problem(&output);
    assert!(stderr(&output).contains("Did you mean:  quill run hello.quill"));
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
    assert!(help.contains("quill run FILE.quill"));
    assert!(help.contains("quill test"));
    assert!(help.contains("quill explain FILE.quill:N"));
    assert!(help.contains("quill new NAME"));
    assert!(help.contains("check total == 38.50"));
    assert!(help.contains("--- example: price > 10"));
    assert!(help.contains("var total = price * 3"));
    assert!(help.contains("say \"Total: {total}\""));
    assert!(help.contains("total > 100 -> say \"Free delivery\""));
    assert!(help.contains("quill run shop/hello.quill"));
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
    assert!(out.contains("checks.quill: 3 checks passed"));
    assert!(out.contains("receipt.quill: 3 checks passed"));
    assert!(out.contains("bring/billing.quill: 1 check passed"), "{out}");
    assert!(out.ends_with("\n7 checks passed.\n"), "{out}");
}

#[test]
fn test_runs_the_checks_in_one_folder() {
    let programs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs");
    let output = quill_in(&programs, &["test", "bring"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("bring/billing.quill: 1 check passed"), "{out}");
    assert!(!out.contains("checks.quill"), "{out}");
    assert!(out.ends_with("\n1 check passed.\n"), "{out}");
}

#[test]
fn explain_describes_a_line() {
    let output = quill(&["explain", "tests/programs/checks.quill:1"]);
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
    assert!(stderr(&output).contains("file.quill:LINE"));
    assert!(stderr(&output).contains("Did you mean:  quill explain hello.quill:1"));
}

#[test]
fn a_missing_line_is_explained() {
    let output = quill(&["explain", "tests/programs/hello.quill:9"]);
    assert_reported_problem(&output);
    assert!(stderr(&output).contains("isn't there"));
}

#[test]
fn new_starts_a_project_that_runs_and_tests() {
    let root = scratch("new-project");
    let output = quill_in(&root, &["new", "shop"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("Created shop/hello.quill."));

    let output = quill_in(&root, &["run", "shop/hello.quill"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "Hello, World!\n");

    let output = quill_in(&root, &["test"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("shop/hello.quill: 1 check passed"));
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
    let file = root.join("shop.quill");
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
    let output = quill(&["run", "tests/programs/receipt.quill"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "Items: 3 x 12.50\nTotal: 37.80\nVisits: 1\nDelivery: 5\n"
    );
}

#[test]
fn adding_text_to_a_number_matches_the_readme() {
    let output = quill(&["run", "tests/programs/kind_mistake.quill"]);
    assert_reported_problem(&output);
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        "Problem on line 2 of tests/programs/kind_mistake.quill:\n\
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
    let output = quill(&["check", "tests/programs/not_var.quill"]);
    assert_reported_problem(&output);
    let err = stderr(&output);
    assert!(err.contains("name can't change, because it isn't marked var."));
    assert!(err.contains("    var name = \"Amina\""));
}

#[test]
fn dividing_by_zero_stops_the_program_with_an_explanation() {
    let output = quill(&["run", "tests/programs/divide_by_zero.quill"]);
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
fn upgrade_is_another_name_for_update() {
    for args in [
        &["upgrade"][..],
        &["upgrade", "latest"],
        &["update", "0.2.0"],
    ] {
        let output = quill(args);
        assert_reported_problem(&output);
        assert!(stderr(&output).contains("quill update can't replace it"));
    }
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
        .env("QUILL_NO_EDITOR_SETUP", "1")
        .output()
        .expect("quill binary should start")
}

fn scratch(name: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn targeted_tests_count_executed_assertions() {
    let dir = scratch("executed-checks");
    std::fs::write(dir.join("selected.quill"), "fn verify()\n  check 1 == 1\nverify()\nfor i in 1..3\n  check i > 0\nwhen\n  false -> check 1 == 2\n  else -> say \"ok\"\n--- example: 2 == 2\n").unwrap();
    std::fs::write(dir.join("other.quill"), "check 1 == 2\n").unwrap();
    let output = quill_in(&dir, &["test", "selected.quill"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "selected.quill: 5 checks passed\n\n5 checks passed.\n"
    );
    let output = quill_in(&dir, &["test"]);
    assert_reported_problem(&output);
    assert!(stdout(&output).contains("5 checks passed, 1 file failed."));
}

#[test]
fn targeted_tests_keep_passes_before_failure() {
    let dir = scratch("partial-checks");
    std::fs::write(
        dir.join("case.quill"),
        "check 1 == 1\ncheck 1 == 2\ncheck 2 == 2\n",
    )
    .unwrap();
    let output = quill_in(&dir, &["test", "case.quill"]);
    assert_reported_problem(&output);
    assert!(stdout(&output).contains("1 check passed, 1 file failed."));
}

#[test]
fn targeted_tests_validate_files_and_accept_zero_checks() {
    let dir = scratch("test-paths");
    std::fs::write(dir.join("empty.quill"), "say \"hello\"\n").unwrap();
    let output = quill_in(&dir, &["test", "empty.quill"]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("0 checks passed."));
    for args in [
        &["test", "missing.quill"][..],
        &["test", "empty.txt"],
        &["test", "a.quill", "b.quill"],
    ] {
        assert_reported_problem(&quill_in(&dir, args));
    }
}

#[test]
fn remainder_overflow_reports_a_problem_without_panicking() {
    let dir = scratch("remainder-overflow");
    std::fs::write(
        dir.join("case.quill"),
        "var n = -170141183460469231731687303715884105727 - 1\nsay n % -1\n",
    )
    .unwrap();
    let output = quill_in(&dir, &["run", "case.quill"]);
    assert_reported_problem(&output);
    assert!(!stderr(&output).contains("panicked"));
}

#[test]
fn bring_uses_the_functions_and_types_of_other_files() {
    let output = quill(&["run", "tests/programs/bring/app.quill"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "Amina pays 100 USD\n");
}

#[test]
fn a_missing_brought_file_suggests_the_closest_one() {
    let output = quill(&["run", "tests/programs/bring/typo.quill"]);
    assert_reported_problem(&output);
    let text = stderr(&output);
    assert!(text.contains("there's no file biling.quill"), "{text}");
    assert!(text.contains("Did you mean:  bring billing"), "{text}");
}

#[test]
fn a_problem_in_a_brought_file_shows_that_file() {
    let output = quill(&["run", "tests/programs/bring/uses_broken.quill"]);
    assert_reported_problem(&output);
    let text = stderr(&output);
    assert!(
        text.starts_with(
            "Problem on line 2 of tests/programs/bring/broken.quill:\n\n      return \"many\""
        ),
        "{text}"
    );
}

#[test]
fn files_and_settings_need_permits() {
    let output = quill(&["run", "tests/programs/files/rates.quill"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "Rates: USD 1\n\nno such file\ntrue\nnot set\n"
    );
}

#[test]
fn a_path_cannot_leave_its_permitted_folder() {
    let output = quill(&["run", "tests/programs/files/escape.quill"]);
    assert_reported_problem(&output);
    let text = stderr(&output);
    assert!(
        text.contains("read_file can't read config/../rates.quill"),
        "{text}"
    );
}

#[test]
fn try_on_its_own_line_explains_on() {
    let output = quill(&["check", "tests/programs/files/bare_try.quill"]);
    assert_reported_problem(&output);
    let text = stderr(&output);
    assert!(text.contains("try can't stand on a line by itself."), "{text}");
    assert!(text.contains("on CantWrite -> fail CantWrite"), "{text}");
    assert!(
        text.contains("Did you mean:  write_file(\"out/note.txt\", text)"),
        "{text}"
    );
    assert!(!text.contains("say try"), "{text}");
}

/// Answers each request on a port of this computer with `status` and `body`, and
/// echoes the request's own body back when it has one.
fn serve(status: &'static str, body: &'static str) -> u16 {
    serve_with(status, body, true)
}

fn serve_with(status: &'static str, body: &'static str, echo: bool) -> u16 {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap_or(0);
                }
            }
            let mut sent = vec![0; length];
            let _ = reader.read_exact(&mut sent);
            let reply = if echo && length > 0 {
                String::from_utf8_lossy(&sent).into_owned()
            } else {
                body.to_string()
            };
            let mut stream = stream;
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                reply.len()
            );
        }
    });
    port
}

fn run_program(source: &str) -> Output {
    let dir =
        std::env::temp_dir().join(format!("quill-cli-{}-{}", std::process::id(), source.len()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.quill"), source).unwrap();
    quill_in(&dir, &["run", "app.quill"])
}

#[test]
fn fetch_and_post_talk_to_a_web_service() {
    let port = serve("200 OK", r#"{"rate": 0.92}"#);
    let source = format!(
        "bring http\npermit net \"127.0.0.1\"\n\ntype Rate\n  rate: number\n\n\
         answer = fetch(\"http://127.0.0.1:{port}/rates\")\n  on NetworkDown -> say \"offline\"\n\
         say answer.status\nrate = answer.body as Rate\n  on Invalid -> Rate rate = 0\nsay rate.rate\n\
         reply = post(\"http://127.0.0.1:{port}/echo\", json(list 1, 2))\n  on NetworkDown -> say \"offline\"\n\
         say reply.body\n"
    );
    let output = run_program(&source);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "200\n0.92\n[1,2]\n");
}

#[test]
fn an_error_status_is_still_an_answer_and_no_host_is_network_down() {
    let port = serve("404 Not Found", "no such page");
    let source = format!(
        "bring http\npermit net \"127.0.0.1\"\n\
         answer = fetch(\"http://127.0.0.1:{port}/missing\")\n  on NetworkDown -> say \"offline\"\n\
         say \"{{answer.status}} {{answer.body}}\"\n\
         gone = fetch(\"http://127.0.0.1:1/\")\n  on NetworkDown -> say \"offline\"\nsay gone.status\n"
    );
    let output = run_program(&source);
    assert_eq!(
        stdout(&output),
        "404 no such page\noffline\n",
        "stderr: {}",
        stderr(&output)
    );
}

fn run_with_settings(source: &str, settings: &[(&str, String)]) -> Output {
    let dir =
        std::env::temp_dir().join(format!("quill-ai-{}-{}", std::process::id(), source.len()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.quill"), source).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_quill"));
    command
        .args(["run", "app.quill"])
        .current_dir(&dir)
        .env("QUILL_NO_EDITOR_SETUP", "1")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("QUILL_AI_MODEL");
    for (name, value) in settings {
        command.env(name, value);
    }
    command.output().expect("quill binary should start")
}

const ASKING: &str = "bring ai\npermit net \"127.0.0.1\"\npermit env \"ANTHROPIC_API_KEY\"\n";

#[test]
fn ask_gives_back_the_model_text() {
    let port = serve_with(
        "200 OK",
        r#"{"type":"message","stop_reason":"end_turn","content":[{"type":"text","text":"A short summary."}]}"#,
        false,
    );
    let source = format!(
        "{ASKING}summary = ask \"Summarize this.\"\n  on ModelFailed -> \"none\"\nsay summary\n"
    );
    let output = run_with_settings(
        &source,
        &[
            ("ANTHROPIC_API_KEY", "test".to_string()),
            ("ANTHROPIC_BASE_URL", format!("http://127.0.0.1:{port}")),
        ],
    );
    assert_eq!(
        stdout(&output),
        "A short summary.\n",
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn ask_as_a_type_checks_the_answer() {
    let good = serve_with(
        "200 OK",
        r#"{"type":"message","stop_reason":"end_turn","content":[{"type":"text","text":"{\"stars\": 4, \"note\": null}"}]}"#,
        false,
    );
    let broken = serve_with(
        "200 OK",
        r#"{"type":"message","stop_reason":"end_turn","content":[{"type":"text","text":"{\"stars\": 9, \"note\": null}"}]}"#,
        false,
    );
    let source = format!(
        "{ASKING}type Review\n  stars: number where 1..5\n  note: maybe text\n\
         review = ask \"Rate it.\" as Review\n  on ModelFailed -> Review stars = 1, note = missing\n  on Invalid -> Review stars = 3, note = missing\n\
         say review.stars\n"
    );
    for (port, wanted) in [(good, "4\n"), (broken, "3\n")] {
        let output = run_with_settings(
            &source,
            &[
                ("ANTHROPIC_API_KEY", "test".to_string()),
                ("ANTHROPIC_BASE_URL", format!("http://127.0.0.1:{port}")),
            ],
        );
        assert_eq!(stdout(&output), wanted, "stderr: {}", stderr(&output));
    }
}

#[test]
fn a_refusal_or_an_error_is_model_failed_and_a_missing_key_stops() {
    let refused = serve_with(
        "200 OK",
        r#"{"type":"message","stop_reason":"refusal","content":[]}"#,
        false,
    );
    let overloaded = serve_with(
        "529 Overloaded",
        r#"{"type":"error","error":{"type":"overloaded_error","message":"busy"}}"#,
        false,
    );
    let source =
        format!("{ASKING}answer = ask \"Hello\"\n  on ModelFailed -> \"no answer\"\nsay answer\n");
    for port in [refused, overloaded] {
        let output = run_with_settings(
            &source,
            &[
                ("ANTHROPIC_API_KEY", "test".to_string()),
                ("ANTHROPIC_BASE_URL", format!("http://127.0.0.1:{port}")),
            ],
        );
        assert_eq!(
            stdout(&output),
            "no answer\n",
            "stderr: {}",
            stderr(&output)
        );
    }
    let output = run_with_settings(&source, &[]);
    assert_reported_problem(&output);
    assert!(
        stderr(&output).contains("ask needs the setting ANTHROPIC_API_KEY, but it isn't set."),
        "{}",
        stderr(&output)
    );
}

#[test]
fn ask_works_with_an_openai_compatible_service() {
    let port = serve_with(
        "200 OK",
        r#"{"choices":[{"message":{"role":"assistant","content":"Hello from the service.","refusal":null}}]}"#,
        false,
    );
    let source = "bring ai\npermit net \"127.0.0.1\"\npermit env \"OPENAI_API_KEY\"\n\
                  answer = ask \"Hello\"\n  on ModelFailed -> \"none\"\nsay answer\n";
    let output = run_with_settings(
        source,
        &[
            ("OPENAI_API_KEY", "test".to_string()),
            ("OPENAI_BASE_URL", format!("http://127.0.0.1:{port}/v1")),
            ("QUILL_AI_MODEL", "local-model".to_string()),
        ],
    );
    assert_eq!(
        stdout(&output),
        "Hello from the service.\n",
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn a_program_with_routes_serves_them() {
    use std::io::{Read, Write};
    let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = free.local_addr().unwrap().port();
    drop(free);
    let dir = std::env::temp_dir().join(format!("quill-web-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.quill"),
        "bring web\nsay \"ready\"\nroute GET \"/hello/{name}\"\n  respond 200, \"Hello, {name}!\"\n",
    )
    .unwrap();
    let mut server = Command::new(env!("CARGO_BIN_EXE_quill"))
        .args(["run", "app.quill"])
        .current_dir(&dir)
        .env("QUILL_NO_EDITOR_SETUP", "1")
        .env("PORT", port.to_string())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let mut reply = String::new();
    for _ in 0..50 {
        if let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) {
            let _ = write!(
                stream,
                "GET /hello/Amina HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
            );
            let _ = stream.read_to_string(&mut reply);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = server.kill();
    let _ = server.wait();
    assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
    assert!(reply.ends_with("Hello, Amina!"), "{reply}");
}

#[test]
fn a_file_ending_in_ql_still_runs_with_a_note() {
    let output = quill(&["run", "tests/programs/old/hello.ql"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "still runs\n");
    let text = stderr(&output);
    assert!(
        text.contains("Note: Quill files now end in .quill. Rename tests/programs/old/hello.ql to tests/programs/old/hello.quill."),
        "{text}"
    );
}

#[test]
fn bring_still_finds_a_file_ending_in_ql() {
    let output = quill(&["run", "tests/programs/old/shop.quill"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "5 USD\n");
}

#[test]
fn a_file_with_another_ending_is_refused() {
    let output = quill(&["run", "notes.txt"]);
    assert_reported_problem(&output);
    let text = stderr(&output);
    assert!(text.contains("notes.txt doesn't end in .quill"), "{text}");
    assert!(text.contains("Did you mean:  notes.quill"), "{text}");
}

#[test]
fn the_sample_programs_in_the_repository_still_work() {
    for path in [
        "hello.quill",
        "examples/events/main.quill",
        "examples/myapi/server.quill",
        "examples/reviews/server.quill",
    ] {
        let output = quill(&["check", path]);
        assert!(output.status.success(), "{path}: {}", stderr(&output));
    }
}
