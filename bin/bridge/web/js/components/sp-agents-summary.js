import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { publishSectionState } from "/assets/js/utils/format.js";

function summarise(list) {
  const enabled = list.filter((h) => h.enabled === true);
  if (enabled.length === 0) {
    return { state: "unknown", dot: "sp-dot--unknown", configured: 0, total: 0, running: 0, partial: 0, absent: 0, label: "no agents enabled" };
  }
  const installed = enabled.filter((h) => h.snapshot?.profile_state?.kind === "installed").length;
  const partial   = enabled.filter((h) => h.snapshot?.profile_state?.kind === "partial").length;
  const stale     = enabled.filter((h) => h.snapshot?.profile_state?.kind === "stale").length;
  const absent    = enabled.filter((h) => (h.snapshot?.profile_state?.kind || "absent") === "absent").length;
  const running   = enabled.filter((h) => h.snapshot?.host_running).length;
  const state = installed === enabled.length ? "ok" : installed > 0 || stale > 0 ? "warn" : "err";
  const dot   = state === "ok" ? "sp-dot--ok" : state === "warn" ? "sp-dot--warn" : "sp-dot--err";
  const label = state === "ok" ? "all configured" : state === "warn" ? "partial coverage" : "not configured";
  return { state, dot, configured: installed, total: enabled.length, running, partial, stale, absent, label };
}

function sectionLabel(state) {
  if (state === "ok") { return "healthy"; }
  if (state === "warn") { return "attention"; }
  if (state === "err") { return "down"; }
  if (state === "probing") { return "checking…"; }
  return "unknown";
}

export class SpAgentsSummary extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
  }

  onConnect() {
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
    this.bridgeSubscribe("host.changed", (host) => this._mergeHost(host));
  }

  _mergeHost(host) {
    if (!host || !host.id || !this.snapshot) { return; }
    const list = (this.snapshot.host_apps || []).slice();
    const idx = list.findIndex((h) => h.id === host.id);
    if (idx >= 0) { list[idx] = host; } else { list.push(host); }
    this.snapshot = { ...this.snapshot, host_apps: list };
  }

  render() {
    const list = (this.snapshot && this.snapshot.host_apps) || [];
    const enabled = list.filter((h) => h.enabled === true);
    const s = summarise(list);
    const footParts = [`${s.configured} configured`, `${s.running} running`];
    if (s.partial) { footParts.push(`${s.partial} partial`); }
    if (s.stale)   { footParts.push(`${s.stale} secret out of date`); }
    if (s.absent)  { footParts.push(`${s.absent} absent`); }

    const perHost = enabled.map((h) => {
      const kind = h.snapshot?.profile_state?.kind || "absent";
      const running = h.snapshot?.host_running ? "running" : "idle";
      const dotCls = kind === "installed" ? "sp-dot--ok" : kind === "partial" || kind === "stale" ? "sp-dot--warn" : "sp-dot--err";
      const name = h.id || h.name || "(unnamed)";
      return `<li>
        <span class="sp-dot ${dotCls}" aria-hidden="true"></span>
        <span class="sp-kpi-card__host-name">${escapeHtml(name)}</span>
        <span class="sp-kpi-card__host-state">${escapeHtml(kind)} · ${escapeHtml(running)}</span>
      </li>`;
    }).join("");

    return `
      <div class="sp-kpi-grid">
        <article class="sp-kpi-card" data-state="${s.state}">
          <div class="sp-kpi-card__head">
            <span data-l10n-id="status-agents-connected">Connected</span>
            <span class="sp-dot ${s.dot}" aria-hidden="true"></span>
          </div>
          <div class="sp-kpi-card__value">
            <span>${s.running}</span>
            <span class="sp-kpi-card__unit">/ ${s.total}</span>
          </div>
          <div class="sp-kpi-card__label">${escapeHtml(s.label)}</div>
          ${enabled.length > 0
            ? `<details>
                 <summary>Per-agent</summary>
                 <ul class="sp-kpi-card__hosts">${perHost}</ul>
               </details>`
            : ""}
          <div class="sp-kpi-card__foot">
            <span class="sp-kpi-card__foot-meta">${escapeHtml(footParts.join(" · "))}</span>
            <button class="sp-btn-ghost" data-jump-tab="agents" type="button" data-l10n-id="status-open-agents">Open agents</button>
          </div>
        </article>
      </div>
    `;
  }

  afterRender() {
    const list = (this.snapshot && this.snapshot.host_apps) || [];
    const s = summarise(list);
    publishSectionState(this, s.state, sectionLabel(s.state));
  }
}

reactive(SpAgentsSummary.prototype, ["snapshot"]);
customElements.define("sp-agents-summary", SpAgentsSummary);
