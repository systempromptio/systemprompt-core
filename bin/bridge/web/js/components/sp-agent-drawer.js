import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { t } from "/assets/js/i18n.js";
import { trapTab, setBackgroundInert } from "/assets/js/utils/focus-trap.js";
import { renderAgentDrawerAdd } from "/assets/js/components/agent-drawer-add.js";
import { renderAgentDrawerDetail } from "/assets/js/components/agent-drawer-detail.js";
import {
  runAgentDrawerAct, runAgentDrawerOpenConfig, runAgentDrawerAddHost,
  runAgentDrawerSaveFilter, runAgentDrawerResetFilter, runAgentDrawerConfirmRemove,
  captureAgentDrawerFilter,
} from "/assets/js/utils/agent-drawer-actions.js";

/**
 * The right-hand slide-over that holds everything the Agents list deliberately
 * does not show. Two modes on one surface:
 *
 *   "detail" — one agent: its health, its models, and (collapsed) the technical
 *              configuration that used to be the entire page.
 *   "add"    — the picker behind the header's "+ Add agent".
 *
 * The app has no <dialog> and no modal anywhere, so the dialog semantics here
 * are hand-rolled: role/aria-modal, Escape to close, focus moved in on open and
 * returned to the opener on close.
 */
export class SpAgentDrawer extends SpElement {
  constructor() {
    super();
    this.mode = null;
    this.hostId = null;
    this.hosts = [];
    this.snapshot = null;
    this.gated = false;
    this.busyId = null;
    this.filterDraft = null;
    this.confirmRemove = false;
    this._returnFocusTo = null;
    this._needsFocus = false;

    this.registerAction("close", () => this.close());
    this.registerAction("scrim", () => this.close());
    this.registerAction("act", (trigger) => runAgentDrawerAct(this, trigger));
    this.registerAction("open-config", () => runAgentDrawerOpenConfig(this));
    this.registerAction("add-host", (trigger) => runAgentDrawerAddHost(this, trigger));
    this.registerAction("change:proto", () => captureAgentDrawerFilter(this));
    this.registerAction("change:model-all", () => captureAgentDrawerFilter(this));
    this.registerAction("saveModelFilter", (trigger) => runAgentDrawerSaveFilter(this, trigger));
    this.registerAction("resetModelFilter", (trigger) => runAgentDrawerResetFilter(this, trigger));
    this.registerAction("remove-agent", () => { this.confirmRemove = true; });
    this.registerAction("cancel-remove", () => { this.confirmRemove = false; });
    this.registerAction("confirm-remove", (trigger) => runAgentDrawerConfirmRemove(this, trigger));
  }

  onConnect() {
    this._onKeydown = (e) => {
      if (!this.mode) { return; }
      if (e.key === "Escape") { e.stopPropagation(); this.close(); return; }
      // aria-modal="true" claims the rest of the page is unavailable. Without
      // this, Tab walked straight out of the dialog into the rail and the
      // activity log, both sitting behind the scrim.
      trapTab(e, this.querySelector(".sp-drawer__panel"));
    };
    document.addEventListener("keydown", this._onKeydown, true);
    this._unsubs.push(() => document.removeEventListener("keydown", this._onKeydown, true));
  }

  /** @param {"detail"|"add"} mode */
  open(mode, hostId, opener) {
    this._returnFocusTo = opener || document.activeElement;
    this._needsFocus = true;
    this.hostId = hostId || null;
    this.filterDraft = null;
    this.confirmRemove = false;
    this.mode = mode;
    document.body.classList.add("is-drawer-open");
    setBackgroundInert(true, this);
  }

  close() {
    if (!this.mode) { return; }
    this.mode = null;
    this.hostId = null;
    this.filterDraft = null;
    this.confirmRemove = false;
    document.body.classList.remove("is-drawer-open");
    setBackgroundInert(false, this);
    const target = this._returnFocusTo;
    this._returnFocusTo = null;
    if (target && typeof target.focus === "function" && target.isConnected) { target.focus(); }
  }

  _hostById(id) {
    return id ? (this.hosts || []).find((h) => h.id === id) || null : null;
  }

  _host() {
    return this._hostById(this.hostId);
  }

  render() {
    if (!this.mode) { return `<div class="sp-drawer" hidden></div>`; }
    const body = this.mode === "add" ? renderAgentDrawerAdd(this) : renderAgentDrawerDetail(this);
    return `
      <div class="sp-drawer" data-open="true">
        <div class="sp-drawer__scrim" data-action="scrim" aria-hidden="true"></div>
        <section class="sp-drawer__panel" role="dialog" aria-modal="true"
                 aria-label="${escapeHtml(body.title)}">
          <header class="sp-drawer__head">
            ${body.head}
            <button class="sp-drawer__close" type="button" data-action="close"
                    aria-label="${escapeHtml(t("drawer-close") || "Close")}">✕</button>
          </header>
          <div class="sp-drawer__body">${body.content}</div>
        </section>
      </div>
    `;
  }

  afterRender() {
    if (this._needsFocus && this.mode) {
      this._needsFocus = false;
      const close = this.querySelector(".sp-drawer__close");
      if (close) { close.focus(); }
    }
  }
}

reactive(SpAgentDrawer.prototype, ["mode", "hostId", "hosts", "snapshot", "gated", "busyId", "filterDraft", "confirmRemove"]);
customElements.define("sp-agent-drawer", SpAgentDrawer);
