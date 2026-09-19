import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { fmtRelative, publishSectionState } from "/assets/js/utils/format.js";
import { runAction } from "/assets/js/utils/action.js";
import { repairHost } from "/assets/js/utils/host-actions.js";
import { healthRows, isFailure } from "/assets/js/utils/health-rows.js";
import { toneBadge, toneSection } from "/assets/js/utils/verdict.js";

function badgeWord(tone) {
  if (tone === "info") { return t("setup-health-info") || "info"; }
  return toneSection(tone);
}

export class SpSetupHealth extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this._failuresOnly = false;
    this.registerAction("run", (trigger) => runAction(trigger, {
      run: () => bridge.validate(),
      success: (v) => (v && v.report && v.report.any_failed)
        ? (t("setup-health-ran-failed") || "Check finished — some checks did not pass.")
        : (t("setup-health-ran-ok") || "All checks passed."),
      context: t("setup-health-run") || "Re-check",
    }));
    this.registerAction("toggle-failures", () => {
      this._failuresOnly = !this._failuresOnly;
      this.invalidate();
    });
    // A row's repair is the host's own: generate, install (UAC when the
    // target is the machine policy), re-probe. Same sequence as the Agents tab.
    this.registerAction("repair", (trigger) => {
      const hostId = trigger && trigger.dataset.host;
      if (!hostId) { return undefined; }
      return runAction(trigger, {
        run: () => repairHost(hostId),
        success: (path) => t("toast-agent-repaired", { name: hostId, path: path || "" })
          || `${hostId} re-configured — wrote ${path}.`,
        context: trigger.textContent.trim(),
      });
    });
  }

  onConnect() {
    this.useSnapshot((s) => { this.snapshot = s; });
  }

  render() {
    const snap = this.snapshot || {};
    const all = healthRows(snap);
    const rows = this._failuresOnly ? all.filter(isFailure) : all;
    const at = snap.last_validation_at_unix;
    const checked = at
      ? (t("setup-health-checked", { ago: fmtRelative(at) }) || `checked ${fmtRelative(at)}`)
      : (t("setup-health-never") || "not checked yet");

    const body = rows.length
      ? rows.map((r) => {
        const action = r.action
          ? `<button type="button" class="sp-btn-ghost" data-action="repair" data-host="${escapeHtml(r.action.hostId)}">${escapeHtml(r.action.label)}</button>`
          : "";
        return `
        <tr data-key="${escapeHtml(`${r.tone}:${r.label}:${r.value}`)}">
          <th scope="row"><span class="sp-badge sp-badge--${toneBadge(r.tone)}">${escapeHtml(badgeWord(r.tone))}</span> <span class="sp-health__label">${escapeHtml(r.label)}</span></th>
          <td class="sp-health__value">${escapeHtml(r.value)}</td>
          <td class="sp-status__actions">${action}</td>
        </tr>`;
      }).join("")
      : `<tr><td colspan="3" class="sp-health__empty">${escapeHtml(
        at ? (t("setup-health-all-passed") || "All checks passed.") : (t("setup-health-never") || "not checked yet")
      )}</td></tr>`;

    const filterLabel = this._failuresOnly
      ? (t("setup-health-all") || "All checks")
      : (t("setup-health-failures-only") || "Failures only");

    return `
      <div class="sp-health__controls">
        <span class="sp-health__checked">${escapeHtml(checked)}</span>
        <button type="button" class="sp-btn-ghost sp-health__filter" data-action="toggle-failures" aria-pressed="${this._failuresOnly}">${escapeHtml(filterLabel)}</button>
        <button type="button" class="sp-btn-ghost" data-action="run">${escapeHtml(t("setup-health-run") || "Re-check")}</button>
      </div>
      <table class="sp-status__board sp-health__table"><tbody>${body}</tbody></table>
    `;
  }

  afterRender() {
    // `health` is the bridge's fold of the same five sources these rows draw.
    const health = (this.snapshot && this.snapshot.health) || { tone: "unknown", code: "not-checked" };
    publishSectionState(this, health.tone, t(`setup-health-label-${health.code}`) || "");
  }
}

reactive(SpSetupHealth.prototype, ["snapshot"]);
customElements.define("sp-setup-health", SpSetupHealth);
