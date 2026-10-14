//! The site's routes (ADR 14.10.26p D2–D4):
//!
//! - `GET /` (sets the session): the market's page. On a player's first
//!   visit it asks Lingara for their plan **as the player** (D3), then mounts
//!   `<lingara-embed>` on it; with no plan it renders the code instead.
//! - `POST /token` (session): mints the widget's token, always bound to the
//!   one allowed origin (D4), and answers exactly `{"token":…,"expires_at":…}`.
//! - `POST /webhook` (no session, mounted only with a webhook secret) and
//!   `GET /rewards` (session): the Bevy server's lanterns, restated here so
//!   the example stays copyable on its own.
//!
//! The session is a stand-in: a random `night_market_player` cookie, used as
//! the `player_ref`. A real site authenticates its visitor first.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU8;
use std::sync::{Arc, Mutex, MutexGuard};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use lingara::events::{Event, InboundEvent, SendEventOptions, Webhook};
use lingara::models::{ReactionStatus, WorldPracticeRequested};
use lingara::{AccessToken, BoxFuture, TokenSource};
use lingara_embed::server::{DirectoryError, EmbedServer, MemoryPlayerDirectory, MintOptions, PlayerRouter, RoutedEvent, token_handoff};

use crate::page;

pub const COOKIE: &str = "night_market_player";
/// The two listeners: the origin the client registers, and one it does not.
pub const DEFAULT_LISTENERS: [&str; 2] = ["localhost:8788", "127.0.0.1:8789"];
pub const DEFAULT_ALLOWED_ORIGIN: &str = "http://localhost:8788";

/// The practice every player asks for: Mei's, at the tea stall.
const TOPIC: &str = "Buying tea at a night-market stall";
const LEVEL: u8 = 2;
const SOURCE_LANG: &str = "en";
const TARGET_LANG: &str = "zh";

/// Where the server's Lingara credentials, URLs and origins come from.
/// Nothing secret here is ever printed.
pub struct Config {
    /// A metered client holding `embed:mint` (and the plan's scopes).
    pub client_id: String,
    pub client_secret: String,
    /// `lgr_whsec_…`; without it `/webhook` is not mounted.
    pub webhook_secret: Option<String>,
    pub base_url: Option<String>,
    pub token_url: Option<String>,
    /// The only origin a widget token is ever bound to.
    pub allowed_origin: String,
    /// The `lingara-origin` for a non-production Lingara web app.
    pub app_origin: Option<String>,
    /// The `host:port`s `main` binds; one router serves both.
    pub listeners: [String; 2],
}

impl Config {
    /// `LINGARA_CLIENT_ID` and `LINGARA_CLIENT_SECRET` (required),
    /// `LINGARA_WEBHOOK_SECRET`, `LINGARA_BASE_URL`, `LINGARA_TOKEN_URL`,
    /// `NIGHT_MARKET_ALLOWED_ORIGIN`, `LINGARA_APP_ORIGIN`.
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// `from_env` over any lookup, so the defaults can be read in a test.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let optional = |name: &str| lookup(name).filter(|v| !v.is_empty());
        let required = |name: &str| optional(name).ok_or_else(|| format!("set {name}"));
        Ok(Self {
            client_id: required("LINGARA_CLIENT_ID")?,
            client_secret: required("LINGARA_CLIENT_SECRET")?,
            webhook_secret: optional("LINGARA_WEBHOOK_SECRET"),
            base_url: optional("LINGARA_BASE_URL"),
            token_url: optional("LINGARA_TOKEN_URL"),
            allowed_origin: optional("NIGHT_MARKET_ALLOWED_ORIGIN").unwrap_or_else(|| DEFAULT_ALLOWED_ORIGIN.into()),
            app_origin: optional("LINGARA_APP_ORIGIN"),
            listeners: DEFAULT_LISTENERS.map(String::from),
        })
    }

    /// The server's own client, on its client credentials.
    pub fn client(&self) -> Result<lingara::Client, String> {
        self.builder().client_credentials(&self.client_id, &self.client_secret).build().map_err(|e| e.to_string())
    }

    /// A client that calls Lingara as one player, on that player's token.
    fn player_client(&self, token: AccessToken) -> Result<lingara::Client, String> {
        self.builder().token_source(Arc::new(PlayerToken(token))).build().map_err(|e| e.to_string())
    }

    fn builder(&self) -> lingara::ClientBuilder {
        let mut builder = lingara::Client::builder().user_agent_suffix("night-market-site/0.1");
        if let Some(url) = &self.base_url {
            builder = builder.base_url(url);
        }
        if let Some(url) = &self.token_url {
            builder = builder.token_url(url);
        }
        builder
    }
}

/// One player's minted token, as a token source. It is never refreshed: the
/// client makes one call with it.
struct PlayerToken(AccessToken);

impl TokenSource for PlayerToken {
    fn token(&self) -> BoxFuture<'_, Result<AccessToken, lingara::Error>> {
        let token = self.0.clone();
        Box::pin(async move { Ok(token) })
    }

    fn invalidate(&self, _token: &AccessToken) {}
}

/// What the server remembers per player, in memory: their plan, their
/// lanterns, and every event id already rewarded.
#[derive(Default)]
struct Memory {
    plans: HashMap<String, String>,
    lanterns: HashMap<String, u32>,
    rewarded: HashSet<String>,
}

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    server: EmbedServer,
    router: Option<PlayerRouter>,
    memory: Arc<Mutex<Memory>>,
}

impl AppState {
    fn memory(&self) -> MutexGuard<'_, Memory> {
        self.memory.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// The four routes over one directory: minting remembers each player's
/// subject, and that is how a webhook finds its player.
pub fn app(config: Config) -> Result<Router, String> {
    let directory = Arc::new(MemoryPlayerDirectory::new());
    let server = EmbedServer::new(config.client()?).with_directory(directory.clone());
    let router = match &config.webhook_secret {
        Some(secret) => Some(PlayerRouter::new(Webhook::new(secret).map_err(|e| e.to_string())?, directory)),
        None => None,
    };
    let mut routes = Router::new().route("/", get(index)).route("/token", post(token)).route("/rewards", get(rewards));
    if router.is_some() {
        routes = routes.route("/webhook", post(webhook));
    }
    Ok(routes.with_state(AppState { config: Arc::new(config), server, router, memory: Arc::default() }))
}

/// The stand-in session: the `night_market_player` cookie. One name to
/// read, so the `Cookie` header is split by hand.
fn session(headers: &HeaderMap) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    let value = cookies.split(';').filter_map(|c| c.trim().split_once('=')).find(|(name, _)| *name == COOKIE)?.1;
    (!value.is_empty()).then(|| value.to_string())
}

/// 16 random bytes in hex: a new visitor's player.
fn new_player() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn json(body: String) -> Response {
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

async fn index(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let (player, set_cookie) = match session(&headers) {
        Some(player) => (player, None),
        None => match new_player() {
            Ok(player) => {
                let cookie = format!("{COOKIE}={player}; HttpOnly; SameSite=Strict; Path=/");
                (player, Some(cookie))
            }
            Err(e) => {
                eprintln!("no session cookie: {e}");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        },
    };
    let html = match plan_for(&state, &player).await {
        Ok(plan_id) => page::with_widget(&plan_id, state.config.app_origin.as_deref()),
        Err(code) => page::without_plan(&code),
    };
    match set_cookie {
        Some(cookie) => ([(header::SET_COOKIE, cookie)], Html(html)).into_response(),
        None => Html(html).into_response(),
    }
}

/// The player's plan: remembered, or asked for once (D3). The error is the
/// code the page shows.
async fn plan_for(state: &AppState, player: &str) -> Result<String, String> {
    if let Some(plan_id) = state.memory().plans.get(player) {
        return Ok(plan_id.clone());
    }
    let plan_id = request_plan(state, player).await?;
    state.memory().plans.insert(player.to_string(), plan_id.clone());
    Ok(plan_id)
}

/// A plan belongs to whoever asks for it, so it is asked for under the
/// player's own unbound token, never the server's client credentials: the
/// widget's bound token shares that subject and can read it.
async fn request_plan(state: &AppState, player: &str) -> Result<String, String> {
    let minted = state.server.mint_token(player, MintOptions::default()).await.map_err(|e| failed("mint_failed", player, e))?;
    let client = state.config.player_client(minted.token).map_err(|e| failed("client_failed", player, e))?;
    let request = WorldPracticeRequested {
        generate: None,
        level: NonZeroU8::new(LEVEL).expect("a level is at least 1"),
        source_lang: SOURCE_LANG.into(),
        tags: Vec::new(),
        target_lang: TARGET_LANG.into(),
        topic: TOPIC.parse().expect("the topic is 1 to 160 characters"),
    };
    let event = InboundEvent::WorldPracticeRequested(request);
    let accepted = client.send_event(&event, SendEventOptions::default()).await.map_err(|e| failed("practice_request_failed", player, e))?;
    let reaction = accepted.into_inner().reaction.ok_or("no_reaction")?;
    if reaction.status != ReactionStatus::Started {
        return Err(reaction.code.unwrap_or_else(|| reaction.status.to_string()));
    }
    reaction.plan_id.ok_or_else(|| "no_plan_id".into())
}

fn failed(code: &str, player: &str, error: impl std::fmt::Display) -> String {
    eprintln!("{code} for {player}: {error}");
    code.into()
}

async fn token(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(player) = session(&headers) else { return StatusCode::UNAUTHORIZED.into_response() };
    let options = MintOptions { origin: Some(state.config.allowed_origin.clone()), scopes: None };
    match state.server.mint_token(&player, options).await {
        Ok(minted) => json(token_handoff(&minted)),
        Err(e) => {
            eprintln!("minting for {player} failed: {e}");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}

async fn webhook(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> StatusCode {
    let Some(router) = &state.router else { return StatusCode::NOT_FOUND };
    let memory = state.memory.clone();
    let on_event = move |routed: RoutedEvent| async move {
        if let (Event::PracticeCompleted(event), Some(player)) = (&routed.event, &routed.player_ref) {
            let mut memory = memory.lock().unwrap_or_else(|p| p.into_inner());
            if memory.rewarded.insert(event.id.clone()) {
                *memory.lanterns.entry(player.clone()).or_default() += 1;
            }
        }
        Ok::<(), DirectoryError>(())
    };
    let status = router.handle(&body, &headers, on_event).await;
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

async fn rewards(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match session(&headers) {
        Some(player) => json(format!("{{\"lanterns\":{}}}", state.memory().lanterns.get(&player).copied().unwrap_or(0))),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}
