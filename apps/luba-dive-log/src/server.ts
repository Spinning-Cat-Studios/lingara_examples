// Luba's Dive Log, served: `npm start`.
//
//   LINGARA_APP_SECRET=lgr_whsec_… [LINGARA_CLIENT_ID=… LINGARA_CLIENT_SECRET=… LUBA_PLAN_ID=…] npm start
//
// LINGARA_APP_SECRET may hold two secrets, comma-separated, during a rotation.

import { createServer } from "node:http";

import { nodeHandler } from "@lingara/apps";

import { createApp, parseSecrets } from "./app.ts";
import { phrasebookFromEnv } from "./phrasebook.ts";

const secrets = parseSecrets(process.env.LINGARA_APP_SECRET);
if (secrets.length === 0) {
  console.error("✗ set LINGARA_APP_SECRET to the app's lgr_whsec_ secret (two, comma-separated, during a rotation)");
  process.exit(2);
}

const port = Number(process.env.LINGARA_APP_PORT ?? 8787);
const app = createApp(secrets, phrasebookFromEnv(process.env));
// Mount nothing in front of it that parses the body: the signature covers the raw bytes.
createServer(nodeHandler(app)).listen(port, () => console.log(`Luba's Dive Log listening on http://localhost:${port}`));
