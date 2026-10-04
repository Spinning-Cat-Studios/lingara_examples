// The fake game's webhook receiver. It verifies each delivery against the raw
// body, answers 204, and prints the event and which secret's signature
// matched; a refused delivery is 400 with the reason, and Lingara retries it
// on its schedule. The public HTTPS tunnel in front of it is the operator's.

import { createServer, type IncomingMessage, type Server } from "node:http";
import type { AddressInfo } from "node:net";

import { UnknownEvent, Webhook, WebhookVerificationError, type Event } from "@lingara/api";

import type { Log } from "./main.ts";

async function rawBody(req: IncomingMessage): Promise<Uint8Array> {
  const chunks: Buffer[] = [];
  for await (const chunk of req) chunks.push(chunk as Buffer);
  return new Uint8Array(Buffer.concat(chunks));
}

function headersOf(req: IncomingMessage): Record<string, string | undefined> {
  const out: Record<string, string | undefined> = {};
  for (const [name, value] of Object.entries(req.headers)) out[name] = Array.isArray(value) ? value.join(", ") : value;
  return out;
}

/**
 * The event, and the 1-based position of the secret that verified it. Each
 * secret is tried on its own so the operator can see a rotation work.
 */
async function verifyEach(webhooks: Webhook[], body: Uint8Array, headers: Record<string, string | undefined>): Promise<[Event, number]> {
  let last: unknown;
  for (const [i, webhook] of webhooks.entries()) {
    try {
      return [await webhook.verify(body, headers), i + 1];
    } catch (e) {
      if (!(e instanceof WebhookVerificationError) || e.reason !== "no_matching_signature") throw e;
      last = e;
    }
  }
  throw last;
}

/**
 * Listens on `port` (0 picks a free one) and resolves with the server, so a
 * caller can learn the port it bound and close it. `secrets` is one secret,
 * or two separated by a comma during a rotation.
 */
export async function receive(port: number, secrets: string, log: Log): Promise<Server> {
  const list = secrets.split(",").map((s) => s.trim()).filter((s) => s !== "");
  if (list.length === 0) throw new Error("set LINGARA_WEBHOOK_SECRET (two, comma-separated, during a rotation)");
  const webhooks = list.map((s) => new Webhook(s));
  const server = createServer((req, res) => {
    void (async () => {
      const body = await rawBody(req);
      try {
        const [event, which] = await verifyEach(webhooks, body, headersOf(req));
        const kind = event instanceof UnknownEvent ? " (a type this library does not know)" : "";
        log(`webhook: ${event.type} ${event.id}${kind}, signed with secret ${which}`);
        res.writeHead(204).end();
      } catch (e) {
        const reason = e instanceof WebhookVerificationError ? e.reason : "internal";
        log(`webhook: refused (${reason})`);
        res.writeHead(400, { "content-type": "text/plain" }).end(reason);
      }
    })();
  });
  await new Promise<void>((resolve) => server.listen(port, () => resolve()));
  log(`receiver: listening on ${(server.address() as AddressInfo).port}`);
  return server;
}
