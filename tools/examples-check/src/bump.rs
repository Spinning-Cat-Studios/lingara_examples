//! `bump` (ADR 4.10.26b D4, D7): one package's version, written into every
//! row that lists it and into each of those examples' manifests. Each edit
//! replaces the one version string, so a bump PR's diff is exactly the version
//! lines. Lock files are the language recipe's (`lock-example-<language>`).

use crate::deps::Ecosystem;
use crate::index::{INDEX_FILE, Registry};
use std::fs;
use std::path::Path;
use toml_edit::{DocumentMut, Item, Value};

/// Writes the new version and returns the paths it touched.
pub fn run(root: &Path, registry: Registry, name: &str, version: &str) -> Result<Vec<String>, String> {
    let text = read(root, INDEX_FILE)?;
    let (bumped, rows) = bump_index(&text, registry, name, version)?;
    let mut touched = Vec::new();
    write_if_changed(root, INDEX_FILE, &text, &bumped, &mut touched)?;
    for (path, language) in rows {
        let ecosystem = Ecosystem::for_language(&language).ok_or_else(|| format!("{path}: no manifest writer for `{language}`"))?;
        let manifest = format!("{path}/{}", ecosystem.manifest());
        let before = read(root, &manifest)?;
        let after = match ecosystem {
            Ecosystem::Npm | Ecosystem::Composer => replace_json_dep(&before, name, version),
            Ecosystem::Go => replace_go_require(&before, name, version),
        };
        write_if_changed(root, &manifest, &before, &after, &mut touched)?;
    }
    Ok(touched)
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
