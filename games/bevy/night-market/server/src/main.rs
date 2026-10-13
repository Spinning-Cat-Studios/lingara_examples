//! `night-market-server`, the Night Market's server half (ADR 13.10.26h D3).
//!
//!     night-market-server serve [--port 8787]
//!     night-market-server delete-player <player_ref>
//!
//! It reads its credentials from the environment (see `routes::Config`) and
//! never prints one.

mod routes;

use std::net::SocketAddr;
use std::process::ExitCode;

use lingara_embed::server::EmbedServer;

const USAGE: &str = "usage: night-market-server serve [--port <port>] | delete-player <player_ref>";
const DEFAULT_PORT: u16 = 8787;

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("night-market-server: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: &[String]) -> Result<(), String> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] | ["serve"] => serve(DEFAULT_PORT).await,
        ["serve", "--port", port] => serve(port.parse().map_err(|_| format!("--port {port} is not a port"))?).await,
        ["delete-player", player_ref] => delete_player(player_ref).await,
        _ => Err(USAGE.into()),
    }
}

async fn serve(port: u16) -> Result<(), String> {
    let config = routes::Config::from_env()?;
    if config.webhook_secret.is_none() {
        println!("LINGARA_WEBHOOK_SECRET is not set: /webhook is off, and no practice earns a lantern");
    }
    let app = routes::app(&config)?;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| format!("{addr}: {e}"))?;
    println!("the Night Market's server is listening on http://{addr}");
    axum::serve(listener, app).await.map_err(|e| e.to_string())
}

/// Deletes the player at Lingara (revoking their tokens) and says so.
async fn delete_player(player_ref: &str) -> Result<(), String> {
    let server = EmbedServer::new(routes::Config::from_env()?.client()?);
    server.delete_player(player_ref).await.map_err(|e| e.to_string())?;
    println!("deleted {player_ref}");
    Ok(())
}
