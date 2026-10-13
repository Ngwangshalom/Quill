//! Keeping Quill up to date: `quill update` (also `quill upgrade`), which
//! installs any release chosen from a menu, and the notice that a new version
//! is available.
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
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// The hidden command the background check runs as.
pub const CHECK_COMMAND: &str = "__update-check";

const CURRENT: &str = env!("CARGO_PKG_VERSION");
const LATEST_RELEASE: &str = "https://api.github.com/repos/Ngwangshalom/Quill/releases/latest";
const RELEASES: &str = "https://api.github.com/repos/Ngwangshalom/Quill/releases?per_page=100";
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

/// `quill update` (or `quill upgrade`): install a release of Quill.
///
/// With no version, shows a menu of every release to choose from with the
/// arrow keys, or installs the latest when there is no terminal to show it on.
/// `wanted` is a version like `0.2.0`, or `latest`.
pub fn update(wanted: Option<&str>) -> ExitCode {
    let exe = env::current_exe().ok();
    if exe.as_ref().is_some_and(|exe| built_from_source(exe)) {
        return problem(
            "This copy of Quill was built from source with cargo, so quill update can't replace it.\n\
             To update it, get the latest code in your Quill folder and install it again:",
            "git pull\n    cargo install --path . --force",
        );
    }
    let Some(exe) = exe else {
        return problem(
            "Quill couldn't find where it is installed, so it can't update itself.\n\
             Install the latest version with the installer instead:",
            install_command("latest"),
        );
    };

    let releases = match fetch_releases() {
        Some(releases) if !releases.is_empty() => releases,
        _ => {
            return problem(
                "Quill couldn't get the list of versions from GitHub.\n\
                 Check your internet connection and try again, or install the latest version with the installer:",
                install_command("latest"),
            )
        }
    };

    let chosen = match wanted {
        Some("latest") => &releases[0],
        Some(version) => {
            let version = version.strip_prefix('v').unwrap_or(version);
            match releases.iter().find(|release| release.version == version) {
                Some(release) => release,
                None => {
                    let known: Vec<&str> = releases.iter().map(|r| r.version.as_str()).collect();
                    eprintln!(
                        "Problem: There's no Quill version {version}.\n\
                         The versions you can install are: {}.",
                        known.join(", ")
                    );
                    return ExitCode::FAILURE;
                }
            }
        }
        None if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() => {
            match choose(&releases) {
                Some(release) => release,
                None => {
                    println!("Nothing was changed.");
                    return ExitCode::SUCCESS;
                }
            }
        }
        None => &releases[0],
    };

    if chosen.version == CURRENT {
        println!("You already have Quill {CURRENT}.");
        return ExitCode::SUCCESS;
    }
    if is_newer(FIRST_WITH_UPDATE, &chosen.version) && !confirm_no_update(&chosen.version) {
        println!("Nothing was changed.");
        return ExitCode::SUCCESS;
    }
    install(&exe, &chosen.version)
}

/// The first version that has `quill update`.
const FIRST_WITH_UPDATE: &str = "0.3.0";

/// One release on GitHub.
#[derive(Debug, Clone, PartialEq)]
struct Release {
    version: String,
    /// Like `13 Oct 2026`.
    date: String,
}

/// Every published release, newest first.
fn fetch_releases() -> Option<Vec<Release>> {
    let json = curl(RELEASES)?;
    Some(parse_releases(&json))
}

fn parse_releases(json: &str) -> Vec<Release> {
    let Ok(serde_json::Value::Array(list)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    let mut releases: Vec<Release> = list
        .iter()
        .filter(|release| release["draft"] != true && release["prerelease"] != true)
        .filter_map(|release| {
            let tag = release["tag_name"].as_str()?;
            let version = tag.strip_prefix('v').unwrap_or(tag);
            parse_version(version)?;
            Some(Release {
                version: version.to_string(),
                date: release["published_at"]
                    .as_str()
                    .map(readable_date)
                    .unwrap_or_default(),
            })
        })
        .collect();
    releases.sort_by_key(|release| std::cmp::Reverse(parse_version(&release.version)));
    releases
}

/// `2026-10-13T11:42:00Z` as `13 Oct 2026`.
fn readable_date(timestamp: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let date = timestamp.get(..10).unwrap_or("");
    let mut parts = date.split('-');
    let (Some(year), Some(month), Some(day)) = (parts.next(), parts.next(), parts.next()) else {
        return String::new();
    };
    let Some(month) = month
        .parse::<usize>()
        .ok()
        .and_then(|m| MONTHS.get(m.wrapping_sub(1)))
    else {
        return String::new();
    };
    format!("{} {month} {year}", day.trim_start_matches('0'))
}

/// The menu lines: version, date, and whether it's the latest or installed.
fn menu_lines(releases: &[Release]) -> Vec<String> {
    releases
        .iter()
        .enumerate()
        .map(|(i, release)| {
            let mut notes = Vec::new();
            if i == 0 {
                notes.push("latest");
            }
            if release.version == CURRENT {
                notes.push("installed");
            }
            format!(
                "{:<8}  {:<11}  {}",
                release.version,
                release.date,
                notes.join(", ")
            )
            .trim_end()
            .to_string()
        })
        .collect()
}

/// Shows the menu. `None` if the person cancels.
fn choose(releases: &[Release]) -> Option<&Release> {
    let choice = dialoguer::Select::new()
        .with_prompt(
            "Which version of Quill do you want? (↑ ↓ to move, Enter to install, Esc to cancel)",
        )
        .items(menu_lines(releases))
        .default(0)
        .interact_opt()
        .ok()??;
    releases.get(choice)
}

/// Versions before 0.3.0 can't run `quill update`, so going back to one is a
/// one-way trip until the installer is run again. Asks first.
fn confirm_no_update(version: &str) -> bool {
    let message = format!(
        "Quill {version} doesn't have quill update. To come back to a newer version later, \
         you'll run the installer again. Install {version} anyway?"
    );
    if !std::io::stdin().is_terminal() {
        // A script asked for this version by name, so it knows what it wants.
        return true;
    }
    dialoguer::Confirm::new()
        .with_prompt(message)
        .default(false)
        .interact_opt()
        .ok()
        .flatten()
        .unwrap_or(false)
}

/// Runs the installer for `version`. A running program can't always be
/// overwritten (Windows refuses, and so can Linux), so this copy of Quill is
/// moved aside first and put back if the installer fails.
fn install(exe: &Path, version: &str) -> ExitCode {
    let aside = exe.with_extension("old");
    let _ = fs::remove_file(&aside);
    if let Err(err) = fs::rename(exe, &aside) {
        return problem(
            &format!(
                "Quill couldn't make room for the new version: {err}.\n\
                 Check that you're allowed to change {}, or install with the installer instead:",
                exe.display()
            ),
            install_command(version),
        );
    }

    println!("Installing Quill {version}...");
    let installed = installer(version)
        .status()
        .is_ok_and(|status| status.success());
    if !installed || !exe.is_file() {
        let _ = fs::rename(&aside, exe);
        return problem(
            &format!(
                "Quill {version} couldn't be installed, so Quill {CURRENT} is still in place.\n\
                 Check your internet connection and try again, or run the installer yourself:"
            ),
            install_command(version),
        );
    }
    // On Windows the running program can't be deleted; the next update removes it.
    let _ = fs::remove_file(&aside);
    let _ = fs::remove_file(cache_file().unwrap_or_default());
    println!("Quill {version} is installed. Run quill version to check.");
    ExitCode::SUCCESS
}

fn installer(version: &str) -> Command {
    let script = installer_url(version);
    if cfg!(windows) {
        let mut command = Command::new("powershell");
        command.args([
            "-ExecutionPolicy",
            "Bypass",
            "-c",
            &format!("irm {script} | iex"),
        ]);
        command
    } else {
        let mut command = Command::new("sh");
        command.args([
            "-c",
            &format!("curl --proto '=https' --tlsv1.2 -LsSf {script} | sh"),
        ]);
        command
    }
}

fn installer_url(version: &str) -> String {
    let file = if cfg!(windows) {
        "quill-installer.ps1"
    } else {
        "quill-installer.sh"
    };
    if version == "latest" {
        format!("https://github.com/Ngwangshalom/Quill/releases/latest/download/{file}")
    } else {
        format!("https://github.com/Ngwangshalom/Quill/releases/download/v{version}/{file}")
    }
}

fn install_command(version: &str) -> String {
    let script = installer_url(version);
    if cfg!(windows) {
        format!("powershell -ExecutionPolicy Bypass -c \"irm {script} | iex\"")
    } else {
        format!("curl --proto '=https' --tlsv1.2 -LsSf {script} | sh")
    }
}

/// Copies installed with `cargo install` live in `.cargo`; `cargo run` uses `target`.
fn built_from_source(exe: &Path) -> bool {
    exe.components()
        .any(|part| part.as_os_str() == ".cargo" || part.as_os_str() == "target")
}

fn problem(message: &str, command: impl AsRef<str>) -> ExitCode {
    eprintln!("Problem: {message}\n\n    {}", command.as_ref());
    ExitCode::FAILURE
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
    tag_version(&curl(LATEST_RELEASE)?)
}

/// Asks GitHub's API for `url`. `None` if offline, if curl is missing, or on an error.
fn curl(url: &str) -> Option<String> {
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
            url,
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
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
    fn lists_releases_newest_first() {
        let json = r#"[
            {"tag_name":"v0.1.0","published_at":"2026-10-13T09:00:00Z","draft":false,"prerelease":false},
            {"tag_name":"v0.10.0","published_at":"2027-01-02T09:00:00Z","draft":false,"prerelease":false},
            {"tag_name":"v0.3.0-beta","published_at":"2026-10-14T09:00:00Z","draft":false,"prerelease":true},
            {"tag_name":"v0.2.0","published_at":"2026-10-13T10:00:00Z","draft":false,"prerelease":false},
            {"tag_name":"nightly","published_at":"2026-10-15T10:00:00Z","draft":false,"prerelease":false}
        ]"#;
        let releases = parse_releases(json);
        let versions: Vec<&str> = releases.iter().map(|r| r.version.as_str()).collect();
        assert_eq!(versions, vec!["0.10.0", "0.2.0", "0.1.0"]);
        assert_eq!(releases[0].date, "2 Jan 2027");
        assert_eq!(parse_releases("not json"), vec![]);
        assert_eq!(
            parse_releases(r#"{"message":"API rate limit exceeded"}"#),
            vec![]
        );
    }

    #[test]
    fn menu_marks_the_latest_and_installed_versions() {
        let release = |version: &str| Release {
            version: version.to_string(),
            date: "13 Oct 2026".to_string(),
        };
        let lines = menu_lines(&[release("9.0.0"), release(CURRENT), release("0.0.1")]);
        assert_eq!(lines[0], "9.0.0     13 Oct 2026  latest");
        assert!(lines[1].ends_with("13 Oct 2026  installed"));
        assert_eq!(lines[2], "0.0.1     13 Oct 2026");
    }

    #[test]
    fn readable_dates() {
        assert_eq!(readable_date("2026-10-13T11:42:00Z"), "13 Oct 2026");
        assert_eq!(readable_date("2026-01-05T00:00:00Z"), "5 Jan 2026");
        assert_eq!(readable_date("garbage"), "");
        assert_eq!(readable_date("2026-13-01T00:00:00Z"), "");
    }

    #[test]
    fn installer_addresses() {
        assert!(installer_url("0.2.0").contains("/releases/download/v0.2.0/quill-installer."));
        assert!(installer_url("latest").contains("/releases/latest/download/quill-installer."));
    }

    #[test]
    fn copies_built_from_source_are_recognised() {
        assert!(built_from_source(Path::new("/home/a/.cargo/bin/quill")));
        assert!(built_from_source(Path::new(
            "/code/quill/target/debug/quill"
        )));
        assert!(!built_from_source(Path::new("/home/a/.quill/bin/quill")));
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
