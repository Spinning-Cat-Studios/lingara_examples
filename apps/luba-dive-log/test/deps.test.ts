// The kit takes @lingara/api as an ordinary dependency, so an exact pin here
// outside the kit's range would not fail `npm ci`: it would install a second,
// nested copy, and the kit would verify with a different library than the
// example calls. This test is the guard against that split.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

/** 4.10.26d AC6 */
test("one copy of the library", () => {
  const lock = JSON.parse(readFileSync(new URL("../package-lock.json", import.meta.url), "utf8")) as {
    packages: Record<string, unknown>;
  };
  const copies = Object.keys(lock.packages).filter((p) => p.endsWith("node_modules/@lingara/api"));
  assert.deepEqual(copies, ["node_modules/@lingara/api"]);
  assert.ok(!("node_modules/@lingara/apps/node_modules/@lingara/api" in lock.packages));
});
