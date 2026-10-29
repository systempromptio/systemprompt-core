import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { bridge } from "/assets/js/bridge.js";
import { runAction } from "/assets/js/utils/action.js";
import { repairHost } from "/assets/js/utils/host-actions.js";
import { t } from "/assets/js/i18n.js";

// One banner for every machine-policy change waiting on an administrator:
// the bridge coalesces them, so one approval applies all of them.
export class SpApprovalBanner extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.registerAction("approve-machine-policy", (trigger) => runAction(trigger, {
      run: async () => {
        await bridge.elevationApprove();
        await repairHost("claude-desktop");
      },
      success: t("toast-approval-applied") || "Claude Desktop's policy is up to date.",
      context: t("approval-banner-title") || "Claude Desktop needs one administrator approval",
    }));
  }

  onConnect() {
    this.useSnapshot((s) => { this.snapshot = s; });
  }

  render() {
    const snap = this.snapshot || {};
    if (snap.stale_bridge_policy) return renderStale();
    if (!snap.admin_approval_pending && !snap.admin_approval_declined) return "";
    const detail = snap.admin_approval_declined
      ? (t("approval-banner-declined") || "The last approval was declined; nothing changed.")
      : (t("approval-banner-pending") || "Pending changes to the machine policy are applied together.");
    return `
      <div class="sp-settings__banner" role="status">
        <strong data-l10n-id="approval-banner-title">Claude Desktop needs one administrator approval</strong>
        <span>${escapeHtml(detail)}</span>
        ${pendingChanges(snap)}
        <button class="sp-btn" type="button" data-action="approve-machine-policy"
          data-l10n-id="approval-banner-action">Approve</button>
      </div>`;
  }
}

// A previous bridge install's policy is replaced only on this explicit
// action; an unattended clear never treats it as this bridge's.
function renderStale() {
  return `
      <div class="sp-settings__banner" role="status">
        <strong data-l10n-id="approval-banner-stale-title">Claude Desktop runs on a previous bridge install's policy</strong>
        <span data-l10n-id="approval-banner-stale-detail">Disable and gateway switches cannot release it until it is replaced. Replacing it needs one administrator approval.</span>
        <button class="sp-btn" type="button" data-action="approve-machine-policy"
          data-l10n-id="approval-banner-stale-action">Replace</button>
      </div>`;
}

// The keys each host's installed policy differs on: what the approval writes.
function pendingChanges(snap) {
  const hosts = (Array.isArray(snap.host_apps) ? snap.host_apps : [])
    .map((h) => ({ name: h.display_name, keys: (h.health && h.health.stale_keys) || [] }))
    .filter((h) => h.keys.length > 0);
  if (!hosts.length) return "";
  const rows = hosts.map((h) => `<li>${escapeHtml(h.name)}: <span class="sp-u-mono">${
    h.keys.map(escapeHtml).join(", ")}</span></li>`).join("");
  return `<ul class="sp-settings__changes" aria-label="${escapeHtml(
    t("approval-banner-changes") || "Changes to apply")}">${rows}</ul>`;
}

reactive(SpApprovalBanner.prototype, ["snapshot"]);
customElements.define("sp-approval-banner", SpApprovalBanner);
