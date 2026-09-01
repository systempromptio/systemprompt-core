import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { publishSectionState } from "/assets/js/utils/format.js";
import { t } from "/assets/js/i18n.js";
import { verdictOf, stateLabel, dotClass, sectionLabel, fleetHeadline } from "/assets/js/utils/verdict.js";

const EMPTY_FLEET = { total: 0, working: 0, ready: 0, running: 0, attention: 0, not_set_up: 0, down: 0, checking: 0, state: "unknown", headline: "none-enabled" };

export class SpAgentsSummary extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
  }

  onConnect() {
    // Why: the fleet summary is folded server-side over every host, so a
    // single-host `host.changed` cannot be merged in here without re-deriving
    // it — which is the divergence this component was rewritten to remove.
    // Host handlers emit `state.changed` alongside, and that carries the fold.
    this.useSnapshot((s) => { this.snapshot = s; });
  }

  _fleet() {
    return (this.snapshot && this.snapshot.agent_fleet && this.snapshot.agent_fleet.all) || EMPTY_FLEET;
  }

  render() {
    const list = (this.snapshot && this.snapshot.host_apps) || [];
    const enabled = list.filter((h) => h.enabled === true);
    const s = this._fleet();

    // `running` is a process-table scan: it says the app is open, not that it
    // is governed. It belongs in the footer, never in the headline.
    const footParts = [
      t("status-agents-foot-configured", { n: s.working + s.ready }) || `${s.working + s.ready} configured`,
      t("status-agents-foot-running", { n: s.running }) || `${s.running} app running`,
    ];
    const needing = s.attention + s.not_set_up + s.down;
    if (needing) {
      footParts.push(t("status-agents-foot-attention", { n: needing }) || `${needing} need attention`);
    }

    const perHost = enabled.map((h) => {
      const v = verdictOf(h);
      const name = h.display_name || h.id || "(unnamed)";
      const running = v.is_running ? "running" : "idle";
      return `<li>
        <span class="sp-dot ${dotClass(v.state)}" aria-hidden="true"></span>
        <span class="sp-kpi-card__host-name">${escapeHtml(name)}</span>
        <span class="sp-kpi-card__host-state">${escapeHtml(stateLabel(v.state))} · ${escapeHtml(running)}</span>
      </li>`;
    }).join("");

    return `
      <div class="sp-kpi-grid">
        <article class="sp-kpi-card" data-state="${escapeHtml(s.state)}">
          <div class="sp-kpi-card__head">
            <span data-l10n-id="status-agents-working">Working</span>
            <span class="sp-dot ${dotClass(s.state)}" aria-hidden="true"></span>
          </div>
          <div class="sp-kpi-card__value">
            <span>${s.working}</span>
            <span class="sp-kpi-card__unit">/ ${s.total}</span>
          </div>
          <div class="sp-kpi-card__label">${escapeHtml(fleetHeadline(s.headline))}</div>
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
    const s = this._fleet();
    publishSectionState(this, s.state, sectionLabel(s.state));
  }
}

reactive(SpAgentsSummary.prototype, ["snapshot"]);
customElements.define("sp-agents-summary", SpAgentsSummary);
