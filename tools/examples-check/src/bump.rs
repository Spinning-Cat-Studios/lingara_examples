//! `bump` (ADR 4.10.26b D4, D7): one package's version, written into every
//! row that lists it and into each of those examples' manifests. Each edit
//! replaces the one version string, so a bump PR's diff is exactly the version
//! lines. The README's table shows each row's versions, so it is regenerated
//! too; without that, every bump PR fails the `index` check. Lock files are
//! the language recipe's (`lock-example-<language>`). A Cargo example has a
//! manifest per package, and each that declares the package is written
//! (ADR 13.10.26h D5). A Gradle example's manifest is its version catalogue
//! (ADR 13.10.26i D5).

use crate::deps::Ecosystem;
use crate::index::{self, INDEX_FILE, README, Registry};
use std::fs;
use std::path::Path;
use toml_edit::{DocumentMut, Item, TableLike, Value};

const CARGO_DEP_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// Writes the new version and returns the paths it touched.
pub fn run(root: &Path, registry: Registry, name: &str, version: &str) -> Result<Vec<String>, String> {
    let text = read(root, INDEX_FILE)?;
    let (bumped, rows) = bump_index(&text, registry, name, version)?;
    let mut touched = Vec::new();
    write_if_changed(root, INDEX_FILE, &text, &bumped, &mut touched)?;
    regenerate_readme(root, &mut touched)?;
    for (path, language) in rows {
        let ecosystem = Ecosystem::for_language(&language).ok_or_else(|| format!("{path}: no manifest writer for `{language}`"))?;
        for rel in ecosystem.manifests(&root.join(&path)) {
            let manifest = format!("{path}/{rel}");
            let before = read(root, &manifest)?;
            let after = match ecosystem {
                Ecosystem::Npm | Ecosystem::Composer => replace_json_dep(&before, name, version),
                Ecosystem::Go => replace_go_require(&before, name, version),
                Ecosystem::Cargo => replace_cargo_dep(&before, name, version).map_err(|e| format!("{manifest}: {e}"))?,
                Ecosystem::Gradle => replace_catalogue_version(&before, name, version).map_err(|e| format!("{manifest}: {e}"))?,
            };
            write_if_changed(root, &manifest, &before, &after, &mut touched)?;
        }
    }
    Ok(touched)
}

/// The README's table from the index just written.
fn regenerate_readme(root: &Path, touched: &mut Vec<String>) -> Result<(), String> {
    let index = index::load(root).map_err(|f| f.to_string())?;
    let before = read(root, README)?;
    let after = index::splice_readme_table(&before, &crate::list::markdown(&index))
        .ok_or_else(|| format!("{README}: no examples markers"))?;
    write_if_changed(root, README, &before, &after, touched)
}

fn read(root: &Path, rel: &str) -> Result<String, String> {
    fs::read_to_string(root.join(rel)).map_err(|e| format!("{rel}: {e}"))
}

fn write_if_changed(root: &Path, rel: &str, before: &str, after: &str, touched: &mut Vec<String>) -> Result<(), String> {
    if before != after {
        fs::write(root.join(rel), after).map_err(|e| format!("{rel}: {e}"))?;
        touched.push(rel.to_string());
    }
    Ok(())
}

/// The rewritten index, and `(path, language)` of every row naming the package.
fn bump_index(text: &str, registry: Registry, name: &str, version: &str) -> Result<(String, Vec<(String, String)>), String> {
    let mut doc: DocumentMut = text.parse().map_err(|e| format!("{INDEX_FILE}: {e}"))?;
    let mut rows = Vec::new();
    let tables = doc.get_mut("example").and_then(Item::as_array_of_tables_mut);
    for table in tables.into_iter().flat_map(|t| t.iter_mut()) {
        let field = |k: &str| table.get(k).and_then(Item::as_str).unwrap_or_default().to_string();
        let (path, language) = (field("path"), field("language"));
        let packages = table.get_mut("packages").and_then(Item::as_array_mut);
        let mut named = false;
        for pkg in packages.into_iter().flat_map(|a| a.iter_mut()).filter_map(Value::as_inline_table_mut) {
            let is = |k: &str, want: &str| pkg.get(k).and_then(Value::as_str) == Some(want);
            if is("registry", registry.as_str())
                && is("name", name)
                && let Some(v) = pkg.get_mut("version")
            {
                set_keeping_decor(v, version);
                named = true;
            }
        }
        if named {
            rows.push((path, language));
        }
    }
    Ok((doc.to_string(), rows))
}

fn set_keeping_decor(value: &mut Value, version: &str) {
    let decor = value.decor().clone();
    *value = Value::from(version);
    *value.decor_mut() = decor;
}

/// Every `"<name>": "<old>"` pair's value, wherever it appears; `"name":
/// "<name>"` (a value, not a key) is left alone.
pub(crate) fn replace_json_dep(text: &str, name: &str, version: &str) -> String {
    let key = format!("\"{name}\"");
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(&key) {
        let (head, tail) = rest.split_at(i + key.len());
        out.push_str(head);
        rest = tail;
        if let Some((start, end)) = string_value_span(rest) {
            out.push_str(&rest[..start]);
            out.push_str(version);
            rest = &rest[end..];
        }
    }
    out.push_str(rest);
    out
}

/// After a key: optional space, `:`, optional space, then a quoted string.
fn string_value_span(s: &str) -> Option<(usize, usize)> {
    let after_colon = s.trim_start().strip_prefix(':')?.trim_start();
    let quote = s.len() - after_colon.len();
    after_colon.starts_with('"').then_some(())?;
    let start = quote + 1;
    Some((start, start + s[start..].find('"')?))
}

/// The version token after `<module> ` on a `require` line or inside a
/// `require (…)` block.
pub(crate) fn replace_go_require(text: &str, module: &str, version: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| {
            let body = line.trim_start().strip_prefix("require").map_or(line.trim_start(), str::trim_start);
            let Some(rest) = body.strip_prefix(module).and_then(|r| r.strip_prefix(' ')) else {
                return line.to_string();
            };
            let old = rest.split_whitespace().next().unwrap_or_default();
            let at = line.len() - rest.len() + (rest.len() - rest.trim_start().len());
            format!("{}{}{}", &line[..at], version, &line[at + old.len()..])
        })
        .collect()
}

/// `<crate> = "=<old>"`, or the `version` of a table form, set to
/// `"=<version>"` in every dependency table, top level and per target. The
/// `=` is kept: the reader refuses a Lingara crate pinned without one.
pub(crate) fn replace_cargo_dep(text: &str, name: &str, version: &str) -> Result<String, String> {
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.message().to_string())?;
    let pinned = format!("={version}");
    let root = doc.as_table_mut();
    set_cargo_dep(root, name, &pinned);
    if let Some(targets) = root.get_mut("target").and_then(Item::as_table_like_mut) {
        for (_, target) in targets.iter_mut() {
            if let Some(target) = target.as_table_like_mut() {
                set_cargo_dep(target, name, &pinned);
            }
        }
    }
    Ok(doc.to_string())
}

/// `[versions].<artifact id>` of a Gradle version catalogue, and nothing
/// else (ADR 13.10.26i D5). Each `com.getlingara` artefact has its own key,
/// so a bump of one moves one pin; `lock-example-kotlin` relocks the rest.
pub(crate) fn replace_catalogue_version(text: &str, name: &str, version: &str) -> Result<String, String> {
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.message().to_string())?;
    let artifact = name.rsplit_once(':').map_or(name, |(_, a)| a);
    if let Some(value) = doc.get_mut("versions").and_then(Item::as_table_like_mut).and_then(|t| t.get_mut(artifact)).and_then(Item::as_value_mut) {
        set_keeping_decor(value, version);
    }
    Ok(doc.to_string())
}

fn set_cargo_dep(parent: &mut dyn TableLike, name: &str, pinned: &str) {
    for table in CARGO_DEP_TABLES {
        let Some(item) = parent.get_mut(table).and_then(Item::as_table_like_mut).and_then(|deps| deps.get_mut(name)) else { continue };
        let version = match item {
            Item::Value(Value::String(_)) => item.as_value_mut(),
            Item::Value(Value::InlineTable(t)) => t.get_mut("version"),
            Item::Table(t) => t.get_mut("version").and_then(Item::as_value_mut),
            _ => None,
        };
        if let Some(value) = version {
            set_keeping_decor(value, pinned);
        }
    }
}
