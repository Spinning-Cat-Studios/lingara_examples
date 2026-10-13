# Luba's Dive Log

**Packages:** [`@lingara/apps`](https://www.npmjs.com/package/@lingara/apps)
and [`@lingara/api`](https://www.npmjs.com/package/@lingara/api), at the
versions `package.json` pins.
**Scopes:** none asked of the learner. The app's own OAuth client holds
`lesson_plans:read`, for one owner-scoped read.

Luba runs a submersible ride-share. Its passengers who learn with Lingara
install Luba's app, and the app puts their next dive where they study: a card
beside Home (`home.side`) and one in the empty plan pane (`plans.empty_detail`).

This is a whole app, not a snippet: it builds its manifest, answers Lingara's
signed render and action requests with cards, uses the context the learner
agreed to share, calls the API as its owner, and can be listed on the shelf.

You need Node 22.18 or later (`.node-version`).

```sh
npm ci
npm test        # offline: nothing reaches the network
```

## What it shows

| Slot | Card |
|---|---|
| `home.side` | "Next dive"; the dive's phrase in the learner's target language; "N words due · M learned"; a progress bar for the learner's plan; "Dives logged: N"; a `Logged` button. A tutor note names the destination |
| `plans.empty_detail` | The dive's phrase; "Luba's phrasebook: <title>" once the owner's read has succeeded; a link to Luba's dive sites |

**Every context slice is optional.** The manifest asks for `languages`,
`review_due` and `plan_summary`, and the install consent card leaves each box
unticked by default. A learner who shares none of them still gets a useful
card: no `languages` shows the English phrase, no `review_due` drops that
line, no `plan_summary` drops the bar. `src/context.ts` is where each branch
lives, and `test/cards.test.ts` renders all eight combinations.

**The learner's data comes only from the slices.** Installing an app grants
it no learner token, and the app's client-credentials token speaks for the
app's **owner**: `GET /v1/lesson-plans/{id}` with that token reads the owner's
plans, never the learner's. So the progress bar uses `plan_summary.title`
from the slice and the app never calls the API with the learner's `plan_id`.
The one API call (`src/phrasebook.ts`) reads a plan the owner generated in
their own account, under a 1 s timeout and with no retries, so a slow API
cannot spend the relay's 3 s render budget.

**State is in memory**, per `subject`. A real app would keep it in its own
database and key each `logged` action on the request's `id`, so a duplicate
delivery counts once; this one counts it twice.

## Running it end to end

You need:

- an OAuth client on your Lingara account with `lesson_plans:read`, and a
  plan you generated in that account (the phrasebook);
- a second account to install the app from. Apps are in preview: the Apps
  page appears only on accounts that have them;
- while the app runs on your own machine, a tunnel that gives it a public
  HTTPS address. This walk uses Cloudflare's (`brew install cloudflared`, or
  see Cloudflare's downloads page), but any tunnel works.

1. **Expose the app.** Lingara calls your app over public HTTPS, so a port on
   your laptop needs a tunnel. Deployed at your own HTTPS host, it needs none:
   use that host in step 2 instead. A Cloudflare quick tunnel needs no
   account:

   ```sh
   cloudflared tunnel --url http://localhost:8787
   ```

   It prints the tunnel's `https://….trycloudflare.com` address. Start it
   before step 2, because the manifest records that address, and leave it
   running for the rest of the walk. A quick tunnel's URL changes on every
   run, so the manifest has to be built and uploaded again each time. A stable
   listing needs a stable host.

2. **Build the manifest.** It needs no secret:

   ```sh
   LUBA_RENDER_URL=https://<your-tunnel>.trycloudflare.com LUBA_LISTED=1 npm run manifest
   ```

   `LUBA_LISTED=1` asks for a place on the shelf. Leave it out to keep the app
   unlisted; it can still be installed from its install link.

3. **Upload it** in the console's apps page against your OAuth client. The
   first upload creates the app and reveals its `lgr_whsec_` signing secret
   once; a re-upload keeps both.

4. **Start the app** with that secret, and your client's credentials for the
   phrasebook:

   ```sh
   LINGARA_APP_SECRET=lgr_whsec_… \
   LINGARA_CLIENT_ID=… LINGARA_CLIENT_SECRET=… LUBA_PLAN_ID=… \
     npm start
   ```

   During a secret rotation, pass both secrets, comma-separated. The three
   phrasebook variables are optional together; without them the phrasebook
   line is simply absent. `LINGARA_APP_PORT` overrides 8787.

5. **Install it** from another account: find Luba's Dive Log under Apps,
   press Install, and choose which slices to share. Try leaving
   `plan_summary` unticked: the `home.side` card has no progress bar.

6. **See the card** beside Home and in the empty plan pane. Press `Logged`
   and the card comes back with "Dives logged: 1".

## Files

| File | What it does |
|---|---|
| `src/server.ts` | The `node:http` server, through the kit's `nodeHandler` |
| `src/app.ts` | `createApp`: the render function and the `logged` action, no I/O of its own |
| `src/cards.ts` | The two cards, built only through the kit's builders |
| `src/context.ts` | Each slice → what the card shows |
| `src/phrasebook.ts` | The owner's one API read, memoised and bounded |
| `src/dives.ts` | The destinations and their phrases: a fixed table |
| `src/manifest.ts` | `buildManifest`, and `npm run manifest` |
