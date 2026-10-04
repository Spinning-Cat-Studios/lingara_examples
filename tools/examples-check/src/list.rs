//! `list` and `lang` (ADR 4.10.26b D4): the index as make, CI and the README
//! read it.

use crate::index::Index;
use serde::Serialize;

/// A GitHub Actions matrix as it stands: `include` alone, since every other
/// matrix key must be an array (D6).
#[derive(Serialize)]
struct Matrix<'a> {
    count: usize,
    include: Vec<Leg<'a>>,
}

#[derive(Serialize)]
struct Leg<'a> {
    path: &'a str,
    language: &'a str,
}

/// One `<path> <language>` line per row, in file order.
pub fn plain(index: &Index) -> String {
    index.examples.iter().map(|e| format!("{} {}\n", e.path, e.language)).collect()
}

/// Exactly one line: `{"count":N,"include":[{"path":…,"language":…},…]}`.
pub fn json(index: &Index) -> String {
    let include: Vec<Leg> =
        index.examples.iter().map(|e| Leg { path: &e.path, language: &e.language }).collect();
    let matrix = Matrix { count: include.len(), include };
    // Two plain strings and a number: serialising them cannot fail.
    serde_json::to_string(&matrix).unwrap_or_default()
}

/// The README's table, between its two markers.
pub fn markdown(index: &Index) -> String {
    if index.examples.is_empty() {
        return "_No examples yet._\n".to_string();
    }
    let mut out = String::from("| Example | What it shows | Language | Lingara packages |\n|---|---|---|---|\n");
    for e in &index.examples {
        let packages: Vec<String> = e.packages.iter().map(|p| format!("`{}` {}", p.name, p.version)).collect();
        out.push_str(&format!("| [`{0}`]({0}/) | {1} | {2} | {3} |\n", e.path, e.title, e.language, packages.join(", ")));
    }
    out
}

/// One row's language, which make's dispatch targets read (D5).
pub fn lang<'a>(index: &'a Index, path: &str) -> Option<&'a str> {
    let path = path.trim_end_matches('/');
    index.examples.iter().find(|e| e.path == path).map(|e| e.language.as_str())
}
