//! `night-market-site`, the Night Market as a website (ADR 14.10.26p).
//!
//!     cargo run --locked
//!
//! It reads its credentials and origins from the environment (see
//! `routes::Config`), never prints a secret, and serves one router on both
//! of `Config`'s listeners: the registered origin, where the widget
//! practises, and an unregistered one, where Lingara refuses it.

mod page;
mod routes;

use std::future::IntoFuture as _;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match serve().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("night-market-site: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn serve() -> Result<(), String> {
    let config = routes::Config::from_env()?;
    if config.webhook_secret.is_none() {
        println!("LINGARA_WEBHOOK_SECRET is not set: /webhook is off, and no practice earns a lantern");
    }
    let allowed = config.allowed_origin.clone();
    let addrs = config.listeners.clone();
    let app = routes::app(config)?;
    let mut servers = Vec::new();
    for addr in &addrs {
        let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| format!("{addr}: {e}"))?;
        let verdict = if allowed == format!("http://{addr}") { "practises (the registered origin)" } else { "should be refused (not registered)" };
        println!("http://{addr} {verdict}");
        servers.push(tokio::spawn(axum::serve(listener, app.clone()).into_future()));
    }
    for server in servers {
        server.await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    }
    Ok(())
}
