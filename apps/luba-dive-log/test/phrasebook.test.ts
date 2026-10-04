// The owner's phrasebook read, offline: the library's `fetch` option answers
// both the token endpoint and the plan, so nothing leaves the process.

import assert from "node:assert/strict";
import { test } from "node:test";

import { Lingara } from "@lingara/api";

import { noPhrasebook, ownerPhrasebook, phrasebookFromEnv } from "../src/phrasebook.ts";

const PLAN_ID = "3f1c2a9e-5b7d-4e21-9a0c-6d8e4f2b1a37";
const PLAN = {
  id: PLAN_ID,
  status: "complete",
  title: "Phrases for the deep",
  source_lang: "en",
  target_lang: "fr",
  level: 2,
  created_at: "2026-10-04T09:12:44Z",
  ai_generated: true,
};

type PlanReply = (signal: AbortSignal | undefined) => Promise<Response>;

const json = (status: number, value: unknown): Response =>
  new Response(JSON.stringify(value), { status, headers: { "content-type": "application/json" } });

/** A client whose `fetch` answers the token itself and the plan with `planReply`, counting plan reads. */
function client(planReply: PlanReply): { lingara: Lingara; reads: () => number } {
  let reads = 0;
  const lingara = new Lingara({
    clientId: "lgr_cid_example",
    clientSecret: "lgr_cs_example",
    maxAttempts: 1,
    fetch: async (input, init) => {
      if (input.endsWith("/oauth/token")) {
        return json(200, { access_token: "lgr_at_example", token_type: "Bearer", expires_in: 3600, scope: "lesson_plans:read" });
      }
      assert.ok(input.endsWith(`/v1/lesson-plans/${PLAN_ID}`), input);
      reads += 1;
      return planReply(init.signal ?? undefined);
    },
  });
  return { lingara, reads: () => reads };
}

test("the phrasebook needs all three of its variables", () => {
  const all = { LINGARA_CLIENT_ID: "lgr_cid_example", LINGARA_CLIENT_SECRET: "lgr_cs_example", LUBA_PLAN_ID: PLAN_ID };
  assert.notEqual(phrasebookFromEnv(all), noPhrasebook);
  for (const missing of Object.keys(all)) {
    assert.equal(phrasebookFromEnv({ ...all, [missing]: "" }), noPhrasebook, missing);
  }
});

/** 4.10.26d AC7 */
test("the owner's read is memoised and bounded", async (t) => {
  await t.test("concurrent reads share one call, and the title is kept", async () => {
    const { lingara, reads } = client(async () => json(200, PLAN));
    const phrasebook = ownerPhrasebook(lingara, PLAN_ID);
    assert.deepEqual(await Promise.all([phrasebook(), phrasebook()]), [PLAN.title, PLAN.title]);
    assert.equal(await phrasebook(), PLAN.title);
    assert.equal(reads(), 1);
  });

  await t.test("a failure yields no title, and the next call reads again", async () => {
    const { lingara, reads } = client(async () => json(404, { error: "not_found", message: "no such plan" }));
    const phrasebook = ownerPhrasebook(lingara, PLAN_ID);
    assert.equal(await phrasebook(), undefined);
    assert.equal(await phrasebook(), undefined);
    assert.equal(reads(), 2);
  });

  await t.test("a hung call is abandoned at the 1 s timeout", async () => {
    const hang: PlanReply = (signal) =>
      new Promise((_, reject) => signal?.addEventListener("abort", () => reject(signal.reason), { once: true }));
    const { lingara } = client(hang);
    // AbortSignal.timeout's timer does not hold the event loop open, and here nothing else would.
    const keepAlive = setTimeout(() => undefined, 5000);
    const started = performance.now();
    assert.equal(await ownerPhrasebook(lingara, PLAN_ID)(), undefined);
    const elapsed = performance.now() - started;
    clearTimeout(keepAlive);
    assert.ok(elapsed >= 900 && elapsed < 2500, `abandoned after ${Math.round(elapsed)} ms`);
  });
});
