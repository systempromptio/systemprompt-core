import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { TAB_DEFS, TAB_GLYPHS, readInitialTab, persistTab } from "/assets/js/utils/rail-tabs.js";
import { onBridgeEvent } from "/assets/js/events/bridge-events.js";

export class SpRail extends SpElement {
  constructor() {
    super();
    this.activeTab = readInitialTab();
    this.agentCount = 0;
    this.marketplaceCount = 0;
    this._onResize = () => this._syncIndicator();
    this._onRailKeydown = (e) => this._handleRailKey(e);
    this._onMktCount = (e) => {
      const total = e.detail && e.detail.total;
      if (typeof total === "number") { this.marketplaceCount = total; }
    };
    this.registerAction("activate-tab", (trigger) => this.activateTab(trigger.dataset.tab));
  }

  onConnect() {
    this.classList.add("sp-rail");
    this.setAttribute("role", "tablist");
    this.setAttribute("aria-label", "Sections");
    this.dataset.activeReady = "false";
    this.bridgeSubscribe("state.changed", (s) => {
      this.agentCount = ((s && s.host_apps) || []).length;
    });
    bridge.stateSnapshot().then((s) => {
      this.agentCount = ((s && s.host_apps) || []).length;
    }).catch(() => {});
    this._unsubMkt = onBridgeEvent("mkt:count", this._onMktCount);
    window.addEventListener("resize", this._onResize);
    this.addEventListener("keydown", this._onRailKeydown);
    queueMicrotask(() => this.activateTab(this.activeTab));
  }

  onDisconnect() {
    if (this._unsubMkt) { this._unsubMkt(); this._unsubMkt = null; }
    window.removeEventListener("resize", this._onResize);
    this.removeEventListener("keydown", this._onRailKeydown);
  }

  _handleRailKey(e) {
    const key = e.key;
    if (key !== "ArrowDown" && key !== "ArrowUp" && key !== "Home" && key !== "End") {
      return;
    }
    const tabs = Array.from(this.querySelectorAll(".sp-rail-tab"));
    if (tabs.length === 0) { return; }
    const currentIndex = tabs.findIndex((t) => t.dataset.tab === this.activeTab);
    let nextIndex = currentIndex;
    if (key === "ArrowDown") {
      nextIndex = currentIndex < 0 ? 0 : (currentIndex + 1) % tabs.length;
    } else if (key === "ArrowUp") {
      nextIndex = currentIndex < 0 ? tabs.length - 1 : (currentIndex - 1 + tabs.length) % tabs.length;
    } else if (key === "Home") {
      nextIndex = 0;
    } else if (key === "End") {
      nextIndex = tabs.length - 1;
    }
    e.preventDefault();
    const next = tabs[nextIndex];
    if (next) {
      this.activateTab(next.dataset.tab);
      next.focus();
    }
  }

  activateTab(name) {
    this.activeTab = name;
    persistTab(name);
    document.dispatchEvent(new CustomEvent("crumb:set", { detail: { name } }));
  }

  afterRender() {
    for (const panel of document.querySelectorAll(".sp-tab__panel")) {
      panel.hidden = panel.dataset.tab !== this.activeTab;
    }
    requestAnimationFrame(() => this._syncIndicator());
  }

  _syncIndicator() {
    const active = this.querySelector(`.sp-rail-tab[data-tab="${this.activeTab}"]`);
    if (active) {
      const railRect = this.getBoundingClientRect();
      const tabRect = active.getBoundingClientRect();
      const y = (tabRect.top - railRect.top) + this.scrollTop;
      this.style.setProperty("--sp-rail-active-y", `${y}px`);
      this.style.setProperty("--sp-rail-active-h", `${tabRect.height}px`);
      this.dataset.activeReady = "true";
    } else {
      this.dataset.activeReady = "false";
    }
  }

  _renderTab(def) {
    const selected = this.activeTab === def.name;
    const count = def.showCount ? this[def.countFor] : null;
    const countNode = count == null
      ? `<span class="sp-rail-tab__count" hidden></span>`
      : `<span class="sp-rail-tab__count">${escapeHtml(count)}</span>`;
    return `
      <button class="sp-rail-tab" data-tab="${def.name}" role="tab" aria-selected="${selected ? "true" : "false"}" tabindex="${selected ? "0" : "-1"}" type="button" data-action="activate-tab">
        <span class="sp-rail-tab__glyph" aria-hidden="true">${TAB_GLYPHS[def.name]}</span>
        <span class="sp-rail-tab__label" data-l10n-id="${def.l10n}">${escapeHtml(def.label)}</span>
        ${countNode}
        <span class="sp-rail-tab__shortcut" aria-hidden="true">${escapeHtml(def.shortcut)}</span>
      </button>
    `;
  }

  render() {
    const versionAttr = this.dataset.version || "";
    return `
      <div class="sp-rail-section">
        <div class="sp-rail-section__label" data-l10n-id="nav-section-navigate">Navigate</div>
        ${TAB_DEFS.map((d) => this._renderTab(d)).join("")}
      </div>
      <div class="sp-rail__spacer"></div>
      <div class="sp-rail__divider" aria-hidden="true"></div>
      <sp-rail-profile data-version="${escapeHtml(versionAttr)}"></sp-rail-profile>
    `;
  }
}

reactive(SpRail.prototype, ["activeTab", "agentCount", "marketplaceCount"]);
customElements.define("sp-rail", SpRail);
