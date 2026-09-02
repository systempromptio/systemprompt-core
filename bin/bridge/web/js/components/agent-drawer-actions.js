import { escapeHtml } from "/assets/js/utils/escape.js";
import { t } from "/assets/js/i18n.js";
import { isSetUp, APP_NOT_INSTALLED, appInstallState } from "/assets/js/utils/verdict.js";
import { agentDrawerWorkingLabel } from "/assets/js/components/agent-drawer-parts.js";
import { fmtDurationLong } from "/assets/js/utils/format.js";

export function ghostButton(kind, label, disabled = false) {
  return `<button class="sp-btn-ghost" type="button" data-action="act" data-kind="${kind}" ${disabled ? "disabled" : ""}>${escapeHtml(label)}</button>`;
}

export function actionButtons(drawer, host, status) {
  const busy = drawer.busyId === host.id;
  const primaryKind = status.action && status.action.code;
  const buttons = [];
  // The recommended action leads and is the only primary button; the rest stay
  // available but visually secondary, so there is never a question of which
  // one to press.
  if (status.action) {
    buttons.push(`<button class="${status.tone !== "ok" ? "sp-btn-primary" : "sp-btn-ghost"}" type="button" data-action="act"
      data-kind="${escapeHtml(primaryKind)}" ${busy ? "disabled" : ""}>${escapeHtml(
        busy ? agentDrawerWorkingLabel() : status.action.label
      )}${primaryKind === "download" ? " ↗" : ""}</button>`);
  }
  // Whether an action is available at all is decided in Rust, beside the state
  // it depends on (`HostEntryPayload.can_*`), and only rendered here. These four
  // buttons used to be unconditional, so a sync-only agent — governed from the
  // gateway, with nothing installed on this computer — offered all of them, and
  // every one reached a handler that could only answer "unknown host:
  // claude-code". Deciding it here as well (`surface === "sync-only"`) would be
  // the same fact derived twice, free to drift.
  if (primaryKind !== "open" && host.can_open !== false && appInstallState(host) !== APP_NOT_INSTALLED) {
    buttons.push(ghostButton("open", t("host-action-open") || "Open"));
  }
  if (primaryKind !== "repair" && primaryKind !== "add" && host.can_repair !== false) {
    buttons.push(ghostButton("repair", t("agent-action-repair") || "Repair", busy));
  }
  if (primaryKind !== "verify" && host.can_verify !== false) {
    buttons.push(ghostButton("verify", t("agent-action-verify") || "Verify"));
  }
  if (host.can_open_config !== false) {
    buttons.push(`<button class="sp-btn-ghost" type="button" data-action="open-config">${escapeHtml(t("agent-action-open-config") || "Show config file")}</button>`);
  }
  return `<div class="sp-drawer__actions">${buttons.join("")}</div>`;
}

export function warnings(drawer, host, status) {
  const out = [];
  const snap = drawer.snapshot || {};
  if (status.action && status.action.code === "repair") {
    out.push(t("agent-repair-explainer")
      || "Repair rewrites this agent's configuration profile and re-applies it. Restart the agent afterwards.");
  }
  if (snap.cached_token && snap.cached_token.ttl_seconds < 600 && isSetUp(host)) {
    const ttl = fmtDurationLong(snap.cached_token.ttl_seconds);
    out.push(t("host-jwt-warn", { ttl }) || `This agent's session expires in ${ttl}. Repair the agent to renew it.`);
  }
  return out.map((w) => `<div class="sp-claude__warn">${escapeHtml(w)}</div>`).join("");
}
