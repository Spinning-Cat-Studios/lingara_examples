//! `Cargo.toml` and `Cargo.lock` (ADR 13.10.26h D5). A Cargo example has one
//! manifest per package, each with its own lock beside it, so the row is held
//! to the union of them: every row package is declared directly by at least
//! one manifest, and a crate that is only transitive (the game's
//! `lingara-embed`, through `bevy_lingara`) is not held to the row.

use super::{Pins, check_locked, compare_declared};
use crate::index::Finding;
use std::fs;
use std::path::Path;
use toml::{Table, Value};

/// How Cargo spells crates.io in a lock, sparse protocol included.
const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";
const DEP_TABLES: [&str; 5] = ["dependencies", "dev-dependencies", "build-dependencies", "dev_dependencies", "build_dependencies"];
/// The keys a table-form dependency may carry and still come from crates.io.
const REGISTRY_KEYS: [&str; 6] = ["version", "features", "default-features", "default_features", "optional", "package"];
/// The keys that point a dependency somewhere else.
const SOURCE_KEYS: [(&str, &str); 4] = [("path", "a local path"), ("git", "git"), ("registry", "another registry"), ("registry-index", "another registry")];

pub fn is_lingara(name: &str) -> bool {
    matches!(name, "lingara" | "lingara-embed" | "bevy_lingara")
}

/// Every `Cargo.toml` under `dir`, relative to it, outside `target/` and
/// dot-directories.
pub(super) fn manifests(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    walk(dir, "", &mut found);
    found.sort();
    found
}

fn walk(dir: &Path, rel: &str, found: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir.join(rel)) else { return };
    for entry in entries.filter_map(Result::ok) {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
        let path = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        if entry.path().is_dir() {
            if name != "target" && !name.starts_with('.') {
                walk(dir, &path, found);
            }
        } else if name == "Cargo.toml" {
            found.push(path);
        }
    }
}

pub(super) fn check(dir: &Path, row: &Pins) -> Vec<Finding> {
    let files = manifests(dir);
    if files.is_empty() {
        return vec![Finding::new("Cargo.toml", "is missing")];
    }
    let mut out = Vec::new();
    let mut union = Pins::new();
    for file in &files {
        let (declared, found) = check_manifest(dir, file, row);
        union.extend(declared);
        out.extend(found);
    }
    let missing = row.keys().filter(|n| !union.contains_key(*n));
    out.extend(missing.map(|name| Finding::new("Cargo.toml", format!("{name} is in examples.toml but no Cargo.toml declares it"))));
    out
}

/// One manifest's sources, Lingara pins and lock, and the Lingara crates it
/// declares directly.
fn check_manifest(dir: &Path, file: &str, row: &Pins) -> (Pins, Vec<Finding>) {
    let doc = match read_toml(dir, file) {
        Ok(doc) => doc,
        Err(f) => return (Pins::new(), vec![f]),
    };
    let mut out: Vec<Finding> = ["patch", "replace"]
        .iter()
        .filter(|t| doc.contains_key(**t))
        .map(|t| Finding::new(file, format!("has a [{t}] table; packages come from crates.io only")))
        .collect();
    let mut lingara = Pins::new();
    for (name, value) in dep_tables(&doc).into_iter().flatten() {
        match classify(name, value) {
            Ok((name, spec)) if is_lingara(&name) => _ = lingara.insert(name, spec),
            Ok(_) => {}
            Err(why) => out.push(Finding::new(file, why)),
        }
    }
    out.extend(check_exact(file, &lingara, row));
    let lock = match file.rsplit_once('/') {
        Some((parent, _)) => format!("{parent}/Cargo.lock"),
        None => "Cargo.lock".to_string(),
    };
    let direct: Pins = row.iter().filter(|(n, _)| lingara.contains_key(*n)).map(|(n, v)| (n.clone(), v.clone())).collect();
    out.extend(check_lock(dir, &lock, &direct));
    (lingara, out)
}

/// Every Lingara spec begins with `=`, which is stripped before it is held
/// to the row: `=0.1.0-alpha.6` matches the row's `0.1.0-alpha.6`.
fn check_exact(file: &str, lingara: &Pins, row: &Pins) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut pins = Pins::new();
    for (name, spec) in lingara {
        match spec.strip_prefix('=') {
            Some(version) => _ = pins.insert(name.clone(), version.trim().to_string()),
            None => out.push(Finding::new(file, format!("{name} is `{spec}`; a Lingara crate is pinned with `=`"))),
        }
    }
    out.extend(compare_declared(file, &pins, row, is_lingara));
    out
}

/// The dependency tables: top level, per target, and the workspace's.
fn dep_tables(doc: &Table) -> Vec<&Table> {
    let mut parents = vec![doc];
    if let Some(targets) = doc.get("target").and_then(Value::as_table) {
        parents.extend(targets.values().filter_map(Value::as_table));
    }
    let mut tables: Vec<&Table> = parents.iter().flat_map(|p| DEP_TABLES.iter().filter_map(|t| p.get(*t)?.as_table())).collect();
    tables.extend(doc.get("workspace").and_then(|w| w.get("dependencies")).and_then(Value::as_table));
    tables
}

/// `(crate name, version spec)` for a crates.io dependency, or why it is not
/// one. A spec this cannot classify is refused, never passed (X1 D4).
fn classify(key: &str, value: &Value) -> Result<(String, String), String> {
    let table = match value {
        Value::String(spec) => return Ok((key.to_string(), spec.clone())),
        Value::Table(table) => table,
        _ => return Err(format!("cannot classify the dependency {key}")),
    };
    if let Some((_, why)) = SOURCE_KEYS.iter().find(|(k, _)| table.contains_key(*k)) {
        return Err(format!("{key} comes from {why}, not crates.io"));
    }
    if let Some(other) = table.keys().find(|k| !REGISTRY_KEYS.contains(&k.as_str())) {
        return Err(format!("cannot classify {key}'s `{other}`"));
    }
    let name = table.get("package").and_then(Value::as_str).unwrap_or(key);
    let spec = table.get("version").and_then(Value::as_str).unwrap_or_default();
    Ok((name.to_string(), spec.to_string()))
}

fn check_lock(dir: &Path, lock: &str, direct: &Pins) -> Vec<Finding> {
    let doc = match read_toml(dir, lock) {
        Ok(doc) => doc,
        Err(f) => return vec![f],
    };
    let mut out = Vec::new();
    let mut locked = Pins::new();
    for pkg in doc.get("package").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_table) {
        let field = |k: &str| pkg.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let (name, version) = (field("name"), field("version"));
        if let Some(source) = pkg.get("source").and_then(Value::as_str).filter(|s| *s != CRATES_IO) {
            out.push(Finding::new(lock, format!("{name} {version} comes from {source}, not crates.io")));
        }
        locked.insert(name, version);
    }
    out.extend(check_locked(lock, &locked, direct));
    out
}

fn read_toml(dir: &Path, file: &str) -> Result<Table, Finding> {
    let text = fs::read_to_string(dir.join(file)).map_err(|_| Finding::new(file, "is missing"))?;
    toml::from_str(&text).map_err(|e| Finding::new(file, format!("is not TOML: {}", e.message())))
}
