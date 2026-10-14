# The Night Market, as a website

Lingara inside a web page. The market's tea stall is a page, and Mei, its
keeper, has a Chinese practice for you in the Lingara widget,
`<lingara-embed>`. Finish the practice and she lights you a lantern.

The example is one Rust server in `server/`. It is built on the Rust server
kit ([`lingara-embed`](https://crates.io/crates/lingara-embed)'s `server`
feature) and [`lingara`](https://crates.io/crates/lingara). The server renders
the page, mints the widget's token, and asks Lingara for the player's plan.
There is no front-end build: the page loads the widget from the kit's CDN
build. The directory is its own Cargo workspace with its own `Cargo.lock`,
so you can copy it out of the repository as it is. `rust-toolchain.toml`
names the Rust release to build with.

## The client to create

In the Lingara developer console, create a **metered** API client, on an
account with embedding switched on, holding:

- **`embed:mint`**, to mint each player's token;
- **`events:write`**, **`lesson_plans:write`** and **`lesson_plans:read`**,
  for the player's plan;
- **`events:read`** and **`embed:play`**, for the lantern webhook.

Under "Websites that can show the Lingara widget", register
**`http://localhost:8788` and nothing else**. The second address this server
listens on must stay unregistered: it is there to be refused.

Keep the client id and secret on the server only. The page never sees them.

## Run it

    export LINGARA_CLIENT_ID=lgr_ci_…
    export LINGARA_CLIENT_SECRET=lgr_cs_…
    make site-example DIR=websites/rust/night-market

(or `cd server && cargo run --locked`). One process serves the same page on
two origins:

| URL | What you should see |
|---|---|
| <http://localhost:8788> | The registered origin. The widget loads and you practise. |
| <http://127.0.0.1:8789> | Not registered. Lingara answers `403 embed_origin_mismatch`, and the page shows "This site is not on the client's list of websites that can show Lingara." |

The browser's network log shows no `/v1/` request from either page's
origin, and no token in any URL. The token travels to Lingara's frame by
`postMessage`.

## How it works

- **`GET /`** gives a new visitor a `night_market_player` cookie, which
  stands in for a real login: a random id, `HttpOnly`, `SameSite=Strict`.
  `localhost` and `127.0.0.1` keep separate cookies, so each origin has its
  own player.
- **The plan is asked for as the player.** On a player's first visit, the
  server mints that player an unbound token and sends
  `world.practice_requested` **under that token**, not under the server's
  client credentials. A plan belongs to whoever asks for it. A plan the
  server asked for itself would be the developer's, and the widget would
  read it as `plan_not_found`. The server remembers the plan id for the
  cookie and mounts the widget on it at once. Lingara's frame waits for a
  plan that is still generating.
- **`POST /token`** is the widget's token endpoint. It mints for the cookie's
  player and **always binds the token to `NIGHT_MARKET_ALLOWED_ORIGIN`**
  (default `http://localhost:8788`), never to the origin that asked. That
  is the whole refusal: on `:8789` the frame tells Lingara the page is
  `http://127.0.0.1:8789`, which is not the token's origin.
- **`POST /webhook`** (mounted only with `LINGARA_WEBHOOK_SECRET`) turns each
  `practice.completed` into one lantern, and **`GET /rewards`** reports the
  player's lanterns. The page polls it after a practice.

If Lingara gives no plan (a refusal such as `rate_limited`, or a failed
call), the page shows the code instead of the widget.

## The plan budget

Each first visit costs one lesson plan. A metered client gets **5 plans a
day across all its players**, and each player gets 4 a day (2 an hour). Keep
your browser's cookies and the server running between attempts: a new cookie
or a restarted server asks for a new plan. This is an example for one
developer's browser, not a public page.

## Environment

| Variable | Default | |
|---|---|---|
| `LINGARA_CLIENT_ID`, `LINGARA_CLIENT_SECRET` | required | The client above |
| `LINGARA_WEBHOOK_SECRET` | unset | `lgr_whsec_…`; without it there are no lanterns |
| `NIGHT_MARKET_ALLOWED_ORIGIN` | `http://localhost:8788` | The one origin every widget token is bound to |
| `LINGARA_BASE_URL`, `LINGARA_TOKEN_URL` | production | A non-production Lingara API |
| `LINGARA_APP_ORIGIN` | production | The widget's `lingara-origin`, for a non-production Lingara web app |

**Lanterns need a webhook Lingara can reach.** Register
`<public URL>/webhook` as the client's webhook endpoint, for example through
a tunnel to port 8788. Without one you still practise, but you earn no
lantern.

The plans, the players and the lanterns live in memory, so they are lost
when the server stops. A real site keeps them in its database.

## Tests

    cd server && cargo test --locked

The tests are offline. They drive the real routes against an in-process stub
of Lingara and check which origin a token is bound to, which token asks for
the plan, one plan per player, and the cookie.
