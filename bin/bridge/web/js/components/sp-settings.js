import { html } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";
import { bridge } from "/assets/js/bridge.js";

export class SpSettings extends BridgeElement {
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
  }

  async _openFolder(e) {
    e.stopPropagation();
    try { await bridge.openConfigFolder(); } catch (err) { console.warn(err); }
  }
  async _validate(e) {
    e.stopPropagation();
    try { await bridge.validate(); } catch (err) { console.warn(err); }
  }
  _changeGateway(e) {
    e.stopPropagation();
    document.body.classList.add("is-setup-mode");
  }

  render() {
    const snap = this.snapshot || {};
    const gateway = snap.gateway_url || "—";
    const plugins = snap.plugins_dir || "—";
    const config = snap.config_file || "—";
    const muted = (v) => v === "—" ? "sp-u-muted" : "";
    return html`
      <div class="sp-kv__grid">
        <div class="sp-kv">
          <label data-l10n-id="settings-gateway-label">Gateway URL</label>
          <div class="sp-value sp-u-mono ${muted(gateway)}">${gateway}</div>
        </div>
        <div class="sp-kv">
          <label data-l10n-id="settings-plugins-label">Plugins directory</label>
          <div class="sp-value sp-u-mono ${muted(plugins)}">${plugins}</div>
        </div>
        <div class="sp-kv">
          <label data-l10n-id="settings-config-label">Config file</label>
          <div class="sp-value sp-u-mono ${muted(config)}">${config}</div>
        </div>
        <div class="sp-kv">
          <label data-l10n-id="settings-schedule-label">Sync schedule</label>
          <div class="sp-value sp-u-muted" data-l10n-id="settings-schedule-value">manual (trigger from Marketplace)</div>
        </div>
      </div>
      <div class="sp-row">
        <button class="sp-btn-ghost" type="button" data-l10n-id="settings-action-open-folder" @click=${(e) => this._openFolder(e)}>Open config folder</button>
        <button class="sp-btn-ghost" type="button" data-l10n-id="settings-action-validate" @click=${(e) => this._validate(e)}>Run validate</button>
        <button class="sp-btn-ghost" type="button" data-l10n-id="settings-action-change-gateway" @click=${(e) => this._changeGateway(e)}>Change gateway</button>
      </div>
      <p class="sp-settings__note sp-u-muted">
        <span data-l10n-id="settings-licensing-note-prefix">Demo build — for production licensing contact</span>
        <a href="mailto:ed@systemprompt.io">ed@systemprompt.io</a>.
      </p>
    `;
  }
}

customElements.define("sp-settings", SpSettings);
