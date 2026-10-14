//! The site's routes (ADR 14.10.26p D7), offline: the real router driven
//! in-process, over an in-process stub of Lingara's token, mint and events
//! endpoints that records each request's body and bearer. No network and no
//! secret. The snippet's own rules are `widget_snippet`'s tests; these test
//! what the example decides: the origin a token is bound to, the token that
//! asks for the plan, one plan per player, and the cookie.

// The binary's modules; the test compiles the same files.
#[allow(dead_code)]
#[path = "../src/page.rs"]
mod page;
#[allow(dead_code)]
#[path = "../src/routes.rs"]
mod routes;

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use tower::ServiceExt as _;

const PLAN_ID: &str = "0f8b5a52-3c1e-4d7a-9b62-6f1d2e3a4b5c";
const SUBJECT: &str = "lgr_sub_night_market_site";
const CLIENT_TOKEN: &str = "lgr_at_stub";
const PLAYER_TOKEN: &str = "lgr_et_stub";
const WEBHOOK_KEY: [u8; 32] = [7; 32];

/// One request the stub received: its path, its bearer and its JSON body.
#[derive(Clone, Debug)]
struct Seen {
    path: String,
    bearer: Option<String>,
    body: Value,
}

/// What the stub answers: a mint status, and the event's `reaction`.
#[derive(Clone)]
struct Answers {
    mint: StatusCode,
    events: StatusCode,
    reaction: Value,
}

impl Default for Answers {
    fn default() -> Self {
        let reaction = json!({"status": "started", "plan_id": PLAN_ID, "plan_status": "generating"});
        Self { mint: StatusCode::OK, events: StatusCode::ACCEPTED, reaction }
    }
}

type Log = Arc<Mutex<Vec<Seen>>>;

fn bearer(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    value.strip_prefix("Bearer ").map(String::from)
}

/// Lingara's three endpoints on a local port.
async fn stub(answers: Answers) -> (String, Log) {
    let log: Log = Arc::default();
    let (mint_log, events_log) = (log.clone(), log.clone());
    let oauth = axum::routing::post(|| async { axum::Json(json!({"access_token": CLIENT_TOKEN, "token_type": "Bearer", "expires_in": 3600})) });
    let mint = axum::routing::post(move |headers: HeaderMap, axum::Json(body): axum::Json<Value>| async move {
        mint_log.lock().unwrap().push(Seen { path: "/v1/embed/tokens".into(), bearer: bearer(&headers), body });
        let answer = json!({"token": PLAYER_TOKEN, "expires_at": "2026-10-14T12:00:00Z", "expires_in": 900, "subject": SUBJECT, "scopes": ["embed:play"], "account_linked": false});
        (answers.mint, axum::Json(answer))
    });
    let reaction = answers.reaction.clone();
    let events = axum::routing::post(move |headers: HeaderMap, axum::Json(body): axum::Json<Value>| async move {
        events_log.lock().unwrap().push(Seen { path: "/v1/events".into(), bearer: bearer(&headers), body });
        let answer = json!({"id": "lgr_evt_1", "type": "world.practice_requested", "created_at": "2026-10-14T09:00:00Z", "reaction": reaction});
        (answers.events, axum::Json(answer))
    });
    let lingara = Router::new().route("/oauth/token", oauth).route("/v1/embed/tokens", mint).route("/v1/events", events);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    tokio::spawn(async move { axum::serve(listener, lingara).await.expect("stub") });
    (url, log)
}

/// The site's router over the stub, configured as the environment would.
async fn site(answers: Answers) -> (Router, Log) {
    let (url, log) = stub(answers).await;
    let webhook = format!("lgr_whsec_{}", STANDARD.encode(WEBHOOK_KEY));
    let token_url = format!("{url}/oauth/token");
    let env = move |name: &str| match name {
        "LINGARA_CLIENT_ID" => Some("lgr_ci_test".to_string()),
        "LINGARA_CLIENT_SECRET" => Some("lgr_cs_test".to_string()),
        "LINGARA_WEBHOOK_SECRET" => Some(webhook.clone()),
        "LINGARA_BASE_URL" => Some(url.clone()),
        "LINGARA_TOKEN_URL" => Some(token_url.clone()),
        _ => None,
    };
    (routes::app(routes::Config::from_lookup(env).expect("config")).expect("app"), log)
}

struct Answer {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

async fn call(app: &Router, method: &str, path: &str, cookie: Option<&str>, body: Vec<u8>, extra: &[(&str, String)]) -> Answer {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, format!("theme=dark; {}={cookie}", routes::COOKIE));
    }
    for (name, value) in extra {
        request = request.header(*name, value);
    }
    let response = app.clone().oneshot(request.body(Body::from(body)).expect("request")).await.expect("infallible");
    let (status, headers) = (response.status(), response.headers().clone());
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("body");
    Answer { status, headers, body: String::from_utf8(bytes.to_vec()).expect("utf-8") }
}

/// The player id a `Set-Cookie` names.
fn player_of(answer: &Answer) -> String {
    let cookie = answer.headers.get(header::SET_COOKIE).expect("a cookie").to_str().expect("ascii");
    let pair = cookie.split(';').next().expect("a pair");
    pair.strip_prefix(&format!("{}=", routes::COOKIE)).expect("the session cookie").to_string()
}

fn requests(log: &Log, path: &str) -> Vec<Seen> {
    log.lock().unwrap().iter().filter(|s| s.path == path).cloned().collect()
}

// 14.10.26p AC2
/// `GET /` sets an `HttpOnly`, `SameSite=Strict` cookie and mounts the
/// widget on that player's plan with `/token` as its endpoint, beside the
/// inline script that turns `origin_refused` into a line of its own.
#[tokio::test]
async fn page_mounts_the_widget_for_the_player() {
    let (app, _) = site(Answers::default()).await;
    let page = call(&app, "GET", "/", None, vec![], &[]).await;
    assert_eq!(page.status, StatusCode::OK);
    let cookie = page.headers.get(header::SET_COOKIE).expect("a cookie").to_str().expect("ascii").to_string();
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict") && cookie.contains("Path=/"), "{cookie}");
    assert_eq!(player_of(&page).len(), 32, "16 bytes in hex");
    assert!(page.body.contains(&format!("<lingara-embed id=\"lingara-embed\" surface=\"practice\" plan-id=\"{PLAN_ID}\"")), "{}", page.body);
    assert!(page.body.contains("fetch(\"/token\""));
    assert!(page.body.contains("lingara:error") && page.body.contains("\"origin_refused\""));
    assert!(page.body.contains(page::ORIGIN_REFUSED));
    assert!(!page.body.contains(PLAYER_TOKEN), "no token in the page");
}

// 14.10.26p AC3
/// `POST /token` mints with `origin` = the allowed origin, never the asking
/// one, and answers only `token` and `expires_at`; with no cookie it is 401.
#[tokio::test]
async fn token_is_bound_to_the_allowed_origin() {
    let (app, log) = site(Answers::default()).await;
    let player = player_of(&call(&app, "GET", "/", None, vec![], &[]).await);

    let origin = [("origin", "http://127.0.0.1:8789".to_string())];
    let answer = call(&app, "POST", "/token", Some(&player), vec![], &origin).await;
    assert_eq!(answer.status, StatusCode::OK);
    let handoff: Value = serde_json::from_str(&answer.body).expect("json");
    let keys: Vec<&str> = handoff.as_object().expect("object").keys().map(String::as_str).collect();
    assert_eq!(keys, ["expires_at", "token"]);
    let mints = requests(&log, "/v1/embed/tokens");
    assert_eq!(mints.last().expect("a mint").body, json!({"player_ref": player, "origin": "http://localhost:8788"}));

    let before = requests(&log, "/v1/embed/tokens").len();
    assert_eq!(call(&app, "POST", "/token", None, vec![], &[]).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(requests(&log, "/v1/embed/tokens").len(), before, "no session, no mint");
}

// 14.10.26p AC4
/// The defaults: `localhost:8788` and `127.0.0.1:8789`, only the first
/// registered; one router answers whichever `Host` a request names.
#[tokio::test]
async fn two_origins_and_one_registered() {
    let env = |name: &str| matches!(name, "LINGARA_CLIENT_ID" | "LINGARA_CLIENT_SECRET").then(|| "x".to_string());
    let config = routes::Config::from_lookup(env).expect("config");
    assert_eq!(config.listeners, ["localhost:8788", "127.0.0.1:8789"]);
    assert_eq!(config.allowed_origin, "http://localhost:8788");
    assert_eq!(config.app_origin, None);

    let (app, _) = site(Answers::default()).await;
    for host in config.listeners {
        let page = call(&app, "GET", "/", None, vec![], &[("host", host.clone())]).await;
        assert!(page.body.contains("<lingara-embed"), "{host} serves the same page");
    }
}

// 14.10.26p AC5
/// A first visit mints an unbound token and sends one
/// `world.practice_requested` under it, never under the client's token; the
/// same cookie again sends none and renders the same plan.
#[tokio::test]
async fn a_player_asks_for_one_plan_as_themself() {
    let (app, log) = site(Answers::default()).await;
    let first = call(&app, "GET", "/", None, vec![], &[]).await;
    let player = player_of(&first);

    assert_eq!(requests(&log, "/v1/embed/tokens")[0].body, json!({"player_ref": player}), "unbound");
    let events = requests(&log, "/v1/events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].bearer.as_deref(), Some(PLAYER_TOKEN), "the player's token, not {CLIENT_TOKEN}");
    assert_eq!(events[0].body["type"], "world.practice_requested");

    let again = call(&app, "GET", "/", Some(&player), vec![], &[]).await;
    assert!(again.headers.get(header::SET_COOKIE).is_none(), "the session is kept");
    assert!(again.body.contains(PLAN_ID));
    assert_eq!(requests(&log, "/v1/events").len(), 1, "one plan per player");
}

// 14.10.26p AC6
/// No plan, no widget: a refused reaction, a reaction with no plan id, and a
/// mint or send Lingara answers with an error each render their code.
#[tokio::test]
async fn no_plan_no_widget() {
    let refused = Answers { reaction: json!({"status": "refused", "code": "rate_limited"}), ..Answers::default() };
    let no_plan = Answers { reaction: json!({"status": "started"}), ..Answers::default() };
    let mint = Answers { mint: StatusCode::FORBIDDEN, ..Answers::default() };
    let send = Answers { events: StatusCode::BAD_REQUEST, ..Answers::default() };
    let cases = [(refused, "rate_limited"), (no_plan, "no_plan_id"), (mint, "mint_failed"), (send, "practice_request_failed")];
    for (answers, code) in cases {
        let (app, _) = site(answers).await;
        let page = call(&app, "GET", "/", None, vec![], &[]).await;
        assert!(page.body.contains(&format!("<code>{code}</code>")), "{code}: {}", page.body);
        assert!(!page.body.contains("<lingara-embed"), "{code}: no widget");
    }
}

/// A `practice.completed` for the stub's subject, signed now.
fn signed_result(id: &str) -> (Vec<u8>, Vec<(&'static str, String)>) {
    let body = json!({
        "id": id, "type": "practice.completed", "created_at": "2026-10-14T09:00:00Z",
        "api_version": "2026-09-equipped-boxfish", "subject": SUBJECT,
        "data": {"plan_id": PLAN_ID, "completed": true, "items": [{"set_number": 1, "question": 1, "outcome": "correct"}]},
    })
    .to_string();
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs().to_string();
    let mut mac = Hmac::<Sha256>::new_from_slice(&WEBHOOK_KEY).expect("key");
    mac.update(format!("{id}.{timestamp}.{body}").as_bytes());
    let signature = format!("v1,{}", STANDARD.encode(mac.finalize().into_bytes()));
    (body.into_bytes(), vec![("webhook-id", id.to_string()), ("webhook-timestamp", timestamp), ("webhook-signature", signature)])
}

// 14.10.26p AC7
/// A verified `practice.completed` is one lantern per event id, and
/// `GET /rewards` reports it for the cookie's player.
#[tokio::test]
async fn webhook_gives_one_lantern_per_event() {
    let (app, _) = site(Answers::default()).await;
    let player = player_of(&call(&app, "GET", "/", None, vec![], &[]).await);
    let lanterns = async |app: &Router| call(app, "GET", "/rewards", Some(&player), vec![], &[]).await.body;
    assert_eq!(lanterns(&app).await, r#"{"lanterns":0}"#);

    let (body, headers) = signed_result("lgr_evt_site_1");
    assert_eq!(call(&app, "POST", "/webhook", None, body.clone(), &headers).await.status, StatusCode::NO_CONTENT);
    assert_eq!(call(&app, "POST", "/webhook", None, body, &headers).await.status, StatusCode::NO_CONTENT);
    assert_eq!(lanterns(&app).await, r#"{"lanterns":1}"#, "a redelivery earns nothing");

    assert_eq!(call(&app, "GET", "/rewards", None, vec![], &[]).await.status, StatusCode::UNAUTHORIZED);
}
