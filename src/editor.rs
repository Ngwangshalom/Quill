//! Editor support for `.ql` files.
//!
//! The first time `quill` runs, it installs the extension in `editors/vscode`
//! into Cursor and VS Code on this computer. That extension colors Quill and
//! shows the feather on every `.ql` file. It does nothing if the same files
//! are already installed, and `QUILL_NO_EDITOR_SETUP=1` skips it.

use std::env;
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

const MANIFEST: &str = include_str!("../editors/vscode/package.json");

struct EditorFile {
    path: &'static str,
    bytes: &'static [u8],
}

const FILES: &[EditorFile] = &[
    EditorFile {
        path: "package.json",
        bytes: MANIFEST.as_bytes(),
    },
    EditorFile {
        path: "language-configuration.json",
        bytes: include_bytes!("../editors/vscode/language-configuration.json"),
    },
    EditorFile {
        path: "syntaxes/quill.tmLanguage.json",
        bytes: include_bytes!("../editors/vscode/syntaxes/quill.tmLanguage.json"),
    },
    EditorFile {
        path: "icons/quill-light.png",
        bytes: include_bytes!("../editors/vscode/icons/quill-light.png"),
    },
    EditorFile {
        path: "icons/quill-dark.png",
        bytes: include_bytes!("../editors/vscode/icons/quill-dark.png"),
    },
    EditorFile {
        path: "extension.js",
        bytes: include_bytes!("../editors/vscode/extension.js"),
    },
];

/// Install coloring and the feather icon for Cursor and VS Code, if needed.
pub fn install() {
    if env::var_os("QUILL_NO_EDITOR_SETUP").is_some() {
        return;
    }
    let Some(home) = home_dir() else {
        return;
    };
    let fresh = install_editor_support_under(&home, &[".cursor", ".vscode"]);
    if fresh && std::io::stderr().is_terminal() {
        eprintln!(
            "\nQuill added colors and the feather icon for .ql files to Cursor and VS Code.\n\
             Reload the editor window to see them."
        );
    }
}

/// Install under `root/<editor>/extensions` for each editor. True when a file changed.
fn install_editor_support_under(root: &Path, editors: &[&str]) -> bool {
    let name = extension_dir_name();
    let mut fresh = false;
    for editor in editors {
        if install_into(&root.join(editor).join("extensions").join(&name)) {
            fresh = true;
        }
    }
    fresh
}

fn extension_dir_name() -> String {
    format!("quill.quill-{}", manifest_version(MANIFEST))
}

fn manifest_version(manifest: &str) -> &str {
    let key = "\"version\": \"";
    let start = manifest
        .find(key)
        .map(|index| index + key.len())
        .unwrap_or(0);
    let rest = &manifest[start..];
    let end = rest.find('"').unwrap_or(rest.len());
    &rest[..end]
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Copy the extension into `dest`. A symlink is left alone, so a checkout
/// that already points the editor at `editors/vscode` is not overwritten.
fn install_into(dest: &Path) -> bool {
    if is_symlink(dest) || files_match(dest) {
        return false;
    }
    let Some(parent) = dest.parent() else {
        return false;
    };
    let staging = parent.join(format!(
        ".{}.installing",
        dest.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("quill")
    ));
    if fs::create_dir_all(&staging).is_err() {
        return false;
    }
    for file in FILES {
        let path = staging.join(file.path);
        if path
            .parent()
            .is_some_and(|dir| fs::create_dir_all(dir).is_err())
            || fs::write(&path, file.bytes).is_err()
        {
            let _ = fs::remove_dir_all(&staging);
            return false;
        }
    }
    let _ = fs::remove_dir_all(dest);
    if fs::rename(&staging, dest).is_err() {
        let _ = fs::remove_dir_all(&staging);
        return false;
    }
    true
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

fn files_match(dest: &Path) -> bool {
    FILES.iter().all(|file| {
        fs::read(dest.join(file.path))
            .ok()
            .is_some_and(|found| found == file.bytes)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_support_installs_the_feather_for_cursor_and_vs_code() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/quill-editor-test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        assert!(install_editor_support_under(&root, &["cursor", "vscode"]));
        let name = extension_dir_name();
        assert_eq!(name, format!("quill.quill-{}", manifest_version(MANIFEST)));

        for editor in ["cursor", "vscode"] {
            let ext = root.join(editor).join("extensions").join(&name);
            let manifest = fs::read_to_string(ext.join("package.json")).unwrap();
            assert!(manifest.contains("\".ql\""));
            assert!(manifest.contains("icons/quill-light.png"));
            assert!(manifest.contains("icons/quill-dark.png"));
            for icon_name in ["icons/quill-light.png", "icons/quill-dark.png"] {
                let icon = fs::read(ext.join(icon_name)).unwrap();
                assert_eq!(
                    &icon[..8],
                    &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']
                );
            }
            let grammar = fs::read_to_string(ext.join("syntaxes/quill.tmLanguage.json")).unwrap();
            assert!(grammar.contains("source.quill"));
            let extension = fs::read_to_string(ext.join("extension.js")).unwrap();
            assert!(extension.contains("publishDiagnostics"));
            assert!(extension.contains("#F0A35E"));
            assert!(extension.contains("#7EAEF5"));
            assert!(extension.contains("#5ECA8C"));
            assert!(extension.contains("#C6D86A"));
            assert!(extension.contains("#D4896A"));
            assert!(extension.contains("#A8C5A0"));
            assert!(extension.contains("#E8C547"));
            assert!(extension.contains("#B07CFF"));
            assert!(extension.contains("#E8A0D4"));
            assert!(extension.contains("#F0B090"));
            assert!(extension.contains("#E08A8A"));
        }

        assert!(!install_editor_support_under(&root, &["cursor", "vscode"]));
        let _ = fs::remove_dir_all(&root);
    }
}
