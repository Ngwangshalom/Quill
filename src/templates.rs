//! The projects `quill new` starts from. Their files live in `templates/` and are built
//! into Quill, so `quill new` works without the internet. Every project also gets
//! `QUILL.md`, a short tour of the language.

pub struct Template {
    pub name: &'static str,
    /// The heading it sits under in the menu and in `quill help`.
    pub group: &'static str,
    pub summary: &'static str,
    /// The file to run first, shown once the project is made.
    pub run: &'static str,
    /// Each file's path in the project and its contents. `{{name}}` becomes the project's name.
    pub files: &'static [(&'static str, &'static str)],
}

macro_rules! files {
    ($dir:literal: $($path:literal),+ $(,)?) => {
        &[$(($path, include_str!(concat!("../templates/", $dir, "/", $path)))),+]
    };
}

pub const GUIDE: (&str, &str) = ("QUILL.md", include_str!("../templates/QUILL.md"));

/// The template `quill new NAME` makes when no one chooses.
pub const DEFAULT: &str = "starter";

pub const TEMPLATES: &[Template] = &[
    Template {
        name: "learn",
        group: "For learning",
        summary: "six exercises with checks that fail until you fix the code",
        run: "exercises/01-values.quill",
        files: files!("learn": "README.md", "exercises/01-values.quill", "exercises/02-numbers.quill",
            "exercises/03-decisions.quill", "exercises/04-data.quill", "exercises/05-text.quill",
            "exercises/06-money.quill"),
    },
    Template {
        name: "starter",
        group: "For learning",
        summary: "one file with values, when, for, a function, and a test",
        run: "hello.quill",
        files: files!("starter": "README.md", "hello.quill"),
    },
    Template {
        name: "backend",
        group: "For backend work",
        summary: "a service that web, desktop, and mobile apps call, with JSON and tests",
        run: "server.quill",
        files: files!("backend": "README.md", "server.quill", "orders.quill"),
    },
    Template {
        name: "integration",
        group: "For backend work",
        summary: "call another service, read its JSON safely, and handle NetworkDown",
        run: "main.quill",
        files: files!("integration": "README.md", "main.quill", "rates.quill"),
    },
    Template {
        name: "webhook",
        group: "For backend work",
        summary: "receive a payment notice, check every field, answer 200 or 400",
        run: "server.quill",
        files: files!("webhook": "README.md", "server.quill", "payments.quill"),
    },
    Template {
        name: "ai",
        group: "For AI",
        summary: "ask a model, and check its answer against a type",
        run: "main.quill",
        files: files!("ai": "README.md", "main.quill", "rules.quill"),
    },
    Template {
        name: "ai-backend",
        group: "For AI",
        summary: "a service route that asks a model, ready to deploy",
        run: "server.quill",
        files: files!("ai-backend": "README.md", "server.quill", "rules.quill"),
    },
    Template {
        name: "finance",
        group: "For business and finance",
        summary: "an invoice with exact money, VAT, split, and contracts",
        run: "main.quill",
        files: files!("finance": "README.md", "main.quill", "billing.quill"),
    },
    Template {
        name: "banking",
        group: "For business and finance",
        summary: "accounts that can never go below zero, with require and ensure",
        run: "main.quill",
        files: files!("banking": "README.md", "main.quill", "bank.quill"),
    },
    Template {
        name: "automation",
        group: "For automation and general programs",
        summary: "read a file, tidy it, count and sort, and save a report",
        run: "main.quill",
        files: files!("automation": "README.md", "main.quill", "report.quill", "data/students.json"),
    },
    Template {
        name: "library",
        group: "For automation and general programs",
        summary: "reusable functions with tests, used with bring",
        run: "main.quill",
        files: files!("library": "README.md", "main.quill", "money.quill"),
    },
];

pub fn find(name: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|template| template.name == name)
}

pub fn names() -> impl Iterator<Item = &'static str> {
    TEMPLATES.iter().map(|template| template.name)
}

/// One line of the help listing: the name, then what it is.
pub fn menu_line(template: &Template) -> String {
    format!("{:<12} {}", template.name, template.summary)
}

/// One line of the menu, which has no headings: the group's first word comes first.
pub fn choice_line(template: &Template) -> String {
    let group = template.group.trim_start_matches("For ");
    let group = group.split_whitespace().next().unwrap_or(group);
    let mut letters = group.chars();
    let group: String = match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    };
    format!("{group:<11} {}", menu_line(template))
}

/// Every template, under its group, as `quill help` and the book show them.
pub fn listing() -> String {
    let mut out = String::new();
    let mut group = "";
    for template in TEMPLATES {
        if template.group != group {
            group = template.group;
            out.push_str(&format!("  {group}\n"));
        }
        out.push_str(&format!("    {}\n", menu_line(template)));
    }
    out
}
