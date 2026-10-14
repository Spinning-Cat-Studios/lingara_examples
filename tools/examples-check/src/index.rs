//! `examples.toml`, the index (ADR 4.10.26b D3), and the `index` checks (D4).

use serde::Deserialize;
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;

pub const INDEX_FILE: &str = "examples.toml";
const SECTIONS: [&str; 4] = ["integrations", "apps", "games", "websites"];
pub const README: &str = "README.md";
const README_START: &str = "<!-- examples:start -->";
const README_END: &str = "<!-- examples:end -->";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
    pub schema: u32,
    #[serde(default, rename = "example")]
    pub examples: Vec<Example>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Example {
    pub path: String,
    pub section: String,
    pub language: String,
    pub title: String,
    #[serde(default)]
    pub live: bool,
    #[serde(default)]
    pub packages: Vec<Package>,
    // Documentation only: the live client must hold their union (D8).
    #[serde(default)]
    #[allow(dead_code)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub registry: Registry,
    pub name: String,
    pub version: String,
}

/// A closed set: a registry arrives with the first example that installs
/// from it, with its reader and its lookup (D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Registry {
    Npm,
    Go,
    Packagist,
    Crates,
    Maven,
}

impl Registry {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "npm" => Some(Self::Npm),
            "go" => Some(Self::Go),
            "packagist" => Some(Self::Packagist),
            "crates" => Some(Self::Crates),
            "maven" => Some(Self::Maven),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Go => "go",
            Self::Packagist => "packagist",
            Self::Crates => "crates",
            Self::Maven => "maven",
        }
    }
}

/// One line of output: `<path>: <message>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub path: String,
    pub message: String,
}

impl Finding {
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self { path: path.into(), message: message.into() }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

pub fn load(root: &Path) -> Result<Index, Finding> {
    let text = fs::read_to_string(root.join(INDEX_FILE)).map_err(|e| Finding::new(INDEX_FILE, e.to_string()))?;
    let index: Index = toml::from_str(&text).map_err(|e| Finding::new(INDEX_FILE, e.message().to_string()))?;
    if index.schema != 1 {
        return Err(Finding::new(INDEX_FILE, format!("schema {} is not 1", index.schema)));
    }
    Ok(index)
}

pub fn check(root: &Path) -> Vec<Finding> {
    let index = match load(root) {
        Ok(index) => index,
        Err(finding) => return vec![finding],
    };
    let mut findings = check_rows(root, &index);
    findings.extend(check_unindexed(root, &index));
    findings.extend(check_readme(root, &index));
    findings
}

fn check_rows(root: &Path, index: &Index) -> Vec<Finding> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for row in &index.examples {
        if !seen.insert(row.path.as_str()) {
            out.push(Finding::new(&row.path, "two rows share this path"));
        }
        if !root.join(&row.path).is_dir() {
            out.push(Finding::new(&row.path, "no such directory"));
        }
        let first = row.path.split('/').next().unwrap_or_default();
        if !SECTIONS.contains(&row.section.as_str()) {
            out.push(Finding::new(&row.path, format!("section `{}` is not integrations, apps, games or websites", row.section)));
        } else if row.section != first {
            out.push(Finding::new(&row.path, format!("section `{}` disagrees with the path", row.section)));
        }
        out.extend(check_recipe(root, row));
    }
    out
}

/// A text read of the recipe, the same parse scs-snapshot's `defined_targets`
/// uses, never a `make -n`.
fn check_recipe(root: &Path, row: &Example) -> Option<Finding> {
    let recipe = format!("make/lang/{}.mk", row.language);
    let Ok(text) = fs::read_to_string(root.join(&recipe)) else {
        return Some(Finding::new(&row.path, format!("{recipe} is missing")));
    };
    let target = format!("live-example-{}:", row.language);
    (row.live && !text.lines().any(|l| l.starts_with(&target)))
        .then(|| Finding::new(&row.path, format!("live = true, but {recipe} has no `{target}` line")))
}

fn check_unindexed(root: &Path, index: &Index) -> Vec<Finding> {
    let indexed: BTreeSet<&str> = index.examples.iter().map(|e| e.path.as_str()).collect();
    example_dirs(root)
        .into_iter()
        .filter(|d| !indexed.contains(d.as_str()))
        .map(|d| Finding::new(d, "an example directory with no row in examples.toml"))
        .collect()
}

/// A direct child of a section, or, for `games/` and `websites/`, of an
/// engine or a stack.
fn example_dirs(root: &Path) -> Vec<String> {
    let mut dirs: Vec<String> = ["integrations", "apps"].iter().flat_map(|s| child_dirs(root, s)).collect();
    for group in ["games", "websites"].iter().flat_map(|s| child_dirs(root, s)) {
        dirs.extend(child_dirs(root, &group));
    }
    dirs
}

fn child_dirs(root: &Path, rel: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root.join(rel)) else {
        return Vec::new();
    };
    let mut dirs: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|name| !name.starts_with('.'))
        .map(|name| format!("{rel}/{name}"))
        .collect();
    dirs.sort();
    dirs
}

/// The README's table is generated, so it is also checked.
fn check_readme(root: &Path, index: &Index) -> Option<Finding> {
    let text = fs::read_to_string(root.join(README)).unwrap_or_default();
    let Some(table) = readme_table(&text) else {
        return Some(Finding::new(README, format!("no {README_START} … {README_END} block")));
    };
    (table.trim() != crate::list::markdown(index).trim())
        .then(|| Finding::new(README, "the examples table differs from `list --markdown`; regenerate it"))
}

fn readme_table(text: &str) -> Option<&str> {
    let start = text.find(README_START)? + README_START.len();
    let end = text[start..].find(README_END)? + start;
    Some(&text[start..end])
}

/// `text` with the table between the markers replaced by `table`, or `None`
/// when the markers are missing.
pub(crate) fn splice_readme_table(text: &str, table: &str) -> Option<String> {
    let start = text.find(README_START)? + README_START.len();
    let end = text[start..].find(README_END)? + start;
    Some(format!("{}\n{table}{}", &text[..start], &text[end..]))
}
