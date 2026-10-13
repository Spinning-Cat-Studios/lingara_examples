//! The Gradle build around the catalogue (ADR 13.10.26i D5): the settings'
//! repositories and modules, each module's build file, and its lockfile.
//! Kotlin DSL read as text, braces matched, comments dropped.

use super::{BUILD, LINGARA_GROUP};
use crate::deps::{Pins, check_locked};
use crate::index::Finding;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const SETTINGS: &str = "settings.gradle.kts";
const PLUGIN_REPOS: [&str; 2] = ["gradlePluginPortal()", "mavenCentral()"];
const DEPENDENCY_REPOS: [&str; 1] = ["mavenCentral()"];

// ── The settings ──

/// The repositories by role, no local or composite source, and the modules
/// `include` names (`:a:b` as `a/b`).
pub(super) fn check_settings(dir: &Path) -> (Vec<String>, Vec<Finding>) {
    let Ok(text) = fs::read_to_string(dir.join(SETTINGS)) else {
        return (Vec::new(), vec![Finding::new(SETTINGS, "is missing")]);
    };
    let text = strip_comments(&text);
    let mut out: Vec<Finding> = [("mavenLocal(", "mavenLocal()"), ("includeBuild(", "an includeBuild")]
        .iter()
        .filter(|(needle, _)| text.contains(needle))
        .map(|(_, what)| Finding::new(SETTINGS, format!("has {what}; every package comes from Maven Central")))
        .collect();
    out.extend(check_repositories(&text, "pluginManagement", &PLUGIN_REPOS));
    out.extend(check_repositories(&text, "dependencyResolutionManagement", &DEPENDENCY_REPOS));
    if !block(&text, "dependencyResolutionManagement").is_some_and(|b| b.contains("RepositoriesMode.FAIL_ON_PROJECT_REPOS")) {
        out.push(Finding::new(SETTINGS, "does not set repositoriesMode to FAIL_ON_PROJECT_REPOS"));
    }
    (modules(&text), out)
}

fn check_repositories(text: &str, owner: &str, allowed: &[&str]) -> Vec<Finding> {
    let Some(repositories) = block(text, owner).and_then(|b| block(b, "repositories")) else {
        return vec![Finding::new(SETTINGS, format!("{owner} has no repositories block"))];
    };
    repositories
        .split([';', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty() && !allowed.contains(s))
        .map(|s| Finding::new(SETTINGS, format!("{owner} names the repository `{s}`; it allows only {}", allowed.join(" and "))))
        .collect()
}

fn modules(text: &str) -> Vec<String> {
    let lines = text.lines().map(str::trim).filter(|l| l.starts_with("include(") || l.starts_with("include \""));
    lines.flat_map(|l| l.split('"').skip(1).step_by(2)).map(|m| m.trim_start_matches(':').replace(':', "/")).collect()
}

// ── The modules ──

/// The Lingara artefacts one module's build file declares, its refusals,
/// and its lockfile held to the row's pins for those artefacts.
pub(super) fn check_module(dir: &Path, module: &str, accessors: &BTreeMap<String, String>, row: &Pins) -> (BTreeSet<String>, Vec<Finding>) {
    let build = format!("{module}/{BUILD}");
    let Ok(text) = fs::read_to_string(dir.join(&build)) else {
        return (BTreeSet::new(), vec![Finding::new(build, "is missing")]);
    };
    let text = strip_comments(&text);
    let mut out = refuse_in_build(&build, &text);
    let refs = libs_refs(&text);
    let declared: BTreeSet<String> = accessors.iter().filter(|(a, _)| refs.contains(*a)).map(|(_, m)| m.clone()).collect();
    let direct: Pins = row.iter().filter(|(n, _)| declared.contains(*n)).map(|(n, v)| (n.clone(), v.clone())).collect();
    out.extend(check_lockfile(dir, &format!("{module}/gradle.lockfile"), &direct));
    (declared, out)
}

pub(super) fn refuse_in_build(file: &str, text: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    if block(text, "repositories").is_some() {
        out.push(Finding::new(file, "has a repositories block; repositories are settings.gradle.kts's"));
    }
    for call in ["files", "fileTree"].into_iter().filter(|c| calls(text, c)) {
        out.push(Finding::new(file, format!("depends on {call}(…); every package comes from Maven Central")));
    }
    if text.contains(&format!("\"{LINGARA_GROUP}:")) || text.contains(&format!("'{LINGARA_GROUP}:")) {
        out.push(Finding::new(file, format!("writes a {LINGARA_GROUP} coordinate as a string; it comes from the catalogue")));
    }
    out
}

/// Every `libs.<accessor>` the build file references.
fn libs_refs(text: &str) -> BTreeSet<String> {
    let starts = text.match_indices("libs.").filter(|(i, _)| !preceded_by_ident(text, *i));
    let tokens = starts.map(|(i, _)| {
        let rest = &text[i + "libs.".len()..];
        rest[..rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.')).unwrap_or(rest.len())].trim_end_matches('.')
    });
    tokens.map(str::to_string).collect()
}

/// Each line a `#` comment, `group:artifact:version=configurations`, or the
/// one `empty=…` line; anything else is refused, never skipped (X1 D4).
fn check_lockfile(dir: &Path, file: &str, direct: &Pins) -> Vec<Finding> {
    let Ok(text) = fs::read_to_string(dir.join(file)) else {
        return vec![Finding::new(file, "is missing")];
    };
    let mut out = Vec::new();
    let mut locked = Pins::new();
    let mut empty = 0;
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        if line.starts_with("empty=") {
            empty += 1;
            if empty == 1 {
                continue;
            }
        }
        match lock_line(line) {
            Some((module, version)) => _ = locked.insert(module, version),
            None => out.push(Finding::new(file, format!("cannot classify the line `{line}`"))),
        }
    }
    out.extend(check_locked(file, &locked, direct));
    out
}

fn lock_line(line: &str) -> Option<(String, String)> {
    let (coordinate, configurations) = line.split_once('=')?;
    let parts: Vec<&str> = coordinate.split(':').collect();
    match parts.as_slice() {
        [group, artifact, version] if !configurations.is_empty() && [group, artifact, version].iter().all(|p| !p.is_empty()) => Some((format!("{group}:{artifact}"), version.to_string())),
        _ => None,
    }
}

// ── Text ──

/// `text` without `//` line comments. A `//` inside a string is code: a
/// `uri("https://…")` in a refused repository must still be read whole.
pub(super) fn strip_comments(text: &str) -> String {
    text.lines().map(code_of).collect::<Vec<_>>().join("\n")
}

fn code_of(line: &str) -> &str {
    let mut quoted = false;
    let bytes = line.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        match b {
            b'"' => quoted = !quoted,
            b'/' if !quoted && bytes.get(i + 1) == Some(&b'/') => return &line[..i],
            _ => {}
        }
    }
    line
}

/// The contents of the first `<name> {…}` block, braces matched.
fn block<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    for (i, _) in text.match_indices(name).filter(|(i, _)| !preceded_by_ident(text, *i)) {
        let after = &text[i + name.len()..];
        let Some(body) = after.trim_start().strip_prefix('{') else { continue };
        let mut depth = 1;
        for (j, c) in body.char_indices() {
            depth += match c {
                '{' => 1,
                '}' => -1,
                _ => 0,
            };
            if depth == 0 {
                return Some(&body[..j]);
            }
        }
    }
    None
}

/// `<name>(` as a call, not the tail of a longer name.
fn calls(text: &str, name: &str) -> bool {
    text.match_indices(&format!("{name}(")).any(|(i, _)| !preceded_by_ident(text, i))
}

fn preceded_by_ident(text: &str, at: usize) -> bool {
    text[..at].chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}
