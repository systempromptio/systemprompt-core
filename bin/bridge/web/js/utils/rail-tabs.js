export const TAB_KEYS = { "1": "marketplace", "2": "agents", "3": "status", "4": "settings" };

export const TAB_GLYPHS = {
  marketplace: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2.5 21 7v10l-9 4.5L3 17V7l9-4.5z"/><path d="M3 7l9 4.5L21 7"/><path d="M12 11.5V21.5"/></svg>`,
  agents: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/></svg>`,
  status: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="5"/><circle cx="12" cy="12" r="1.5" fill="currentColor"/></svg>`,
  settings: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.04 1.56V21a2 2 0 0 1-4 0v-.09a1.7 1.7 0 0 0-1.04-1.56 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.56-1.04H3a2 2 0 0 1 0-4h.09A1.7 1.7 0 0 0 4.6 9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1.04-1.56V3a2 2 0 0 1 4 0v.09A1.7 1.7 0 0 0 15 4.6a1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9a1.7 1.7 0 0 0 1.56 1.04H21a2 2 0 0 1 0 4h-.09A1.7 1.7 0 0 0 19.4 15z"/></svg>`,
};

export const TAB_DEFS = [
  { name: "marketplace", label: "Marketplace", l10n: "nav-marketplace", shortcut: "⌘1", showCount: true, countFor: "marketplaceCount" },
  { name: "agents", label: "Agents", l10n: "nav-agents", shortcut: "⌘2", showCount: true, countFor: "agentCount" },
  { name: "status", label: "Status", l10n: "nav-status", shortcut: "⌘3", showCount: false },
  { name: "settings", label: "Settings", l10n: "nav-settings", shortcut: "⌘4", showCount: false },
];

export function readInitialTab() {
  try { return localStorage.getItem("cowork.tab") || "marketplace"; }
  catch (_) { return "marketplace"; }
}

export function persistTab(name) {
  try { localStorage.setItem("cowork.tab", name); } catch (_) {}
}

export function isTextInput(target) {
  if (!target) { return false; }
  return target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;
}
