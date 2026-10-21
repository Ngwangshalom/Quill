//! `bring`: reading the files a program brings in.
//!
//! `bring billing` reads billing.quill from the same folder, and `bring "shared/money.quill"`
//! reads a file by its path. A brought file holds functions and types, which the
//! program can then use by name. Its own tests run only when that file is tested.
//! Each file keeps its own number in every span, so a problem points at the file it
//! is in.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostic::{closest_match, Diagnostic, Span};
use crate::lexer::{self, Piece, Token, TokenKind};
use crate::parser::{self, Program, Statement};

/// Quill's own modules. `bring web` names one of these, not a file.
pub const MODULES: [&str; 4] = ["web", "db", "ai", "http"];

/// A file of the program, so problems can show its lines.
#[derive(Debug, Clone, PartialEq)]
pub struct File {
    pub path: String,
    pub source: String,
}

/// Reads the program in `path` and every file it brings, as one program. The files
/// come back either way, so problems can be shown with the right file.
pub fn load(path: &str, source: &str) -> (Result<Program, Vec<Diagnostic>>, Vec<File>) {
    let mut loader = Loader {
        files: vec![File {
            path: path.to_string(),
            source: source.to_string(),
        }],
        open: vec![canonical(Path::new(path))],
        brought: HashMap::new(),
    };
    let result = loader.program();
    (result, loader.files)
}

struct Loader {
    files: Vec<File>,
    /// The files being read right now, from the program down, to find a circle.
    open: Vec<PathBuf>,
    /// Files already brought in, so one brought twice is read once.
    brought: HashMap<PathBuf, usize>,
}

impl Loader {
    fn program(&mut self) -> Result<Program, Vec<Diagnostic>> {
        let main = self.parse(0)?;
        let mut statements = self.bring_all(0, &main)?;
        self.no_name_twice(&statements, &main.statements)?;
        statements.extend(main.statements);
        Ok(Program { statements })
    }

    fn parse(&self, file: usize) -> Result<Program, Vec<Diagnostic>> {
        let mut tokens = lexer::lex(&self.files[file].source).map_err(|mut problem| {
            problem.file = file;
            vec![problem]
        })?;
        mark(&mut tokens, file);
        parser::parse(&tokens).map_err(|problem| vec![problem])
    }

    /// The functions and types of every file `program` brings, the deepest first.
    fn bring_all(
        &mut self,
        file: usize,
        program: &Program,
    ) -> Result<Vec<Statement>, Vec<Diagnostic>> {
        let mut statements = Vec::new();
        for statement in &program.statements {
            let Statement::Bring {
                target,
                quoted,
                target_span,
                ..
            } = statement
            else {
                continue;
            };
            if !quoted && MODULES.contains(&target.as_str()) {
                statements.push(statement.clone());
                continue;
            }
            let path = self.resolve(file, target, *quoted, *target_span)?;
            let key = canonical(&path);
            if self.open.contains(&key) {
                return Err(vec![Diagnostic::new(
                    *target_span,
                    format!(
                        "{} brings {}, which brings {} back.\n\
                         Files can't bring each other in a circle. Move what both of them \
                         need into a third file, and bring that one from each.",
                        self.name(file),
                        shown(&path),
                        self.name(file)
                    ),
                )]);
            }
            if self.brought.contains_key(&key) {
                continue;
            }
            let source = fs::read_to_string(&path)
                .map_err(|_| vec![self.missing(file, target, *quoted, &path, *target_span)])?;
            let index = self.files.len();
            self.files.push(File {
                path: shown(&path),
                source,
            });
            self.brought.insert(key.clone(), index);
            let brought = self.parse(index)?;
            self.open.push(key);
            let nested = self.bring_all(index, &brought);
            self.open.pop();
            statements.extend(nested?);
            for statement in brought.statements {
                match statement {
                    Statement::Function { .. } | Statement::Type { .. } => {
                        statements.push(statement)
                    }
                    Statement::Bring { .. } => {}
                    // A brought file's tests run when that file is tested.
                    Statement::Check { .. } => {}
                    Statement::Permit { span, .. } => {
                        return Err(vec![Diagnostic::new(
                            span,
                            format!(
                                "{} is brought into {}, and a brought file can't give permits.\n\
                                 Permits belong to the program you run, so you see everything \
                                 it may touch. Move this line to the top of {}.",
                                self.files[index].path,
                                self.name(file),
                                self.name(0)
                            ),
                        )]);
                    }
                    other => {
                        return Err(vec![Diagnostic::new(
                            span_of(&other),
                            format!(
                                "{} is brought into {}, so it holds functions and types, \
                                 but this line does something.\n\
                                 Move it into the program that brings {}, or into a function.",
                                self.files[index].path,
                                self.name(file),
                                target
                            ),
                        )]);
                    }
                }
            }
        }
        Ok(statements)
    }

    /// Where `bring target` points, next to `file`.
    fn resolve(
        &self,
        file: usize,
        target: &str,
        quoted: bool,
        span: Span,
    ) -> Result<PathBuf, Vec<Diagnostic>> {
        let folder = Path::new(&self.files[file].path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        if quoted {
            if crate::quill_stem(target).is_none() {
                return Err(vec![Diagnostic::new(
                    span,
                    format!(
                        "bring \"{target}\" needs the whole file name, ending in .quill.\n\
                         Quill files end in .quill."
                    ),
                )
                .suggest(format!("bring \"{target}.quill\""))]);
            }
            return Ok(folder.join(target));
        }
        let path = folder.join(format!("{target}{}", crate::EXTENSION));
        // A project from before 0.8 still has billing.ql; bring it until it's renamed.
        let old = folder.join(format!("{target}{}", crate::OLD_EXTENSION));
        if !path.exists() && old.exists() {
            return Ok(old);
        }
        Ok(path)
    }

    fn missing(
        &self,
        file: usize,
        target: &str,
        quoted: bool,
        path: &Path,
        span: Span,
    ) -> Diagnostic {
        let folder = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let shown_folder = if folder.as_os_str().is_empty() {
            "this folder".to_string()
        } else {
            shown(&folder)
        };
        let problem = Diagnostic::new(
            span,
            format!(
                "{} brings {}, but there's no file {} in {shown_folder}.\n\
                 bring {} looks for {} next to {}.",
                self.name(file),
                if quoted {
                    target.to_string()
                } else {
                    format!("{target}.quill")
                },
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                target,
                if quoted {
                    target.to_string()
                } else {
                    format!("{target}.quill")
                },
                self.name(file)
            ),
        );
        if quoted {
            return problem;
        }
        let read = fs::read_dir(if folder.as_os_str().is_empty() {
            Path::new(".")
        } else {
            &folder
        });
        let names: Vec<String> = read
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                crate::quill_stem(&name).map(str::to_string)
            })
            .collect();
        match closest_match(target, names.iter().map(String::as_str)) {
            Some(fixed) => problem.suggest(format!("bring {fixed}")),
            None => problem,
        }
    }

    /// Two files can't write a function or a type with the same name.
    fn no_name_twice(
        &self,
        brought: &[Statement],
        main: &[Statement],
    ) -> Result<(), Vec<Diagnostic>> {
        let mut seen: HashMap<String, Span> = HashMap::new();
        let mut problems = Vec::new();
        for statement in brought.iter().chain(main) {
            let (name, span, word) = match statement {
                Statement::Function {
                    name, name_span, ..
                } => (name, *name_span, "function"),
                Statement::Type {
                    name, name_span, ..
                } => (name, *name_span, "type"),
                _ => continue,
            };
            match seen.get(name) {
                // The checker reports two with the same name in one file.
                Some(first) if first.file != span.file => problems.push(Diagnostic::new(
                    span,
                    format!(
                        "{name} is already a {word}, written in {} on line {}.\n\
                         Each {word} needs its own name, across every file the program brings. \
                         Rename one of them.",
                        self.files[first.file].path, first.line
                    ),
                )),
                Some(_) => {}
                None => {
                    seen.insert(name.clone(), span);
                }
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }

    /// The file's name as it is shown: `billing.quill`.
    fn name(&self, file: usize) -> String {
        Path::new(&self.files[file].path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.files[file].path.clone())
    }
}

/// Every token, including those inside `{}` in text, belongs to `file`.
fn mark(tokens: &mut [Token], file: usize) {
    for token in tokens {
        token.file = file;
        if let TokenKind::Template(pieces) = &mut token.kind {
            for piece in pieces {
                if let Piece::Code { tokens, .. } = piece {
                    mark(tokens, file);
                }
            }
        }
    }
}

fn span_of(statement: &Statement) -> Span {
    let line = statement.line();
    match statement {
        Statement::Say { span, .. }
        | Statement::Set { span, .. }
        | Statement::When { span, .. }
        | Statement::For { span, .. }
        | Statement::Return { span, .. }
        | Statement::Call { span, .. }
        | Statement::Match { span, .. }
        | Statement::Fail { span, .. } => *span,
        Statement::Let { name_span, .. } => *name_span,
        _ => Span {
            line,
            ..Span::default()
        },
    }
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// A path with `/` between folders on every system.
fn shown(path: &Path) -> String {
    let text = path.to_string_lossy();
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.into_owned()
    }
}

/// Every problem, each shown with the lines of its own file.
pub fn render(problems: &[Diagnostic], files: &[File]) -> String {
    let mut text = String::new();
    for (i, problem) in problems.iter().enumerate() {
        if i > 0 {
            text.push('\n');
        }
        let file = files.get(problem.file).or(files.first());
        let (path, source) =
            file.map_or(("", ""), |file| (file.path.as_str(), file.source.as_str()));
        text.push_str(&problem.render(path, source));
    }
    if problems.len() > 1 {
        text.push_str(&format!("\nQuill found {} problems.\n", problems.len()));
    }
    text
}
