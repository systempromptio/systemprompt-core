import { SpElement } from "/assets/js/components/sp-element.js";
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

export class SpActivityLog extends SpElement {
  constructor() {
    super();
    this._virtual = null;
    this._pending = [];
    this.registerAction("open-log-folder", async () => {
      try { await bridge.openLogFolder(); } catch (e) { console.warn("open log folder", e); }
    });
    this.registerAction("export-bundle", async () => {
      try { await bridge.diagnosticsExportBundle(); } catch (e) { console.warn("export bundle", e); }
    });
  }

  onConnect() {
    this.classList.add("sp-activity");
    this.setAttribute("aria-label", "Activity log");
    bridge.stateSnapshot().then((s) => this._applyStats(s)).catch(() => {});
    this.bridgeSubscribe("state.changed", (s) => this._applyStats(s));
    this.bridgeSubscribe("proxy.stats", () => {
      bridge.stateSnapshot().then((s) => this._applyStats(s)).catch(() => {});
    });
    this.bridgeSubscribe("log", (entry) => this._appendLog(entry && entry.line));
  }

  afterRender() {
    const root = this.querySelector(".sp-log-virtual");
    if (root && !this._virtual) {
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

  _applyStats(snap) {
    const stats = (snap && snap.proxy_stats) || {};
    this._setStat("msgs", fmtCount(stats.messages_total));
    this._setStat("tin", fmtCount(stats.tokens_in_total));
    this._setStat("tout", fmtCount(stats.tokens_out_total));
  }

  _setStat(name, value) {
    const el = this.querySelector(`[data-stat="${name}"]`);
    if (el) { el.textContent = value; }
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

  render() {
    return `
      <header class="sp-activity__header">
        <span class="sp-activity__title" data-l10n-id="activity-title">Activity</span>
        <div class="sp-activity-lane" aria-label="Activity totals">
          <span class="sp-activity-lane__stat"><b data-stat="msgs">0</b><span class="sp-activity-lane__label" data-l10n-id="activity-msgs">msgs</span></span>
          <span class="sp-activity-lane__stat"><b data-stat="tin">0</b><span class="sp-activity-lane__label" data-l10n-id="activity-tin">in</span></span>
          <span class="sp-activity-lane__stat"><b data-stat="tout">0</b><span class="sp-activity-lane__label" data-l10n-id="activity-tout">out</span></span>
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
          <button class="sp-btn-ghost" type="button" data-l10n-id="activity-open-log-folder" data-action="open-log-folder">Open log folder</button>
          <button class="sp-btn-ghost" type="button" data-l10n-id="activity-export-bundle" data-action="export-bundle">Export diagnostic bundle</button>
        </div>
      </section>
    `;
  }
}

customElements.define("sp-activity-log", SpActivityLog);
