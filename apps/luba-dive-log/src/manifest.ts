// The manifest: `npm run manifest` writes manifest.json, which you upload in
// the console against the app's OAuth client. It needs no secret.
//
//   LUBA_RENDER_URL=https://<your tunnel> [LUBA_LISTED=1] npm run manifest

import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { manifest, type AppManifest } from "@lingara/apps";

export interface ManifestOptions {
  readonly renderUrl: string;
  readonly listed: boolean;
}

/**
 * Asks the learner for three context slices, each of which they may leave
 * unticked, and for no API scopes: the app reads no learner data through the
 * API. Its own client holds `lesson_plans:read` for the owner's phrasebook,
 * which is the client's business, not the learner's.
 */
export function buildManifest(options: ManifestOptions): AppManifest {
  return manifest()
    .defaultLocale("en")
    .name("Luba's Dive Log")
    .description("Your next dive with Luba, the phrase to say on the way down, and the dives you've logged.")
    .renderUrl(options.renderUrl)
    .slots("home.side", "plans.empty_detail")
    .context("languages", "review_due", "plan_summary")
    .tutorNote()
    .listed(options.listed)
    .build();
}

function main(): void {
  const renderUrl = process.env.LUBA_RENDER_URL;
  if (!renderUrl) {
    console.error("✗ set LUBA_RENDER_URL to the app's public https URL (your tunnel)");
    process.exit(2);
  }
  const built = buildManifest({ renderUrl, listed: process.env.LUBA_LISTED === "1" });
  writeFileSync("manifest.json", `${JSON.stringify(built, null, 2)}\n`);
  console.log(`wrote manifest.json (listed: ${built.listed})`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
