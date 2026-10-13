use crate::outdated::{self, RegistryUrls};
use crate::test_support::{snapshot, tree};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

/// A tiny HTTP fixture: each request's path is answered from `routes`, or 404.
fn serve(routes: Vec<(&'static str, String)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    thread::spawn(move || {
        for mut stream in listener.incoming().filter_map(Result::ok) {
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = request.split_whitespace().nth(1).unwrap_or_default().to_string();
            let (status, body) = match routes.iter().find(|(p, _)| *p == path) {
                Some((_, body)) => ("200 OK", body.as_str()),
                None => ("404 Not Found", ""),
            };
            let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        }
    });
    format!("http://{addr}")
}

const INDEX: &str = r#"schema = 1

[[example]]
path = "integrations/behind"
section = "integrations"
language = "typescript"
title = "Behind"
packages = [{ registry = "npm", name = "@lingara/api", version = "0.1.0-alpha.9" }]

[[example]]
path = "apps/current"
section = "apps"
language = "typescript"
title = "Current"
packages = [{ registry = "npm", name = "@lingara/api", version = "0.1.0-alpha.10" }]

[[example]]
path = "integrations/go"
section = "integrations"
language = "go"
title = "Go"
packages = [{ registry = "go", name = "github.com/Spinning-Cat-Studios/lingara_api_clients/go", version = "v0.1.0-alpha.9" }]

[[example]]
path = "apps/php"
section = "apps"
language = "php"
title = "PHP"
packages = [{ registry = "packagist", name = "spinningcatstudios/lingara", version = "v0.1.0-alpha.9" }]
"#;

fn routes() -> Vec<(&'static str, String)> {
    // `latest` names an older version, as it does when a family publishes
    // with `--tag next`; alpha.2 sorts after alpha.10 as a string, never by SemVer.
    let npm = r#"{"dist-tags":{"latest":"0.1.0-alpha.9"},"versions":{"0.1.0-alpha.2":{},"0.1.0-alpha.9":{},"0.1.0-alpha.10":{}}}"#;
    let go = "v0.1.0-alpha.2\nv0.1.0-alpha.10\nv0.1.0-alpha.9\n";
    let packagist = r#"{"packages":{"spinningcatstudios/lingara":[{"version":"v0.1.0-alpha.10"},{"version":"v0.1.0-alpha.9"},{"version":"v0.1.0-alpha.2"}]}}"#;
    vec![
        ("/@lingara%2Fapi", npm.to_string()),
        ("/github.com/!spinning-!cat-!studios/lingara_api_clients/go/@v/list", go.to_string()),
        ("/p2/spinningcatstudios/lingara.json", packagist.to_string()),
    ]
}

/// 4.10.26b AC4 — newest by SemVer precedence, never by dist-tag; `v`-prefixed
/// Go and Packagist versions keep their `v`; nothing is written.
#[test]
fn reports_behind_and_writes_nothing() {
    let base = serve(routes());
    let urls = RegistryUrls { npm: base.clone(), go: format!("{base}/"), packagist: base.clone(), crates: base.clone(), maven: base };
    let dir = tree(&[("examples.toml", INDEX)]);
    let before = snapshot(dir.path());

    let mut out = Vec::new();
    outdated::run(dir.path(), &urls, &mut out).expect("outdated");

    assert_eq!(
        String::from_utf8(out).expect("utf-8"),
        "integrations/behind @lingara/api 0.1.0-alpha.9 → 0.1.0-alpha.10\n\
         integrations/go github.com/Spinning-Cat-Studios/lingara_api_clients/go v0.1.0-alpha.9 → v0.1.0-alpha.10\n\
         apps/php spinningcatstudios/lingara v0.1.0-alpha.9 → v0.1.0-alpha.10\n"
    );
    assert_eq!(snapshot(dir.path()), before, "outdated never writes a file");
}

/// The shipped base-URL file parses.
#[test]
fn the_shipped_registry_file_parses() {
    let urls = outdated::load_urls(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/registries.toml"))).expect("parses");
    assert_eq!(urls.npm, "https://registry.npmjs.org");
    assert_eq!(urls.crates, "https://index.crates.io/");
    assert_eq!(urls.maven, "https://repo1.maven.org/maven2/");
}

const CRATES_INDEX: &str = r#"schema = 1

[[example]]
path = "games/bevy/g"
section = "games"
language = "rust"
title = "Bevy"
packages = [
  { registry = "crates", name = "bevy_lingara", version = "0.1.0-alpha.6" },
  { registry = "crates", name = "lingara", version = "0.2.0-alpha.1" },
]
"#;

/// 13.10.26h AC8 — the sparse-index path for 1-, 2-, 3- and 4+-character
/// names (lower-cased), and the newest by SemVer with pre-releases included
/// and yanked releases excluded.
#[test]
fn crates_takes_the_newest_unyanked() {
    assert_eq!(outdated::sparse_path("a"), "1/a");
    assert_eq!(outdated::sparse_path("ab"), "2/ab");
    assert_eq!(outdated::sparse_path("abc"), "3/a/abc");
    assert_eq!(outdated::sparse_path("Bevy_Lingara"), "be/vy/bevy_lingara");

    // alpha.10 is yanked; alpha.9 sorts after alpha.10 as a string, never by SemVer.
    let bevy = [("0.1.0-alpha.6", false), ("0.1.0-alpha.9", false), ("0.1.0-alpha.7", false), ("0.1.0-alpha.10", true)]
        .map(|(v, y)| format!(r#"{{"name":"bevy_lingara","vers":"{v}","deps":[],"yanked":{y}}}"#))
        .join("\n");
    let lingara = r#"{"name":"lingara","vers":"0.2.0-alpha.1","yanked":false}"#.to_string();
    let base = serve(vec![("/be/vy/bevy_lingara", bevy), ("/li/ng/lingara", lingara)]);
    let urls = RegistryUrls { npm: base.clone(), go: base.clone(), packagist: base.clone(), crates: format!("{base}/"), maven: base };
    let dir = tree(&[("examples.toml", CRATES_INDEX)]);

    let mut out = Vec::new();
    outdated::run(dir.path(), &urls, &mut out).expect("outdated");
    assert_eq!(String::from_utf8(out).expect("utf-8"), "games/bevy/g bevy_lingara 0.1.0-alpha.6 → 0.1.0-alpha.9\n");
}

const MAVEN_INDEX: &str = r#"schema = 1

[[example]]
path = "games/jvm/g"
section = "games"
language = "kotlin"
title = "JVM"
packages = [{ registry = "maven", name = "com.getlingara:lingara-embed-kotlin", version = "0.1.0-alpha.6" }]
"#;

/// 13.10.26i AC8 — the `maven-metadata.xml` path for `group:artifact`, the
/// newest `<version>` by SemVer with pre-releases included (never
/// `<latest>`), and every non-SemVer version named as skipped.
#[test]
fn maven_takes_the_newest_semver() {
    assert_eq!(outdated::maven_metadata_path("com.getlingara:lingara-embed-kotlin").as_deref(), Some("com/getlingara/lingara-embed-kotlin/maven-metadata.xml"));
    assert_eq!(outdated::maven_metadata_path("no-colon"), None);

    // `<latest>` names an older version; alpha.9 sorts after alpha.10 as a string, never by SemVer.
    let metadata = "<metadata><groupId>com.getlingara</groupId><versioning><latest>0.1.0-alpha.7</latest><versions>\
        <version>0.1.0-alpha.6</version><version>0.1.0-alpha.10</version><version>0.1.0-alpha.9</version><version>0.1-SNAPSHOT</version>\
        </versions></versioning></metadata>";
    let base = serve(vec![("/com/getlingara/lingara-embed-kotlin/maven-metadata.xml", metadata.to_string())]);
    let urls = RegistryUrls { npm: base.clone(), go: base.clone(), packagist: base.clone(), crates: base.clone(), maven: format!("{base}/") };
    let dir = tree(&[("examples.toml", MAVEN_INDEX)]);

    let mut out = Vec::new();
    outdated::run(dir.path(), &urls, &mut out).expect("outdated");
    assert_eq!(String::from_utf8(out).expect("utf-8"), "games/jvm/g com.getlingara:lingara-embed-kotlin 0.1.0-alpha.6 → 0.1.0-alpha.10\n");

    let versions = ["0.1.0-alpha.9", "0.1-SNAPSHOT", "0.1.0-alpha.10"].map(String::from);
    assert_eq!(outdated::newest_of(&versions), (Some("0.1.0-alpha.10".to_string()), vec!["0.1-SNAPSHOT".to_string()]), "the skip is named, for stderr");
}
