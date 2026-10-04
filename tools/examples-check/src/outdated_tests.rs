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
    let urls = RegistryUrls { npm: base.clone(), go: format!("{base}/"), packagist: base };
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
}
