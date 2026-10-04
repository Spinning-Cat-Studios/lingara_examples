//! `package.json`, `package-lock.json` and `.npmrc` (ADR 4.10.26b D4).

use super::{Pins, check_locked, compare_pins, json_deps, read_json};
use crate::index::Finding;
use std::fs;
use std::path::Path;

const REGISTRY: &str = "https://registry.npmjs.org/";
const DEP_FIELDS: [&str; 4] = ["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"];
/// Spec prefixes that install from somewhere other than the registry.
const SOURCES: [(&str, &str); 9] = [
    ("file:", "a local path"),
    ("link:", "a link"),
    ("workspace:", "a workspace"),
    ("git+", "git"),
    ("git:", "git"),
    ("github:", "GitHub"),
    ("http:", "a URL"),
    ("https:", "a URL"),
    ("npm:", "an alias"),
];

pub fn is_lingara(name: &str) -> bool {
    name.starts_with("@lingara/")
}

pub(super) fn check(dir: &Path, row: &Pins) -> Vec<Finding> {
    let manifest = match read_json(dir, "package.json") {
        Ok(v) => v,
        Err(f) => return vec![f],
    };
    let declared = json_deps(&manifest, &DEP_FIELDS);
    let mut out: Vec<Finding> = declared
        .iter()
        .filter_map(|(name, spec)| source(spec).map(|why| Finding::new("package.json", format!("{name} comes from {why}, not the npm registry"))))
        .collect();
    out.extend(compare_pins("package.json", &declared, row, is_lingara));
    out.extend(check_lock(dir, row));
    out.extend(check_npmrc(dir));
    out
}

/// `None` for a registry version or range; otherwise where the spec points.
/// A spec this cannot classify is refused, never passed.
fn source(spec: &str) -> Option<&'static str> {
    let spec = spec.trim();
    if let Some((_, why)) = SOURCES.iter().find(|(prefix, _)| spec.starts_with(prefix)) {
        return Some(why);
    }
    if spec.contains('/') {
        return Some("a GitHub shorthand or a path");
    }
    let plain = |c: char| c.is_ascii_alphanumeric() || " .-+^~<>=|*".contains(c);
    (!spec.chars().all(plain)).then_some("a spec this check cannot classify")
}

fn check_lock(dir: &Path, row: &Pins) -> Vec<Finding> {
    let lock = match read_json(dir, "package-lock.json") {
        Ok(v) => v,
        Err(f) => return vec![f],
    };
    let Some(packages) = lock.get("packages").and_then(|p| p.as_object()) else {
        return vec![Finding::new("package-lock.json", "has no `packages` (lockfileVersion 2 or later is required)")];
    };
    let mut out = Vec::new();
    let mut locked = Pins::new();
    for (key, entry) in packages.iter().filter(|(k, _)| !k.is_empty()) {
        if entry.get("link").and_then(|l| l.as_bool()) == Some(true) {
            out.push(Finding::new("package-lock.json", format!("{key} is a link")));
        }
        if let Some(resolved) = entry.get("resolved").and_then(|r| r.as_str()).filter(|r| !r.starts_with(REGISTRY)) {
            out.push(Finding::new("package-lock.json", format!("{key} resolves from {resolved}, not {REGISTRY}")));
        }
        let top_level = key.strip_prefix("node_modules/").filter(|n| !n.contains("/node_modules/"));
        if let (Some(name), Some(version)) = (top_level, entry.get("version").and_then(|v| v.as_str())) {
            locked.insert(name.to_string(), version.to_string());
        }
    }
    out.extend(check_locked("package-lock.json", &locked, row));
    out
}

fn check_npmrc(dir: &Path) -> Vec<Finding> {
    let Ok(text) = fs::read_to_string(dir.join(".npmrc")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate().map(|(n, l)| (n + 1, l.trim())) {
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        match line.split_once('=') {
            Some((key, _)) if key.contains("registry") => out.push(Finding::new(".npmrc", format!("line {n}: sets a registry"))),
            Some(_) => {}
            None => out.push(Finding::new(".npmrc", format!("line {n}: cannot classify `{line}`"))),
        }
    }
    out
}
