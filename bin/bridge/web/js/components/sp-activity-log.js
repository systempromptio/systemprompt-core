import { SpElement } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { announce } from "/assets/js/utils/announce.js";
import { toLogEntry, logFilterFor } from "/assets/js/utils/log-format.js";
import { proxyStatCells, trimToCapacity } from "/assets/js/utils/log-stats.js";
import { renderActivityHeader, renderActivityControls, renderActivityHelp } from "/assets/js/components/activity-parts.js";
import { registerActivityToolActions } from "/assets/js/components/activity-actions.js";
import { DEFAULT_CAPACITY, createLogVirtual } from "/assets/js/components/log-virtual.js";

export class SpActivityLog extends SpElement {
  constructor() {
    super();
    this._virtual = null;
    this._entries = [];
    this._query = "";
    this._level = "all";
    this._expanded = null;
    registerActivityToolActions(this);
    this._registerViewActions();
  }

  _registerViewActions() {
    this.registerAction("set-level", (trigger) => {
      this._level = trigger.dataset.level || "all";
      this._applyFilter();
      this.invalidate();
    });
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
      this._entries = list.map(toLogEntry);
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
    this._virtual.setFilter(logFilterFor(this._query, this._level));
  }

  _applyStats(snap) {
    for (const [name, value] of proxyStatCells(snap)) {
      const el = this.querySelector(`[data-stat="${name}"]`);
      if (el) { el.textContent = value; }
    }
  }

  _appendLog(record) {
    if (!record) { return; }
    const entry = toLogEntry(record);
    this._entries.push(entry);
    trimToCapacity(this._entries, DEFAULT_CAPACITY);
    if (this._virtual) { this._virtual.append(entry); }
    // The viewport rewrites itself on every scroll frame, so it cannot itself be a
    // live region — it would re-read the whole visible window as the user merely
    // scrolls, and a backfill would do it a thousand times at once. Only genuinely
    // new lines are announced, through the app's single polite region.
    announce(entry.meta ? entry.meta.line : entry.text);
  }

  render() {
    const expanded = this._expanded
      ? `<div class="sp-log__detail" data-action="collapse-line" tabindex="0" role="button" aria-label="${escapeHtml(t("activity-collapse-line") || "Collapse")}">${escapeHtml(this._expanded)}</div>`
      : "";
    const empty = this._backfillFailed
      ? `<p class="sp-log__empty">${escapeHtml(t("activity-empty") || "No activity yet.")}</p>`
      : "";
    return `
      ${renderActivityHeader()}
      ${renderActivityControls(this)}
      <div class="sp-log sp-log-virtual" role="log" aria-live="off" tabindex="0" aria-label="${escapeHtml(t("activity-log-aria") || "Activity log")}" data-preserve>
        <div class="sp-log-virtual__spacer-top" aria-hidden="true"></div>
        <ol class="sp-log-virtual__viewport"></ol>
        <div class="sp-log-virtual__spacer-bottom" aria-hidden="true"></div>
      </div>
      ${empty}
      ${expanded}
      ${renderActivityHelp()}
    `;
  }
}

customElements.define("sp-activity-log", SpActivityLog);
