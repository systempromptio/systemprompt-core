// The problem banners at the top of the Agents tab. Each names what is wrong
// in plain words and carries the same fix button the row offers, so the one
// thing to do next is never more than a glance away. Which agent leads, and
// what its fix is, both arrive from Rust on the verdict; this only renders.

import { escapeHtml } from "/assets/js/utils/escape.js";
import { t } from "/assets/js/i18n.js";
import { statusOf } from "/assets/js/utils/verdict.js";
import { runHostAction } from "/assets/js/utils/host-actions.js";
import { hostActionSuccessLine } from "/assets/js/utils/host-action-messages.js";
import { notifyOk, notifyErr } from "/assets/js/utils/notify.js";

// Literal keys, so the catalogue lint can see each one is used; a state with
// no headline of its own reads as the generic "needs attention".
const BANNER_KEY = {
  "needs-approval": "agents-banner-needs-approval",
  "approval-expired": "agents-banner-approval-expired",
  "needs-admin-approval": "agents-banner-needs-admin-approval",
  "not-set-up": "agents-banner-not-set-up",
  "needs-reapply": "agents-banner-needs-reapply",
  "restart-needed": "agents-banner-restart-needed",
  "down": "agents-banner-down",
};

function needsUser(host) {
  const s = statusOf(host);
  return (s.tone === "warn" || s.tone === "err") && s.action ? s : null;
}

/** The banner for the first listed agent that waits on the user, or "". */
export function agentBanner(hosts, busy) {
  const waiting = hosts.filter(needsUser);
  if (waiting.length === 0) { return ""; }
  const host = waiting[0];
  const status = statusOf(host);
  const name = host.display_name || host.id;
  const title = t(BANNER_KEY[status.state] || "agents-banner-attention", { name }) || name;
  const more = waiting.length > 1 ? t("agents-banner-more", { count: waiting.length - 1 }) || "" : "";
  return `<div class="sp-agents-banner" role="status" data-tone="${escapeHtml(status.tone)}">
    <div class="sp-agents-banner__text">
      <span class="sp-agents-banner__title">${escapeHtml(title)}</span>
      <span class="sp-agents-banner__detail">${escapeHtml(status.reason)}${more ? ` · ${escapeHtml(more)}` : ""}</span>
    </div>
    <button class="sp-btn-primary" type="button" data-action="banner-fix"
            data-host-id="${escapeHtml(host.id)}" data-kind="${escapeHtml(status.action.code)}"
            ${busy ? "disabled" : ""}>${escapeHtml(busy ? (t("agent-action-working") || "Working…") : status.action.label)}</button>
  </div>`;
}

/** Run the banner's fix for `host` and report it the way the row does. */
export async function runBannerFix(host, kind) {
  if (!host || !kind) { return; }
  const name = host.display_name || host.id;
  try {
    const line = hostActionSuccessLine(kind, name, await runHostAction(kind, host));
    if (line) { notifyOk(line); }
  } catch (e) {
    notifyErr(e, t(`agent-action-${kind}`) || kind);
  }
}
