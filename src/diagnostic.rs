//! Problems found in user code, and how they are shown.
//!
//! Every stage reports problems the same way, in the README style:
//! where it happened, what went wrong, why, and a likely fix.

/// A place in the source: 1-based line and column, counted in characters.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Span {
    /// Which file: 0 is the program that was run, and each brought file has its own number.
    pub file: usize,
    pub line: usize,
    pub column: usize,
    /// How many characters the span covers.
    pub length: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    /// The file the problem is in, as in `Span::file`.
    pub file: usize,
    pub line: usize,
    pub column: usize,
    /// How many characters to underline, starting at `column`.
    pub length: usize,
    /// What went wrong, then why.
    pub message: String,
    /// A corrected version the user probably meant.
    pub suggestion: Option<String>,
}

impl Diagnostic {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            file: span.file,
            line: span.line,
            column: span.column,
            length: span.length,
            message: message.into(),
            suggestion: None,
        }
    }

    pub fn suggest(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Formats the problem in the README style: where, what, why, and a fix.
    pub fn render(&self, file_name: &str, source: &str) -> String {
        let line_text = source
            .lines()
            .nth(self.line.saturating_sub(1))
            .unwrap_or("");
        // Keep tabs so the carets line up with the source line.
        let padding: String = line_text
            .chars()
            .take(self.column.saturating_sub(1))
            .map(|c| if c == '\t' { '\t' } else { ' ' })
            .collect();
        let carets = "^".repeat(self.length.max(1));

        let mut out = format!(
            "Problem on line {} of {}:\n\n    {}\n    {}{}\n{}\n",
            self.line, file_name, line_text, padding, carets, self.message
        );
        if let Some(suggestion) = &self.suggestion {
            out.push_str(&format!("\nDid you mean:  {suggestion}\n"));
        }
        out
    }
}

/// Finds the candidate closest to `word`, if it is close enough to be a typo.
pub fn closest_match<'a>(
    word: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let limit = (word.chars().count() / 3).max(1);
    candidates
        .into_iter()
        .map(|candidate| (edit_distance(word, candidate), candidate))
        .filter(|(distance, _)| *distance <= limit)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, candidate)| candidate)
}

/// Levenshtein distance, where swapping two neighbouring letters counts as one edit.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut table = vec![vec![0; b.len() + 1]; a.len() + 1];
    for (i, row) in table.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in table[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (table[i - 1][j] + 1)
                .min(table[i][j - 1] + 1)
                .min(table[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(table[i - 2][j - 2] + 1);
            }
            table[i][j] = best;
        }
    }
    table[a.len()][b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_typos() {
        assert_eq!(edit_distance("visits", "vistis"), 1);
        assert_eq!(closest_match("vistis", ["visits", "name"]), Some("visits"));
        assert_eq!(closest_match("greting", ["greeting"]), Some("greeting"));
        assert_eq!(closest_match("rnu", ["run", "check"]), Some("run"));
    }

    #[test]
    fn ignores_names_that_are_too_different() {
        assert_eq!(closest_match("x", ["name", "total"]), None);
        assert_eq!(closest_match("price", ["greeting"]), None);
    }

    #[test]
    fn renders_without_suggestion() {
        let diagnostic = Diagnostic::new(
            Span {
                file: 0,
                line: 1,
                column: 5,
                length: 3,
            },
            "Something is wrong.\nHere is why.",
        );
        assert_eq!(
            diagnostic.render("a.ql", "say foo"),
            "Problem on line 1 of a.ql:\n\n    say foo\n        ^^^\nSomething is wrong.\nHere is why.\n"
        );
    }
}
