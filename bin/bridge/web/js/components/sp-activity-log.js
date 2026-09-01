import { SpElement, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { runAction } from "/assets/js/utils/action.js";
import { announce } from "/assets/js/utils/announce.js";
import { TAB_DEFS, shortcut } from "/assets/js/utils/rail-tabs.js";
import { DEFAULT_CAPACITY, createLogVirtual } from "/assets/js/components/log-virtual.js";

const LEVELS = ["all", "warn", "error"];

function fmtCount(n) {
  const v = Number(n) || 0;
  if (v >= 1_000_000) { return `${(v / 1_000_000).toFixed(1)}M`; }
  if (v >= 1_000) { return `${(v / 1_000).toFixed(1)}k`; }
  return String(v);
}

function fmtClock(tsUnix) {
  if (!tsUnix) { return "--:--:--"; }
  return new Date(tsUnix * 1000).toLocaleTimeString();
}

// Rust stamps the entry when it happens; deriving it from arrival time mislabelled
// anything queued, batched or replayed — which is every backfilled line.
function toEntry(record) {
  const line = record && record.line ? record.line : String(record ?? "");
  const level = (record && record.level) || "info";
  return { text: `[${fmtClock(record && record.ts_unix)}] ${line}`, level, meta: { line, level } };
}

export class SpActivityLog extends SpElement {
  constructor() {
    super();
    this._virtual = null;
    this._entries = [];
    this._query = "";
    this._level = "all";
    this._expanded = null;
    this.registerAction("open-log-folder", (trigger) => runAction(trigger, {
      run: () => bridge.openLogFolder(),
      context: t("activity-open-log-folder") || "Open log folder",
    }));
    this.registerAction("export-bundle", (trigger) => runAction(trigger, {
      run: () => bridge.diagnosticsExportBundle(),
      success: (v) => (v && v.path)
        ? (t("activity-bundle-written", { path: v.path }) || `Diagnostic bundle written to ${v.path}`)
        : (t("activity-bundle-done") || "Diagnostic bundle written."),
      context: t("activity-export-bundle") || "Export diagnostic bundle",
    }));
    this.registerAction("set-level", (trigger) => {
      this._level = trigger.dataset.level || "all";
      this._applyFilter();
      this.invalidate();
    });
    this.registerAction("copy", (trigger) => runAction(trigger, {
      run: () => this._copy(),
      success: t("activity-copied") || "Copied",
      context: t("activity-copy") || "Copy",
    }));
    this.registerAction("expand-line", (trigger) => {
      const idx = Number(trigger.dataset.index);
      const rows = this._virtual ? this._virtual.entries() : [];
      const row = rows[idx];
      this._expanded = row && row.meta ? row.meta.line : null;
      this.invalidate();
    });
    this.registerAction("collapse-line", () => {
      this._expanded = null;
      this.invalidate();
    });
    this.registerAction("input:search", (trigger) => {
      this._query = trigger.value || "";
      this._applyFilter();
    });
  }

  onConnect() {
    this.classList.add("sp-activity");
    // The name belongs on the element that carries role="log". Setting it on
    // this host, which has no role, put the label on a generic element where it
    // is ignored and left the live region itself unnamed.
    this.useSnapshot((s) => this._applyStats(s));
    this.bridgeSubscribe("proxy.stats", (stats) => this._applyStats({ proxy_stats: stats }));
    this.bridgeSubscribe("log", (entry) => this._appendLog(entry));
    this._backfill();
  }

  // The Rust ring has held up to 1000 entries all along; the pane just never asked
  // for them, so every webview reload started at "Ready." and forgot the rest.
  _backfill() {
    bridge.activityRecent(DEFAULT_CAPACITY).then((res) => {
      const list = (res && res.entries) || [];
      this._entries = list.map(toEntry);
      if (this._virtual) { this._virtual.setAll(this._entries); this._applyFilter(); }
      this.invalidate();
    }).catch((e) => {
      console.warn("activity backfill failed", e);
      this._backfillFailed = true;
      this.invalidate();
    });
  }

  afterRender() {
    const root = this.querySelector(".sp-log-virtual");
    if (!root) { return; }
    if (this._virtual && this._virtual.root === root) { return; }
    try {
      this._virtual = createLogVirtual(root, { initial: this._entries });
      this._applyFilter();
    } catch (e) {
      this._virtual = null;
      console.error("log-virtual init failed", e);
    }
  }

  _applyFilter() {
    if (!this._virtual) { return; }
    const q = this._query.trim().toLowerCase();
    const level = this._level;
    if (!q && level === "all") { this._virtual.setFilter(null); return; }
    this._virtual.setFilter((entry) => {
      if (level === "warn" && entry.level === "info") { return false; }
      if (level === "error" && entry.level !== "error") { return false; }
      return !q || entry.text.toLowerCase().includes(q);
    });
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

  _appendLog(record) {
    if (!record) { return; }
    const entry = toEntry(record);
    this._entries.push(entry);
    if (this._entries.length > DEFAULT_CAPACITY) {
      this._entries.splice(0, this._entries.length - DEFAULT_CAPACITY);
    }
    if (this._virtual) { this._virtual.append(entry); }
    // The viewport rewrites itself on every scroll frame, so it cannot itself be a
    // live region — it would re-read the whole visible window as the user merely
    // scrolls, and a backfill would do it a thousand times at once. Only genuinely
    // new lines are announced, through the app's single polite region.
    announce(entry.meta ? entry.meta.line : entry.text);
  }

  _copy() {
    const rows = this._virtual ? this._virtual.entries() : this._entries;
    return navigator.clipboard.writeText(rows.map((r) => r.text).join("\n"));
  }

  render() {
    const levelBtns = LEVELS.map((lv) => {
      const label = lv === "all" ? (t("activity-level-all") || "All")
        : lv === "warn" ? (t("activity-level-warn") || "Warnings")
          : (t("activity-level-error") || "Errors");
      const pressed = this._level === lv ? "true" : "false";
      return `<button type="button" class="sp-btn-ghost sp-log__filter" data-action="set-level" data-level="${lv}" aria-pressed="${pressed}">${escapeHtml(label)}</button>`;
    }).join("");

    const expanded = this._expanded
      ? `<div class="sp-log__detail" data-action="collapse-line" tabindex="0" role="button" aria-label="${escapeHtml(t("activity-collapse-line") || "Collapse")}">${escapeHtml(this._expanded)}</div>`
      : "";

    const empty = this._backfillFailed
      ? `<p class="sp-log__empty">${escapeHtml(t("activity-empty") || "No activity yet.")}</p>`
      : "";

    return `
      <header class="sp-activity__header">
        <span class="sp-activity__title" data-l10n-id="activity-title">Activity</span>
        <div class="sp-activity-lane" data-l10n-aria="activity-totals-aria" aria-label="Activity totals">
          <span class="sp-activity-lane__stat"><b data-stat="msgs" data-preserve>0</b><span class="sp-activity-lane__label" data-l10n-id="activity-msgs">msgs</span></span>
          <span class="sp-activity-lane__stat"><b data-stat="tin" data-preserve>0</b><span class="sp-activity-lane__label" data-l10n-id="activity-tin">in</span></span>
          <span class="sp-activity-lane__stat"><b data-stat="tout" data-preserve>0</b><span class="sp-activity-lane__label" data-l10n-id="activity-tout">out</span></span>
        </div>
      </header>
      <div class="sp-log__controls">
        <input type="search" class="sp-log__search" data-input="search" value="${escapeHtml(this._query)}"
          placeholder="${escapeHtml(t("activity-search-placeholder") || "Filter activity…")}"
          aria-label="${escapeHtml(t("activity-search-placeholder") || "Filter activity…")}">
        <div class="sp-log__filters" role="group">${levelBtns}</div>
        <button type="button" class="sp-btn-ghost" data-action="copy">${escapeHtml(t("activity-copy") || "Copy")}</button>
      </div>
      <div class="sp-log sp-log-virtual" role="log" aria-live="off" tabindex="0" aria-label="${escapeHtml(t("activity-log-aria") || "Activity log")}" data-preserve>
        <div class="sp-log-virtual__spacer-top" aria-hidden="true"></div>
        <ol class="sp-log-virtual__viewport"></ol>
        <div class="sp-log-virtual__spacer-bottom" aria-hidden="true"></div>
      </div>
      ${empty}
      ${expanded}
      <section class="sp-activity__help" data-l10n-aria="activity-help-aria" aria-label="Help and support">
        <header class="sp-activity__help-title" data-l10n-id="activity-help-title">Help &amp; Support</header>
        <div class="sp-activity__help-actions">
          <button class="sp-btn-ghost" type="button" data-l10n-id="activity-open-log-folder" data-action="open-log-folder">Open log folder</button>
          <button class="sp-btn-ghost" type="button" data-l10n-id="activity-export-bundle" data-action="export-bundle">Export diagnostic bundle</button>
        </div>
        <details class="sp-activity__shortcuts">
          <summary data-l10n-id="activity-shortcuts-title">Keyboard shortcuts</summary>
          <dl class="sp-shortcuts">
            ${TAB_DEFS.map((d) => `
              <dt><kbd class="sp-kbd">${escapeHtml(shortcut(d.key))}</kbd></dt>
              <dd>${escapeHtml(t(d.l10n) || d.label)}</dd>`).join("")}
            <dt><kbd class="sp-kbd">${escapeHtml(shortcut("F"))}</kbd></dt>
            <dd>${escapeHtml(t("activity-shortcut-search") || "Search the marketplace")}</dd>
            <dt><kbd class="sp-kbd">Esc</kbd></dt>
            <dd>${escapeHtml(t("activity-shortcut-escape") || "Close the open panel")}</dd>
          </dl>
        </details>
      </section>
    `;
  }
}

customElements.define("sp-activity-log", SpActivityLog);
