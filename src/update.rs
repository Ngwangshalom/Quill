//! Keeping Quill up to date: `quill update`, and the notice that a new
//! version is available.
//!
//! At most once a day, `quill` asks GitHub for the latest release in a
//! background process and remembers the answer in `~/.quill/update-check`.
//! When a newer version exists, commands end with a one-line notice. Nothing is
//! downloaded or changed until you run `quill update`.
//!
//! The check never slows a command down: it runs in a separate process, and
//! the notice uses the answer remembered from an earlier check. It is skipped
//! when offline, when `curl` isn't installed, when output isn't going to a
//! terminal, on CI, and whenever `QUILL_NO_UPDATE_CHECK` is set.

use std::env;
use std::fs;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// The hidden command the background check runs as.
pub const CHECK_COMMAND: &str = "__update-check";

const CURRENT: &str = env!("CARGO_PKG_VERSION");
const LATEST_RELEASE: &str = "https://api.github.com/repos/Ngwangshalom/Quill/releases/latest";
const DAY_IN_SECONDS: u64 = 24 * 60 * 60;

/// After a command: say if a newer Quill is available, and refresh the
/// remembered answer in the background when it is a day old.
pub fn notice() {
    if checks_disabled() || !std::io::stderr().is_terminal() {
        return;
    }
    let Some(file) = cache_file() else {
        return;
    };
    let cached = fs::read_to_string(&file)
        .ok()
        .and_then(|text| parse_cache(&text));

    if let Some((_, Some(latest))) = &cached {
        if is_newer(latest, CURRENT) {
            eprintln!(
                "\nQuill {latest} is available (you have {CURRENT}). Run quill update to get it."
            );
        }
    }

    let stale = match &cached {
        Some((checked, _)) => now().saturating_sub(*checked) >= DAY_IN_SECONDS,
        None => true,
    };
    if stale {
        start_background_check();
    }
}

/// The background check: ask GitHub for the latest release and remember it.
/// Runs in its own process, so it never delays the command that started it.
pub fn background_check() -> ExitCode {
    let Some(file) = cache_file() else {
        return ExitCode::SUCCESS;
    };
    let previous = fs::read_to_string(&file)
        .ok()
        .and_then(|text| parse_cache(&text))
        .and_then(|(_, latest)| latest);
    // Offline or no curl: remember the time anyway, so we don't retry until tomorrow.
    let latest = fetch_latest().or(previous);
    if let Some(dir) = file.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(
        &file,
        format!("{} {}\n", now(), latest.as_deref().unwrap_or("-")),
    );
    ExitCode::SUCCESS
}

/// `quill update`: replace this copy of Quill with the latest release.
pub fn update() -> ExitCode {
    let exe = env::current_exe().ok();
    let updater_name = if cfg!(windows) {
        "quill-update.exe"
    } else {
        "quill-update"
    };
    let updater = exe
        .as_ref()
        .and_then(|exe| exe.parent())
        .map(|dir| dir.join(updater_name))
        .filter(|updater| updater.is_file());

    if let Some(updater) = updater {
        println!("Looking for a new version of Quill (you have {CURRENT})...");
        return match Command::new(&updater).status() {
            Ok(status) if status.success() => ExitCode::SUCCESS,
            Ok(_) => problem(
                "The updater couldn't finish, so Quill wasn't changed.\n\
                 Check your internet connection and try again. If it keeps failing, \
                 install the latest version with the installer:",
                install_command(),
            ),
            Err(err) => problem(
                &format!(
                    "Quill couldn't start its updater ({}): {err}.\n\
                     Install the latest version with the installer instead:",
                    updater.display()
                ),
                install_command(),
            ),
        };
    }

    let built_from_source = exe.as_ref().is_some_and(|exe| {
        exe.components()
            .any(|part| part.as_os_str() == ".cargo" || part.as_os_str() == "target")
    });
    if built_from_source {
        problem(
            "This copy of Quill was built from source with cargo, so quill update can't replace it.\n\
             To update it, get the latest code in your Quill folder and install it again:",
            "git pull\n    cargo install --path . --force",
        )
    } else {
        problem(
            "This copy of Quill was installed without its updater, so quill update can't replace it.\n\
             Install the latest version once with the installer, and quill update will work from then on:",
            install_command(),
        )
    }
}

fn problem(message: &str, command: &str) -> ExitCode {
    eprintln!("Problem: {message}\n\n    {command}");
    ExitCode::FAILURE
}

fn install_command() -> &'static str {
    if cfg!(windows) {
        "powershell -ExecutionPolicy Bypass -c \"irm https://github.com/Ngwangshalom/Quill/releases/latest/download/quill-installer.ps1 | iex\""
    } else {
        "curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Ngwangshalom/Quill/releases/latest/download/quill-installer.sh | sh"
    }
}

fn checks_disabled() -> bool {
    env::var_os("QUILL_NO_UPDATE_CHECK").is_some() || env::var_os("CI").is_some()
}

fn start_background_check() {
    let Ok(exe) = env::current_exe() else {
        return;
    };
    let _ = Command::new(exe)
        .arg(CHECK_COMMAND)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// The latest released version, like `0.3.0`, or `None` if GitHub can't be reached.
fn fetch_latest() -> Option<String> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--location",
            "--max-time",
            "10",
            "--header",
            "Accept: application/vnd.github+json",
            "--user-agent",
            concat!("quill/", env!("CARGO_PKG_VERSION")),
            LATEST_RELEASE,
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    tag_version(&String::from_utf8_lossy(&output.stdout))
}

/// The version in a GitHub release's `"tag_name": "v0.3.0"`.
fn tag_version(json: &str) -> Option<String> {
    let key = "\"tag_name\"";
    let rest = &json[json.find(key)? + key.len()..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let tag = &rest[..rest.find('"')?];
    let version = tag.strip_prefix('v').unwrap_or(tag);
    parse_version(version)?;
    Some(version.to_string())
}

/// The remembered check: when it ran, and the latest version it found.
fn parse_cache(text: &str) -> Option<(u64, Option<String>)> {
    let mut parts = text.split_whitespace();
    let checked = parts.next()?.parse().ok()?;
    let latest = parts
        .next()
        .filter(|version| parse_version(version).is_some())
        .map(str::to_string);
    Some((checked, latest))
}

fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

fn cache_file() -> Option<PathBuf> {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .filter(|home| !home.is_empty())?;
    Some(PathBuf::from(home).join(".quill").join("update-check"))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_version_from_a_github_release() {
        assert_eq!(
            tag_version(r#"{"url":"x","tag_name":"v0.3.0","name":"v0.3.0"}"#).as_deref(),
            Some("0.3.0")
        );
        assert_eq!(
            tag_version("{\n  \"tag_name\": \"v1.12.4\",\n}").as_deref(),
            Some("1.12.4")
        );
        assert_eq!(tag_version(r#"{"message":"Not Found"}"#), None);
        assert_eq!(tag_version(r#"{"tag_name":"nightly"}"#), None);
        assert_eq!(tag_version(r#"{"tag_name":"#), None);
    }

    #[test]
    fn compares_versions_by_number() {
        assert!(is_newer("0.3.0", "0.2.0"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("garbage", "0.2.0"));
    }

    #[test]
    fn reads_the_remembered_check() {
        assert_eq!(
            parse_cache("1760000000 0.3.0\n"),
            Some((1760000000, Some("0.3.0".to_string())))
        );
        assert_eq!(parse_cache("1760000000 -\n"), Some((1760000000, None)));
        assert_eq!(parse_cache("1760000000"), Some((1760000000, None)));
        assert_eq!(parse_cache("not a time 0.3.0"), None);
        assert_eq!(parse_cache(""), None);
    }
}
