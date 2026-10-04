// The one API call: the owner's phrasebook plan, read as the owner.
//
// The app holds no learner token: install consent issues none, and this
// client's client-credentials token speaks for the app's owner. So it reads
// only a plan the owner generated in their own account (LUBA_PLAN_ID), and
// never the learner's `plan_summary.plan_id`, which it could not see.

import { Lingara } from "@lingara/api";

/** The phrasebook's title, or `undefined` when it is unknown for now. */
export type Phrasebook = () => Promise<string | undefined>;

export const PHRASEBOOK_TIMEOUT_MS = 1000;

/** No phrasebook configured: the line is simply absent. */
export const noPhrasebook: Phrasebook = () => Promise.resolve(undefined);

/**
 * Reads plan `planId` once. Concurrent first renders share the in-flight read;
 * the title is kept once it arrives; a failure or a timeout answers
 * `undefined` and the next render reads again. The 1 s timeout keeps a slow
 * API from spending the relay's 3 s render budget.
 */
export function ownerPhrasebook(client: Lingara, planId: string, timeoutMs = PHRASEBOOK_TIMEOUT_MS): Phrasebook {
  let known = false;
  let title: string | undefined;
  let inFlight: Promise<string | undefined> | undefined;
  return () => {
    if (known) return Promise.resolve(title);
    inFlight ??= client
      .getLessonPlan({ id: planId }, { signal: AbortSignal.timeout(timeoutMs) })
      .then(
        (plan) => {
          known = true;
          title = plan.title?.trim() || undefined;
          return title;
        },
        () => undefined,
      )
      .finally(() => {
        inFlight = undefined;
      });
    return inFlight;
  };
}

/** The phrasebook from LINGARA_CLIENT_ID, LINGARA_CLIENT_SECRET and LUBA_PLAN_ID, all three or none. */
export function phrasebookFromEnv(env: NodeJS.ProcessEnv): Phrasebook {
  const { LINGARA_CLIENT_ID: clientId, LINGARA_CLIENT_SECRET: clientSecret, LUBA_PLAN_ID: planId } = env;
  if (!clientId || !clientSecret || !planId) return noPhrasebook;
  // One attempt: a render cannot afford the library's retry ladder, and the next render is the retry.
  return ownerPhrasebook(new Lingara({ clientId, clientSecret, scopes: ["lesson_plans:read"], maxAttempts: 1 }), planId);
}
