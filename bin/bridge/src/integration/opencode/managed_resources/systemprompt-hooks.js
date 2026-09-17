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

const withSessionHeader = async (headers, sessionID) => ({
  ...(headers || {}),
  "x-opencode-session": await sessionUuid(sessionID),
});

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

const text = (parts) =>
  (parts || [])
    .filter((part) => part && part.type === "text" && typeof part.text === "string")
    .map((part) => part.text)
    .join("\n");

// OpenCode's bus events, translated to the canonical hook vocabulary Claude
// Code speaks natively, so the gateway sees one session lifecycle whichever
// host produced it. `session.idle` is the end of an assistant turn (Stop);
// `session.deleted` is the end of the session (SessionEnd).
const LIFECYCLE = {
  "session.created": "SessionStart",
  "session.idle": "Stop",
  "session.deleted": "SessionEnd",
};

const lifecycleSession = (event) => {
  const props = (event && event.properties) || {};
  if (typeof props.sessionID === "string") return props.sessionID;
  if (props.info && typeof props.info.id === "string") return props.info.id;
  return "";
};

export const SystempromptHooks = async ({ directory }) => ({
  event: async ({ event }) => {
    const name = event && LIFECYCLE[event.type];
    const native = lifecycleSession(event);
    if (!name || !native) return;
    await post({
      hook_event_name: name,
      session_id: await sessionUuid(native),
      native_session_id: native,
      cwd: directory,
      native_host: HOST,
      event_id: `${native}:${event.type}:${Date.now()}`,
    });
  },
  // Whichever of these the pinned OpenCode supports carries the session to
  // the proxy, which moves it into `metadata.user_id` for the gateway.
  "chat.headers": async (input, output) => {
    output.headers = output.headers || {};
    output.headers["x-opencode-session"] = await sessionUuid(input.sessionID);
  },
  "chat.params": async (input, output) => {
    output.options = {
      ...(output.options || {}),
      headers: await withSessionHeader(output.options && output.options.headers, input.sessionID),
    };
  },
  "chat.message": async (input, output) => {
    const prompt = text(output && output.parts);
    if (!prompt.trim()) return;
    await post({
      hook_event_name: "UserPromptSubmit",
      session_id: await sessionUuid(input.sessionID),
      native_session_id: input.sessionID,
      cwd: directory,
      prompt,
      prompt_id: input.messageID || crypto.randomUUID(),
      native_host: HOST,
    });
  },
  "tool.execute.after": async (input, output) => {
    if (input.tool !== "skill") return;
    const args = input.args || {};
    const name = typeof args.name === "string" ? args.name : "";
    if (!name) return;
    await post({
      hook_event_name: "PostToolUse",
      session_id: await sessionUuid(input.sessionID),
      native_session_id: input.sessionID,
      cwd: directory,
      tool_name: "skill",
      tool_input: { name },
      tool_use_id: input.callID,
      tool_response: {
        title: output && output.title,
        output: String((output && output.output) || "").slice(0, 4096),
      },
      native_host: HOST,
      skill_ref: SKILL_MAP[name] || `opencode:${name}`,
    });
  },
});
