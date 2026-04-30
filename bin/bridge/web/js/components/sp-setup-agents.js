import { html } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";

export class SpSetupAgents extends BridgeElement {
  static properties = { snapshot: { state: true } };

  constructor() {
    super();
    this.snapshot = null;
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
    this.classList.add("sp-setup-agent-list");
    this.setAttribute("aria-live", "polite");
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

  async _install(host, e) {
    e.stopPropagation();
    try { await bridge.hostProfileGenerate(host.id); } catch (err) { console.warn("generate", err); }
  }

  render() {
    const hosts = (this.snapshot && this.snapshot.host_apps) || [];
    if (hosts.length === 0) {
      return html`<div class="sp-u-muted">${t("setup-agents-empty") || "No agents detected on this device."}</div>`;
    }
    return html`${hosts.map((host) => {
      const installed = host.snapshot?.profile_state?.kind === "installed";
      const suffix = host.kind === "cli_tool" ? " · CLI" : " · Desktop";
      return html`
        <div class="sp-setup-agent" data-state=${installed ? "installed" : "absent"}>
          <div class="sp-setup-agent__meta">
            <div class="sp-setup-agent__name">${host.display_name + suffix}</div>
            <div class="sp-setup-agent__desc">${host.description || ""}</div>
          </div>
          <button type="button" class=${installed ? "sp-btn-ghost" : "sp-btn-primary"} ?disabled=${installed} @click=${(e) => this._install(host, e)}>${installed ? "Installed ✓" : "Install profile"}</button>
        </div>
      `;
    })}`;
  }
}

customElements.define("sp-setup-agents", SpSetupAgents);
