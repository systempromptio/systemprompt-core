// Written by astound-bridge on every sync; edits are overwritten.
//
// Reports skill use from OpenCode to the organisation's gateway through the
// bridge's loopback proxy, the same path Claude Code's hooks take. The bearer
// is a per-plugin hook token, never the loopback secret or the API key.
const TRACK_URL = "__TRACK_URL__";
const AUTHORIZATION = "__AUTHORIZATION__";
const SKILL_MAP = __SKILL_MAP__;
const HOST = "opencode";
// Fixed namespace shared with the bridge proxy: both derive the same v5 UUID
// from a native `ses_…` id, so hook events and chat requests land on one
// gateway context.
const SESSION_NAMESPACE = "__SESSION_NAMESPACE__";

const hex = (bytes) => Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");

const namespaceBytes = () =>
  Uint8Array.from(SESSION_NAMESPACE.replace(/-/g, "").match(/.{2}/g), (pair) =>
    parseInt(pair, 16),
  );

const sessionCache = new Map();

// RFC 4122 v5: SHA-1 over namespace bytes + name, version and variant bits set.
const sessionUuid = async (native) => {
  const key = String(native || "");
  const cached = sessionCache.get(key);
  if (cached) return cached;
  const name = new TextEncoder().encode(key);
  const ns = namespaceBytes();
  const input = new Uint8Array(ns.length + name.length);
  input.set(ns, 0);
  input.set(name, ns.length);
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-1", input)).slice(0, 16);
  digest[6] = (digest[6] & 0x0f) | 0x50;
  digest[8] = (digest[8] & 0x3f) | 0x80;
  const h = hex(digest);
  const uuid = `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
  sessionCache.set(key, uuid);
  return uuid;
};

const post = (body) => {
  const headers = {
    "content-type": "application/json",
    authorization: AUTHORIZATION,
    "x-systemprompt-host": HOST,
  };
  const eventId = body.tool_use_id || body.prompt_id || body.event_id;
  if (eventId) headers["x-ingestion-event-id"] = String(eventId);
  return fetch(TRACK_URL, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(5000),
  }).catch(() => {});
};

const text = (content) => {
  if (typeof content === "string") return content;
  return (Array.isArray(content) ? content : [])
    .filter((part) => part && part.type === "text" && typeof part.text === "string")
    .map((part) => part.text)
    .join("\n");
};

// OpenCode's session events, translated to the canonical hook vocabulary
// Claude Code speaks natively, so the gateway sees one session lifecycle
// whichever host produced it. `session.idle` is the end of an assistant turn
// (Stop); `session.deleted` is the end of the session (SessionEnd).
const LIFECYCLE = {
  "session.created": "SessionStart",
  "session.idle": "Stop",
  "session.deleted": "SessionEnd",
};

const directories = new Map();
const lastPrompt = new Map();

const directoryOf = (sessionID) => directories.get(sessionID) || process.cwd();

const onEvent = async (event) => {
  const name = event && LIFECYCLE[event.type];
  const native = event && event.data && event.data.sessionID;
  if (!name || typeof native !== "string") return;
  const location = event.data.location || event.location;
  if (location && typeof location.directory === "string") {
    directories.set(native, location.directory);
  }
  await post({
    hook_event_name: name,
    session_id: await sessionUuid(native),
    native_session_id: native,
    cwd: directoryOf(native),
    native_host: HOST,
    event_id: event.id || `${native}:${event.type}:${Date.now()}`,
  });
  if (event.type === "session.deleted") {
    directories.delete(native);
    lastPrompt.delete(native);
  }
};

const watchEvents = (events) => {
  const stream = events.subscribe();
  const iterator = stream[Symbol.asyncIterator]();
  (async () => {
    for (;;) {
      const next = await iterator.next();
      if (next.done) return;
      await onEvent(next.value).catch(() => {});
    }
  })().catch(() => {});
  return () => (iterator.return ? iterator.return() : undefined);
};

// The chat request carries the session to the proxy, which moves it into
// `metadata.user_id` for the gateway.
const stampSession = async (input) => {
  const headers = new Headers(input.request.headers);
  headers.set("x-opencode-session", await sessionUuid(input.sessionID));
  input.request = new Request(input.request, { headers });
};

// The context hook runs before every model step, so a prompt is reported once:
// when the latest user message differs from the one last reported.
const reportPrompt = async (input) => {
  const messages = Array.isArray(input.messages) ? input.messages : [];
  const latest = [...messages].reverse().find((m) => m && m.role === "user");
  if (!latest) return;
  const prompt = text(latest.content);
  if (!prompt.trim()) return;
  const key = `${messages.lastIndexOf(latest)}:${prompt}`;
  if (lastPrompt.get(input.sessionID) === key) return;
  lastPrompt.set(input.sessionID, key);
  await post({
    hook_event_name: "UserPromptSubmit",
    session_id: await sessionUuid(input.sessionID),
    native_session_id: input.sessionID,
    cwd: directoryOf(input.sessionID),
    prompt,
    prompt_id: latest.id || crypto.randomUUID(),
    native_host: HOST,
  });
};

// Every tool completion is reported, not only skills: the gateway ingests
// the result as an artifact keyed by the call id, so OpenCode's tool use
// counts the same as Claude Code's. The output travels whole — the server
// bounds, scans and stores it; nothing is trimmed on the way out.
const reportTool = async (input) => {
  const tool = typeof input.tool === "string" ? input.tool : "";
  if (!tool) return;
  const args = input.input && typeof input.input === "object" ? input.input : {};
  const isSkill = tool === "skill";
  const name = isSkill && typeof args.name === "string" ? args.name : "";
  if (isSkill && !name) return;
  const result = input.status === "completed" ? input.result || {} : {};
  const output =
    input.status === "error"
      ? String((input.error && input.error.message) || input.error || "")
      : result.output !== undefined
        ? typeof result.output === "string"
          ? result.output
          : JSON.stringify(result.output)
        : "";
  const body = {
    hook_event_name: "PostToolUse",
    session_id: await sessionUuid(input.sessionID),
    native_session_id: input.sessionID,
    cwd: directoryOf(input.sessionID),
    tool_name: tool,
    tool_input: isSkill ? { name } : args,
    tool_use_id: input.id,
    tool_response: {
      title: result.title,
      output,
      metadata: result.metadata,
    },
    native_host: HOST,
  };
  if (isSkill) body.skill_ref = SKILL_MAP[name] || `opencode:${name}`;
  await post(body);
};

export default {
  id: "systemprompt.hooks",
  setup: async (ctx) => {
    const registrations = await Promise.all([
      ctx.session.hook("http.request", stampSession),
      ctx.session.hook("context", reportPrompt),
      ctx.tool.hook("execute.after", reportTool),
    ]);
    const stopEvents = watchEvents(ctx.event);
    return async () => {
      await stopEvents();
      await Promise.all(registrations.map((r) => r.dispose()));
    };
  },
};
