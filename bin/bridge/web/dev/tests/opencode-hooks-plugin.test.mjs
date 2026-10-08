// The OpenCode hooks plugin, loaded the way OpenCode loads it.
//
// This file exists because the template shipped for three weeks with a stray
// closing block that OpenCode rejected at load time ("Unexpected }"), so no
// OpenCode host reported skill use or linked its sessions. The Rust side only
// checks that the rendered text contains the expected substrings; importing
// the rendered module is the one check that proves it parses and that each
// hook posts the body the gateway's track route expects.
import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const TEMPLATE = resolve(
  HERE,
  "../../../src/integration/opencode/managed_resources/systemprompt-hooks.js",
);
const TRACK_URL = "http://127.0.0.1:48217/api/public/hooks/track?plugin_id=acme-commons";
const AUTHORIZATION = "Bearer test-hook-token";
const SKILL_MAP = { "deep-research": "acme-dev:deep-research" };
const NAMESPACE = "7c1f5b6e-3a2d-4e8f-9b0c-2d6a1e4f8c73";
const UUID_V5 = /^[0-9a-f]{8}-[0-9a-f]{4}-5[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

async function renderedPlugin() {
  const body = (await readFile(TEMPLATE, "utf8"))
    .replace("__TRACK_URL__", TRACK_URL)
    .replace("__AUTHORIZATION__", AUTHORIZATION)
    .replace("__SKILL_MAP__", JSON.stringify(SKILL_MAP))
    .replace("__SESSION_NAMESPACE__", NAMESPACE);
  const dir = await mkdtemp(join(tmpdir(), "sp-opencode-hooks-"));
  const file = join(dir, "systemprompt-hooks.mjs");
  await writeFile(file, body);
  return import(pathToFileURL(file).href);
}

function captureFetch() {
  const calls = [];
  globalThis.fetch = async (url, init) => {
    calls.push({ url, init, body: JSON.parse(init.body) });
    return { ok: true };
  };
  return calls;
}

test("the rendered plugin is a module OpenCode can load", async () => {
  const mod = await renderedPlugin();
  assert.equal(typeof mod.SystempromptHooks, "function");
  const hooks = await mod.SystempromptHooks({ directory: "/work" });
  for (const name of ["event", "chat.headers", "chat.params", "chat.message", "tool.execute.after"]) {
    assert.equal(typeof hooks[name], "function", `${name} is a hook`);
  }
});

test("a skill run posts PostToolUse with the mapped skill reference", async () => {
  const calls = captureFetch();
  const { SystempromptHooks } = await renderedPlugin();
  const hooks = await SystempromptHooks({ directory: "/work" });
  await hooks["tool.execute.after"](
    { tool: "skill", args: { name: "deep-research" }, sessionID: "ses_abc", callID: "call_1" },
    { title: "deep-research", output: "done", metadata: {} },
  );
  assert.equal(calls.length, 1);
  const { url, init, body } = calls[0];
  assert.equal(url, TRACK_URL);
  assert.equal(init.headers.authorization, AUTHORIZATION);
  assert.equal(init.headers["x-systemprompt-host"], "opencode");
  assert.equal(init.headers["x-ingestion-event-id"], "call_1");
  assert.equal(body.hook_event_name, "PostToolUse");
  assert.equal(body.tool_name, "skill");
  assert.deepEqual(body.tool_input, { name: "deep-research" });
  assert.equal(body.skill_ref, "acme-dev:deep-research");
  assert.equal(body.native_session_id, "ses_abc");
  assert.match(body.session_id, UUID_V5);
  assert.equal(body.tool_response.output, "done");
});

test("an unmapped skill and an ordinary tool are both reported", async () => {
  const calls = captureFetch();
  const { SystempromptHooks } = await renderedPlugin();
  const hooks = await SystempromptHooks({ directory: "/work" });
  await hooks["tool.execute.after"](
    { tool: "skill", args: { name: "local-only" }, sessionID: "ses_abc", callID: "call_2" },
    { output: "" },
  );
  await hooks["tool.execute.after"](
    { tool: "bash", args: { command: "ls" }, sessionID: "ses_abc", callID: "call_3" },
    { output: "a\nb" },
  );
  assert.equal(calls[0].body.skill_ref, "opencode:local-only");
  assert.equal(calls[1].body.tool_name, "bash");
  assert.deepEqual(calls[1].body.tool_input, { command: "ls" });
  assert.equal(calls[1].body.skill_ref, undefined);
});

test("chat headers carry the same v5 session id the lifecycle events use", async () => {
  const calls = captureFetch();
  const { SystempromptHooks } = await renderedPlugin();
  const hooks = await SystempromptHooks({ directory: "/work" });
  const output = { headers: {} };
  await hooks["chat.headers"]({ sessionID: "ses_abc" }, output);
  const linked = output.headers["x-opencode-session"];
  assert.match(linked, UUID_V5);
  await hooks.event({ event: { type: "session.created", properties: { sessionID: "ses_abc" } } });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].body.hook_event_name, "SessionStart");
  assert.equal(calls[0].body.session_id, linked);
  await hooks.event({ event: { type: "message.updated", properties: {} } });
  assert.equal(calls.length, 1, "non-lifecycle events are not reported");
});
