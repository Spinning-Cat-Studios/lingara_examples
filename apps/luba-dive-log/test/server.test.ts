// The app behind the kit's node:http adapter, on port 0, with requests signed
// the way Lingara signs them. @lingara/api has no signing helper, so this
// signs with node:crypto, as the kit's own adapter test does.

import assert from "node:assert/strict";
import { createHmac } from "node:crypto";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { after, before, test } from "node:test";

import { nodeHandler } from "@lingara/apps";

import { createApp, parseSecrets } from "../src/app.ts";
import { noPhrasebook } from "../src/phrasebook.ts";

// Visibly fake secrets, built at run time: lgr_whsec_ + base64 of 32 ASCII bytes.
const secret = (seed: string): string => `lgr_whsec_${Buffer.from(seed.padEnd(32, "!")).toString("base64")}`;
const OLD = secret("luba-dive-log-old");
const NEW = secret("luba-dive-log-new");

/** Standard Webhooks: v1,<base64 HMAC-SHA256 over id.timestamp.body>. */
function sign(body: string, key: string, id: string): Record<string, string> {
  const timestamp = String(Math.floor(Date.now() / 1000));
  const raw = Buffer.from(key.slice("lgr_whsec_".length), "base64");
  const signature = createHmac("sha256", raw).update(`${id}.${timestamp}.${body}`).digest("base64");
  return {
    "content-type": "application/json",
    "webhook-id": id,
    "webhook-timestamp": timestamp,
    "webhook-signature": `v1,${signature}`,
  };
}

let ids = 0;
const nextId = (): string => `lgr_msg_${String(++ids).padStart(32, "0")}`;
const base = (subject: string) => ({
  install_id: "7c1d2e3f-4a5b-4c6d-8e7f-9a0b1c2d3e4f",
  subject,
  slot: "home.side",
  locale: "en",
  context: [{ kind: "languages", source_lang: "en", target_lang: "es" }],
});

const server = createServer(nodeHandler(createApp([OLD, NEW], noPhrasebook)));
let url = "";

before(async () => {
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  url = `http://127.0.0.1:${(server.address() as AddressInfo).port}/`;
});
after(() => new Promise<void>((resolve) => server.close(() => resolve())));

async function post(payload: object, key = NEW): Promise<Response> {
  const id = nextId();
  const body = JSON.stringify({ id, ...payload });
  return fetch(url, { method: "POST", body, headers: sign(body, key, id) });
}

/** 4.10.26d AC3 */
test("a signed render is answered and a tampered one refused", async () => {
  const id = nextId();
  const body = JSON.stringify({ id, type: "app.render", ...base("lgr_sub_render") });
  const headers = sign(body, NEW, id);

  const ok = await fetch(url, { method: "POST", body, headers });
  assert.equal(ok.status, 200);
  const reply = (await ok.json()) as { card: { elements: { type: string; text?: string }[] }; tutor_note?: string };
  assert.equal(reply.card.elements[0]?.text, "Next dive");
  assert.match(reply.tutor_note ?? "", /next dive with Luba/);

  const tampered = body.replace("lgr_sub_render", "lgr_sub_rendes");
  const refused = await fetch(url, { method: "POST", body: tampered, headers });
  assert.equal(refused.status, 401);
});

test("LINGARA_APP_SECRET holds one secret, or two during a rotation", () => {
  assert.deepEqual(parseSecrets(undefined), []);
  assert.deepEqual(parseSecrets(" , "), []);
  assert.deepEqual(parseSecrets(OLD), [OLD]);
  assert.deepEqual(parseSecrets(`${OLD}, ${NEW}`), [OLD, NEW]);
});

/** 4.10.26d AC4 */
test("each logged action records one dive", async () => {
  const action = { type: "app.action", ...base("lgr_sub_logger"), action_id: "logged", card_etag: "c1" };
  const texts = async (response: Response): Promise<string[]> => {
    assert.equal(response.status, 200);
    const reply = (await response.json()) as { card: { elements: { type: string; text?: string }[] } };
    return reply.card.elements.filter((e) => e.type === "text").map((e) => e.text!);
  };
  assert.ok((await texts(await post(action, OLD))).includes("Dives logged: 1"));
  assert.ok((await texts(await post(action))).includes("Dives logged: 2"));
});
