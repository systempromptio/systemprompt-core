// One table for every marketplace kind. The rail and the detail pane each
// carried their own copy and they had drifted: the detail's had no `artifacts`
// row at all, so an artifact's chip showed the raw kind id.
const STROKE = `viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"`;

export const MKT_KIND = {
  plugins: {
    label: "Plugins",
    singular: "Plugin",
    glyph: `<svg ${STROKE}><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M18 13v4a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4V7a4 4 0 0 1 4-4h6"/><path d="M9 13h6"/><path d="M9 17h4"/></svg>`,
  },
  skills: {
    label: "Skills",
    singular: "Skill",
    glyph: `<svg ${STROKE}><path d="M12 2v4"/><path d="M12 18v4"/><path d="M4.93 4.93l2.83 2.83"/><path d="M16.24 16.24l2.83 2.83"/><path d="M2 12h4"/><path d="M18 12h4"/><path d="M4.93 19.07l2.83-2.83"/><path d="M16.24 7.76l2.83-2.83"/></svg>`,
  },
  hooks: {
    label: "Hooks",
    singular: "Hook",
    glyph: `<svg ${STROKE}><path d="M12 4v8"/><path d="M12 12a4 4 0 1 0 4 4"/></svg>`,
  },
  mcp: {
    label: "MCP servers",
    singular: "MCP server",
    glyph: `<svg ${STROKE}><rect x="3" y="4" width="18" height="6" rx="2"/><rect x="3" y="14" width="18" height="6" rx="2"/><path d="M7 7h.01"/><path d="M7 17h.01"/></svg>`,
  },
  agents: {
    label: "Agents",
    singular: "Agent",
    glyph: `<svg ${STROKE}><circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/></svg>`,
  },
  artifacts: {
    label: "Artifacts",
    singular: "Artifact",
    glyph: `<svg ${STROKE}><rect x="4" y="3" width="16" height="18" rx="2"/><path d="M4 9h16"/><path d="M10 9v12"/></svg>`,
  },
};

export const MKT_KIND_L10N = {
  plugins: "marketplace-cat-plugins",
  skills: "marketplace-cat-skills",
  hooks: "marketplace-cat-hooks",
  mcp: "marketplace-cat-mcp",
  agents: "marketplace-cat-agents",
  artifacts: "marketplace-cat-artifacts",
};

export const MKT_CHILD_KIND_ORDER = ["skills", "agents", "mcp", "hooks"];

export function mktKindSingular(kind) {
  return (MKT_KIND[kind] || {}).singular || kind;
}
