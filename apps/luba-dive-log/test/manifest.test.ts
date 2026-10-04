import assert from "node:assert/strict";
import { test } from "node:test";

import { validateManifest } from "@lingara/apps";

import { buildManifest } from "../src/manifest.ts";

/** 4.10.26d AC5 */
test("the manifest lists only when asked", () => {
  for (const listed of [true, false]) {
    const built = buildManifest({ renderUrl: "https://dive-log.example.trycloudflare.com/", listed });
    validateManifest(built);
    assert.deepEqual(built.slots, ["home.side", "plans.empty_detail"]);
    assert.deepEqual(built.context, ["languages", "review_due", "plan_summary"]);
    assert.equal(built.tutor_note, true);
    assert.deepEqual(built.scopes, []);
    assert.equal(built.listed, listed);
  }
});
