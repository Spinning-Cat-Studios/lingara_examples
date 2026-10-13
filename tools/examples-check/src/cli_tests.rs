use crate::cli::{BAD_INPUT, CLEAN, Cli, FINDINGS, exit_code};
use crate::test_support::{tree, write};
use clap::Parser;
use std::path::Path;

const ROW: &str = "schema = 1\n[[example]]\npath = \"apps/a\"\nsection = \"apps\"\nlanguage = \"php\"\ntitle = \"A\"\n";

/// The exit code for `lingara-examples-check --root <root> <args…>`.
fn code(root: &Path, args: &[&str]) -> u8 {
    let mut argv = vec!["lingara-examples-check", "--root", root.to_str().expect("utf-8 path")];
    argv.extend_from_slice(args);
    exit_code(&Cli::try_parse_from(argv).expect("valid arguments"))
}

/// 4.10.26b D4 — the exit-code contract `make` and the workflows read:
/// 0 clean, 1 on a finding (or `lang` on a path with no row), 2 on input
/// the command cannot read.
#[test]
fn exit_codes_follow_the_contract() {
    let empty = tree(&[("examples.toml", "schema = 1\n"), ("README.md", "<!-- examples:start -->\n_No examples yet._\n<!-- examples:end -->\n")]);
    let root = empty.path();
    assert_eq!(code(root, &["index"]), CLEAN);
    assert_eq!(code(root, &["deps"]), CLEAN);
    assert_eq!(code(root, &["list", "--json"]), CLEAN);
    // 13.10.26h D5: crates is in the closed set, so a bump no row names is clean.
    assert_eq!(code(root, &["bump", "crates", "x", "1.0.0"]), CLEAN, "crates is a known registry");

    write(root, "apps/orphan/README.md", "x");
    assert_eq!(code(root, &["index"]), FINDINGS, "a directory with no row");

    let indexed = tree(&[("examples.toml", ROW), ("apps/a/composer.json", "{}")]);
    assert_eq!(code(indexed.path(), &["deps"]), FINDINGS, "no composer.lock");
    assert_eq!(code(indexed.path(), &["lang", "apps/a"]), CLEAN);
    assert_eq!(code(indexed.path(), &["lang", "apps/missing"]), FINDINGS, "a path with no row");

    let bare = tree(&[]);
    assert_eq!(code(bare.path(), &["index"]), FINDINGS, "a missing index is the index's own finding");
    assert_eq!(code(bare.path(), &["list"]), BAD_INPUT);
    assert_eq!(code(bare.path(), &["lang", "apps/a"]), BAD_INPUT);
    assert_eq!(code(indexed.path(), &["bump", "pypi", "x", "1.0.0"]), BAD_INPUT, "an unknown registry");
    assert_eq!(code(indexed.path(), &["outdated", "--registry-urls", "missing.toml"]), BAD_INPUT);
}

/// 13.10.26i AC10 — `maven` is in the closed set: a row may install from it,
/// and a bump no row names is clean.
#[test]
fn maven_is_accepted() {
    let row = "schema = 1\n[[example]]\npath = \"games/jvm/g\"\nsection = \"games\"\nlanguage = \"kotlin\"\ntitle = \"G\"\npackages = [{ registry = \"maven\", name = \"com.getlingara:lingara-embed-kotlin\", version = \"0.1.0-alpha.7\" }]\n";
    let dir = tree(&[("examples.toml", row), ("README.md", "x")]);
    assert_eq!(code(dir.path(), &["list", "--json"]), CLEAN, "a maven row parses");
    assert_eq!(code(dir.path(), &["lang", "games/jvm/g"]), CLEAN);
    let empty = tree(&[("examples.toml", "schema = 1\n"), ("README.md", "<!-- examples:start -->\n_No examples yet._\n<!-- examples:end -->\n")]);
    assert_eq!(code(empty.path(), &["bump", "maven", "com.getlingara:x", "1.0.0"]), CLEAN, "maven is a known registry");
}
