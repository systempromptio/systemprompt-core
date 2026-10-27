import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { runAction } from "/assets/js/utils/action.js";
import { toneDot } from "/assets/js/utils/verdict.js";

const OFF = { verdict: { tone: "unknown", code: "off" }, conflicts: [], pending_uploads: 0, editing_count: 0 };

function conflictRow(notice) {
  const text = t("drive-conflict-row", { path: notice.path, sibling: notice.sibling })
    || `${notice.path}: your edit is in ${notice.sibling}`;
  return `<li class="sp-drive-status__conflict" data-key="${escapeHtml(notice.sibling)}">${escapeHtml(text)}</li>`;
}

function counts(drive) {
  const parts = [];
  if (drive.pending_uploads > 0) {
    parts.push(t("drive-pending", { count: drive.pending_uploads }) || `${drive.pending_uploads} pending`);
  }
  if (drive.editing_count > 0) {
    parts.push(t("drive-editing", { count: drive.editing_count }) || `${drive.editing_count} editing`);
  }
  return parts.map((p) => `<span class="sp-drive-status__count">${escapeHtml(p)}</span>`).join("");
}

export class SpDriveStatus extends SpElement {
  constructor() {
    super();
    this.drive = OFF;
    this.registerAction("drive-sync", (trigger) => runAction(trigger, {
      run: () => bridge.invoke("drive.sync_now"),
      context: t("drive-sync-now") || "Sync now",
    }));
  }

  onConnect() {
    this.classList.add("sp-drive-status");
    this.bridgeSubscribe("drive.changed", (payload) => { this.drive = payload || OFF; });
    this._load();
  }

  async _load() {
    try {
      this.drive = (await bridge.invoke("drive.status", {}, { timeoutMs: 30_000 })) || OFF;
    } catch (err) {
      console.error("drive.status failed", err);
      this.drive = OFF;
    }
  }

  render() {
    const drive = this.drive || OFF;
    const verdict = drive.verdict || OFF.verdict;
    const label = t(`drive-status-${verdict.code}`) || verdict.code;
    const root = drive.root ? `<span class="sp-drive-status__root">${escapeHtml(drive.root)}</span>` : "";
    const conflicts = (drive.conflicts || []).map(conflictRow).join("");
    const list = conflicts ? `<ul class="sp-drive-status__conflicts">${conflicts}</ul>` : "";
    const error = drive.last_error
      ? `<p class="sp-drive-status__error" role="status">${escapeHtml(drive.last_error)}</p>`
      : "";
    return `
      <div class="sp-drive-status__line" data-state="${escapeHtml(verdict.tone)}">
        <span class="sp-dot ${toneDot(verdict.tone)}" aria-hidden="true"></span>
        <span class="sp-drive-status__label">${escapeHtml(label)}</span>
        ${counts(drive)}
        <button type="button" class="sp-btn sp-btn--ghost" data-action="drive-sync">${escapeHtml(t("drive-sync-now") || "Sync now")}</button>
      </div>
      ${root}
      ${error}
      ${list}
    `;
  }
}

reactive(SpDriveStatus.prototype, ["drive"]);
customElements.define("sp-drive-status", SpDriveStatus);
