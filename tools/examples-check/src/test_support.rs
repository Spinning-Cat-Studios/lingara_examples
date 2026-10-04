//! Fixture trees for the unit tests.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// A temporary tree holding exactly these files.
pub fn tree(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    for (path, body) in files {
        write(dir.path(), path, body);
    }
    dir
}

pub fn write(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    fs::write(path, body).expect("write");
}

/// Every file under `root`, with its bytes.
pub fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("read_dir").map(|e| e.expect("entry").path()) {
            if entry.is_dir() {
                stack.push(entry);
            } else {
                files.insert(entry.clone(), fs::read(&entry).expect("read"));
            }
        }
    }
    files
}

/// The finding messages, as `<path>: <message>` lines.
pub fn lines(findings: &[crate::index::Finding]) -> Vec<String> {
    findings.iter().map(ToString::to_string).collect()
}
