import { escapeHtml } from "/assets/js/utils/escape.js";
import { t } from "/assets/js/i18n.js";
import { statusOf, badgeSuffix, isSetUp, APP_NOT_INSTALLED, appInstallState } from "/assets/js/utils/verdict.js";
import { hostLogoMarkup } from "/assets/js/components/sp-agent-row.js";
import {
  agentDrawerRow, agentDrawerDetailText, agentDrawerSection, agentHostName, agentDrawerWorkingLabel,
} from "/assets/js/components/agent-drawer-parts.js";
import { renderAgentDrawerHealth, renderAgentDrawerModels } from "/assets/js/components/agent-drawer-health.js";
import { actionButtons, warnings } from "/assets/js/components/agent-drawer-actions.js";

function configRows(host, hs) {
  const rows = [];
  const text = agentDrawerDetailText;
  if (hs && hs.profile_source) { rows.push(agentDrawerRow(t("agent-row-config-location") || "Config location", text(hs.profile_source, { mono: true }))); }
  if (host.kind) { rows.push(agentDrawerRow(t("host-kind") || "Host kind", text(host.kind, { mono: true }))); }
  if (host.config_format) { rows.push(agentDrawerRow(t("host-config-format") || "Config format", text(host.config_format, { mono: true }))); }
  if (host.install_action_label) { rows.push(agentDrawerRow(t("host-install-label") || "Install action", text(host.install_action_label))); }
  const lastGen = host.last_generated_profile || null;
  if (!lastGen) { return rows; }
  rows.push(agentDrawerRow(t("host-last-generated") || "Last generated",
    text(lastGen.path, { mono: true }) + text(`${(lastGen.bytes / 1024).toFixed(1)} KB`, { muted: true })));
  if (lastGen.profile_uuid) { rows.push(agentDrawerRow(t("host-profile-uuid") || "Profile UUID", text(lastGen.profile_uuid, { mono: true }))); }
  if (lastGen.payload_uuid) { rows.push(agentDrawerRow(t("host-payload-uuid") || "Payload UUID", text(lastGen.payload_uuid, { mono: true }))); }
  return rows;
}

function configSection(host, hs) {
  const prefs = (hs && hs.profile_keys) || {};
  const prefsText = Object.keys(prefs).length === 0
    ? (t("host-prefs-empty") || "(none)")
    : Object.entries(prefs).map(([k, v]) => `${k} = ${v}`).join("\n");
  const prefsBlock = `<details class="sp-status__prefs"><summary>${escapeHtml(t("host-resolved-keys") || "Resolved profile keys")}</summary><pre class="sp-log">${escapeHtml(prefsText)}</pre></details>`;
  const rows = configRows(host, hs);
  const table = rows.length === 0 ? "" : `<table class="sp-status__board"><tbody>${rows.join("")}</tbody></table>`;
  return agentDrawerSection("agent-section-config", "Technical detail", `${table}${prefsBlock}`, true);
}

// Adding an agent was one click and removing one was impossible. The confirm
// is inline rather than a modal because the app has no modal, and it names the
// file that is about to lose its keys.
function removeSection(drawer, host, hs) {
  // `can_remove` is false when there is nothing local to remove — a sync-only
  // agent is set up (its manifest synced) but owns no file on this computer.
  if (!isSetUp(host) || host.can_remove === false) { return ""; }
  const path = (hs && hs.profile_source) || "";
  const name = agentHostName(host);
  const busy = drawer.busyId === host.id;
  const body = drawer.confirmRemove
    ? `
      <p>${escapeHtml(path
          ? t("agent-remove-confirm-path", { name, path }) || `Remove ${name} from systemprompt? This strips its systemprompt keys from ${path}.`
          : t("agent-remove-confirm", { name }) || `Remove ${name} from systemprompt?`)}</p>
      <div class="sp-drawer__filter-actions">
        <button class="sp-btn-danger" type="button" data-action="confirm-remove" ${busy ? "disabled" : ""}>${escapeHtml(
          busy ? agentDrawerWorkingLabel() : t("agent-remove-confirm-button") || "Remove it")}</button>
        <button class="sp-btn-ghost" type="button" data-action="cancel-remove">${escapeHtml(t("agent-remove-cancel") || "Keep it")}</button>
      </div>`
    : `
      <p class="sp-u-muted">${escapeHtml(t("agent-remove-explainer") || "Removing takes this agent's settings back out of its configuration file. It does not uninstall the app.")}</p>
      <div class="sp-drawer__filter-actions">
        <button class="sp-btn-ghost sp-btn-ghost--danger" type="button" data-action="remove-agent">${escapeHtml(t("agent-action-remove") || "Remove agent")}</button>
      </div>`;
  return `<section class="sp-drawer__section sp-drawer__section--danger">
    <h3 class="sp-drawer__section-title">${escapeHtml(t("agent-section-remove") || "Remove")}</h3>
    ${body}
  </section>`;
}

function goneView() {
  // An empty head renders a panel with no title and no close button, so
  // Escape and the scrim are the only ways out of it.
  const gone = t("agents-detail-gone") || "Agent not available";
  return {
    title: gone,
    head: `<div class="sp-drawer__headmeta"><h2 class="sp-drawer__title">${escapeHtml(gone)}</h2></div>`,
    content: `<p class="sp-u-muted">${escapeHtml(t("agents-detail-gone-body") || "This agent is no longer available on this computer.")}</p>`,
  };
}

export function renderAgentDrawerDetail(drawer) {
  const host = drawer._host();
  if (!host) { return goneView(); }
  const name = host.display_name || host.id;
  const status = statusOf(host);
  const hs = host.health || null;
  return {
    title: name,
    head: `
      ${hostLogoMarkup(host.icon || host.id || "", "sp-drawer__logo")}
      <div class="sp-drawer__headmeta">
        <h2 class="sp-drawer__title">${escapeHtml(name)}</h2>
        <span class="sp-drawer__status">
          <span class="sp-badge sp-badge--${escapeHtml(badgeSuffix(status.tone))}">${escapeHtml(status.label)}</span>
        </span>
      </div>
    `,
    content: `
      <p class="sp-drawer__lede">${escapeHtml(status.reason)}</p>
      ${actionButtons(drawer, host, status)}
      ${warnings(drawer, host, status)}
      ${renderAgentDrawerHealth(hs)}
      ${renderAgentDrawerModels(drawer, host)}
      ${configSection(host, hs)}
      ${removeSection(drawer, host, hs)}
    `,
  };
}
