import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { runAction } from "/assets/js/utils/action.js";

const MAX_ROWS = 500;

function fmtClock(tsUnix) {
  if (!tsUnix) { return "--:--:--"; }
  return new Date(tsUnix * 1000).toLocaleTimeString();
}

function fmtTokens(row) {
  const parts = [];
  if (row.tokens_in != null) { parts.push(`${row.tokens_in} in`); }
  if (row.tokens_out != null) { parts.push(`${row.tokens_out} out`); }
  return parts.length ? parts.join(" / ") : "—";
}

// A denial the bridge made itself is a fact; a gateway decision is only known once
// the platform reports it, and "unknown" is the honest answer until then.
export function verdictOf(row) {
  if (row.verdict === "denied") {
    return { key: "deny", label: t("requests-verdict-deny") || "denied", reason: row.deny_reason || "" };
  }
  if (row.gateway_decision) {
    const allow = row.gateway_decision === "allow";
    return {
      key: allow ? "allow" : "deny",
      label: allow ? (t("requests-verdict-allow") || "allowed") : (t("requests-verdict-deny") || "denied"),
      reason: row.gateway_policy || "",
    };
  }
  return { key: "unknown", label: t("requests-verdict-unknown") || "—", reason: "" };
}

function statusClass(row) {
  if (row.verdict === "denied") { return "err"; }
  const s = Number(row.status) || 0;
  if (s >= 500) { return "err"; }
  if (s >= 400) { return "warn"; }
  return "ok";
}

export class SpRequestStream extends SpElement {
  constructor() {
    super();
    this.rows = [];
    this._query = "";
    this._deniedOnly = false;
    this._failed = false;
    this.registerAction("input:search", (trigger) => {
      this._query = trigger.value || "";
      this.invalidate();
    });
    this.registerAction("toggle-denied", () => {
      this._deniedOnly = !this._deniedOnly;
      this.invalidate();
    });
    this.registerAction("copy", (trigger) => runAction(trigger, {
      run: () => this._copy(),
      success: t("activity-copied") || "Copied",
      context: t("activity-copy") || "Copy",
    }));
  }

  onConnect() {
    this._backfill();
    this.bridgeSubscribe("request", (record) => this._upsert(record));
  }

  _backfill() {
    bridge.requestsRecent(MAX_ROWS).then((res) => {
      this.rows = ((res && res.entries) || []).slice();
      this._failed = false;
    }).catch((e) => {
      console.warn("request backfill failed", e);
      this._failed = true;
      this.invalidate();
    });
  }

  // A row is emitted when it is forwarded and again when its usage settles, so the
  // stream updates in place rather than showing the same request twice.
  _upsert(record) {
    if (!record || record.id == null) { return; }
    const next = this.rows.slice();
    const at = next.findIndex((r) => r.id === record.id);
    if (at >= 0) { next[at] = record; } else { next.push(record); }
    if (next.length > MAX_ROWS) { next.splice(0, next.length - MAX_ROWS); }
    this.rows = next;
  }

  _visible() {
    const q = this._query.trim().toLowerCase();
    return this.rows.filter((row) => {
      if (this._deniedOnly && verdictOf(row).key !== "deny") { return false; }
      if (!q) { return true; }
      return [row.agent, row.method, row.path, String(row.status ?? ""), row.model]
        .filter(Boolean)
        .some((v) => String(v).toLowerCase().includes(q));
    });
  }

  _copy() {
    const text = this._visible().map((r) =>
      [fmtClock(r.ts_unix), r.agent, r.method, r.path, r.status ?? "", verdictOf(r).label].join("\t")
    ).join("\n");
    return navigator.clipboard.writeText(text);
  }

  _row(row) {
    const verdict = verdictOf(row);
    return `
      <tr data-key="${escapeHtml(String(row.id))}" data-verdict="${verdict.key}">
        <td class="sp-u-mono">${escapeHtml(fmtClock(row.ts_unix))}</td>
        <td>${escapeHtml(row.agent || "unknown")}</td>
        <td class="sp-u-mono sp-req__path" title="${escapeHtml(`${row.method} ${row.path}`)}">${escapeHtml(row.method)} ${escapeHtml(row.path)}</td>
        <td><span class="sp-badge sp-badge--${statusClass(row)}">${escapeHtml(String(row.status ?? "—"))}</span></td>
        <td class="sp-u-mono">${row.latency_ms != null ? `${escapeHtml(String(row.latency_ms))}ms` : "—"}</td>
        <td class="sp-u-mono">${escapeHtml(fmtTokens(row))}</td>
        <td title="${escapeHtml(verdict.reason)}">${escapeHtml(verdict.label)}</td>
      </tr>
    `;
  }

  render() {
    const rows = this._visible();
    const head = [
      ["requests-col-time", "Time"], ["requests-col-agent", "Agent"],
      ["requests-col-request", "Request"], ["requests-col-status", "Status"],
      ["requests-col-latency", "Latency"], ["requests-col-tokens", "Tokens"],
      ["requests-col-verdict", "Verdict"],
    ].map(([id, en]) => `<th scope="col">${escapeHtml(t(id) || en)}</th>`).join("");

    const body = rows.length
      ? rows.map((r) => this._row(r)).join("")
      : `<tr><td colspan="7" class="sp-req__empty">${escapeHtml(
        this._failed
          ? (t("requests-backfill-failed") || "Could not read the request history from the proxy.")
          : (t("requests-empty") || "No requests yet — start a governed agent and they appear here.")
      )}</td></tr>`;

    const deniedLabel = this._deniedOnly ? (t("requests-filter-all") || "All") : (t("requests-filter-denied") || "Denied only");

    return `
      <header class="sp-req__header">
        <div>
          <h2>${escapeHtml(t("requests-heading") || "Governed requests")}</h2>
          <p class="sp-req__caption">${escapeHtml(t("requests-caption") || "Every request the proxy forwarded, and what your gateway decided about it.")}</p>
        </div>
      </header>
      <div class="sp-req__controls">
        <input type="search" class="sp-req__search" data-input="search" value="${escapeHtml(this._query)}"
          placeholder="${escapeHtml(t("requests-search-placeholder") || "Search requests…")}"
          aria-label="${escapeHtml(t("requests-search-placeholder") || "Search requests…")}">
        <button type="button" class="sp-btn-ghost" data-action="toggle-denied" aria-pressed="${this._deniedOnly}">${escapeHtml(deniedLabel)}</button>
        <button type="button" class="sp-btn-ghost" data-action="copy">${escapeHtml(t("activity-copy") || "Copy")}</button>
      </div>
      <div class="sp-req__scroll sp-scroll">
        <table class="sp-status__board sp-req__table">
          <thead><tr>${head}</tr></thead>
          <tbody>${body}</tbody>
        </table>
      </div>
    `;
  }
}

reactive(SpRequestStream.prototype, ["rows"]);
customElements.define("sp-request-stream", SpRequestStream);
