import { escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";
import { isSetUp, APP_NOT_INSTALLED, appInstallState } from "/assets/js/utils/verdict.js";
import { hostLogoMarkup } from "/assets/js/components/sp-agent-row.js";
import { agentDrawerWorkingLabel } from "/assets/js/components/agent-drawer-parts.js";

function addItemAction(drawer, host, added, appState) {
  if (added) {
    return `<span class="sp-badge sp-badge--ok">${escapeHtml(t("agents-add-added") || "Added")}</span>`;
  }
  if (appState === APP_NOT_INSTALLED && host.download_url) {
    return `<button class="sp-btn-ghost" type="button" data-action="act"
                    data-kind="download" data-host-id="${escapeHtml(host.id)}"
                    title="${escapeHtml(host.download_url)}">${escapeHtml(t("host-action-download") || "Download")} ↗</button>`;
  }
  const busy = drawer.busyId === host.id;
  return `<button class="sp-btn-primary" type="button" data-action="add-host"
                  data-host-id="${escapeHtml(host.id)}" ${busy ? "disabled" : ""}>${escapeHtml(
                    busy ? agentDrawerWorkingLabel() : (t("agent-action-add") || "Add")
                  )}</button>`;
}

export function renderAgentDrawerAddItem(drawer, host) {
  const added = isSetUp(host);
  const appState = appInstallState(host);
  const suffix = t(`agent-kind-${host.kind}`) || "";
  const note = !added && appState === APP_NOT_INSTALLED
    ? (t("agent-reason-app-missing") || "The app is not installed on this computer")
    : (host.description || "");

  return `
    <div class="sp-drawer__item" data-key="${escapeHtml(host.id)}" data-added="${added}">
      ${hostLogoMarkup(host.icon || host.id || "", "sp-drawer__item-logo")}
      <div class="sp-drawer__item-meta">
        <div class="sp-drawer__item-name">${escapeHtml(host.display_name || host.id)}</div>
        <div class="sp-drawer__item-desc">${escapeHtml(suffix)}${note ? ` · ${escapeHtml(note)}` : ""}</div>
      </div>
      ${addItemAction(drawer, host, added, appState)}
    </div>
  `;
}

export function renderAgentDrawerAdd(drawer) {
  const title = t("agents-add-heading") || "Add an agent";
  const hosts = drawer.hosts || [];
  const gateNote = drawer.gated
    ? ""
    : `<p class="sp-drawer__note sp-u-muted">${escapeHtml(
        t("agents-add-provisional")
          || "This list is provisional until this computer has synced with systemprompt."
      )}</p>`;

  const items = hosts.length === 0
    ? `<p class="sp-u-muted">${escapeHtml(t("agents-add-empty") || "No agents are available for this installation.")}</p>`
    : hosts.map((host) => renderAgentDrawerAddItem(drawer, host)).join("");

  return {
    title,
    head: `<h2 class="sp-drawer__title">${escapeHtml(title)}</h2>`,
    content: `
      <p class="sp-drawer__lede">${escapeHtml(
        t("agents-add-lede")
          || "Pick a coding agent to route through systemprompt. Adding one writes its configuration profile — you do not need to configure anything by hand."
      )}</p>
      ${gateNote}
      <div class="sp-drawer__list">${items}</div>
    `,
  };
}
