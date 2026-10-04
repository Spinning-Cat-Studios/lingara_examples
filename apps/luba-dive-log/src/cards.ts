// The two cards, built only through the kit's builders: `build()` refuses
// anything the relay would clamp (CardLimitError), so a card that builds here
// is the card the learner sees.

import { card, reply, type AppReply, type Card, type CardBuilder } from "@lingara/apps";

import { phraseFor, planProgress, reviewLine, type Slices } from "./context.ts";
import type { Dive } from "./dives.ts";

export const DIVE_SITES_URL = "https://luba.example/dives";

function phrase(builder: CardBuilder, dive: Dive, slices: Slices): CardBuilder {
  const { word, gloss, lang, fallbackNote } = phraseFor(dive, slices.languages);
  builder.term({ word, gloss, lang });
  return fallbackNote === undefined ? builder : builder.text(fallbackNote);
}

/** `home.side`: the next dive, what is due, the plan's bar, the log, and a button. */
export function homeCard(dive: Dive, slices: Slices, logged: number): AppReply {
  const builder = phrase(card().heading("Next dive"), dive, slices);
  const review = reviewLine(slices.reviewDue);
  if (review !== undefined) builder.text(review);
  const progress = planProgress(slices.plan);
  if (progress !== undefined) builder.progress(progress.value, progress.label);
  const built: Card = builder.text(`Dives logged: ${logged}`).button("Logged", "logged").build();
  // The manifest declares `tutor_note`, so a reply may carry one; the tutor reads it, the learner does not.
  return reply(built).tutorNote(`This learner's next dive with Luba is to ${dive.name}.`);
}

/** `plans.empty_detail`: the dive's phrase, the owner's phrasebook title when known, and a link. */
export function emptyDetailCard(dive: Dive, slices: Slices, phrasebookTitle: string | undefined): AppReply {
  const builder = phrase(card(), dive, slices);
  if (phrasebookTitle !== undefined) builder.text(`Luba's phrasebook: ${phrasebookTitle}`);
  return builder.link("Luba's dive sites", DIVE_SITES_URL).build();
}
