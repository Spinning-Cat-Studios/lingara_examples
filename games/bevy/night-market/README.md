# The Night Market, in Bevy

Lingara inside a Bevy game. You walk a street at night with three stalls on
it. At the tea stall, Mei speaks Chinese; at the book stall, Professor Tanaka
speaks Japanese. Each answers at your level, and you can change it. Ask
either of them for a practice. When Lingara has the result, the keeper thanks
you and gives you a lantern.

The example is two halves in one directory:

- `game/` — the Bevy game, on [`bevy_lingara`](https://crates.io/crates/bevy_lingara).
- `server/` — the game's server, on the Rust server kit
  ([`lingara-embed`](https://crates.io/crates/lingara-embed)'s `server`
  feature) and [`lingara`](https://crates.io/crates/lingara). It mints each
  player's Lingara token and turns a practice result into a lantern.

Each is its own Cargo workspace with its own `Cargo.lock`, so you can copy
this directory out of the repository as it is. Everything comes from
crates.io at the exact versions in each `Cargo.toml`, and
`rust-toolchain.toml` names the Rust release to build with.

## The client to create

The server needs a Lingara API client that can mint tokens for players:

- a **metered** client (embedding is metered only);
- holding the **`embed:mint`** scope;
- on an account with embedding switched on.

Create it in the Lingara developer console and keep its id and secret on the
server only. The game never sees them.

## The server

**The server trusts the `X-Night-Market-Player` header as the player's
identity.** That header stands in for a real login so the example stays small.
A real game server authenticates its player first and then uses its own
player id as the `player_ref`.

    cd server
    export LINGARA_CLIENT_ID=lgr_ci_…
    export LINGARA_CLIENT_SECRET=lgr_cs_…
    cargo run -- serve              # http://127.0.0.1:8787 (or --port <port>)

It answers three routes:

| Route | Session | Does |
|---|---|---|
| `POST /token` | yes | Mints the player's token (no `origin`, no `scopes`, so the player gets Lingara's default set) and answers exactly `{"token":…,"expires_at":…}` |
| `POST /webhook` | no | Verifies a Lingara webhook, finds its player, and gives one lantern per `practice.completed`, once per event id. Answers `204`, `401`, `400` or `500`, with an empty body |
| `GET /rewards` | yes | `{"lanterns":N}`, the player's running total |

A route that needs the session and has no header answers `401`.

**Lanterns are optional.** `/webhook` is mounted only when
`LINGARA_WEBHOOK_SECRET` (`lgr_whsec_…`) is set. Lingara has to reach it, so it
needs a public URL: put a tunnel in front of port 8787 and register
`<tunnel>/webhook` as the client's webhook endpoint. Without it the game still
runs, and the keepers still talk and practise; you just earn no lanterns.

To forget a player at Lingara and revoke their tokens:

    cargo run -- delete-player player-1

`LINGARA_BASE_URL` and `LINGARA_TOKEN_URL` point the server somewhere other
than production; the tests use them for a local stub.

The rewards and the player directory live in memory, so they are lost when
the server stops. A real server keeps both in its database.

## The game

Start the server first, then:

    cd game
    cargo run

| Key | Does |
|---|---|
| `←` `→` | Walk along the street. Arriving at a keeper's stall tells Lingara where you are |
| `Enter` | Type a line to the keeper here; `Enter` sends it, `Esc` drops it. Pinyin and kana input work while a line is open |
| `+` `-` | The keeper's level (1–9). Each keeper keeps their own |
| `P` | Practise with the keeper here |
| `1`–`4`, `Space`, `Y` `N`, `Esc` | In a practice: choose, reveal or go on, grade yourself, finish early. A typed line answers a question |

`NIGHT_MARKET_SERVER` (default `http://127.0.0.1:8787`) is the server's
address, and `NIGHT_MARKET_PLAYER` (default `player-1`) is the stand-in
session the game sends. Run a second copy with another `NIGHT_MARKET_PLAYER`
to be a second player.

After a practice, the game asks the server for your lantern total every five
seconds for up to five minutes. Lingara sends the result to `/webhook`, so
without a tunnel no lantern arrives.

Bevy's built-in font has no Chinese or Japanese characters, so the game
bundles a subset of Noto Sans CJK SC under the SIL Open Font Licence
(`game/assets/fonts/`, licence in `OFL.txt`).

## How it is built

- `game/src/logic.rs` holds every rule of the market and draws nothing.
  `game/src/view.rs` draws it and turns keys into the game's own Messages.
  The logic is the only part that writes `bevy_lingara`'s Messages.
- `game/tests/market.rs` runs the logic headless, with no Lingara and no
  network.
- `server/tests/server.rs` runs the server's routes against a local stub of
  Lingara, with a webhook signed the way Lingara signs one.

Run both from each half's directory:

    cargo test --locked

The game builds for desktop only: `bevy_lingara` does not support `wasm32`.
On Linux, Bevy needs `libasound2-dev libudev-dev libwayland-dev
libxkbcommon-dev`.
