import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { fmtRelative, publishSectionState } from "/assets/js/utils/format.js";
import { runAction } from "/assets/js/utils/action.js";
import { toneBadge, toneSection } from "/assets/js/utils/verdict.js";

const RANK = { err: 0, warn: 1, unknown: 2, probing: 3, ok: 4 };

// One row per thing that can be wrong, worst first. `validate` has produced this
// structure all along; it was flattened to a text blob at the IPC boundary.
export function healthRows(snapshot) {
  const rows = [];
  const report = snapshot && snapshot.last_validation;
  for (const line of (report && report.lines) || []) {
    rows.push({ tone: line.tone, label: line.label, value: line.value });
  }
  for (const p of (snapshot && snapshot.provider_health) || []) {
    if (p.configured) { continue; }
    rows.push({
      tone: "warn",
      label: p.name,
      value: p.config_issue || (t("setup-health-provider-unconfigured") || "not configured"),
    });
  }
  const malformed = snapshot && snapshot.malformed_plugin_count;
  if (malformed) {
    rows.push({
      tone: "err",
      label: t("setup-health-malformed-plugins") || "malformed plugins",
      value: String(malformed),
    });
  }
  for (const f of ((snapshot && snapshot.last_sync_report && snapshot.last_sync_report.host_failures) || [])) {
    rows.push({ tone: "err", label: f.host_id, value: f.error });
  }
  for (const d of ((snapshot && snapshot.last_sync_report && snapshot.last_sync_report.diagnostics) || [])) {
    rows.push({ tone: "warn", label: t("setup-health-diagnostic") || "gateway diagnostic", value: d });
  }
  rows.sort((a, b) => (RANK[a.tone] ?? 5) - (RANK[b.tone] ?? 5));
  return rows;
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
  }

  onConnect() {
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
  }

  render() {
    const snap = this.snapshot || {};
    const all = healthRows(snap);
    const rows = this._failuresOnly ? all.filter((r) => r.tone === "err" || r.tone === "warn") : all;
    const at = snap.last_validation_at_unix;
    const checked = at
      ? (t("setup-health-checked", { ago: fmtRelative(at) }) || `checked ${fmtRelative(at)}`)
      : (t("setup-health-never") || "not checked yet");

    const body = rows.length
      ? rows.map((r) => `
        <tr data-key="${escapeHtml(`${r.tone}:${r.label}`)}">
          <th scope="row"><span class="sp-badge sp-badge--${toneBadge(r.tone)}">${escapeHtml(toneSection(r.tone))}</span> ${escapeHtml(r.label)}</th>
          <td>${escapeHtml(r.value)}</td>
        </tr>`).join("")
      : `<tr><td colspan="2" class="sp-health__empty">${escapeHtml(
        at ? (t("setup-health-all-passed") || "All checks passed.") : (t("setup-health-never") || "not checked yet")
      )}</td></tr>`;

    const filterLabel = this._failuresOnly
      ? (t("setup-health-all") || "All checks")
      : (t("setup-health-failures-only") || "Failures only");

    return `
      <div class="sp-health__controls">
        <span class="sp-health__checked">${escapeHtml(checked)}</span>
        <button type="button" class="sp-btn-ghost" data-action="toggle-failures" aria-pressed="${this._failuresOnly}">${escapeHtml(filterLabel)}</button>
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
