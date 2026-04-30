export const KIND_LABEL = {
  plugins: "Plugin",
  skills: "Skill",
  hooks: "Hook",
  mcp: "MCP server",
  agents: "Agent",
};

export const KIND_EMPTY_TITLE = {
  plugins: "No plugins yet",
  skills: "No skills yet",
  hooks: "No hooks yet",
  mcp: "No MCP servers yet",
  agents: "No agents yet",
};

export const MKT_KINDS = ["plugins", "skills", "hooks", "mcp", "agents"];

export const mktState = {
  data: null,
  kind: "plugins",
  selectedId: null,
  search: "",
  lastSyncSummary: null,
  inFlight: false,
};

export function filterItems() {
  const items = (mktState.data && mktState.data[mktState.kind]) || [];
  if (!mktState.search) {
    return items;
  } else {
    const q = mktState.search.toLowerCase();
    return items.filter((it) =>
      (it.name || "").toLowerCase().includes(q) ||
      (it.id || "").toLowerCase().includes(q) ||
      (it.summary || "").toLowerCase().includes(q));
  }
}
