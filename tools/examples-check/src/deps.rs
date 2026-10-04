//! `deps` (ADR 4.10.26b D4): each row's manifest and committed lock file
//! agree with the row, pin Lingara's packages exactly, and resolve every
//! dependency from the ecosystem's public default registry.
//!
//! A later registry adds its reader here (D3).

mod composer;
mod go;
mod npm;

use crate::index::{self, Example, Finding, Registry};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// name → version (or spec), as one file declares or locks it.
type Pins = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ecosystem {
    Npm,
    Go,
    Composer,
}

impl Ecosystem {
    pub fn for_language(language: &str) -> Option<Self> {
        match language {
            "typescript" | "javascript" => Some(Self::Npm),
            "go" => Some(Self::Go),
            "php" => Some(Self::Composer),
            _ => None,
        }
    }

    pub fn registry(self) -> Registry {
        match self {
            Self::Npm => Registry::Npm,
            Self::Go => Registry::Go,
            Self::Composer => Registry::Packagist,
        }
    }

    pub fn manifest(self) -> &'static str {
        match self {
            Self::Npm => "package.json",
            Self::Go => "go.mod",
            Self::Composer => "composer.json",
        }
    }
}

pub fn check(root: &Path) -> Vec<Finding> {
    match index::load(root) {
        Ok(index) => index.examples.iter().flat_map(|row| check_row(root, row)).collect(),
        Err(finding) => vec![finding],
    }
}

fn check_row(root: &Path, row: &Example) -> Vec<Finding> {
    let Some(eco) = Ecosystem::for_language(&row.language) else {
        return vec![Finding::new(&row.path, format!("no dependency reader for language `{}`", row.language))];
    };
    let mut out: Vec<Finding> = row
        .packages
        .iter()
        .filter(|p| p.registry != eco.registry())
        .map(|p| Finding::new(&row.path, format!("{} is a {} package; a {} example installs from {}", p.name, p.registry.as_str(), row.language, eco.registry().as_str())))
        .collect();
    let dir = root.join(&row.path);
    let pins = row_pins(row, eco.registry());
    let found = match eco {
        Ecosystem::Npm => npm::check(&dir, &pins),
        Ecosystem::Go => go::check(root, &dir, &pins),
        Ecosystem::Composer => composer::check(&dir, &pins),
    };
    out.extend(found.into_iter().map(|f| Finding::new(format!("{}/{}", row.path, f.path), f.message)));
    out
}

fn row_pins(row: &Example, registry: Registry) -> Pins {
    row.packages.iter().filter(|p| p.registry == registry).map(|p| (p.name.clone(), p.version.clone())).collect()
}

/// The manifest names exactly the row's Lingara packages, each at the row's
/// exact version — never a range.
fn compare_pins(file: &str, declared: &Pins, row: &Pins, is_lingara: fn(&str) -> bool) -> Vec<Finding> {
    let mut out = Vec::new();
    for (name, spec) in declared.iter().filter(|(n, _)| is_lingara(n)) {
        match row.get(name) {
            None => out.push(Finding::new(file, format!("{name} is not in this row's packages in examples.toml"))),
            Some(v) if v != spec => out.push(Finding::new(file, format!("{name} is `{spec}`; examples.toml pins exactly `{v}`"))),
            Some(_) => {}
        }
    }
    for name in row.keys().filter(|n| !declared.contains_key(*n)) {
        out.push(Finding::new(file, format!("{name} is in examples.toml but not in {file}")));
    }
    out
}

/// The lock file resolves each of the row's packages to the row's version.
fn check_locked(file: &str, locked: &Pins, row: &Pins) -> Vec<Finding> {
    row.iter()
        .filter_map(|(name, want)| match locked.get(name) {
            Some(got) if got == want => None,
            Some(got) => Some(Finding::new(file, format!("resolves {name} to {got}, not {want}"))),
            None => Some(Finding::new(file, format!("does not lock {name}"))),
        })
        .collect()
}

fn read_json(dir: &Path, file: &str) -> Result<serde_json::Value, Finding> {
    let text = fs::read_to_string(dir.join(file)).map_err(|_| Finding::new(file, "is missing"))?;
    serde_json::from_str(&text).map_err(|e| Finding::new(file, format!("is not JSON: {e}")))
}

/// Every string-valued entry of the named objects, merged.
fn json_deps(manifest: &serde_json::Value, fields: &[&str]) -> Pins {
    let mut pins = Pins::new();
    for field in fields {
        let Some(obj) = manifest.get(field).and_then(|v| v.as_object()) else { continue };
        for (name, spec) in obj {
            let spec = spec.as_str().map_or_else(|| spec.to_string(), str::to_owned);
            pins.insert(name.clone(), spec);
        }
    }
    pins
}
