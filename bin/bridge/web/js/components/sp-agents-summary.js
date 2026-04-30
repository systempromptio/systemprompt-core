import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";

function summaryView(list) {
  if (list.length === 0) {
    return { dot: "sp-dot--unknown", label: "no agents registered" };
  }
  const installed = list.filter((h) => h.snapshot?.profile_state?.kind === "installed").length;
  const running = list.filter((h) => h.snapshot?.host_running).length;
  const dot = installed === list.length ? "sp-dot--ok" : installed > 0 ? "sp-dot--warn" : "sp-dot--err";
  return { dot, label: `${installed} of ${list.length} agents configured · ${running} running` };
}

export class SpAgentsSummary extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
  }

  onConnect() {
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch(() => {});
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
    const view = summaryView(list);
    return `
      <table class="sp-status__board">
        <tbody>
          <tr>
            <th data-l10n-id="status-agents-connected">Connected</th>
            <td>
              <div class="sp-status__row">
                <span class="sp-dot ${view.dot}" aria-hidden="true"></span>
                <span>${escapeHtml(view.label)}</span>
              </div>
            </td>
            <td class="sp-status__actions">
              <button class="sp-btn-ghost" data-jump-tab="agents" type="button" data-l10n-id="status-open-agents">Open agents</button>
            </td>
          </tr>
        </tbody>
      </table>
    `;
  }
}

reactive(SpAgentsSummary.prototype, ["snapshot"]);
customElements.define("sp-agents-summary", SpAgentsSummary);
