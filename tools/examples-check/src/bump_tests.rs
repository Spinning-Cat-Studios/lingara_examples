use crate::bump::{self, replace_go_require};
use crate::index::Registry;
use crate::test_support::{tree, write};
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

/// 4.10.26b AC6 — only the named package's version lines change; rows,
/// manifests and lock files that do not name it are byte-identical.
#[test]
fn bump_rewrites_only_the_named_package() {
    let dir = tree(&[
        ("examples.toml", INDEX),
        ("integrations/a/package.json", PACKAGE_A),
        ("integrations/a/package-lock.json", LOCK),
        ("apps/b/package.json", PACKAGE_B),
        ("apps/b/package-lock.json", LOCK),
        ("apps/c/package.json", PACKAGE_C),
    ]);
    let root = dir.path();
    let touched = bump::run(root, Registry::Npm, "@lingara/api", "0.1.0-alpha.10").expect("bump");

    assert_eq!(touched, ["examples.toml", "integrations/a/package.json", "apps/b/package.json"]);
    let up = |s: &str| s.replace("\"0.1.0-alpha.9\"", "\"0.1.0-alpha.10\"");
    assert_eq!(read(root, "examples.toml"), up(INDEX), "only the two version strings move");
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
    let dir = tree(&[("examples.toml", index), ("apps/p/composer.json", composer)]);
    write(dir.path(), "apps/p/composer.lock", "{}");
    bump::run(dir.path(), Registry::Packagist, "spinningcatstudios/lingara", "v0.1.0-alpha.10").expect("bump");
    assert_eq!(read(dir.path(), "apps/p/composer.json"), composer.replace("v0.1.0-alpha.9", "v0.1.0-alpha.10"));
    assert_eq!(read(dir.path(), "apps/p/composer.lock"), "{}");
}
