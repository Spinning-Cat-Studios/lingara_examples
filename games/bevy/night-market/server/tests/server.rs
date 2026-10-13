//! The server's routes (ADR 13.10.26h D4), offline: the real router driven
//! in-process, over an in-process stub of Lingara's two endpoints. No
//! network, no secret, and no kit under test: the stub answers what the
//! library expects, and the webhook is signed here the way Lingara signs one.

// The routes are the binary's module; the test compiles the same file.
#[allow(dead_code)]
#[path = "../src/routes.rs"]
mod routes;

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use tower::ServiceExt as _;

const PLAYER: &str = "player-1";
const SUBJECT: &str = "lgr_sub_night_market_test";
const WEBHOOK_KEY: [u8; 32] = [7; 32];

/// Every mint request body the stub received.
type Mints = Arc<Mutex<Vec<Value>>>;

/// Lingara's token and mint endpoints on a local port, recording each mint.
async fn stub() -> (String, Mints) {
    let mints: Mints = Arc::default();
    let seen = mints.clone();
    let oauth = axum::routing::post(|| async { axum::Json(json!({"access_token": "lgr_at_stub", "token_type": "Bearer", "expires_in": 3600})) });
    let mint = axum::routing::post(move |axum::Json(body): axum::Json<Value>| async move {
        seen.lock().unwrap().push(body);
        let answer = json!({"token": "lgr_et_stub", "expires_at": "2026-10-13T12:00:00Z", "expires_in": 900, "subject": SUBJECT, "scopes": ["embed:play"], "account_linked": false});
        axum::Json(answer)
    });
    let lingara = Router::new().route("/oauth/token", oauth).route("/v1/embed/tokens", mint);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    tokio::spawn(async move { axum::serve(listener, lingara).await.expect("stub") });
    (url, mints)
}

/// The server's router, pointed at the stub as `LINGARA_BASE_URL` and
/// `LINGARA_TOKEN_URL` would point it.
async fn server() -> (Router, Mints) {
    let (url, mints) = stub().await;
    let config = routes::Config {
        client_id: "lgr_ci_test".into(),
        client_secret: "lgr_cs_test".into(),
        webhook_secret: Some(format!("lgr_whsec_{}", STANDARD.encode(WEBHOOK_KEY))),
        base_url: Some(url.clone()),
        token_url: Some(format!("{url}/oauth/token")),
    };
    (routes::app(&config).expect("app"), mints)
}

/// One request to the router; the status and the body as text.
async fn call(app: &Router, method: &str, path: &str, player: Option<&str>, body: Vec<u8>, extra: &[(&str, String)]) -> (StatusCode, String) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(player) = player {
        request = request.header(routes::PLAYER_HEADER, player);
    }
    for (name, value) in extra {
        request = request.header(*name, value);
    }
    let response = app.clone().oneshot(request.body(Body::from(body)).expect("request")).await.expect("infallible");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("body");
    (status, String::from_utf8(bytes.to_vec()).expect("utf-8"))
}

// 13.10.26h AC5
/// `POST /token` with the session mints for that player with no `origin`,
/// and answers only `token` and `expires_at`; without the session it is
/// `401` and Lingara sees no mint.
#[tokio::test]
async fn the_handoff_needs_a_session_and_is_token_and_expiry_only() {
    let (app, mints) = server().await;

    let (status, body) = call(&app, "POST", "/token", Some(PLAYER), vec![], &[]).await;
    assert_eq!(status, StatusCode::OK);
    let handoff: Value = serde_json::from_str(&body).expect("json");
    // serde_json's map is sorted, so this is the key set, not its order.
    let keys: Vec<&str> = handoff.as_object().expect("object").keys().map(String::as_str).collect();
    assert_eq!(keys, ["expires_at", "token"]);
    assert_eq!(handoff["token"], "lgr_et_stub");
    let sent = mints.lock().unwrap().clone();
    assert_eq!(sent, [json!({"player_ref": PLAYER})], "no origin, no scopes");

    let (status, _) = call(&app, "POST", "/token", None, vec![], &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(mints.lock().unwrap().len(), 1, "no session, no mint");
}

/// A `practice.completed` for the stub's subject, and its Standard Webhooks
/// headers, signed now.
fn signed_result(id: &str) -> (Vec<u8>, Vec<(&'static str, String)>) {
    let body = json!({
        "id": id, "type": "practice.completed", "created_at": "2026-10-13T09:00:00Z",
        "api_version": "2026-09-equipped-boxfish", "subject": SUBJECT,
        "data": {"plan_id": "plan-1", "completed": true, "items": [{"set_number": 1, "question": 1, "outcome": "correct"}]},
    })
    .to_string();
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs().to_string();
    let mut mac = Hmac::<Sha256>::new_from_slice(&WEBHOOK_KEY).expect("key");
    mac.update(format!("{id}.{timestamp}.{body}").as_bytes());
    let signature = format!("v1,{}", STANDARD.encode(mac.finalize().into_bytes()));
    let headers = vec![("webhook-id", id.to_string()), ("webhook-timestamp", timestamp), ("webhook-signature", signature)];
    (body.into_bytes(), headers)
}

async fn lanterns(app: &Router) -> String {
    let (status, body) = call(app, "GET", "/rewards", Some(PLAYER), vec![], &[]).await;
    assert_eq!(status, StatusCode::OK);
    body
}

// 13.10.26h AC6
/// After a mint, a signed `practice.completed` for that subject is `204` and
/// earns one lantern; the same delivery again earns none; a body with one
/// byte changed is `401` and earns none; `GET /rewards` needs the session.
#[tokio::test]
async fn a_signed_result_rewards_its_player_once() {
    let (app, _) = server().await;
    assert_eq!(lanterns(&app).await, r#"{"lanterns":0}"#);
    call(&app, "POST", "/token", Some(PLAYER), vec![], &[]).await;

    let (body, headers) = signed_result("lgr_evt_night_market_1");
    assert_eq!(call(&app, "POST", "/webhook", None, body.clone(), &headers).await.0, StatusCode::NO_CONTENT);
    assert_eq!(lanterns(&app).await, r#"{"lanterns":1}"#);

    assert_eq!(call(&app, "POST", "/webhook", None, body.clone(), &headers).await.0, StatusCode::NO_CONTENT, "a redelivery is acknowledged");
    assert_eq!(lanterns(&app).await, r#"{"lanterns":1}"#, "and earns nothing");

    let mut tampered = body;
    let at = tampered.iter().position(|b| *b == b'1').expect("a digit");
    tampered[at] = b'2';
    assert_eq!(call(&app, "POST", "/webhook", None, tampered, &headers).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(lanterns(&app).await, r#"{"lanterns":1}"#);

    assert_eq!(call(&app, "GET", "/rewards", None, vec![], &[]).await.0, StatusCode::UNAUTHORIZED);
}
