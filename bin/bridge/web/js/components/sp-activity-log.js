import { html } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";
import { bridge } from "/assets/js/bridge.js";
import { createLogVirtual } from "/assets/js/components/log-virtual.js";

function fmtCount(n) {
  const v = Number(n) || 0;
  if (v >= 1_000_000) { return `${(v / 1_000_000).toFixed(1)}M`; }
  if (v >= 1_000) { return `${(v / 1_000).toFixed(1)}k`; }
  return String(v);
}

function classifyLevel(line) {
  return /(fail|error|refused|denied|reject)/i.test(line) ? "error" :
    /(warn)/i.test(line) ? "warn" : "info";
}

export class SpActivityLog extends BridgeElement {
  static properties = { snapshot: { state: true } };

  constructor() {
    super();
    this.snapshot = null;
    this._virtual = null;
    this._pending = [];
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
    this.classList.add("sp-activity");
    this.setAttribute("aria-label", "Activity log");
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch(() => {});
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
    this.bridgeSubscribe("proxy.stats", () => {
      bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch(() => {});
    });
    this.bridgeSubscribe("log", (entry) => this._appendLog(entry && entry.line));
  }

  firstUpdated() {
    const root = this.querySelector(".sp-log-virtual");
    if (root) {
      try {
        this._virtual = createLogVirtual(root);
        this._virtual.append({ text: "Ready.", level: "info" });
        for (const line of this._pending) { this._appendLine(line); }
        this._pending = [];
      } catch (e) {
        console.error("log-virtual init failed", e);
      }
    }
  }

  _appendLog(line) {
    if (!line) { return; }
    if (!this._virtual) { this._pending.push(line); return; }
    this._appendLine(line);
  }

  _appendLine(line) {
    const ts = new Date().toLocaleTimeString();
    this._virtual.append({ text: `[${ts}] ${line}`, level: classifyLevel(line) });
  }

  async _onOpenLogFolder() {
    try { await bridge.openLogFolder(); } catch (e) { console.warn("open log folder", e); }
  }
  async _onExportBundle() {
    try { await bridge.diagnosticsExportBundle(); } catch (e) { console.warn("export bundle", e); }
  }

  render() {
    const stats = (this.snapshot && this.snapshot.proxy_stats) || {};
    return html`
      <header class="sp-activity__header">
        <span class="sp-activity__title" data-l10n-id="activity-title">Activity</span>
        <div class="sp-activity-lane" aria-label="Activity totals">
          <span class="sp-activity-lane__stat"><b>${fmtCount(stats.messages_total)}</b><span class="sp-activity-lane__label" data-l10n-id="activity-msgs">msgs</span></span>
          <span class="sp-activity-lane__stat"><b>${fmtCount(stats.tokens_in_total)}</b><span class="sp-activity-lane__label" data-l10n-id="activity-tin">in</span></span>
          <span class="sp-activity-lane__stat"><b>${fmtCount(stats.tokens_out_total)}</b><span class="sp-activity-lane__label" data-l10n-id="activity-tout">out</span></span>
        </div>
      </header>
      <div class="sp-log sp-log-virtual" role="log" aria-live="polite" aria-atomic="false" tabindex="0">
        <div class="sp-log-virtual__spacer-top" aria-hidden="true"></div>
        <ol class="sp-log-virtual__viewport"></ol>
        <div class="sp-log-virtual__spacer-bottom" aria-hidden="true"></div>
      </div>
      <section class="sp-activity__help" aria-label="Help &amp; support">
        <header class="sp-activity__help-title" data-l10n-id="activity-help-title">Help &amp; Support</header>
        <div class="sp-activity__help-actions">
          <button class="sp-btn-ghost" type="button" data-l10n-id="activity-open-log-folder" @click=${(e) => { e.stopPropagation(); this._onOpenLogFolder(); }}>Open log folder</button>
          <button class="sp-btn-ghost" type="button" data-l10n-id="activity-export-bundle" @click=${(e) => { e.stopPropagation(); this._onExportBundle(); }}>Export diagnostic bundle</button>
        </div>
      </section>
    `;
  }
}

customElements.define("sp-activity-log", SpActivityLog);
