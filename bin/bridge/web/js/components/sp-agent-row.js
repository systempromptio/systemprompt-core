import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";
import { notifyOk, notifyErr } from "/assets/js/utils/notify.js";
import { hostStatus, badgeSuffix } from "/assets/js/utils/host-status.js";
import { runHostAction } from "/assets/js/utils/host-actions.js";

/**
 * The Agents list, one line per agent: who it is, whether it is working, and
 * the single button that fixes it if it is not. Everything else — paths, UUIDs,
 * model filter, resolved keys — lives in the drawer this row opens.
 */

/**
 * Host logos are inert <template id="tpl-host-logo-*"> elements in index.html,
 * keyed by the host's `icon_id()` from Rust. Cloning beats inlining: a
 * white-label overlay can replace the template without touching JS.
 */
export function hostLogoMarkup(iconId, cls) {
  const tpl = document.getElementById(`tpl-host-logo-${iconId}`);
  const svg = tpl && tpl.content && tpl.content.firstElementChild;
  return svg
    ? svg.outerHTML.replace(/^<svg/, `<svg class="${cls}"`)
    : `<svg class="${cls}" aria-hidden="true" viewBox="0 0 24 24"></svg>`;
}

export class SpAgentRow extends SpElement {
  constructor() {
    super();
    this.host = null;
    this.snapshot = null;
    this.busy = false;
    this.registerAction("primary", async (trigger) => {
      const kind = trigger.dataset.kind;
      if (!kind || this.busy) { return; }
      this.busy = true;
      const name = (this.host && (this.host.display_name || this.host.id)) || "this agent";
      try {
        const path = await runHostAction(kind, this.host);
        if (kind === "repair" || kind === "add") {
          notifyOk(t("toast-agent-repaired", { name, path: path || "" })
            || `${name} re-configured — wrote ${path || ""}. Restart ${name} to pick it up.`);
        } else if (kind === "verify") {
          notifyOk(t("toast-agent-verified", { name }) || `${name} re-checked.`);
        }
      } catch (e) {
        notifyErr(e, t(`agent-action-${kind}`) || kind);
      } finally {
        this.busy = false;
      }
    });
  }

  render() {
    const host = this.host || {};
    const status = hostStatus(host, this.snapshot);
    const probing = !!host.probe_in_flight;
    const name = host.display_name || "—";
    const openLabel = t("agent-open-details", { name }) || `Open details for ${name}`;

    const actionMarkup = status.action
      ? `<button class="${status.state === "ok" ? "sp-btn-ghost" : "sp-btn-primary"} sp-agent-row__action" type="button"
                 data-action="primary" data-kind="${escapeHtml(status.action.kind)}"
                 ${this.busy ? "disabled" : ""}>${escapeHtml(
                   this.busy ? (t("agent-action-working") || "Working…") : status.action.label
                 )}${status.action.kind === "download" ? " ↗" : ""}</button>`
      : "";

    return `
      <div class="sp-agent-row" data-state="${escapeHtml(status.state)}">
        ${hostLogoMarkup(host.icon || host.id || "", "sp-agent-row__logo")}
        <button class="sp-agent-row__main" type="button"
                data-action="select-agent" data-host-id="${escapeHtml(host.id || "")}"
                aria-label="${escapeHtml(openLabel)}">
          <span class="sp-agent-row__name">${escapeHtml(name)}</span>
          <span class="sp-agent-row__reason">${escapeHtml(status.reason)}</span>
        </button>
        <span class="sp-agent-row__state">
          ${probing ? `<span class="sp-dot sp-dot--probing" aria-hidden="true"></span>` : ""}
          <span class="sp-badge sp-badge--${escapeHtml(badgeSuffix(status.state))}">${escapeHtml(status.label)}</span>
        </span>
        ${actionMarkup}
        <span class="sp-agent-row__chevron" aria-hidden="true">›</span>
      </div>
    `;
  }
}

reactive(SpAgentRow.prototype, ["host", "snapshot", "busy"]);
customElements.define("sp-agent-row", SpAgentRow);
