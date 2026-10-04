// Each context slice → what the card shows. A slice is present only when the
// learner ticked it on the install consent card, so every one of them is
// optional here: each function answers `undefined` (or a fallback) when its
// slice is absent, and the card simply leaves that line out.

import { truncate, limits, type ContextSlice, type ContextSliceLanguages, type ContextSlicePlanSummary, type ContextSliceReviewDue } from "@lingara/apps";

import { phraseLang, type Dive } from "./dives.ts";

/** The three slices this app's manifest asks for, each possibly missing. */
export interface Slices {
  readonly languages?: ContextSliceLanguages;
  readonly reviewDue?: ContextSliceReviewDue;
  readonly plan?: ContextSlicePlanSummary;
}

export function slicesOf(context: readonly ContextSlice[]): Slices {
  const slices: { languages?: ContextSliceLanguages; reviewDue?: ContextSliceReviewDue; plan?: ContextSlicePlanSummary } = {};
  for (const slice of context) {
    if (slice.kind === "languages") slices.languages = slice;
    if (slice.kind === "review_due") slices.reviewDue = slice;
    if (slice.kind === "plan_summary") slices.plan = slice;
  }
  return slices;
}

/** The dive's phrase, in the learner's target language when the table has it. */
export interface Phrase {
  readonly word: string;
  readonly gloss?: string;
  readonly lang: string;
  /** Set when the learner's target language is outside the table: the card says so. */
  readonly fallbackNote?: string;
}

export function phraseFor(dive: Dive, languages: ContextSliceLanguages | undefined): Phrase {
  // No `languages` slice: the learner did not share it, so show the English phrase.
  if (languages === undefined) return { word: dive.en, lang: "en" };
  const lang = phraseLang(languages.target_lang);
  if (lang === undefined) {
    return { word: dive.en, lang: "en", fallbackNote: "Luba's phrasebook doesn't speak your target language yet, so here it is in English." };
  }
  return { word: dive.phrases[lang], gloss: dive.en, lang };
}

/** "N words due · M learned", or nothing without `review_due`. */
export function reviewLine(reviewDue: ContextSliceReviewDue | undefined): string | undefined {
  if (reviewDue === undefined) return undefined;
  return `${reviewDue.due} words due · ${reviewDue.learned} learned`;
}

/** The plan's progress bar, or nothing without `plan_summary` or with an empty plan. */
export function planProgress(plan: ContextSlicePlanSummary | undefined): { value: number; label: string } | undefined {
  if (plan === undefined || plan.set_count <= 0) return undefined;
  const value = Math.min(1, Math.max(0, plan.sets_completed / plan.set_count));
  const title = plan.title?.trim();
  const label = title ? truncate(title, limits.PROGRESS_LABEL_MAX_CHARS) : "Your plan";
  return { value, label };
}
