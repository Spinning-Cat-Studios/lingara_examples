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

const CARGO_INDEX: &str = r#"schema = 1

[[example]]
path = "games/bevy/g"
section = "games"
language = "rust"
title = "Bevy"
packages = [
  { registry = "crates", name = "bevy_lingara", version = "0.1.0-alpha.6" },
  { registry = "crates", name = "lingara-embed", version = "0.1.0-alpha.6" },
  { registry = "crates", name = "lingara", version = "0.2.0-alpha.1" },
]
"#;
const GAME_TOML: &str = "[package]\nname = \"game\"\n\n[workspace]\n\n[dependencies]\nbevy = \"=0.20.0\"\nbevy_lingara = \"=0.1.0-alpha.6\"\nureq = \"=3.4.2\"\n";
const SERVER_TOML: &str = "[package]\nname = \"server\"\n\n[workspace]\n\n[dependencies]\nlingara-embed = { version = \"=0.1.0-alpha.6\", features = [\"server\"] }\nlingara = \"=0.2.0-alpha.1\"\n\n[dev-dependencies]\ntempfile = \"3\"\n";
const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

/// A lock holding the package itself and these crates.io packages.
fn cargo_lock(own: &str, packages: &[(&str, &str)]) -> String {
    let mut lock = format!("version = 4\n\n[[package]]\nname = \"{own}\"\nversion = \"0.0.0\"\n");
    for (name, version) in packages {
        lock.push_str(&format!("\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\nsource = \"{CRATES_IO}\"\n"));
    }
    lock
}

fn cargo_clean() -> tempfile::TempDir {
    // The game's lock holds lingara-embed at another version: transitive, so not the row's.
    let game_lock = cargo_lock("game", &[("bevy_lingara", "0.1.0-alpha.6"), ("lingara-embed", "0.1.0-alpha.5"), ("ureq", "3.4.2")]);
    let server_lock = cargo_lock("server", &[("lingara-embed", "0.1.0-alpha.6"), ("lingara", "0.2.0-alpha.1")]);
    tree(&[
        ("examples.toml", CARGO_INDEX),
        ("games/bevy/g/game/Cargo.toml", GAME_TOML),
        ("games/bevy/g/game/Cargo.lock", &game_lock),
        ("games/bevy/g/server/Cargo.toml", SERVER_TOML),
        ("games/bevy/g/server/Cargo.lock", &server_lock),
        ("games/bevy/g/game/target/debug/x/Cargo.toml", "[dependencies]\nx = { path = \"..\" }\n"),
    ])
}

fn cargo_after(edit: impl FnOnce(&Path)) -> Vec<String> {
    let dir = cargo_clean();
    edit(dir.path());
    lines(&deps::check(dir.path()))
}

/// 13.10.26h AC7 — the Cargo reader refuses every source but crates.io, a
/// Lingara crate not pinned `=`, and a manifest with no lock; matches `=X`
/// to row `X`; holds the row to the union of manifests and not a transitive
/// crate; and passes the example's real manifests.
#[test]
fn cargo_refuses_every_non_registry_source() {
    assert_eq!(lines(&deps::check(cargo_clean().path())), Vec::<String>::new(), "a clean tree passes; target/ is not read");

    let (game, server) = ("games/bevy/g/game/Cargo.toml", "games/bevy/g/server/Cargo.toml");
    assert_finds(&cargo_after(|r| replace_in(r, game, "ureq = \"=3.4.2\"", "ureq = { path = \"../u\" }")), &format!("{game}: ureq comes from a local path, not crates.io"));
    assert_finds(&cargo_after(|r| replace_in(r, server, "tempfile = \"3\"", "tempfile = { git = \"https://x/y\" }")), "tempfile comes from git");
    assert_finds(&cargo_after(|r| replace_in(r, game, "bevy = \"=0.20.0\"", "bevy = { version = \"0.20\", registry = \"mine\" }")), "bevy comes from another registry");
    let build_dep = |r: &Path| replace_in(r, game, "[dependencies]", "[build-dependencies]\ncc = { path = \"cc\" }\n\n[dependencies]");
    assert_finds(&cargo_after(build_dep), "cc comes from a local path");
    assert_finds(&cargo_after(|r| replace_in(r, game, "ureq = \"=3.4.2\"", "ureq = { workspace = true }")), "cannot classify ureq's `workspace`");
    assert_finds(&cargo_after(|r| replace_in(r, game, "[workspace]", "[workspace]\n\n[patch.crates-io]\nbevy = { path = \"b\" }")), &format!("{game}: has a [patch] table"));
    assert_finds(&cargo_after(|r| replace_in(r, server, "[workspace]", "[workspace]\n\n[replace]\n\"x:1.0.0\" = { path = \"x\" }")), "has a [replace] table");

    assert_finds(&cargo_after(|r| replace_in(r, server, "lingara = \"=0.2.0-alpha.1\"", "lingara = \"0.2.0-alpha.1\"")), "lingara is `0.2.0-alpha.1`; a Lingara crate is pinned with `=`");
    assert_finds(&cargo_after(|r| replace_in(r, game, "\"=0.1.0-alpha.6\"", "\"=0.1.0-alpha.5\"")), "bevy_lingara is `0.1.0-alpha.5`; examples.toml pins exactly `0.1.0-alpha.6`");
    assert_finds(&cargo_after(|r| replace_in(r, "games/bevy/g/server/Cargo.lock", "name = \"lingara\"\nversion = \"0.2.0-alpha.1\"", "name = \"lingara\"\nversion = \"0.2.0-alpha.0\"")), "server/Cargo.lock: resolves lingara to 0.2.0-alpha.0");
    let off_index = |r: &Path| replace_in(r, "games/bevy/g/game/Cargo.lock", &format!("version = \"3.4.2\"\nsource = \"{CRATES_IO}\""), "version = \"3.4.2\"\nsource = \"git+https://x/ureq\"");
    assert_finds(&cargo_after(off_index), "game/Cargo.lock: ureq 3.4.2 comes from git+https://x/ureq, not crates.io");
    assert_finds(&cargo_after(|r| fs::remove_file(r.join("games/bevy/g/server/Cargo.lock")).expect("rm")), "games/bevy/g/server/Cargo.lock: is missing");
    assert_finds(&cargo_after(|r| replace_in(r, game, "bevy_lingara = \"=0.1.0-alpha.6\"\n", "")), "bevy_lingara is in examples.toml but no Cargo.toml declares it");

    // The example this repo ships passes as committed.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let real: Vec<String> = lines(&deps::check(&repo)).into_iter().filter(|l| l.starts_with("games/")).collect();
    assert_eq!(real, Vec::<String>::new(), "the shipped Cargo examples are clean");
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

const GRADLE_INDEX: &str = r#"schema = 1

[[example]]
path = "games/jvm/g"
section = "games"
language = "kotlin"
title = "JVM"
packages = [
  { registry = "maven", name = "com.getlingara:lingara-embed-kotlin", version = "0.1.0-alpha.7" },
  { registry = "maven", name = "com.getlingara:lingara-embed-server-kotlin", version = "0.1.0-alpha.7" },
]
"#;
const CATALOGUE: &str = r#"[versions]
lingara-embed-kotlin = "0.1.0-alpha.7"
lingara-embed-server-kotlin = "0.1.0-alpha.7"
gdx = "1.14.2"
kotlin = "2.4.10"

[libraries]
lingara-embed-kotlin = { module = "com.getlingara:lingara-embed-kotlin", version.ref = "lingara-embed-kotlin" }
lingara-embed-server-kotlin = { module = "com.getlingara:lingara-embed-server-kotlin", version.ref = "lingara-embed-server-kotlin" }
gdx = { module = "com.badlogicgames.gdx:gdx", version.ref = "gdx" }

[plugins]
kotlin-jvm = { id = "org.jetbrains.kotlin.jvm", version.ref = "kotlin" }
"#;
const SETTINGS: &str = "pluginManagement {\n    repositories {\n        gradlePluginPortal()\n        mavenCentral()\n    }\n}\n\ndependencyResolutionManagement {\n    repositoriesMode = RepositoriesMode.FAIL_ON_PROJECT_REPOS\n    repositories {\n        mavenCentral() // the only one\n    }\n}\n\ninclude(\"game\", \"server\")\n";
const GAME_BUILD: &str = "plugins {\n    alias(libs.plugins.kotlin.jvm)\n}\n\ndependencies {\n    api(libs.gdx)\n    api(libs.lingara.embed.kotlin)\n}\n";
const SERVER_BUILD: &str = "dependencies {\n    implementation(libs.lingara.embed.server.kotlin)\n}\n";
const LOCK_HEAD: &str = "# This is a Gradle generated file for dependency locking.\n";

fn gradle_clean() -> tempfile::TempDir {
    // The server's lockfile holds lingara-embed-kotlin at another version: transitive, so not the row's.
    let game_lock = format!("{LOCK_HEAD}com.badlogicgames.gdx:gdx:1.14.2=compileClasspath\ncom.getlingara:lingara-embed-kotlin:0.1.0-alpha.7=compileClasspath,runtimeClasspath\nempty=annotationProcessor\n");
    let server_lock = format!("{LOCK_HEAD}com.getlingara:lingara-embed-kotlin:0.1.0-alpha.5=runtimeClasspath\ncom.getlingara:lingara-embed-server-kotlin:0.1.0-alpha.7=runtimeClasspath\nempty=\n");
    tree(&[
        ("examples.toml", GRADLE_INDEX),
        ("games/jvm/g/gradle/libs.versions.toml", CATALOGUE),
        ("games/jvm/g/settings.gradle.kts", SETTINGS),
        ("games/jvm/g/build.gradle.kts", "subprojects {\n    dependencyLocking { lockAllConfigurations() }\n}\n"),
        ("games/jvm/g/game/build.gradle.kts", GAME_BUILD),
        ("games/jvm/g/game/gradle.lockfile", &game_lock),
        ("games/jvm/g/server/build.gradle.kts", SERVER_BUILD),
        ("games/jvm/g/server/gradle.lockfile", &server_lock),
    ])
}

const CAT: &str = "games/jvm/g/gradle/libs.versions.toml";
const SET: &str = "games/jvm/g/settings.gradle.kts";
const GAME: &str = "games/jvm/g/game/build.gradle.kts";

/// One edit to the clean Gradle tree.
type Edit = fn(&Path);

/// Each edit, and a finding it must produce.
const GRADLE_CASES: [(Edit, &str); 18] = [
    (|r| replace_in(r, CAT, "gdx = \"1.14.2\"", "gdx = \"1.14.+\""), "[versions] gdx is not one exact version"),
    (|r| replace_in(r, CAT, "version.ref = \"gdx\"", "version = { strictly = \"1.14.2\" }"), "gdx has a rich version"),
    (|r| replace_in(r, CAT, "version.ref = \"lingara-embed-kotlin\"", "version.ref = \"gdx\""), "a com.getlingara library takes its own key, `lingara-embed-kotlin`"),
    (|r| replace_in(r, CAT, "version.ref = \"lingara-embed-kotlin\"", "version = \"0.1.0-alpha.7\""), "lingara-embed-kotlin writes its version inline"),
    (|r| replace_in(r, SET, "        gradlePluginPortal()\n", "        google()\n"), "pluginManagement names the repository `google()`"),
    (|r| replace_in(r, SET, "        mavenCentral() // the only one\n", "        mavenCentral()\n        gradlePluginPortal()\n"), "dependencyResolutionManagement names the repository `gradlePluginPortal()`"),
    (|r| replace_in(r, SET, "        mavenCentral() // the only one\n", "        mavenLocal()\n"), "has mavenLocal(); every package comes from Maven Central"),
    (|r| replace_in(r, SET, "include(", "includeBuild(\"../kit\")\ninclude("), "has an includeBuild"),
    (|r| replace_in(r, SET, "FAIL_ON_PROJECT_REPOS", "PREFER_SETTINGS"), "does not set repositoriesMode to FAIL_ON_PROJECT_REPOS"),
    (|r| replace_in(r, GAME, "dependencies {", "repositories {\n    maven { url = uri(\"https://x\") }\n}\n\ndependencies {"), "game/build.gradle.kts: has a repositories block"),
    (|r| replace_in(r, GAME, "    api(libs.gdx)\n", "    api(\"com.getlingara:lingara-embed-java:0.1.0-alpha.7\")\n"), "writes a com.getlingara coordinate as a string"),
    (|r| replace_in(r, GAME, "    api(libs.gdx)\n", "    implementation(files(\"libs/x.jar\"))\n"), "depends on files(…)"),
    (|r| fs::remove_file(r.join("games/jvm/g/server/gradle.lockfile")).expect("rm"), "games/jvm/g/server/gradle.lockfile: is missing"),
    (|r| replace_in(r, "games/jvm/g/game/gradle.lockfile", "lingara-embed-kotlin:0.1.0-alpha.7", "lingara-embed-kotlin:0.1.0-alpha.6"), "game/gradle.lockfile: resolves com.getlingara:lingara-embed-kotlin to 0.1.0-alpha.6, not 0.1.0-alpha.7"),
    (|r| replace_in(r, "games/jvm/g/game/gradle.lockfile", "empty=annotationProcessor\n", "empty=annotationProcessor\nmystery line\n"), "cannot classify the line `mystery line`"),
    (|r| replace_in(r, "games/jvm/g/server/gradle.lockfile", "empty=\n", "empty=\nempty=x\n"), "cannot classify the line `empty=x`"),
    (|r| replace_in(r, CAT, "id = \"org.jetbrains.kotlin.jvm\", version.ref = \"kotlin\"", "id = \"org.jetbrains.kotlin.jvm\", version = \"2.+\""), "kotlin-jvm is `2.+`; examples pin one exact version"),
    // The `group` and `name` form names the same module as `module` does.
    (|r| replace_in(r, CAT, "module = \"com.getlingara:lingara-embed-server-kotlin\", version.ref = \"lingara-embed-server-kotlin\"", "group = \"com.getlingara\", name = \"lingara-embed-server-kotlin\", version.ref = \"gdx\""), "takes its own key, `lingara-embed-server-kotlin`"),
];

/// 13.10.26i AC7 — the Gradle reader refuses an inexact or rich version, a
/// com.getlingara library off its own `[versions]` key or inline, a foreign
/// or misplaced repository, `mavenLocal()`, `includeBuild`, a missing
/// `FAIL_ON_PROJECT_REPOS`, a module `repositories` block, a string
/// coordinate, `files(…)`, a module without a lockfile, a declaring module's
/// pin that differs from the row, an unclassifiable lockfile line (a second
/// `empty=` included) and an inexact plugin; it reads a `group`/`name` entry
/// as its module; it
/// does not hold a transitive artefact to a row; it passes the real build.
#[test]
fn gradle_refuses_every_non_central_source() {
    let clean = lines(&deps::check(gradle_clean().path()));
    assert_eq!(clean, Vec::<String>::new(), "a clean tree passes; the server's transitive lingara-embed-kotlin is not the row's");
    for (edit, want) in GRADLE_CASES {
        let dir = gradle_clean();
        edit(dir.path());
        assert_finds(&lines(&deps::check(dir.path())), want);
    }

    let unlocked = gradle_clean();
    replace_in(unlocked.path(), GAME, "    api(libs.lingara.embed.kotlin)\n", "");
    assert_finds(&lines(&deps::check(unlocked.path())), "com.getlingara:lingara-embed-kotlin is in examples.toml but no module's build file declares it");

    // The example this repo ships passes as committed.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let real: Vec<String> = lines(&deps::check(&repo)).into_iter().filter(|l| l.starts_with("games/jvm/")).collect();
    assert_eq!(real, Vec::<String>::new(), "the shipped Gradle example is clean");
}
