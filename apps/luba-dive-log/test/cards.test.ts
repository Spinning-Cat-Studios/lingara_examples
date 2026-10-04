// The cards, offline: every combination of the three context slices a
// learner may or may not have ticked, at the longest phrase, against the
// kit's own rules.

import assert from "node:assert/strict";
import { test } from "node:test";

import { Reply, validateReply, type AppReply, type ContextSlice, type Card } from "@lingara/apps";

import { emptyDetailCard, homeCard } from "../src/cards.ts";
import { slicesOf } from "../src/context.ts";
import { DIVES, PHRASE_LANGS, type Dive } from "../src/dives.ts";

const languages = (target_lang: string): ContextSlice => ({ kind: "languages", source_lang: "en", target_lang, level: 2 });
const reviewDue = (due: number, learned: number): ContextSlice => ({ kind: "review_due", due, learned });
const plan = (set_count: number, sets_completed: number, title?: string): ContextSlice => ({
  kind: "plan_summary",
  plan_id: "3f1c2a9e-5b7d-4e21-9a0c-6d8e4f2b1a37",
  target_lang: "fr",
  set_count,
  sets_completed,
  ...(title === undefined ? {} : { title }),
});

const length = (s: string): number => [...s].length;
/** The dive whose longest phrase (any language, or its name for the tutor note) is longest. */
const longest: Dive = [...DIVES].sort((a, b) => {
  const size = (d: Dive) => Math.max(length(d.en), length(d.name), ...PHRASE_LANGS.map((l) => length(d.phrases[l])));
  return size(b) - size(a);
})[0]!;

const wire = (r: AppReply) => (r instanceof Reply ? r.toJSON() : { card: r });
const cardOf = (r: AppReply): Card => wire(r).card;

function assertValid(r: AppReply): void {
  const validated = validateReply(wire(r));
  assert.ok(validated.ok, validated.ok ? "" : `refused: ${validated.reason}`);
}

/** Every subset of the three slices: eight. */
function* combinations(lang: ContextSlice, due: ContextSlice, summary: ContextSlice): Generator<ContextSlice[]> {
  for (let mask = 0; mask < 8; mask++) yield [lang, due, summary].filter((_, i) => mask & (1 << i));
}

/** 4.10.26d AC1 */
test("every slice combination builds a valid reply", () => {
  let cases = 0;
  for (const context of combinations(languages("fr"), reviewDue(12, 340), plan(10, 3, "Night dives"))) {
    const slices = slicesOf(context);
    assertValid(homeCard(longest, slices, 7));
    assertValid(emptyDetailCard(longest, slices, undefined));
    assertValid(emptyDetailCard(longest, slices, "Phrases for the deep"));
    cases += 2;
  }
  assert.equal(cases, 16);

  const edges: ContextSlice[][] = [
    [reviewDue(0, 0)],
    [plan(0, 0)],
    [plan(3, 9)],
    [plan(4, 1, "   ")],
    [plan(4, 1, "T".repeat(200))],
  ];
  for (const context of edges) assertValid(homeCard(longest, slicesOf(context), 0));

  const bars = (context: ContextSlice[]) =>
    cardOf(homeCard(longest, slicesOf(context), 0)).elements.filter((e) => e.type === "progress");
  assert.deepEqual(bars([plan(0, 0)]), []);
  assert.deepEqual(bars([plan(3, 9)]), [{ type: "progress", value: 1, label: "Your plan" }]);
  assert.equal((bars([plan(4, 1, "   ")])[0] as { label: string }).label, "Your plan");
  assert.equal(length((bars([plan(4, 1, "T".repeat(200))])[0] as { label: string }).label), 60);
});

/** 4.10.26d AC2 */
test("an unknown target language falls back to English", () => {
  const dive = DIVES[0]!;
  const elements = cardOf(homeCard(dive, slicesOf([languages("ko")]), 0)).elements;
  assert.deepEqual(elements[1], { type: "term", word: dive.en, lang: "en" });
  assert.equal(elements[2]?.type, "text");
  assert.match((elements[2] as { text: string }).text, /English/);

  const hans = cardOf(emptyDetailCard(dive, slicesOf([languages("zh-Hans")]), undefined)).elements;
  assert.deepEqual(hans[0], { type: "term", word: dive.phrases.zh, gloss: dive.en, lang: "zh" });
});
