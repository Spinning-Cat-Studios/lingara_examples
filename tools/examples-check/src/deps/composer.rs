//! `composer.json` and `composer.lock` (ADR 4.10.26b D4).

use super::{Pins, check_locked, compare_pins, json_deps, read_json};
use crate::index::Finding;
use serde_json::Value;
use std::path::Path;

pub fn is_lingara(name: &str) -> bool {
    name.starts_with("spinningcatstudios/")
}

pub(super) fn check(dir: &Path, row: &Pins) -> Vec<Finding> {
    let manifest = match read_json(dir, "composer.json") {
        Ok(v) => v,
        Err(f) => return vec![f],
    };
    let mut out = Vec::new();
    if manifest.get("repositories").is_some_and(|r| !is_empty(r)) {
        out.push(Finding::new("composer.json", "has a `repositories` entry; packages come from Packagist only"));
    }
    let declared = json_deps(&manifest, &["require", "require-dev"]);
    out.extend(compare_pins("composer.json", &declared, row, is_lingara));
    out.extend(check_lock(dir, row));
    out
}

fn is_empty(v: &Value) -> bool {
    match v {
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        Value::Null => true,
        _ => false,
    }
}

fn check_lock(dir: &Path, row: &Pins) -> Vec<Finding> {
    let lock = match read_json(dir, "composer.lock") {
        Ok(v) => v,
        Err(f) => return vec![f],
    };
    let mut out = Vec::new();
    let mut locked = Pins::new();
    let entries = ["packages", "packages-dev"].into_iter().filter_map(|k| lock.get(k).and_then(Value::as_array)).flatten();
    for pkg in entries {
        let name = pkg.get("name").and_then(Value::as_str).unwrap_or("<unnamed>");
        for kind in ["dist", "source"] {
            if pkg.get(kind).and_then(|d| d.get("type")).and_then(Value::as_str) == Some("path") {
                out.push(Finding::new("composer.lock", format!("{name}'s {kind} is a local path")));
            }
        }
        if let Some(version) = pkg.get("version").and_then(Value::as_str) {
            locked.insert(name.to_string(), version.to_string());
        }
    }
    out.extend(check_locked("composer.lock", &locked, row));
    out
}
