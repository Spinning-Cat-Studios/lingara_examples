//! `go.mod`, `go.sum` and `go.work` (ADR 4.10.26b D4), read line by line.

use super::{Pins, compare_pins};
use crate::index::Finding;
use std::fs;
use std::path::Path;

/// Directives that change nothing about where a module comes from.
const INERT: [&str; 6] = ["module", "go", "toolchain", "exclude", "retract", "godebug"];

pub fn is_lingara(module: &str) -> bool {
    module.starts_with("github.com/Spinning-Cat-Studios/")
}

pub(super) fn check(root: &Path, dir: &Path, row: &Pins) -> Vec<Finding> {
    let Ok(text) = fs::read_to_string(dir.join("go.mod")) else {
        return vec![Finding::new("go.mod", "is missing")];
    };
    let (requires, mut out) = parse(&text);
    out.extend(compare_pins("go.mod", &requires, row, is_lingara));
    out.extend(check_sum(dir, row));
    out.extend(check_work(root, dir));
    out
}

/// The `require`s, and a finding for every `replace` and every line this
/// cannot classify.
fn parse(text: &str) -> (Pins, Vec<Finding>) {
    let mut requires = Pins::new();
    let mut findings = Vec::new();
    let mut block: Option<&str> = None;
    for (n, raw) in text.lines().enumerate().map(|(n, l)| (n + 1, l)) {
        let line = raw.split("//").next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        if let Some(verb) = block {
            if line == ")" {
                block = None;
            } else {
                classify(verb, line, n, &mut requires, &mut findings);
            }
            continue;
        }
        let (verb, args) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        if args.trim() == "(" {
            block = Some(verb);
        } else {
            classify(verb, args.trim(), n, &mut requires, &mut findings);
        }
    }
    (requires, findings)
}

fn classify(verb: &str, args: &str, n: usize, requires: &mut Pins, findings: &mut Vec<Finding>) {
    if INERT.contains(&verb) {
        return;
    }
    let words: Vec<&str> = args.split_whitespace().collect();
    match (verb, words.as_slice()) {
        ("require", [module, version]) => {
            requires.insert(module.to_string(), version.to_string());
        }
        ("replace", _) => findings.push(Finding::new("go.mod", format!("line {n}: a replace directive"))),
        _ => findings.push(Finding::new("go.mod", format!("line {n}: cannot classify `{verb} {args}`"))),
    }
}

fn check_sum(dir: &Path, row: &Pins) -> Vec<Finding> {
    let Ok(sum) = fs::read_to_string(dir.join("go.sum")) else {
        return vec![Finding::new("go.sum", "is missing")];
    };
    row.iter()
        .filter(|(module, version)| !sum.lines().any(|l| l.starts_with(&format!("{module} {version} "))))
        .map(|(module, version)| Finding::new("go.sum", format!("has no line for {module} {version}")))
        .collect()
}

/// A `go.work` from the example up to the repo root, never above it, so an
/// operator's own outside the repo is not a finding.
fn check_work(root: &Path, dir: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    for ancestor in dir.ancestors() {
        if ancestor.join("go.work").exists() {
            let at = ancestor.strip_prefix(root).unwrap_or(ancestor).display().to_string();
            let at = if at.is_empty() { "the repository root".to_string() } else { at };
            out.push(Finding::new("go.mod", format!("a go.work at {at} overrides the module graph")));
        }
        if ancestor == root || !ancestor.starts_with(root) {
            break;
        }
    }
    out
}
