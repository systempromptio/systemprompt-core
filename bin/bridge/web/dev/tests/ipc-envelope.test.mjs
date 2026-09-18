// The request envelope bridge.js posts is what src/wire/ipc.rs::IpcRequest
// deserialises, field for field. 0.53.0 added `mount` on the Rust side and
// nothing on this side sent it: every request was refused as a bad request,
// no reply came back, and every control that waited on one dimmed for the
// life of the window. The Rust tests wrote `mount` into their own JSON, so
// they passed. This test posts through the real bootstrap and the real
// `invoke`, and holds the keys to the committed ts-rs binding, which
// scripts/check-bridge-bindings.sh holds to the Rust struct.
import test, { beforeEach } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { WEB_ROOT } from "./resolve-assets.mjs";
import { freshPage, installBootstrap } from "./page.mjs";

const BINDING = readFileSync(join(WEB_ROOT, "../bindings/web/js/types/IpcRequest.ts"), "utf8");

function bindingKeys() {
  const body = BINDING.match(/IpcRequest = \{([^}]*)\}/)[1];
  return body.split(",").map((f) => f.split(":")[0].trim()).filter(Boolean).sort();
}

let posted;
let invoke;
let bridge;

beforeEach(async () => {
  posted = freshPage();
  ({ invoke, bridge } = await import("/assets/js/bridge.js"));
});

test("the bootstrap mints one positive integer mount per page", () => {
  const first = window.__bridge.mount;
  assert.ok(Number.isSafeInteger(first) && first > 0, `mount: ${first}`);
  installBootstrap();
  assert.equal(window.__bridge.mount, first, "a second load is a no-op");
  freshPage();
  assert.notEqual(window.__bridge.mount, first, "a new page is a new mount");
});

test("invoke posts exactly the IpcRequest fields, with the page's mount", () => {
  invoke("state.snapshot", { a: 1 });
  assert.equal(posted.length, 1);
  const req = posted[0];
  assert.deepEqual(Object.keys(req).sort(), bindingKeys());
  assert.equal(req.mount, window.__bridge.mount);
  assert.equal(req.cmd, "state.snapshot");
  assert.deepEqual(req.args, { a: 1 });
  assert.ok(Number.isInteger(req.id) && req.id > 0);
});

test("a reply settles the promise it was addressed to and nothing else", async () => {
  const p = invoke("settings.get");
  const { id, mount } = posted[0];
  window.__bridge.reply(mount + 1, id, { ok: true, value: "stale page" });
  assert.equal(window.__bridge.pending.size, 1, "a foreign mount's reply is dropped");
  window.__bridge.reply(mount, id, { ok: true, value: "mine" });
  assert.equal(await p, "mine");
  assert.equal(window.__bridge.pending.size, 0);
});

test("an error reply rejects with the BridgeError", async () => {
  const p = invoke("host.probe", { hostId: "x" });
  const { id, mount } = posted[0];
  const error = { scope: "internal", code: "invalid_format", message: "ipc: bad request" };
  window.__bridge.reply(mount, id, { ok: false, error });
  await assert.rejects(p, (e) => e.code === "invalid_format");
});

test("a read-style command that never answers rejects instead of pending forever", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const p = bridge.stateSnapshot();
  t.mock.timers.tick(30_000);
  await assert.rejects(p, (e) => e.code === "timeout");
});

test("a page without the bootstrap cannot invoke", () => {
  globalThis.window = { ipc: { postMessage() {} } };
  assert.throws(() => invoke("quit"), (e) => e.code === "internal");
});
