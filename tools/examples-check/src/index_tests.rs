use crate::index::{self, Index};
use crate::list;
use crate::test_support::{lines, tree, write};
use std::path::Path;

const ROW: &str = r#"
[[example]]
path     = "integrations/fake-game"
section  = "integrations"
language = "typescript"
title    = "A headless game"
live     = true
packages = [{ registry = "npm", name = "@lingara/api", version = "0.1.0-alpha.10" }]
scopes   = ["events:read"]
"#;

fn loaded(root: &Path) -> Index {
    index::load(root).expect("a valid index")
}

/// A clean tree: one live row, its directory, a recipe with a live target and
/// a README whose table is `list --markdown`.
fn clean() -> tempfile::TempDir {
    let dir = tree(&[
        ("examples.toml", &format!("schema = 1\n{ROW}")),
        ("integrations/fake-game/README.md", "x"),
        ("make/lang/typescript.mk", "build-example-typescript:\n\t@:\nlive-example-typescript:\n\t@:\n"),
    ]);
    let table = list::markdown(&loaded(dir.path()));
    write(dir.path(), "README.md", &format!("# x\n<!-- examples:start -->\n{table}<!-- examples:end -->\n"));
    dir
}

fn findings_after(edit: impl FnOnce(&Path)) -> Vec<String> {
    let dir = clean();
    edit(dir.path());
    lines(&index::check(dir.path()))
}

fn with_index(root: &Path, rows: &str) {
    write(root, "examples.toml", &format!("schema = 1\n{rows}"));
}

fn assert_one(found: &[String], want: &str) {
    assert!(found.iter().any(|f| f.contains(want)), "expected `{want}` in {found:?}");
}

/// 4.10.26b AC1 — each index gap is a finding; a clean tree passes.
#[test]
fn every_index_gap_is_a_finding() {
    let dir = clean();
    assert_eq!(lines(&index::check(dir.path())), Vec::<String>::new(), "a clean tree passes");

    let unindexed = findings_after(|r| write(r, "apps/orphan/README.md", "x"));
    assert_one(&unindexed, "apps/orphan: an example directory with no row");
    let game = findings_after(|r| write(r, "games/bevy/orphan/README.md", "x"));
    assert_one(&game, "games/bevy/orphan: an example directory with no row");

    let missing = findings_after(|r| with_index(r, &ROW.replace("integrations/fake-game", "integrations/gone")));
    assert_one(&missing, "integrations/gone: no such directory");

    let section = findings_after(|r| with_index(r, &ROW.replace("section  = \"integrations\"", "section  = \"apps\"")));
    assert_one(&section, "section `apps` disagrees with the path");

    let duplicate = findings_after(|r| with_index(r, &format!("{ROW}{ROW}")));
    assert_one(&duplicate, "two rows share this path");

    let recipe = findings_after(|r| with_index(r, &ROW.replace("\"typescript\"", "\"cobol\"")));
    assert_one(&recipe, "make/lang/cobol.mk is missing");

    let live = findings_after(|r| write(r, "make/lang/typescript.mk", "build-example-typescript:\n\t@:\n"));
    assert_one(&live, "has no `live-example-typescript:` line");

    let unknown = findings_after(|r| with_index(r, &ROW.replace("live     = true", "live = true\nowner = \"x\"")));
    assert_one(&unknown, "examples.toml: unknown field `owner`");

    let readme = findings_after(|r| with_index(r, &ROW.replace("A headless game", "A renamed game")));
    assert_one(&readme, "README.md: the examples table differs");
}

/// 14.10.26p AC1 — `websites` is a section: a row under it passes, an
/// unindexed `websites/<stack>/<example>` is found, and a section outside
/// the four is refused.
#[test]
fn websites_is_a_section() {
    let site = ROW.replace("integrations/fake-game", "websites/rust/night-market").replace("\"integrations\"", "\"websites\"");
    let accepted = findings_after(|r| {
        with_index(r, &site);
        write(r, "websites/rust/night-market/README.md", "x");
    });
    assert!(accepted.iter().all(|f| !f.contains("websites/rust/night-market:")), "a websites row passes: {accepted:?}");

    let unindexed = findings_after(|r| write(r, "websites/rust/orphan/README.md", "x"));
    assert_one(&unindexed, "websites/rust/orphan: an example directory with no row");

    let unknown = findings_after(|r| with_index(r, &ROW.replace("section  = \"integrations\"", "section  = \"blogs\"")));
    assert_one(&unknown, "section `blogs` is not integrations, apps, games or websites");
}

/// 4.10.26b AC5 — `list`, `list --json`, `list --markdown` and `lang`.
#[test]
fn list_prints_every_row_in_order() {
    let second = ROW.replace("integrations/fake-game", "apps/cards").replace("\"integrations\"", "\"apps\"").replace("\"typescript\"", "\"php\"");
    let dir = tree(&[("examples.toml", &format!("schema = 1\n{ROW}{second}"))]);
    let index = loaded(dir.path());

    assert_eq!(list::plain(&index), "integrations/fake-game typescript\napps/cards php\n");
    assert_eq!(
        list::json(&index),
        r#"{"count":2,"include":[{"path":"integrations/fake-game","language":"typescript"},{"path":"apps/cards","language":"php"}]}"#
    );
    let table = list::markdown(&index);
    let fake_game = table.find("integrations/fake-game").expect("first row");
    assert!(fake_game < table.find("apps/cards").expect("second row"), "file order");
    assert!(table.contains("`@lingara/api` 0.1.0-alpha.10"));

    assert_eq!(list::lang(&index, "apps/cards"), Some("php"));
    assert_eq!(list::lang(&index, "apps/cards/"), Some("php"), "a trailing slash from make");
    assert_eq!(list::lang(&index, "apps/nope"), None);

    let empty = tree(&[("examples.toml", "schema = 1\n")]);
    assert_eq!(list::json(&loaded(empty.path())), r#"{"count":0,"include":[]}"#);
    assert_eq!(list::plain(&loaded(empty.path())), "");
}
