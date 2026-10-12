// The OpenCode hooks plugin, loaded the way OpenCode 2.x loads it: a default
// export `{ id, setup(ctx) }`, driven here through a fake plugin context.
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

function fakeContext() {
  const hooks = {};
  const register = (domain) => async (name, callback) => {
    hooks[`${domain}.${name}`] = callback;
    return { dispose: async () => delete hooks[`${domain}.${name}`] };
  };
  const queue = [];
  let wake = null;
  let closed = false;
  const events = {
    subscribe: () => ({
      [Symbol.asyncIterator]: () => ({
        next: async () => {
          while (!queue.length && !closed) await new Promise((r) => (wake = r));
          return queue.length ? { done: false, value: queue.shift() } : { done: true };
        },
        return: async () => {
          closed = true;
          if (wake) wake();
          return { done: true };
        },
      }),
    }),
  };
  const emit = async (event) => {
    queue.push(event);
    if (wake) wake();
    await new Promise((r) => setTimeout(r, 10));
  };
  const ctx = { session: { hook: register("session") }, tool: { hook: register("tool") }, event: events };
  return { ctx, hooks, emit };
}

async function loaded() {
  const mod = await renderedPlugin();
  const fake = fakeContext();
  const cleanup = await mod.default.setup(fake.ctx);
  return { ...fake, cleanup };
}

test("the rendered plugin is a module OpenCode 2.x can load", async () => {
  const mod = await renderedPlugin();
  assert.equal(typeof mod.default.id, "string");
  assert.equal(typeof mod.default.setup, "function");
  const { hooks, cleanup } = await loaded();
  for (const name of ["session.http.request", "session.context", "tool.execute.after"]) {
    assert.equal(typeof hooks[name], "function", `${name} is registered`);
  }
  await cleanup();
  assert.deepEqual(Object.keys(hooks), [], "cleanup disposes every hook");
});

test("a skill run posts PostToolUse with the mapped skill reference", async () => {
  const calls = captureFetch();
  const { hooks, cleanup } = await loaded();
  await hooks["tool.execute.after"]({
    tool: "skill",
    input: { name: "deep-research" },
    sessionID: "ses_abc",
    messageID: "msg_1",
    id: "call_1",
    status: "completed",
    result: { title: "deep-research", output: "done", metadata: {} },
  });
  await cleanup();
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

test("an unmapped skill, an ordinary tool and a failed tool are all reported", async () => {
  const calls = captureFetch();
  const { hooks, cleanup } = await loaded();
  const after = hooks["tool.execute.after"];
  await after({ tool: "skill", input: { name: "local-only" }, sessionID: "ses_abc", id: "call_2", status: "completed", result: { output: "" } });
  await after({ tool: "bash", input: { command: "ls" }, sessionID: "ses_abc", id: "call_3", status: "completed", result: { output: "a\nb" } });
  await after({ tool: "bash", input: { command: "false" }, sessionID: "ses_abc", id: "call_4", status: "error", error: { message: "exit 1" } });
  await cleanup();
  assert.equal(calls[0].body.skill_ref, "opencode:local-only");
  assert.equal(calls[1].body.tool_name, "bash");
  assert.deepEqual(calls[1].body.tool_input, { command: "ls" });
  assert.equal(calls[1].body.tool_response.output, "a\nb");
  assert.equal(calls[1].body.skill_ref, undefined);
  assert.equal(calls[2].body.tool_response.output, "exit 1");
});

test("a prompt is reported once however many model steps it spans", async () => {
  const calls = captureFetch();
  const { hooks, cleanup } = await loaded();
  const messages = [{ id: "msg_u1", role: "user", content: [{ type: "text", text: "hello" }] }];
  await hooks["session.context"]({ sessionID: "ses_abc", messages });
  await hooks["session.context"]({ sessionID: "ses_abc", messages: [...messages, { role: "assistant", content: "hi" }] });
  await cleanup();
  assert.equal(calls.length, 1);
  assert.equal(calls[0].body.hook_event_name, "UserPromptSubmit");
  assert.equal(calls[0].body.prompt, "hello");
  assert.equal(calls[0].body.prompt_id, "msg_u1");
});

test("chat requests carry the same v5 session id the lifecycle events use", async () => {
  const calls = captureFetch();
  const { hooks, emit, cleanup } = await loaded();
  const input = { sessionID: "ses_abc", request: new Request("http://127.0.0.1/v1/chat/completions", { method: "POST", headers: { a: "b" } }) };
  await hooks["session.http.request"](input);
  const linked = input.request.headers.get("x-opencode-session");
  assert.match(linked, UUID_V5);
  assert.equal(input.request.headers.get("a"), "b", "existing headers are kept");
  await emit({ id: "evt_1", type: "session.created", data: { sessionID: "ses_abc", location: { directory: "/work" } } });
  await emit({ id: "evt_2", type: "session.text.delta", data: { sessionID: "ses_abc" } });
  await cleanup();
  assert.equal(calls.length, 1, "non-lifecycle events are not reported");
  assert.equal(calls[0].body.hook_event_name, "SessionStart");
  assert.equal(calls[0].body.session_id, linked);
  assert.equal(calls[0].body.cwd, "/work");
});
