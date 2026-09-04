// The rail's tab set, glyphs and keyboard shortcuts.
//
// This is the single source of truth for what panes exist and what they are
// called. `sp-crumb` used to keep a second, hardcoded, English-only copy that
// was missing an entry; it is gone.

/**
 * The platform's modifier prefix. Windows and Linux users were shown macOS's
 * Command glyph for shortcuts they have to press Ctrl for.
 */
export function modKey() {
  return document.body.classList.contains("is-platform-macos") ? "\u2318" : "Ctrl+";
}

export function shortcut(key) {
  return `${modKey()}${key}`;
}

export const TAB_KEYS = { "1": "account", "2": "marketplace", "3": "agents", "4": "settings", "5": "status", "6": "activity" };

export const TAB_GLYPHS = {
  account: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><circle cx="12" cy="10" r="3.2"/><path d="M5.5 19a7 7 0 0 1 13 0"/></svg>`,
  marketplace: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2.5 21 7v10l-9 4.5L3 17V7l9-4.5z"/><path d="M3 7l9 4.5L21 7"/><path d="M12 11.5V21.5"/></svg>`,
  agents: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/></svg>`,
  status: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="5"/><circle cx="12" cy="12" r="1.5" fill="currentColor"/></svg>`,
  activity: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12h4l2.5-6.5L14 18l2.5-6H21"/></svg>`,
  settings: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.04 1.56V21a2 2 0 0 1-4 0v-.09a1.7 1.7 0 0 0-1.04-1.56 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.56-1.04H3a2 2 0 0 1 0-4h.09A1.7 1.7 0 0 0 4.6 9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1.04-1.56V3a2 2 0 0 1 4 0v.09A1.7 1.7 0 0 0 15 4.6a1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9a1.7 1.7 0 0 0 1.56 1.04H21a2 2 0 0 1 0 4h-.09A1.7 1.7 0 0 0 19.4 15z"/></svg>`,
};

// Ordered by how often a person needs the pane, not by how much is on it.
// Account leads because it answers the question every other pane depends on --
// which account this computer is linked to -- and it is where a person lands on
// open; Status and Activity are diagnostics, not daily destinations, so they
// close the rail.
export const TAB_DEFS = [
  { name: "account", label: "Account", l10n: "nav-account", key: "1", showCount: false },
  { name: "marketplace", label: "Marketplace", l10n: "nav-marketplace", key: "2", showCount: true, countFor: "marketplaceCount" },
  { name: "agents", label: "Agents", l10n: "nav-agents", key: "3", showCount: true, countFor: "agentCount" },
  { name: "settings", label: "Settings", l10n: "nav-settings", key: "4", showCount: false },
  { name: "status", label: "Status", l10n: "nav-status", key: "5", showCount: false },
  { name: "activity", label: "Activity", l10n: "nav-activity", key: "6", showCount: false },
];

// Why: the bridge is a window a person opens to check on their deployment, not
// a document they resume editing, so every open starts on the same pane rather
// than wherever they happened to stop last time.
export const DEFAULT_TAB = "account";

export function readInitialTab() {
  return DEFAULT_TAB;
}

export function isTextInput(target) {
  if (!target) { return false; }
  return target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;
}
