// A headless fake game: it takes the feed's horizon cursor, tells Lingara the
// world changed with `generate: true`, and waits for that plan's
// `lesson_plan.ready` (or `.failed`) by the feed, or by the tail with
// `--tail`. One line per step. `--receive <port>` runs the webhook receiver
// instead (receiver.ts).
//
//   npm start -- --scene "A night market after rain" --source-lang en --target-lang zh --level 2 [--tail]
//   npm start -- --receive 8787

import { parseArgs } from "node:util";
import { pathToFileURL } from "node:url";

import { InboundEvent, Lingara, UnknownEvent, type Event } from "@lingara/api";

import { receive } from "./receiver.ts";

/** What the game says happened in its world (the fields of `world.context_changed`). */
export interface Scene {
  scene: string;
  sourceLang: string;
  targetLang: string;
  level: number;
}

export type Log = (line: string) => void;

/** Everything this game asks for, and nothing more: the client may hold other scopes. */
export const SCOPES = ["events:read", "events:write", "lesson_plans:read", "lesson_plans:write"];

/** How long the feed loop waits before asking again. */
const POLL_MS = 2000;

/** The plan's terminal event, or `undefined` when `event` is about another plan. */
function planOutcome(event: Event, planId: string): "ready" | "failed" | undefined {
  if (event instanceof UnknownEvent) return undefined;
  if (event.type === "lesson_plan.ready" && event.data.plan_id === planId) return "ready";
  if (event.type === "lesson_plan.failed" && event.data.plan_id === planId) return "failed";
  return undefined;
}

/**
 * The game side. The cursor is taken **before** `sendEvent`, so a
 * `lesson_plan.*` that fires between the send and the first read is never
 * missed. Returns the process exit code.
 */
export async function runGame(client: Lingara, scene: Scene, log: Log, tail = false): Promise<number> {
  const horizon = client.events({ start: "latest" });
  for await (const event of horizon) log(`feed: skipping ${event.type} ${event.id}`);
  const cursor = horizon.cursor;
  log(`feed: horizon cursor ${cursor ?? "(none)"}`);

  const accepted = await client.sendEvent(
    InboundEvent.worldContextChanged({
      scene: scene.scene,
      source_lang: scene.sourceLang,
      target_lang: scene.targetLang,
      level: scene.level,
      generate: true,
    }),
  );
  const reaction = accepted.reaction;
  log(`sent: ${accepted.type} ${accepted.id} reaction=${reaction?.status ?? "none"}`);
  // Only a reaction that started a plan which is still generating promises a
  // lesson_plan.* event; anything else has nothing to wait for.
  if (reaction?.status !== "started" || !reaction.plan_id) {
    log(`no plan started: ${reaction?.code ?? reaction?.status ?? "generate was off"}`);
    return 1;
  }
  if (reaction.plan_status !== "generating") {
    log(`plan ${reaction.plan_id} was served from the pool (${reaction.plan_status}), no event promised`);
    return 1;
  }
  const planId = reaction.plan_id;
  return tail ? waitByTail(client, cursor, planId, log) : waitByFeed(client, cursor, planId, log);
}

async function waitByFeed(client: Lingara, from: string | undefined, planId: string, log: Log): Promise<number> {
  let cursor = from;
  for (;;) {
    const feed = client.events(cursor === undefined ? {} : { cursor });
    for await (const event of feed) {
      const outcome = planOutcome(event, planId);
      if (outcome !== undefined) return report(event, outcome, planId, log);
      log(`feed: ${event.type} ${event.id} (not this plan)`);
    }
    cursor = feed.cursor;
    await new Promise((resolve) => setTimeout(resolve, POLL_MS));
  }
}

async function waitByTail(client: Lingara, cursor: string | undefined, planId: string, log: Log): Promise<number> {
  const controller = new AbortController();
  const tail = client.tailEvents(cursor === undefined ? {} : { cursor }, { signal: controller.signal });
  try {
    for await (const event of tail) {
      const outcome = planOutcome(event, planId);
      if (outcome !== undefined) return report(event, outcome, planId, log);
      log(`tail: ${event.type} ${event.id} (not this plan)`);
    }
  } finally {
    controller.abort();
  }
  return 1;
}

function report(event: Event, outcome: "ready" | "failed", planId: string, log: Log): number {
  log(`feed: ${event.type} ${event.id} plan=${planId}`);
  return outcome === "ready" ? 0 : 1;
}

/**
 * The client from `LINGARA_CLIENT_ID` and `LINGARA_CLIENT_SECRET`, asking for
 * {@link SCOPES} only. `LINGARA_BASE_URL` and `LINGARA_TOKEN_URL` override the
 * production endpoints.
 */
export function clientFromEnv(env: NodeJS.ProcessEnv = process.env): Lingara {
  const clientId = env.LINGARA_CLIENT_ID;
  const clientSecret = env.LINGARA_CLIENT_SECRET;
  if (!clientId || !clientSecret) throw new Error("set LINGARA_CLIENT_ID and LINGARA_CLIENT_SECRET");
  const baseUrl = env.LINGARA_BASE_URL;
  const tokenUrl = env.LINGARA_TOKEN_URL;
  return new Lingara({
    clientId,
    clientSecret,
    scopes: SCOPES,
    ...(baseUrl ? { baseUrl } : {}),
    ...(tokenUrl ? { tokenUrl } : {}),
  });
}

async function main(): Promise<number> {
  const { values } = parseArgs({
    options: {
      scene: { type: "string" },
      "source-lang": { type: "string", default: "en" },
      "target-lang": { type: "string", default: "zh" },
      level: { type: "string", default: "2" },
      tail: { type: "boolean", default: false },
      receive: { type: "string" },
    },
  });
  if (values.receive !== undefined) {
    await receive(Number(values.receive), process.env.LINGARA_WEBHOOK_SECRET ?? "", console.log);
    return 0;
  }
  if (!values.scene) throw new Error('--scene "<what the learner sees>" is required');
  const scene: Scene = {
    scene: values.scene,
    sourceLang: values["source-lang"],
    targetLang: values["target-lang"],
    level: Number(values.level),
  };
  return runGame(clientFromEnv(), scene, console.log, values.tail);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  main().then(
    (code) => (process.exitCode = code),
    (e: unknown) => {
      console.error(e instanceof Error ? e.message : e);
      process.exitCode = 1;
    },
  );
}
