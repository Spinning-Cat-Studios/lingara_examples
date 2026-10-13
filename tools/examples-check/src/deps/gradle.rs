//! A Gradle build's version catalogue, settings, build files and lockfiles
//! (ADR 13.10.26i D5), read as text, line by line: Gradle is never run, as
//! the Cargo reader never runs Cargo. The catalogue is the one file that
//! writes a version; every module locks its own configurations in a
//! `gradle.lockfile` beside its build file. The row is held to the union of
//! the modules, as a Cargo row is to the union of its manifests: an artefact
//! is held to its pin in the lockfile of every module whose build file names
//! it, and one that is only transitive (`lingara-embed-kotlin` in `lwjgl3`,
//! through `core`) is not held to the row.

mod build;

use super::{Pins, compare_pins};
use crate::index::Finding;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use toml::{Table, Value};

/// The one file a version is written in, so the one the bump writes.
pub(super) const CATALOGUE: &str = "gradle/libs.versions.toml";
const BUILD: &str = "build.gradle.kts";
const LINGARA_GROUP: &str = "com.getlingara";

/// A row's `name` is `group:artifact`.
pub fn is_lingara(name: &str) -> bool {
    name.split_once(':').is_some_and(|(group, _)| group == LINGARA_GROUP)
}

pub(super) fn check(dir: &Path, row: &Pins) -> Vec<Finding> {
    let (accessors, mut out) = check_catalogue(dir, row);
    let (modules, found) = build::check_settings(dir);
    out.extend(found);
    if let Ok(text) = fs::read_to_string(dir.join(BUILD)) {
        out.extend(build::refuse_in_build(BUILD, &build::strip_comments(&text)));
    }
    let mut union = BTreeSet::new();
    for module in &modules {
        let (declared, found) = build::check_module(dir, module, &accessors, row);
        union.extend(declared);
        out.extend(found);
    }
    let missing = row.keys().filter(|n| !union.contains(*n));
    out.extend(missing.map(|name| Finding::new(BUILD, format!("{name} is in examples.toml but no module's build file declares it"))));
    out
}

// ── The catalogue ──

/// Every version exact, every `com.getlingara` library on its own
/// `[versions]` key, and the catalogue naming exactly the row's artefacts.
/// Returns each Lingara library's accessor (`libs.<accessor>`) and module.
fn check_catalogue(dir: &Path, row: &Pins) -> (BTreeMap<String, String>, Vec<Finding>) {
    let doc = match read_toml(dir, CATALOGUE) {
        Ok(doc) => doc,
        Err(f) => return (BTreeMap::new(), vec![f]),
    };
    let versions = section(&doc, "versions");
    let mut out: Vec<Finding> = versions
        .iter()
        .filter(|(_, v)| !v.as_str().is_some_and(is_exact))
        .map(|(key, _)| Finding::new(CATALOGUE, format!("[versions] {key} is not one exact version")))
        .collect();
    let mut accessors = BTreeMap::new();
    let mut pins = Pins::new();
    for (alias, value) in &section(&doc, "libraries") {
        match library(alias, value, &versions) {
            Ok((module, version)) if is_lingara(&module) => {
                accessors.insert(accessor(alias), module.clone());
                pins.insert(module, version);
            }
            Ok(_) => {}
            Err(why) => out.push(Finding::new(CATALOGUE, why)),
        }
    }
    for (alias, value) in &section(&doc, "plugins") {
        if let Err(why) = plugin(alias, value, &versions) {
            out.push(Finding::new(CATALOGUE, why));
        }
    }
    out.extend(compare_pins(CATALOGUE, &pins, row, is_lingara));
    (accessors, out)
}

fn section(doc: &Table, name: &str) -> Table {
    doc.get(name).and_then(Value::as_table).cloned().unwrap_or_default()
}

/// One version: never a range, a dynamic `+`, `latest.*` or a rich form.
fn is_exact(version: &str) -> bool {
    !version.is_empty() && !version.starts_with("latest.") && !version.contains(['+', '[', ']', '(', ')', ',', '*', ' '])
}

/// Gradle's accessor rule: `-` and `_` in an alias read as `.`.
fn accessor(alias: &str) -> String {
    alias.replace(['-', '_'], ".")
}

/// `(module, version)` for a library entry, or why it is refused.
fn library(alias: &str, value: &Value, versions: &Table) -> Result<(String, String), String> {
    let table = match value {
        Value::String(notation) => {
            let (module, version) = notation.rsplit_once(':').ok_or_else(|| format!("cannot classify the library {alias}"))?;
            return if is_lingara(module) { Err(inline(alias, module)) } else { exact(alias, version).map(|v| (module.to_string(), v)) };
        }
        Value::Table(table) => table,
        _ => return Err(format!("cannot classify the library {alias}")),
    };
    let module = match (table.get("module").and_then(Value::as_str), table.get("group").and_then(Value::as_str), table.get("name").and_then(Value::as_str)) {
        (Some(module), _, _) => module.to_string(),
        (None, Some(group), Some(name)) => format!("{group}:{name}"),
        _ => return Err(format!("the library {alias} names no module")),
    };
    let version = version_of(alias, &module, table.get("version"), versions)?;
    Ok((module, version))
}

/// A plugin is held to the same exact-version rule as a library.
fn plugin(alias: &str, value: &Value, versions: &Table) -> Result<(), String> {
    match value {
        Value::String(notation) => notation.rsplit_once(':').map(|(_, v)| exact(alias, v).map(drop)).unwrap_or_else(|| Err(format!("the plugin {alias} has no version"))),
        Value::Table(table) => version_of(alias, "", table.get("version"), versions).map(drop),
        _ => Err(format!("cannot classify the plugin {alias}")),
    }
}

/// An inline exact version, or a `version.ref` to an exact `[versions]` key.
/// A `com.getlingara` module takes only the ref named by its own artifact id,
/// so the bump moves one pin and nothing else (D5).
fn version_of(alias: &str, module: &str, version: Option<&Value>, versions: &Table) -> Result<String, String> {
    let artifact = module.rsplit_once(':').map_or(module, |(_, a)| a);
    match version {
        Some(Value::String(_)) if is_lingara(module) => Err(inline(alias, module)),
        Some(Value::String(v)) => exact(alias, v),
        Some(Value::Table(rich)) => match rich.get("ref").and_then(Value::as_str).filter(|_| rich.len() == 1) {
            Some(key) if is_lingara(module) && key != artifact => Err(format!("{alias} takes version.ref `{key}`; a {LINGARA_GROUP} library takes its own key, `{artifact}`")),
            Some(key) => versions.get(key).and_then(Value::as_str).map(str::to_string).ok_or_else(|| format!("{alias}'s version.ref `{key}` is not a [versions] key")),
            None => Err(format!("{alias} has a rich version; examples pin one exact version")),
        },
        None => Err(format!("{alias} has no version; examples pin one exact version")),
        Some(_) => Err(format!("cannot classify {alias}'s version")),
    }
}

fn exact(alias: &str, version: &str) -> Result<String, String> {
    if is_exact(version) { Ok(version.to_string()) } else { Err(format!("{alias} is `{version}`; examples pin one exact version")) }
}

fn inline(alias: &str, module: &str) -> String {
    let artifact = module.rsplit_once(':').map_or(module, |(_, a)| a);
    format!("{alias} writes its version inline; a {LINGARA_GROUP} library takes version.ref = \"{artifact}\"")
}

fn read_toml(dir: &Path, file: &str) -> Result<Table, Finding> {
    let text = fs::read_to_string(dir.join(file)).map_err(|_| Finding::new(file, "is missing"))?;
    toml::from_str(&text).map_err(|e| Finding::new(file, format!("is not TOML: {}", e.message())))
}
