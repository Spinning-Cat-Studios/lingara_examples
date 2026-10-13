//! The Night Market, in Bevy (ADR 13.10.26h). Start the server half first
//! (`../server`, README.md), then `cargo run` here.
//!
//! `NIGHT_MARKET_SERVER` (default `http://127.0.0.1:8787`) is the server's
//! base URL, and `NIGHT_MARKET_PLAYER` (default `player-1`) stands in for the
//! player's session: it is sent as `X-Night-Market-Player` on the token call
//! and on the rewards poll. The game holds no secret.

mod logic;
mod view;

use bevy::prelude::*;
use bevy_lingara::lingara_embed::Header;
use bevy_lingara::{LingaraConfig, LingaraPlugin};

use logic::MarketLogic;
use view::MarketView;

/// The stand-in session header the server reads as the `player_ref`.
pub const PLAYER_HEADER: &str = "X-Night-Market-Player";

fn main() {
    let server = std::env::var("NIGHT_MARKET_SERVER").unwrap_or_else(|_| "http://127.0.0.1:8787".into());
    let player = std::env::var("NIGHT_MARKET_PLAYER").unwrap_or_else(|_| "player-1".into());
    let session = Header { name: PLAYER_HEADER.into(), value: player.clone().into() };
    // The stream stays paused: the reward arrives by webhook at the server,
    // so the game reads no stream event and pays for none.
    let lingara = LingaraConfig::new()
        .token_endpoint(format!("{}/token", server.trim_end_matches('/')), vec![session])
        .source_lang(logic::SOURCE_LANG)
        .stream_on_start(false)
        .user_agent_suffix("night-market-bevy/0.1");
    let window = Window { title: "The Night Market".into(), ..default() };
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(window), ..default() }))
        .add_plugins(LingaraPlugin::new(lingara))
        .add_plugins((MarketLogic, MarketView { server, player }))
        .run();
}
