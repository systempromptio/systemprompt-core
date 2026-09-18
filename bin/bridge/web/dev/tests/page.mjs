// A page for the GUI modules to run in: a `window` carrying the real IPC bus
// from js/ipc-bootstrap.js — the bytes the webview injects — and an
// `ipc.postMessage` that records what the page sent.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { WEB_ROOT } from "./resolve-assets.mjs";

const BOOTSTRAP = readFileSync(join(WEB_ROOT, "js/ipc-bootstrap.js"), "utf8");

export function installBootstrap() {
  new Function(BOOTSTRAP)();
}

/** Replace `globalThis.window` with a fresh page; returns the posted envelopes. */
export function freshPage(overrides = {}) {
  const posted = [];
  globalThis.window = {
    crypto: globalThis.crypto,
    ipc: { postMessage: (raw) => posted.push(JSON.parse(raw)) },
    ...overrides,
  };
  installBootstrap();
  return posted;
}

/** Answer the most recent request on the page's own mount. */
export function answerLast(posted, payload) {
  const { id, mount } = posted.at(-1);
  window.__bridge.reply(mount, id, payload);
}
