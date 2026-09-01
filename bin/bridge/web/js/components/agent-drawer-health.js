import { escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";
import { toneDot } from "/assets/js/utils/verdict.js";
import {
  AGENT_WIRE_SURFACES, AGENT_SURFACE_L10N, AGENT_SURFACE_LABEL,
  agentDrawerRow, agentDrawerDetailText, agentDrawerSection,
} from "/assets/js/components/agent-drawer-parts.js";

function dotRow(dot, text) {
  return `<div class="sp-status__row"><span class="sp-dot ${dot}" aria-hidden="true"></span><span>${escapeHtml(text)}</span></div>`;
}

function processRow(hs) {
  const running = !!hs.host_running;
  const processes = Array.isArray(hs.host_processes) ? hs.host_processes : [];
  const runningText = running ? (t("host-process-running") || "running") : (t("host-process-not-running") || "not running");
  return dotRow(running ? "sp-dot--ok" : "sp-dot--warn", runningText)
    + (processes.length ? agentDrawerDetailText(processes.join(", "), { mono: true }) : "");
}

export function renderAgentDrawerHealth(hs) {
  if (!hs) {
    return agentDrawerSection("agent-section-health", "Health",
      `<p class="sp-u-muted">${escapeHtml(t("agent-state-checking") || "Checking…")}</p>`);
  }
  const missing = hs.missing_required || [];
  const profile = hs.profile || { tone: "unknown", code: "absent" };
  const app = hs.app || { tone: "unknown", code: "unknown" };
  const rows = [
    agentDrawerRow(t("agent-row-profile") || "Configuration profile",
      dotRow(toneDot(profile.tone), t(`host-profile-${profile.code}`, { missing: missing.join(", ") }) || "")),
    agentDrawerRow(t("agent-row-app") || "Application", dotRow(toneDot(app.tone), t(`host-app-${app.code}`) || "")),
    agentDrawerRow(t("agent-row-process") || "Process", processRow(hs)),
  ];
  if (missing.length) {
    rows.push(agentDrawerRow(t("host-missing-keys") || "Missing required keys",
      agentDrawerDetailText(missing.join(", "), { mono: true })));
  }
  return agentDrawerSection("agent-section-health", "Health",
    `<table class="sp-status__board"><tbody>${rows.join("")}</tbody></table>`);
}

function modelFilterBody(draft, effective, allModels, overridden) {
  const checks = AGENT_WIRE_SURFACES.map((p) =>
    `<label class="sp-drawer__proto"><input type="checkbox" data-change="proto" data-proto="${escapeHtml(p)}" ${effective.includes(p) ? "checked" : ""}> <span class="sp-drawer__proto-name">${escapeHtml(t(AGENT_SURFACE_L10N[p]) || AGENT_SURFACE_LABEL[p])}</span> <span class="sp-u-mono sp-u-muted">${escapeHtml(p)}</span></label>`
  ).join("");
  return `
    <label class="sp-drawer__proto"><input type="checkbox" data-change="model-all" data-model-all ${allModels ? "checked" : ""}> <span>${escapeHtml(t("host-model-filter-all") || "All models")}</span></label>
    <div class="sp-drawer__protos">${checks}</div>
    ${agentDrawerDetailText(overridden ? (t("host-model-filter-custom") || "custom override") : (t("host-model-filter-default") || "host default"), { muted: true })}
    ${agentDrawerDetailText(t("agent-model-filter-hint") || "Saved to your systemprompt account — you must be signed in.", { muted: true })}
    ${draft ? `<div class="sp-drawer__dirty">${escapeHtml(t("host-model-filter-unsaved") || "Unsaved changes.")}</div>` : ""}
    <div class="sp-drawer__filter-actions">
      <button class="sp-btn-primary" type="button" data-action="saveModelFilter" ${draft ? "" : "disabled"}>${escapeHtml(t("host-model-filter-save") || "Save filter")}</button>
      <button class="sp-btn-ghost" type="button" data-action="resetModelFilter">${escapeHtml(t("host-model-filter-reset") || "Reset to default")}</button>
    </div>`;
}

export function renderAgentDrawerModels(drawer, host) {
  // "Not checked yet" is a third state, and rendering nothing for it made it
  // indistinguishable from an agent with no models at all.
  if (!host.models_checked) {
    return agentDrawerSection("agent-section-models", "Models", `
      <p class="sp-u-muted">${escapeHtml(t("agent-models-unchecked") || "This agent's models have not been checked on this computer yet.")}</p>
      <div class="sp-drawer__filter-actions">
        <button class="sp-btn-ghost" type="button" data-action="act" data-kind="verify">${escapeHtml(t("agent-action-verify") || "Verify")}</button>
      </div>
    `);
  }
  const compatible = Array.isArray(host.compatible_models) ? host.compatible_models : [];
  const saved = Array.isArray(host.model_protocols) ? host.model_protocols : [];
  const draft = drawer.filterDraft;
  const modelsBody = compatible.length
    ? `<details class="sp-status__prefs"><summary>${escapeHtml(t("agent-models-count", { count: compatible.length }) || `${compatible.length} models available`)}</summary><div class="sp-status__detail sp-u-mono">${escapeHtml(compatible.join(", "))}</div></details>`
    : `<div class="sp-status__detail sp-u-muted">${escapeHtml(t("host-no-compatible-models") || "none available")}</div>`;
  const rows = [
    agentDrawerRow(t("host-compatible-models") || "Compatible models", modelsBody),
    agentDrawerRow(t("host-model-filter") || "Model filter",
      modelFilterBody(draft, draft ? draft.protocols : saved, draft ? draft.all : saved.length === 0, !!host.model_protocols_overridden)),
  ];
  return agentDrawerSection("agent-section-models", "Models",
    `<table class="sp-status__board"><tbody>${rows.join("")}</tbody></table>`);
}
