// Written by astound-bridge on every sync; edits are overwritten.
//
// Reports skill use from OpenCode to the organisation's gateway through the
// bridge's loopback proxy, the same path Claude Code's hooks take. The bearer
// is a per-plugin hook token, never the loopback secret or the API key.
const TRACK_URL = "__TRACK_URL__";
const AUTHORIZATION = "__AUTHORIZATION__";
const SKILL_MAP = __SKILL_MAP__;
const HOST = "opencode";

const post = (body) => {
  const headers = {
    "content-type": "application/json",
    authorization: AUTHORIZATION,
    "x-systemprompt-host": HOST,
  };
  const eventId = body.tool_use_id || body.prompt_id;
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

export const SystempromptHooks = async ({ directory }) => ({
  "chat.message": async (input, output) => {
    const prompt = text(output && output.parts);
    if (!prompt.trim()) return;
    await post({
      hook_event_name: "UserPromptSubmit",
      session_id: input.sessionID,
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
      session_id: input.sessionID,
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
