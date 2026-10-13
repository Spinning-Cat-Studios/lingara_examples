//! The server contract (ADR 13.10.26h D3), which the Godot Night Market runs
//! unchanged and the JVM one writes again:
//!
//! - `POST /token` (session): mints the player's Lingara token, no `origin`
//!   and no `scopes`, and answers exactly `{"token":…,"expires_at":…}`.
//! - `POST /webhook` (no session, mounted only with a webhook secret):
//!   verifies Lingara's delivery, routes it to its player, and gives one
//!   lantern per `practice.completed` event id. Empty body, `handle`'s status.
//! - `GET /rewards` (session): `{"lanterns":N}`, a running total that is
//!   never consumed, so a repeated poll cannot add one.
//!
//! The session is a stand-in: the `X-Night-Market-Player` header, used as
//! the `player_ref`. A real game server authenticates its player first.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use lingara::events::{Event, Webhook};
use lingara_embed::server::{DirectoryError, EmbedServer, MemoryPlayerDirectory, MintOptions, PlayerRouter, RoutedEvent, token_handoff};

pub const PLAYER_HEADER: &str = "x-night-market-player";

/// Where the server's Lingara credentials and URLs come from. Nothing here
/// is ever printed.
pub struct Config {
    /// A metered client holding `embed:mint`.
    pub client_id: String,
    pub client_secret: String,
    /// `lgr_whsec_…`; without it `/webhook` is not mounted.
    pub webhook_secret: Option<String>,
    pub base_url: Option<String>,
    pub token_url: Option<String>,
}

impl Config {
    /// `LINGARA_CLIENT_ID` and `LINGARA_CLIENT_SECRET` (required),
    /// `LINGARA_WEBHOOK_SECRET`, `LINGARA_BASE_URL`, `LINGARA_TOKEN_URL`.
    pub fn from_env() -> Result<Self, String> {
        let required = |name: &str| std::env::var(name).map_err(|_| format!("set {name}"));
        let optional = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        Ok(Self {
            client_id: required("LINGARA_CLIENT_ID")?,
            client_secret: required("LINGARA_CLIENT_SECRET")?,
            webhook_secret: optional("LINGARA_WEBHOOK_SECRET"),
            base_url: optional("LINGARA_BASE_URL"),
            token_url: optional("LINGARA_TOKEN_URL"),
        })
    }

    pub fn client(&self) -> Result<lingara::Client, String> {
        let mut builder = lingara::Client::builder().client_credentials(&self.client_id, &self.client_secret).user_agent_suffix("night-market-server/0.1");
        if let Some(url) = &self.base_url {
            builder = builder.base_url(url);
        }
        if let Some(url) = &self.token_url {
            builder = builder.token_url(url);
        }
        builder.build().map_err(|e| e.to_string())
    }
}

/// Each player's lantern total, and every event id already rewarded.
#[derive(Default)]
pub struct Rewards {
    ledger: Mutex<Ledger>,
}

#[derive(Default)]
struct Ledger {
    lanterns: HashMap<String, u32>,
    rewarded: HashSet<String>,
}

impl Rewards {
    pub fn total(&self, player: &str) -> u32 {
        self.ledger().lanterns.get(player).copied().unwrap_or(0)
    }

    /// One lantern for an event id's first delivery; a redelivery adds none.
    fn award(&self, event_id: &str, player: &str) {
        let mut ledger = self.ledger();
        if ledger.rewarded.insert(event_id.to_string()) {
            *ledger.lanterns.entry(player.to_string()).or_default() += 1;
        }
    }

    fn ledger(&self) -> MutexGuard<'_, Ledger> {
        self.ledger.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[derive(Clone)]
struct AppState {
    server: EmbedServer,
    router: Option<PlayerRouter>,
    rewards: Arc<Rewards>,
}

/// The three routes over one directory: minting remembers each player's
/// subject, and that is how a webhook finds its player.
pub fn app(config: &Config) -> Result<Router, String> {
    let directory = Arc::new(MemoryPlayerDirectory::new());
    let server = EmbedServer::new(config.client()?).with_directory(directory.clone());
    let router = match &config.webhook_secret {
        Some(secret) => Some(PlayerRouter::new(Webhook::new(secret).map_err(|e| e.to_string())?, directory)),
        None => None,
    };
    let mut routes = Router::new().route("/token", post(token)).route("/rewards", get(rewards));
    if router.is_some() {
        routes = routes.route("/webhook", post(webhook));
    }
    Ok(routes.with_state(AppState { server, router, rewards: Arc::default() }))
}

/// The stand-in session: a non-empty `X-Night-Market-Player`.
fn session(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(PLAYER_HEADER)?.to_str().ok()?.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn json(body: String) -> Response {
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

async fn token(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(player) = session(&headers) else { return StatusCode::UNAUTHORIZED.into_response() };
    match state.server.mint_token(&player, MintOptions::default()).await {
        Ok(minted) => json(token_handoff(&minted)),
        Err(e) => {
            eprintln!("minting for {player} failed: {e}");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}

async fn webhook(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> StatusCode {
    let Some(router) = &state.router else { return StatusCode::NOT_FOUND };
    let rewards = state.rewards.clone();
    let on_event = move |routed: RoutedEvent| async move {
        reward(&rewards, &routed);
        Ok::<(), DirectoryError>(())
    };
    let status = router.handle(&body, &headers, on_event).await;
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

/// A finished practice for a player this server minted for is a lantern.
/// Every other event, and a subject this server never saw, changes nothing.
fn reward(rewards: &Rewards, routed: &RoutedEvent) {
    if let (Event::PracticeCompleted(event), Some(player)) = (&routed.event, &routed.player_ref) {
        rewards.award(&event.id, player);
    }
}

async fn rewards(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match session(&headers) {
        Some(player) => json(format!("{{\"lanterns\":{}}}", state.rewards.total(&player))),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}
