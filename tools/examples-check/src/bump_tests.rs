use crate::bump::{self, replace_go_require};
use crate::index::{self, Registry};
use crate::list;
use crate::test_support::{lines, tree, write};
use std::fs;

const INDEX: &str = r#"schema = 1

[[example]]
path     = "integrations/a"
section  = "integrations"
language = "typescript"
title    = "A"
packages = [
  { registry = "npm", name = "@lingara/api",  version = "0.1.0-alpha.9" },
  { registry = "npm", name = "@lingara/apps", version = "0.2.0" },
]

[[example]]
path = "apps/b"
section = "apps"
language = "typescript"
title = "B"
packages = [{registry="npm",name="@lingara/api",version="0.1.0-alpha.9"}]

[[example]]
path     = "apps/c"
section  = "apps"
language = "typescript"
title    = "C"
packages = [{ registry = "npm", name = "@lingara/apps", version = "0.2.0" }]
"#;

const PACKAGE_A: &str = "{\n  \"name\": \"a\",\n  \"dependencies\": {\n    \"@lingara/apps\": \"0.2.0\",\n    \"@lingara/api\" :  \"0.1.0-alpha.9\",\n    \"zod\": \"^3.0.0\"\n  }\n}\n";
const PACKAGE_B: &str = "{\"devDependencies\":{\"@lingara/api\":\"0.1.0-alpha.9\"}}";
const PACKAGE_C: &str = "{ \"dependencies\": { \"@lingara/apps\": \"0.2.0\" } }\n";
const LOCK: &str = "{\"packages\":{\"node_modules/@lingara/api\":{\"version\":\"0.1.0-alpha.9\"}}}";

fn read(root: &std::path::Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).expect("read")
}

/// A README whose table is the given index's, with prose either side.
fn readme_for(index_toml: &str) -> String {
    let dir = tree(&[("examples.toml", index_toml)]);
    let table = list::markdown(&index::load(dir.path()).expect("load"));
    format!("# Examples\n\n<!-- examples:start -->\n{table}<!-- examples:end -->\n\nProse after.\n")
}

/// 4.10.26b AC6 — only the named package's version lines change; rows,
/// manifests and lock files that do not name it are byte-identical.
#[test]
fn bump_rewrites_only_the_named_package() {
    let readme = readme_for(INDEX);
    let dir = tree(&[
        ("examples.toml", INDEX),
        ("README.md", &readme),
        ("integrations/a/package.json", PACKAGE_A),
        ("integrations/a/package-lock.json", LOCK),
        ("apps/b/package.json", PACKAGE_B),
        ("apps/b/package-lock.json", LOCK),
        ("apps/c/package.json", PACKAGE_C),
    ]);
    let root = dir.path();
    let touched = bump::run(root, Registry::Npm, "@lingara/api", "0.1.0-alpha.10").expect("bump");

    assert_eq!(touched, ["examples.toml", "README.md", "integrations/a/package.json", "apps/b/package.json"]);
    let up = |s: &str| s.replace("\"0.1.0-alpha.9\"", "\"0.1.0-alpha.10\"");
    assert_eq!(read(root, "examples.toml"), up(INDEX), "only the two version strings move");
    assert_eq!(read(root, "README.md"), readme_for(&up(INDEX)), "the table moves; the prose does not");
    let readme_findings: Vec<String> = lines(&index::check(root)).into_iter().filter(|l| l.starts_with("README.md")).collect();
    assert!(readme_findings.is_empty(), "a bump leaves the index check's README clean: {readme_findings:?}");
    assert_eq!(read(root, "integrations/a/package.json"), up(PACKAGE_A));
    assert_eq!(read(root, "apps/b/package.json"), up(PACKAGE_B));
    assert_eq!(read(root, "apps/c/package.json"), PACKAGE_C, "a manifest that does not name it");
    assert_eq!(read(root, "integrations/a/package-lock.json"), LOCK, "lock files are the recipe's");
    assert_eq!(read(root, "apps/b/package-lock.json"), LOCK);
}

/// 4.10.26b AC6 — the Go and Composer manifests, the same way.
#[test]
fn bump_rewrites_go_and_composer_manifests() {
    let module = "github.com/Spinning-Cat-Studios/lingara_api_clients/go";
    let go_mod = format!("module x\n\ngo 1.23\n\nrequire (\n\t{module} v0.1.0-alpha.9\n\tgolang.org/x/text v0.1.0-alpha.9 // indirect\n)\n");
    let want = go_mod.replacen(&format!("{module} v0.1.0-alpha.9"), &format!("{module} v0.1.0-alpha.10"), 1);
    assert_eq!(replace_go_require(&go_mod, module, "v0.1.0-alpha.10"), want);
    let one_line = format!("require {module} v0.1.0-alpha.9\n");
    assert_eq!(replace_go_require(&one_line, module, "v0.1.0-alpha.10"), format!("require {module} v0.1.0-alpha.10\n"));

    let index = "schema = 1\n[[example]]\npath = \"apps/p\"\nsection = \"apps\"\nlanguage = \"php\"\ntitle = \"P\"\npackages = [{ registry = \"packagist\", name = \"spinningcatstudios/lingara\", version = \"v0.1.0-alpha.9\" }]\n";
    let composer = "{\n    \"require\": {\n        \"php\": \">=8.2\",\n        \"spinningcatstudios/lingara\": \"v0.1.0-alpha.9\"\n    }\n}\n";
    let dir = tree(&[("examples.toml", index), ("README.md", &readme_for(index)), ("apps/p/composer.json", composer)]);
    write(dir.path(), "apps/p/composer.lock", "{}");
    bump::run(dir.path(), Registry::Packagist, "spinningcatstudios/lingara", "v0.1.0-alpha.10").expect("bump");
    assert_eq!(read(dir.path(), "apps/p/composer.json"), composer.replace("v0.1.0-alpha.9", "v0.1.0-alpha.10"));
    assert_eq!(read(dir.path(), "apps/p/composer.lock"), "{}");
}

const CARGO_INDEX: &str = r#"schema = 1
[[example]]
path = "games/bevy/g"
section = "games"
language = "rust"
title = "G"
packages = [
  { registry = "crates", name = "bevy_lingara", version = "0.1.0-alpha.6" },
  { registry = "crates", name = "lingara-embed", version = "0.1.0-alpha.6" },
]
"#;
const GAME: &str = "[package]\nname = \"game\"\n\n[workspace]\n\n[dependencies]\nbevy = \"=0.20.0\"\nbevy_lingara = \"=0.1.0-alpha.6\" # the kit\nureq = \"=3.4.2\"\n";
const SERVER: &str = "[package]\nname = \"server\"\n\n[dependencies]\nlingara-embed = { version = \"=0.1.0-alpha.6\", features = [\"server\"] }\n\n[target.'cfg(unix)'.dev-dependencies.lingara-embed]\nversion = \"=0.1.0-alpha.6\"\n";

/// 13.10.26h AC9 — a Cargo pin is rewritten to `=<new>` in the string and
/// the table forms, in every manifest of the example that declares it, and
/// nothing else moves: not the other crate, not a lock, not `target/`.
#[test]
fn cargo_bump_rewrites_only_the_pin() {
    let stale = "[dependencies]\nlingara-embed = \"=0.1.0-alpha.6\"\n";
    let dir = tree(&[
        ("examples.toml", CARGO_INDEX),
        ("README.md", &readme_for(CARGO_INDEX)),
        ("games/bevy/g/game/Cargo.toml", GAME),
        ("games/bevy/g/game/Cargo.lock", "# lock\n"),
        ("games/bevy/g/server/Cargo.toml", SERVER),
        ("games/bevy/g/game/target/debug/build/x/Cargo.toml", stale),
    ]);
    let root = dir.path();

    let touched = bump::run(root, Registry::Crates, "lingara-embed", "0.1.0-alpha.7").expect("bump");
    assert_eq!(touched, ["examples.toml", "README.md", "games/bevy/g/server/Cargo.toml"], "the game declares it only transitively");
    assert_eq!(read(root, "games/bevy/g/server/Cargo.toml"), SERVER.replace("=0.1.0-alpha.6", "=0.1.0-alpha.7"), "both forms, `=` kept");
    assert_eq!(read(root, "games/bevy/g/game/Cargo.toml"), GAME);
    assert_eq!(read(root, "games/bevy/g/game/target/debug/build/x/Cargo.toml"), stale, "target/ is not the example's");

    let touched = bump::run(root, Registry::Crates, "bevy_lingara", "0.1.0-alpha.7").expect("bump");
    assert_eq!(touched, ["examples.toml", "README.md", "games/bevy/g/game/Cargo.toml"]);
    assert_eq!(read(root, "games/bevy/g/game/Cargo.toml"), GAME.replace("=0.1.0-alpha.6", "=0.1.0-alpha.7"), "the comment and the neighbours stay");
    assert_eq!(read(root, "games/bevy/g/game/Cargo.lock"), "# lock\n", "lock files are the recipe's");
}

/// A README without the markers cannot be regenerated, so the bump refuses
/// rather than open a PR the index check will fail.
#[test]
fn bump_refuses_a_readme_without_markers() {
    let dir = tree(&[("examples.toml", INDEX), ("README.md", "# no table\n"), ("integrations/a/package.json", PACKAGE_A), ("apps/b/package.json", PACKAGE_B)]);
    let err = bump::run(dir.path(), Registry::Npm, "@lingara/api", "0.1.0-alpha.10").expect_err("no markers");
    assert_eq!(err, "README.md: no examples markers");
}

const GRADLE_INDEX: &str = r#"schema = 1

[[example]]
path = "games/jvm/g"
section = "games"
language = "kotlin"
title = "G"
packages = [
  { registry = "maven", name = "com.getlingara:lingara-embed-kotlin", version = "0.1.0-alpha.6" },
  { registry = "maven", name = "com.getlingara:lingara-embed-natives-desktop", version = "0.1.0-alpha.6" },
]
"#;
const CATALOGUE: &str = "[versions]\nlingara-embed-kotlin = \"0.1.0-alpha.6\" # the kit\nlingara-embed-natives-desktop = \"0.1.0-alpha.6\"\ngdx = \"1.14.2\"\n\n[libraries]\nlingara-embed-kotlin = { module = \"com.getlingara:lingara-embed-kotlin\", version.ref = \"lingara-embed-kotlin\" }\n";

/// 13.10.26i AC9 — a Maven bump rewrites `[versions].<artifact id>` in the
/// catalogue and no other entry: not the natives (merged on its own PR), not
/// a lockfile.
#[test]
fn gradle_bump_rewrites_only_its_own_version() {
    let dir = tree(&[
        ("examples.toml", GRADLE_INDEX),
        ("README.md", &readme_for(GRADLE_INDEX)),
        ("games/jvm/g/gradle/libs.versions.toml", CATALOGUE),
        ("games/jvm/g/game/gradle.lockfile", "com.getlingara:lingara-embed-kotlin:0.1.0-alpha.6=runtimeClasspath\n"),
    ]);
    let root = dir.path();

    let touched = bump::run(root, Registry::Maven, "com.getlingara:lingara-embed-kotlin", "0.1.0-alpha.7").expect("bump");
    assert_eq!(touched, ["examples.toml", "README.md", "games/jvm/g/gradle/libs.versions.toml"]);
    let want = CATALOGUE.replacen("lingara-embed-kotlin = \"0.1.0-alpha.6\"", "lingara-embed-kotlin = \"0.1.0-alpha.7\"", 1);
    assert_eq!(read(root, "games/jvm/g/gradle/libs.versions.toml"), want, "one line moves; the comment, the natives and the library table stay");
    assert_eq!(read(root, "games/jvm/g/game/gradle.lockfile"), "com.getlingara:lingara-embed-kotlin:0.1.0-alpha.6=runtimeClasspath\n", "lockfiles are the recipe's");
    let index = index::load(root).expect("index");
    let versions: Vec<&str> = index.examples[0].packages.iter().map(|p| p.version.as_str()).collect();
    assert_eq!(versions, ["0.1.0-alpha.7", "0.1.0-alpha.6"]);
}
