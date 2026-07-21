import { SpElement } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { DEFAULT_CAPACITY, createLogVirtual } from "/assets/js/components/log-virtual.js";

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
    this._lines = [{ text: "Ready.", level: "info" }];
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
    bridge.stateSnapshot().then((s) => this._applyStats(s)).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => this._applyStats(s));
    this.bridgeSubscribe("proxy.stats", () => {
      bridge.stateSnapshot().then((s) => this._applyStats(s)).catch((e) => console.warn("snapshot failed", e));
    });
    this.bridgeSubscribe("log", (entry) => this._appendLog(entry && entry.line));
  }

  // Every render replaces innerHTML, detaching the nodes the previous virtual
  // list closed over; rebinding is what keeps the log visible across the
  // i18n-ready re-render.
  afterRender() {
    const root = this.querySelector(".sp-log-virtual");
    if (!root || (this._virtual && this._virtual.root === root)) { return; }
    try {
      this._virtual = createLogVirtual(root, { initial: this._lines });
    } catch (e) {
      this._virtual = null;
      console.error("log-virtual init failed", e);
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
    const ts = new Date().toLocaleTimeString();
    const entry = { text: `[${ts}] ${line}`, level: classifyLevel(line) };
    this._lines.push(entry);
    if (this._lines.length > DEFAULT_CAPACITY) {
      this._lines.splice(0, this._lines.length - DEFAULT_CAPACITY);
    }
    if (this._virtual) { this._virtual.append(entry); }
  }

  render() {
    return `
      <header class="sp-activity__header">
        <span class="sp-activity__title" data-l10n-id="activity-title">Activity</span>
        <div class="sp-activity-lane" aria-label="Activity totals">
          <span class="sp-activity-lane__stat"><b data-stat="msgs" data-preserve>0</b><span class="sp-activity-lane__label" data-l10n-id="activity-msgs">msgs</span></span>
          <span class="sp-activity-lane__stat"><b data-stat="tin" data-preserve>0</b><span class="sp-activity-lane__label" data-l10n-id="activity-tin">in</span></span>
          <span class="sp-activity-lane__stat"><b data-stat="tout" data-preserve>0</b><span class="sp-activity-lane__label" data-l10n-id="activity-tout">out</span></span>
        </div>
      </header>
      <div class="sp-log sp-log-virtual" role="log" aria-live="polite" aria-atomic="false" tabindex="0" data-preserve>
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
