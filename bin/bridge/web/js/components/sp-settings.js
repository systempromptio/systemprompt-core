import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";

export class SpSettings extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.registerAction("open-folder", async () => {
      try { await bridge.openConfigFolder(); } catch (err) { console.warn(err); }
    });
    this.registerAction("validate", async () => {
      try { await bridge.validate(); } catch (err) { console.warn(err); }
    });
    this.registerAction("change-gateway", () => {
      document.body.classList.add("is-setup-mode");
    });
  }

  onConnect() {
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch(() => {});
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
  }

  render() {
    const snap = this.snapshot || {};
    const gateway = snap.gateway_url || "—";
    const plugins = snap.plugins_dir || "—";
    const config = snap.config_file || "—";
    const muted = (v) => v === "—" ? "sp-u-muted" : "";
    return `
      <div class="sp-kv__grid">
        <div class="sp-kv">
          <label data-l10n-id="settings-gateway-label">Gateway URL</label>
          <div class="sp-value sp-u-mono ${muted(gateway)}">${escapeHtml(gateway)}</div>
        </div>
        <div class="sp-kv">
          <label data-l10n-id="settings-plugins-label">Plugins directory</label>
          <div class="sp-value sp-u-mono ${muted(plugins)}">${escapeHtml(plugins)}</div>
        </div>
        <div class="sp-kv">
          <label data-l10n-id="settings-config-label">Config file</label>
          <div class="sp-value sp-u-mono ${muted(config)}">${escapeHtml(config)}</div>
        </div>
        <div class="sp-kv">
          <label data-l10n-id="settings-schedule-label">Sync schedule</label>
          <div class="sp-value sp-u-muted" data-l10n-id="settings-schedule-value">manual (trigger from Marketplace)</div>
        </div>
      </div>
      <div class="sp-row">
        <button class="sp-btn-ghost" type="button" data-l10n-id="settings-action-open-folder" data-action="open-folder">Open config folder</button>
        <button class="sp-btn-ghost" type="button" data-l10n-id="settings-action-validate" data-action="validate">Run validate</button>
        <button class="sp-btn-ghost" type="button" data-l10n-id="settings-action-change-gateway" data-action="change-gateway">Change gateway</button>
      </div>
      <p class="sp-settings__note sp-u-muted">
        <span data-l10n-id="settings-licensing-note-prefix">Demo build — for production licensing contact</span>
        <a href="mailto:ed@systemprompt.io">ed@systemprompt.io</a>.
      </p>
    `;
  }
}

reactive(SpSettings.prototype, ["snapshot"]);
customElements.define("sp-settings", SpSettings);
