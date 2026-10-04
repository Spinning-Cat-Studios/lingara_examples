// The app: the render function and the one action, with no I/O of its own.
// The phrasebook read is injected, so the tests pass a fake and nothing in
// here reaches the network.

import { lingaraApp, type AppActionRequest, type AppRenderRequest, type AppReply, type LingaraApp } from "@lingara/apps";

import { emptyDetailCard, homeCard } from "./cards.ts";
import { slicesOf } from "./context.ts";
import { nextDive } from "./dives.ts";
import type { Phrasebook } from "./phrasebook.ts";

/** LINGARA_APP_SECRET: one secret, or two comma-separated during a rotation. */
export function parseSecrets(value: string | undefined): string[] {
  return (value ?? "")
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s !== "");
}

export function createApp(secrets: string | readonly string[], phrasebook: Phrasebook): LingaraApp {
  // Dives logged, per `subject` (the learner as this app sees them), in memory.
  // A real app would keep this in its own database, keyed on the same `subject`.
  const logged = new Map<string, number>();

  async function render(request: AppRenderRequest | AppActionRequest): Promise<AppReply> {
    const count = logged.get(request.subject) ?? 0;
    const dive = nextDive(count);
    const slices = slicesOf(request.context);
    if (request.slot === "home.side") return homeCard(dive, slices, count);
    return emptyDetailCard(dive, slices, await phrasebook());
  }

  return lingaraApp({
    secret: secrets,
    render,
    actions: {
      // Each delivery records one dive. The relay does not retry, but a
      // duplicate delivery would count twice: a real app would key on the
      // request's `id` and record each one once.
      logged: (request) => {
        logged.set(request.subject, (logged.get(request.subject) ?? 0) + 1);
        return render({ ...request, slot: "home.side" });
      },
    },
  });
}
