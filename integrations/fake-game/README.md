# The fake game

**Package:** [`@lingara/api`](https://www.npmjs.com/package/@lingara/api), at
the version `package.json` pins.
**Scopes:** `events:read`, `events:write`, `lesson_plans:read`,
`lesson_plans:write`.

A headless stand-in for a game that uses Lingara's events. It tells Lingara
what is happening in its world, waits for the lesson plan that produces, and
can receive Lingara's webhooks. It asks for exactly the four scopes above,
even when its client holds more.

You need Node 22.18 or later (`.node-version`) and an OAuth client that holds
the four scopes.

```sh
npm ci
npm test        # offline: an in-process stub stands in for Lingara
```

## The game side

```sh
LINGARA_CLIENT_ID=… LINGARA_CLIENT_SECRET=… \
  npm start -- --scene "A night market after rain" --source-lang en --target-lang zh --level 2 [--tail]
```

`LINGARA_BASE_URL` and `LINGARA_TOKEN_URL` override the production
endpoints.

It prints one line per step:

1. it reads the feed to its end with `start: latest` and keeps the horizon
   cursor, **before** it sends anything, so a `lesson_plan.*` that fires
   between the send and the first read is never missed;
2. it sends `world.context_changed` with `generate: true`;
3. it reads the feed from that cursor (or tails it, with `--tail`) until the
   `lesson_plan.ready` or `.failed` whose `data.plan_id` is the `202`'s
   `reaction.plan_id`, skipping every other event.

Only `reaction.status: started` with `plan_status: generating` promises an
event. Any other reaction ends the run with a line and a non-zero exit. A
plan served from the pool is readable at once and no event is promised, so
pick a scene Lingara cannot already serve: the same scene on a second day is
usually served from the pool.

## The receiver

```sh
LINGARA_WEBHOOK_SECRET=lgr_whsec_… npm start -- --receive 8787
```

During a rotation, set both secrets, comma-separated. The receiver verifies
the raw body, answers `204`, and prints the event and which secret's
signature matched. A delivery it refuses is answered `400` with the reason,
and Lingara keeps retrying it on its schedule. Put your own public HTTPS
tunnel in front of the port and register its URL as the endpoint.
