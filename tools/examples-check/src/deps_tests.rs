use crate::deps;
use crate::test_support::{lines, tree, write};
use std::fs;
use std::path::Path;

const GO_MODULE: &str = "github.com/Spinning-Cat-Studios/lingara_api_clients/go";

const INDEX: &str = r#"schema = 1

[[example]]
path = "integrations/n"
section = "integrations"
language = "typescript"
title = "npm"
packages = [{ registry = "npm", name = "@lingara/api", version = "0.1.0-alpha.10" }]

[[example]]
path = "integrations/g"
section = "integrations"
language = "go"
title = "Go"
packages = [{ registry = "go", name = "github.com/Spinning-Cat-Studios/lingara_api_clients/go", version = "v0.1.0-alpha.10" }]

[[example]]
path = "apps/p"
section = "apps"
language = "php"
title = "Composer"
packages = [{ registry = "packagist", name = "spinningcatstudios/lingara", version = "v0.1.0-alpha.10" }]
"#;

const PACKAGE: &str = r#"{"dependencies":{"@lingara/api":"0.1.0-alpha.10","zod":"^3.23.0"}}"#;
const PACKAGE_LOCK: &str = r#"{"lockfileVersion":3,"packages":{"":{"name":"n"},
"node_modules/@lingara/api":{"version":"0.1.0-alpha.10","resolved":"https://registry.npmjs.org/@lingara/api/-/api-0.1.0-alpha.10.tgz"},
"node_modules/zod":{"version":"3.23.8","resolved":"https://registry.npmjs.org/zod/-/zod-3.23.8.tgz"}}}"#;
const COMPOSER: &str = r#"{"require":{"php":">=8.2","spinningcatstudios/lingara":"v0.1.0-alpha.10"}}"#;
const COMPOSER_LOCK: &str = r#"{"packages":[{"name":"spinningcatstudios/lingara","version":"v0.1.0-alpha.10",
"dist":{"type":"zip","url":"https://api.github.com/repos/x/y/zipball/abc"}}],"packages-dev":[]}"#;

fn go_mod() -> String {
    format!("module example.com/g\n\ngo 1.23\n\nrequire {GO_MODULE} v0.1.0-alpha.10\n")
}

fn go_sum() -> String {
    format!("{GO_MODULE} v0.1.0-alpha.10 h1:abc=\n{GO_MODULE} v0.1.0-alpha.10/go.mod h1:def=\n")
}

fn clean() -> tempfile::TempDir {
    tree(&[
        ("examples.toml", INDEX),
        ("integrations/n/package.json", PACKAGE),
        ("integrations/n/package-lock.json", PACKAGE_LOCK),
        ("integrations/g/go.mod", &go_mod()),
        ("integrations/g/go.sum", &go_sum()),
        ("apps/p/composer.json", COMPOSER),
        ("apps/p/composer.lock", COMPOSER_LOCK),
    ])
}

/// The findings after one edit to the clean tree.
fn after(edit: impl FnOnce(&Path)) -> Vec<String> {
    let dir = clean();
    edit(dir.path());
    lines(&deps::check(dir.path()))
}

fn replace_in(root: &Path, rel: &str, from: &str, to: &str) {
    let text = fs::read_to_string(root.join(rel)).expect("read");
    assert!(text.contains(from), "{rel} has no `{from}`");
    write(root, rel, &text.replace(from, to));
}

fn assert_finds(found: &[String], want: &str) {
    assert!(found.iter().any(|f| f.contains(want)), "expected `{want}` in {found:?}");
}

/// 4.10.26b AC2 — the row, the manifest and the lock file agree, exactly,
/// for npm, Go and Composer; a missing lock file is a finding.
#[test]
fn pins_must_match_the_index() {
    let dir = clean();
    assert_eq!(lines(&deps::check(dir.path())), Vec::<String>::new(), "a clean tree passes");

    // In the row, not the manifest.
    assert_finds(&after(|r| write(r, "integrations/n/package.json", r#"{"dependencies":{}}"#)), "integrations/n/package.json: @lingara/api is in examples.toml but not in package.json");
    assert_finds(&after(|r| write(r, "integrations/g/go.mod", "module example.com/g\n")), &format!("integrations/g/go.mod: {GO_MODULE} is in examples.toml but not in go.mod"));
    assert_finds(&after(|r| replace_in(r, "apps/p/composer.json", "\"spinningcatstudios/lingara\"", "\"other/lib\"")), "apps/p/composer.json: spinningcatstudios/lingara is in examples.toml");

    // In the manifest, not the row.
    assert_finds(&after(|r| replace_in(r, "integrations/n/package.json", "\"zod\"", "\"@lingara/apps\"")), "@lingara/apps is not in this row's packages");
    assert_finds(&after(|r| replace_in(r, "integrations/g/go.mod", "\n\ngo 1.23", "\n\ngo 1.23\nrequire github.com/Spinning-Cat-Studios/other v1.0.0")), "github.com/Spinning-Cat-Studios/other is not in this row's packages");
    assert_finds(&after(|r| replace_in(r, "apps/p/composer.json", "\"php\"", "\"spinningcatstudios/apps\"")), "spinningcatstudios/apps is not in this row's packages");

    // A range, or another version, instead of the exact pin.
    assert_finds(&after(|r| replace_in(r, "integrations/n/package.json", "\"0.1.0-alpha.10\"", "\"^0.1.0-alpha.10\"")), "@lingara/api is `^0.1.0-alpha.10`; examples.toml pins exactly `0.1.0-alpha.10`");
    assert_finds(&after(|r| replace_in(r, "integrations/n/package.json", "\"0.1.0-alpha.10\"", "\"~0.1.0-alpha.10\"")), "is `~0.1.0-alpha.10`");
    assert_finds(&after(|r| replace_in(r, "integrations/g/go.mod", "v0.1.0-alpha.10", "v0.1.0-alpha.9")), "is `v0.1.0-alpha.9`; examples.toml pins exactly");
    assert_finds(&after(|r| replace_in(r, "apps/p/composer.json", "\"v0.1.0-alpha.10\"", "\"^0.1\"")), "spinningcatstudios/lingara is `^0.1`");

    // The lock file resolves another version.
    assert_finds(&after(|r| replace_in(r, "integrations/n/package-lock.json", "\"version\":\"0.1.0-alpha.10\"", "\"version\":\"0.1.0-alpha.9\"")), "package-lock.json: resolves @lingara/api to 0.1.0-alpha.9, not 0.1.0-alpha.10");
    assert_finds(&after(|r| write(r, "integrations/g/go.sum", &go_sum().replace("alpha.10", "alpha.9"))), &format!("go.sum: has no line for {GO_MODULE} v0.1.0-alpha.10"));
    assert_finds(&after(|r| replace_in(r, "apps/p/composer.lock", "\"version\":\"v0.1.0-alpha.10\"", "\"version\":\"v0.1.0-alpha.9\"")), "composer.lock: resolves spinningcatstudios/lingara to v0.1.0-alpha.9");

    // A missing lock file.
    for (lock, want) in [
        ("integrations/n/package-lock.json", "integrations/n/package-lock.json: is missing"),
        ("integrations/g/go.sum", "integrations/g/go.sum: is missing"),
        ("apps/p/composer.lock", "apps/p/composer.lock: is missing"),
    ] {
        assert_finds(&after(|r| fs::remove_file(r.join(lock)).expect("rm")), want);
    }
}

/// 4.10.26b AC3 — every non-default source is refused, and a line a reader
/// cannot classify is a finding, not a pass.
#[test]
fn non_default_sources_are_refused() {
    for spec in ["file:../api", "link:../api", "workspace:*", "git+https://github.com/x/y.git", "https://example.com/z.tgz", "someone/zod", "zod@{weird}"] {
        let found = after(|r| replace_in(r, "integrations/n/package.json", "\"^3.23.0\"", &format!("\"{spec}\"")));
        assert_finds(&found, "integrations/n/package.json: zod comes from");
    }
    let off_registry = after(|r| replace_in(r, "integrations/n/package-lock.json", "https://registry.npmjs.org/zod", "https://npm.example.com/zod"));
    assert_finds(&off_registry, "node_modules/zod resolves from https://npm.example.com/zod");
    let link = after(|r| replace_in(r, "integrations/n/package-lock.json", "\"version\":\"3.23.8\"", "\"version\":\"3.23.8\",\"link\":true"));
    assert_finds(&link, "node_modules/zod is a link");

    assert_finds(&after(|r| write(r, "integrations/n/.npmrc", "@lingara:registry=https://npm.example.com/\n")), ".npmrc: line 1: sets a registry");
    assert_finds(&after(|r| write(r, "integrations/n/.npmrc", "registry=https://npm.example.com/\n")), ".npmrc: line 1: sets a registry");
    assert_finds(&after(|r| write(r, "integrations/n/.npmrc", "# fine\nengine-strict=true\nnonsense\n")), ".npmrc: line 3: cannot classify");

    let replace = after(|r| write(r, "integrations/g/go.mod", &format!("{}replace {GO_MODULE} => ../../go\n", go_mod())));
    assert_finds(&replace, "integrations/g/go.mod: line 6: a replace directive");
    let block = after(|r| write(r, "integrations/g/go.mod", &format!("{}replace (\n\t{GO_MODULE} => ../go\n)\n", go_mod())));
    assert_finds(&block, "line 7: a replace directive");
    assert_finds(&after(|r| write(r, "go.work", "go 1.23\nuse ./integrations/g\n")), "a go.work at the repository root overrides the module graph");
    assert_finds(&after(|r| write(r, "integrations/g/go.work", "go 1.23\n")), "a go.work at integrations/g");
    assert_finds(&after(|r| write(r, "integrations/g/go.mod", &format!("{}frobnicate x\n", go_mod()))), "line 6: cannot classify `frobnicate x`");

    assert_finds(&after(|r| replace_in(r, "apps/p/composer.json", "{\"require\"", "{\"repositories\":[{\"type\":\"path\",\"url\":\"../x\"}],\"require\"")), "composer.json: has a `repositories` entry");
    assert_finds(&after(|r| replace_in(r, "apps/p/composer.lock", "\"type\":\"zip\"", "\"type\":\"path\"")), "composer.lock: spinningcatstudios/lingara's dist is a local path");
}

/// A go.work above the repository root is the operator's, not a finding.
#[test]
fn a_go_work_above_the_root_is_not_a_finding() {
    let outer = tree(&[("go.work", "go 1.23\n")]);
    let root = outer.path().join("repo");
    for (rel, body) in [("examples.toml", INDEX.to_string()), ("integrations/g/go.mod", go_mod()), ("integrations/g/go.sum", go_sum())] {
        write(&root, rel, &body);
    }
    let found = lines(&deps::check(&root));
    assert!(!found.iter().any(|f| f.contains("go.work")), "{found:?}");
}
