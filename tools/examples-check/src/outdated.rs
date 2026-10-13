//! `outdated` (ADR 4.10.26b D4): the newest release of every row's Lingara
//! packages, by SemVer 2 precedence with pre-releases included. Never a
//! dist-tag — `latest` lags when a family publishes with `--tag next` — and
//! never an edit: `bump` is the writer.

use crate::index::{self, Registry};
use semver::Version;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

/// Each registry's public metadata base URL, read from a file so the tests
/// point the same code at a fixture server and never at the network.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryUrls {
    pub npm: String,
    pub go: String,
    pub packagist: String,
    /// crates.io's sparse index (ADR 13.10.26h D5).
    pub crates: String,
    /// Maven Central's repository root (ADR 13.10.26i D5).
    pub maven: String,
}

pub fn load_urls(path: &Path) -> Result<RegistryUrls, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e.message()))
}

/// Prints `<path> <name> <pinned> → <newest>` for each package behind.
pub fn run(root: &Path, urls: &RegistryUrls, out: &mut dyn Write) -> Result<(), String> {
    let index = index::load(root).map_err(|f| f.to_string())?;
    let mut newest: BTreeMap<(Registry, String), Option<String>> = BTreeMap::new();
    for row in &index.examples {
        for pkg in &row.packages {
            let key = (pkg.registry, pkg.name.clone());
            if !newest.contains_key(&key) {
                let versions = fetch_versions(urls, pkg.registry, &pkg.name)?;
                let (found, skipped) = newest_of(&versions);
                for version in skipped {
                    eprintln!("{} {}: skipped {version}, which is not SemVer", pkg.registry.as_str(), pkg.name);
                }
                newest.insert(key.clone(), found);
            }
            if let Some(n) = newest[&key].as_deref().filter(|n| is_behind(&pkg.version, n)) {
                writeln!(out, "{} {} {} → {}", row.path, pkg.name, pkg.version, n).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// Every published version, in the registry's own spelling.
fn fetch_versions(urls: &RegistryUrls, registry: Registry, name: &str) -> Result<Vec<String>, String> {
    match registry {
        Registry::Npm => {
            let body = get(&format!("{}/{}", base(&urls.npm), name.replace('/', "%2F")))?;
            let doc = parse_json(&body)?;
            Ok(doc.get("versions").and_then(Value::as_object).map(|o| o.keys().cloned().collect()).unwrap_or_default())
        }
        Registry::Go => {
            let body = get(&format!("{}/{}/@v/list", base(&urls.go), escape_module(name)))?;
            Ok(body.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
        }
        Registry::Packagist => {
            let body = get(&format!("{}/p2/{name}.json", base(&urls.packagist)))?;
            let doc = parse_json(&body)?;
            let releases = doc.get("packages").and_then(|p| p.get(name)).and_then(Value::as_array);
            Ok(releases.into_iter().flatten().filter_map(|r| r.get("version")?.as_str().map(String::from)).collect())
        }
        Registry::Crates => {
            let body = get(&format!("{}/{}", base(&urls.crates), sparse_path(name)))?;
            body.lines().map(str::trim).filter(|l| !l.is_empty()).map(unyanked).filter_map(Result::transpose).collect()
        }
        Registry::Maven => {
            let path = maven_metadata_path(name).ok_or_else(|| format!("{name} is not group:artifact"))?;
            Ok(metadata_versions(&get(&format!("{}/{path}", base(&urls.maven)))?))
        }
    }
}

/// `com.getlingara:lingara-embed-kotlin` →
/// `com/getlingara/lingara-embed-kotlin/maven-metadata.xml`.
pub(crate) fn maven_metadata_path(name: &str) -> Option<String> {
    let (group, artifact) = name.split_once(':')?;
    Some(format!("{}/{artifact}/maven-metadata.xml", group.replace('.', "/")))
}

/// Every `<version>` in a `maven-metadata.xml`. `<latest>` and `<release>`
/// are ignored, as npm's dist-tags are: newest is the reader's decision.
fn metadata_versions(xml: &str) -> Vec<String> {
    xml.split("<version>").skip(1).filter_map(|rest| rest.split_once("</version>")).map(|(v, _)| v.trim().to_string()).collect()
}

/// The sparse index's path for a crate: `1/`, `2/`, `3/<c1>/` or
/// `<c1c2>/<c3c4>/` by the name's length, then the lower-cased name.
pub(crate) fn sparse_path(name: &str) -> String {
    let name = name.to_ascii_lowercase();
    match name.len() {
        1 => format!("1/{name}"),
        2 => format!("2/{name}"),
        3 => format!("3/{}/{name}", &name[..1]),
        _ => format!("{}/{}/{name}", &name[..2], &name[2..4]),
    }
}

/// One index line's `vers`, or `None` when it is yanked.
fn unyanked(line: &str) -> Result<Option<String>, String> {
    let entry = parse_json(line)?;
    let yanked = entry.get("yanked").and_then(Value::as_bool).unwrap_or(false);
    let version = entry.get("vers").and_then(Value::as_str).ok_or("a crates index line has no `vers`")?;
    Ok((!yanked).then(|| version.to_string()))
}

fn base(url: &str) -> &str {
    url.trim_end_matches('/')
}

fn get(url: &str) -> Result<String, String> {
    let mut response = ureq::get(url).call().map_err(|e| format!("{url}: {e}"))?;
    response.body_mut().read_to_string().map_err(|e| format!("{url}: {e}"))
}

fn parse_json(body: &str) -> Result<Value, String> {
    serde_json::from_str(body).map_err(|e| format!("registry metadata is not JSON: {e}"))
}

/// The Go proxy's case encoding: an upper-case letter is `!` and its lower case.
fn escape_module(module: &str) -> String {
    module
        .chars()
        .flat_map(|c| if c.is_ascii_uppercase() { vec!['!', c.to_ascii_lowercase()] } else { vec![c] })
        .collect()
}

/// Go and Packagist spell a version with one leading `v`, which is stripped
/// for parsing only; npm never writes one.
fn semver_of(version: &str) -> Option<Version> {
    Version::parse(version.strip_prefix('v').unwrap_or(version)).ok()
}

/// The newest SemVer release, and every version skipped because it is not
/// SemVer: `run` names those on stderr, so a skip cannot hide a release
/// (ADR 13.10.26i D5).
pub(crate) fn newest_of(versions: &[String]) -> (Option<String>, Vec<String>) {
    let mut skipped = Vec::new();
    let parsed = versions.iter().filter_map(|v| match semver_of(v) {
        Some(s) => Some((s, v)),
        None => {
            skipped.push(v.clone());
            None
        }
    });
    let newest = parsed.max_by(|a, b| a.0.cmp(&b.0)).map(|(_, v)| v.clone());
    (newest, skipped)
}

fn is_behind(pinned: &str, newest: &str) -> bool {
    matches!((semver_of(pinned), semver_of(newest)), (Some(p), Some(n)) if n > p)
}
