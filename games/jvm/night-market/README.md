# The Night Market, in libGDX

Lingara inside a JVM game. You walk a street at night with three stalls on
it. At the tea stall, Mei speaks Chinese; at the book stall, Professor Tanaka
speaks Japanese. Each answers at your level, and you can change it. Ask
either of them for a practice. When Lingara has the result, the keeper thanks
you and gives you a lantern.

It is the same game as [the Bevy Night Market](../../bevy/night-market/),
written in Kotlin. The example is one Gradle build with three modules:

- `core/` — the game, on
  [`lingara-embed-kotlin`](https://central.sonatype.com/artifact/com.getlingara/lingara-embed-kotlin).
- `lwjgl3/` — the desktop launcher, with libGDX's natives and
  [`lingara-embed-natives-desktop`](https://central.sonatype.com/artifact/com.getlingara/lingara-embed-natives-desktop)
  (Windows x64, macOS arm64 and x64, Linux x64).
- `server/` — the game's server, on
  [`lingara-embed-server-kotlin`](https://central.sonatype.com/artifact/com.getlingara/lingara-embed-server-kotlin)
  and [`lingara-kotlin`](https://central.sonatype.com/artifact/com.getlingara/lingara-kotlin).
  It mints each player's Lingara token and turns a practice result into a
  lantern.

Everything comes from Maven Central, at the exact versions in
`gradle/libs.versions.toml`, and each module locks what it resolves in its own
`gradle.lockfile`. The Gradle wrapper is committed, so you need only a JDK,
17 or newer, and you can copy this directory out of the repository as it is.

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

    export LINGARA_CLIENT_ID=lgr_ci_…
    export LINGARA_CLIENT_SECRET=lgr_cs_…
    ./gradlew :server:run              # http://127.0.0.1:8787

It answers three routes, on the JDK's own HTTP server:

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

    ./gradlew :server:run --args="delete-player player-1"

`LINGARA_BASE_URL` and `LINGARA_TOKEN_URL` point the server somewhere other
than production; the tests use them for a local stub.

Two simplifications a real server would not make: the rewards and the player
directory live in memory, so they are lost when the server stops, and the
kit's `suspend` calls run under `runBlocking` on the request thread.

## The game

Start the server first, then:

    ./gradlew :lwjgl3:run

| Key | Does |
|---|---|
| `←` `→` | Walk along the street. Arriving at a keeper's stall tells Lingara where you are |
| `Enter` | Type a line to the keeper here; `Enter` sends it, `Esc` drops it |
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

libGDX's default font has no Chinese or Japanese characters, so the game
bundles a subset of Noto Sans CJK SC under the SIL Open Font Licence
(`core/src/main/resources/fonts/`, licence in `OFL.txt`) and draws it through
`gdx-freetype`. On macOS the launcher starts the JVM with
`-XstartOnFirstThread`, because GLFW must own the main thread.

## How it is built

- `core/src/main/kotlin/MarketLogic.kt` holds every rule of the market and
  draws nothing. `MarketScreen.kt` draws it and turns keys into the game's own
  `MarketIntent`s. The logic is the only part that calls the kit, through the
  small `MarketPort` interface in `MarketPort.kt`.
- `core/src/test/kotlin/MarketLogicTest.kt` runs the logic over a fake port,
  with no natives and no network.
- `server/src/test/kotlin/ServerTest.kt` runs the server's routes against a
  local stub of Lingara, with a webhook signed the way Lingara signs one.

Run both:

    ./gradlew test

## Keeping it current

Each `com.getlingara` artefact has its own line in
`gradle/libs.versions.toml`, so an update moves one version. After changing
one, relock every module:

    ./gradlew resolveAndLockAll --write-locks

**`lingara-embed-kotlin` and `lingara-embed-natives-desktop` must move
together.** The kit refuses, when the game starts, a native library of another
version, and nothing in the build notices: an update of one without the other
builds, passes its tests and fails at launch.
