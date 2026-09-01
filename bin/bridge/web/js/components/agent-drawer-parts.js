import { escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";

// These are `ApiSurface` tags, which is what `host.model_protocols` carries and
// what `hostModelFilterSet` parses back. They are NOT `ProviderProtocol` tags:
// this list once held "openai-chat"/"openai-responses", which matched nothing
// coming from the bridge (the surface tag is plain "openai"), so the OpenAI
// boxes rendered unchecked while OpenAI was on, and ticking one saved a filter
// the bridge then dropped as unparseable.
//
// Only the advertised surfaces get a checkbox. `backend` is never advertised to
// a client, so there is nothing for a user to decide about it -- but it can
// still be in effect, and `captureAgentDrawerFilter` carries it through
// untouched rather than silently dropping it on save.
export const AGENT_WIRE_SURFACES = ["anthropic", "openai", "gemini"];

// The wire name is what the filter actually stores, so it stays on screen -- but
// as the secondary line. A checkbox whose whole accessible name is "openai"
// tells a user nothing about what they are turning off.
export const AGENT_SURFACE_L10N = {
  "anthropic": "proto-anthropic",
  "openai": "proto-openai",
  "gemini": "proto-gemini",
};

export const AGENT_SURFACE_LABEL = {
  "anthropic": "Claude models",
  "openai": "OpenAI models",
  "gemini": "Gemini models",
};

export function agentDrawerRow(label, body) {
  return `<tr><th>${escapeHtml(label)}</th><td>${body}</td></tr>`;
}

export function agentHostName(host) {
  return (host && (host.display_name || host.id)) || "this agent";
}

export function agentDrawerDetailText(value, { mono = false, muted = false } = {}) {
  const cls = ["sp-status__detail", mono ? "sp-u-mono" : "", muted ? "sp-u-muted" : ""]
    .filter(Boolean).join(" ");
  return `<div class="${cls}">${escapeHtml(value)}</div>`;
}

export function agentDrawerWorkingLabel() {
  return t("agent-action-working") || "Working…";
}

/** Sections are plain headings; the technical one is a collapsed disclosure. */
export function agentDrawerSection(l10nId, fallback, body, collapsible = false) {
  const heading = t(l10nId) || fallback;
  if (collapsible) {
    return `<details class="sp-drawer__section sp-drawer__section--advanced">
      <summary class="sp-drawer__section-title">${escapeHtml(heading)}</summary>
      ${body}
    </details>`;
  }
  return `<section class="sp-drawer__section">
    <h3 class="sp-drawer__section-title">${escapeHtml(heading)}</h3>
    ${body}
  </section>`;
}
