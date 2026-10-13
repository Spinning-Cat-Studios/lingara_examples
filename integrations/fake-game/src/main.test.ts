// The fake game, offline: an in-process stub stands in for Lingara, so these
// tests need no client and never reach production.
//
//   npm test

import assert from "node:assert/strict";
import { createHmac, randomBytes } from "node:crypto";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";
import { test, type TestContext } from "node:test";

import { clientFromEnv, runGame, SCOPES } from "./main.ts";
import { receive } from "./receiver.ts";

const PLAN_ID = "3f1c2a9e-5b7d-4e21-9a0c-6d8e4f2b1a37";

type Reply = (req: IncomingMessage, body: string, res: ServerResponse) => void;

function json(res: ServerResponse, status: number, value: unknown): void {
  res.writeHead(status, { "content-type": "application/json" }).end(JSON.stringify(value));
}

/** One recorded exchange: the token, an empty horizon, the 202, then the plan's `lesson_plan.ready`. */
const routes: Record<string, Reply> = {
  "POST /oauth/token": (_req, _body, res) =>
    json(res, 200, { access_token: "lgr_at_example", token_type: "Bearer", expires_in: 3600, scope: SCOPES.join(" ") }),
  "GET /v1/events?start=latest": (_req, _body, res) => json(res, 200, { items: [], next_cursor: "h0", has_more: false }),
  "POST /v1/events": (_req, _body, res) =>
    json(res, 202, {
      id: "lgr_evt_sent",
      type: "world.context_changed",
      created_at: "2026-10-04T09:12:44Z",
      reaction: { status: "started", plan_id: PLAN_ID, plan_status: "generating" },
    }),
  "GET /v1/events?cursor=h0": (_req, _body, res) =>
    json(res, 200, {
      items: [
        {
          id: "lgr_evt_ready",
          type: "lesson_plan.ready",
          created_at: "2026-10-04T09:14:44Z",
          api_version: "2026-09-equipped-boxfish",
          subject: "lgr_sub_example",
          data: { plan_id: PLAN_ID, status: "complete", title: "A night market after rain", source_lang: "en", target_lang: "zh", level: 2 },
        },
      ],
      next_cursor: "c1",
      has_more: false,
    }),
};

/** The stub Lingara over `routes` (with `overrides` on top), and the client that talks to it. */
async function stubbed(t: TestContext, overrides: Record<string, Reply> = {}) {
  const table = { ...routes, ...overrides };
  const tokenForms: URLSearchParams[] = [];
  const stub = createServer((req, res) => {
    let body = "";
    req.on("data", (chunk: Buffer) => (body += chunk.toString()));
    req.on("end", () => {
      const key = `${req.method} ${req.url}`;
      if (key === "POST /oauth/token") tokenForms.push(new URLSearchParams(body));
      const reply = table[key];
      if (reply) reply(req, body, res);
      else json(res, 404, { error: `the stub has no ${key}` });
    });
  });
  await new Promise<void>((resolve) => stub.listen(0, "127.0.0.1", () => resolve()));
  t.after(() => stub.close());
  const base = `http://127.0.0.1:${(stub.address() as AddressInfo).port}`;

  const client = clientFromEnv({
    LINGARA_CLIENT_ID: "lgr_cid_example0000000000000",
    LINGARA_CLIENT_SECRET: "lgr_cs_example000000000000000000000000000000000000",
    LINGARA_BASE_URL: base,
    LINGARA_TOKEN_URL: `${base}/oauth/token`,
  });
  return { client, tokenForms };
}

const SCENE = { scene: "A night market after rain", sourceLang: "en", targetLang: "zh", level: 2 };

test("the game side, offline", async (t) => {
  const { client, tokenForms } = await stubbed(t);
  const lines: string[] = [];
  const code = await runGame(client, SCENE, (line) => lines.push(line));

  assert.equal(tokenForms.length, 1);
  assert.deepEqual(tokenForms[0]?.get("scope")?.split(" ").sort(), [...SCOPES].sort());
  assert.deepEqual(lines, [
    "feed: horizon cursor h0",
    "sent: world.context_changed lgr_evt_sent reaction=started",
    `feed: lesson_plan.ready lgr_evt_ready plan=${PLAN_ID}`,
  ]);
  assert.equal(code, 0);
});

test("a plan served from the pool is a success with nothing to wait for", async (t) => {
  const { client } = await stubbed(t, {
    "POST /v1/events": (_req, _body, res) =>
      json(res, 202, {
        id: "lgr_evt_sent",
        type: "world.context_changed",
        created_at: "2026-10-05T08:04:50Z",
        reaction: { status: "started", plan_id: PLAN_ID, plan_status: "complete" },
      }),
    // The feed is never read past the horizon: a read would mean the game waited.
    "GET /v1/events?cursor=h0": (_req, _body, res) => json(res, 500, { error: "the game should not wait" }),
  });
  const lines: string[] = [];
  const code = await runGame(client, SCENE, (line) => lines.push(line));

  assert.deepEqual(lines, [
    "feed: horizon cursor h0",
    "sent: world.context_changed lgr_evt_sent reaction=started",
    `plan ${PLAN_ID} was served from the pool (complete), no event promised`,
  ]);
  assert.equal(code, 0);
});

// 6.10.26k AC6
test("require-event refuses a plan served from the pool", async (t) => {
  const served = {
    "POST /v1/events": ((_req, _body, res) =>
      json(res, 202, {
        id: "lgr_evt_sent",
        type: "world.context_changed",
        created_at: "2026-10-06T08:04:50Z",
        reaction: { status: "started", plan_id: PLAN_ID, plan_status: "complete" },
      })) as Reply,
    // The feed is never read past the horizon, with or without the flag.
    "GET /v1/events?cursor=h0": ((_req, _body, res) => json(res, 500, { error: "the game should not wait" })) as Reply,
  };

  const lenient = await stubbed(t, served);
  assert.equal(await runGame(lenient.client, SCENE, () => {}), 0);

  const strict = await stubbed(t, served);
  const lines: string[] = [];
  assert.equal(await runGame(strict.client, SCENE, (line) => lines.push(line), false, true), 1);
  assert.equal(
    lines.at(-1),
    `plan ${PLAN_ID} was served from the pool (complete); --require-event wants a generation`,
  );

  const generating = await stubbed(t);
  const ready: string[] = [];
  assert.equal(await runGame(generating.client, SCENE, (line) => ready.push(line), false, true), 0);
  assert.equal(ready.at(-1), `feed: lesson_plan.ready lgr_evt_ready plan=${PLAN_ID}`);
});

/** Standard Webhooks: HMAC-SHA256 over `id.timestamp.body`, keyed with the secret's base64 bytes. */
function sign(secret: string, id: string, timestamp: string, body: string): string {
  const key = Buffer.from(secret.slice("lgr_whsec_".length), "base64");
  return `v1,${createHmac("sha256", key).update(`${id}.${timestamp}.${body}`).digest("base64")}`;
}

test("the receiver, offline", async (t) => {
  const secret = `lgr_whsec_${randomBytes(32).toString("base64")}`;
  const server = await receive(0, secret, () => {});
  t.after(() => server.close());
  const url = `http://127.0.0.1:${(server.address() as AddressInfo).port}/`;

  const id = "lgr_evt_delivered";
  const timestamp = String(Math.floor(Date.now() / 1000));
  const body = JSON.stringify({
    id,
    type: "lesson_plan.ready",
    created_at: "2026-10-04T09:14:44Z",
    api_version: "2026-09-equipped-boxfish",
    subject: "lgr_sub_example",
    data: { plan_id: PLAN_ID, status: "complete", title: "A night market after rain", source_lang: "en", target_lang: "zh", level: 2 },
  });
  const headers = { "content-type": "application/json", "webhook-id": id, "webhook-timestamp": timestamp, "webhook-signature": sign(secret, id, timestamp, body) };

  const signed = await fetch(url, { method: "POST", headers, body });
  assert.equal(signed.status, 204);

  const tampered = body.replace("night", "nIght");
  const refused = await fetch(url, { method: "POST", headers, body: tampered });
  assert.equal(refused.status, 400);
  assert.equal(await refused.text(), "no_matching_signature");
});
