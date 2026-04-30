import { html } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";
import { bridge } from "/assets/js/bridge.js";

function presenceState(host) {
  const kind = host.snapshot?.profile_state?.kind;
  if (kind === "installed" && host.snapshot?.host_running) { return "ok"; }
  if (kind === "installed" || kind === "partial") { return "warn"; }
  if (kind === "absent") { return "err"; }
  return "unknown";
}

function presenceLabel(state) {
  if (state === "ok") { return "running"; }
  if (state === "warn") { return "needs attention"; }
  if (state === "err") { return "not installed"; }
  return "unknown";
}

function syncRailCount(count) {
  const el = document.getElementById("rail-count-agents");
  if (el) { el.textContent = String(count); }
}

export class SpAgentPresence extends BridgeElement {
  static properties = { snapshot: { state: true } };

  constructor() {
    super();
    this.snapshot = null;
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
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

  updated() {
    const list = (this.snapshot && this.snapshot.host_apps) || [];
    syncRailCount(list.length);
  }

  render() {
    const list = (this.snapshot && this.snapshot.host_apps) || [];
    return html`${list.map((host) => {
      const state = presenceState(host);
      return html`<span
        class="sp-agent__dot"
        data-action="agent-jump"
        data-agent=${host.id}
        data-state=${state}
        title="${host.display_name} · ${presenceLabel(state)}"
      ></span>`;
    })}`;
  }
}

customElements.define("sp-agent-presence", SpAgentPresence);
