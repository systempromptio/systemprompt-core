import { html } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";
import { bridge } from "/assets/js/bridge.js";

const TAB_LABELS = {
  marketplace: "Marketplace",
  agents: "Agents",
  status: "Status",
  settings: "Settings",
};

const TAB_KEYS = { "1": "marketplace", "2": "agents", "3": "status", "4": "settings" };

function readInitialTab() {
  try { return localStorage.getItem("cowork.tab") || "marketplace"; }
  catch (_) { return "marketplace"; }
}

function persistTab(name) {
  try { localStorage.setItem("cowork.tab", name); } catch (_) { /* ignore */ }
}

function isTextInput(target) {
  if (!target) { return false; }
  return target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;
}

export class SpRail extends BridgeElement {
  static properties = {
    activeTab: { state: true },
    agentCount: { state: true },
    marketplaceCount: { state: true },
  };

  constructor() {
    super();
    this.activeTab = readInitialTab();
    this.agentCount = 0;
    this.marketplaceCount = 0;
    this._onResize = () => this._syncIndicator();
    this._onKeydown = (e) => this._handleKey(e);
    this._onRailKeydown = (e) => this._handleRailKey(e);
    this._onMktCount = (e) => {
      const total = e.detail && e.detail.total;
      if (typeof total === "number") { this.marketplaceCount = total; }
    };
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
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
    document.addEventListener("keydown", this._onKeydown);
    document.addEventListener("mkt:count", this._onMktCount);
    window.addEventListener("resize", this._onResize);
  }

  disconnectedCallback() {
    document.removeEventListener("keydown", this._onKeydown);
    document.removeEventListener("mkt:count", this._onMktCount);
    window.removeEventListener("resize", this._onResize);
    this.removeEventListener("keydown", this._onRailKeydown);
    super.disconnectedCallback();
  }

  firstUpdated() {
    this.activateTab(this.activeTab);
    this.addEventListener("keydown", this._onRailKeydown);
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

  updated() {
    this._syncPanels();
    requestAnimationFrame(() => this._syncIndicator());
  }

  activateTab(name) {
    this.activeTab = name;
    persistTab(name);
    document.dispatchEvent(new CustomEvent("crumb:set", { detail: { name } }));
  }

  _syncPanels() {
    for (const panel of document.querySelectorAll(".sp-tab__panel")) {
      panel.hidden = panel.dataset.tab !== this.activeTab;
    }
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

  _handleKey(e) {
    const mod = e.metaKey || e.ctrlKey;
    if (!mod) { return; }
    if (e.key === "f") {
      const search = document.getElementById("mkt-search");
      if (search) { e.preventDefault(); search.focus(); search.select(); }
      return;
    }
    if (TAB_KEYS[e.key] && !isTextInput(e.target)) {
      e.preventDefault();
      this.activateTab(TAB_KEYS[e.key]);
    }
  }

  _tab(name, label, l10n, shortcut, glyph, count) {
    const selected = this.activeTab === name;
    return html`
      <button class="sp-rail-tab" data-tab=${name} role="tab" aria-selected=${selected ? "true" : "false"} tabindex=${selected ? "0" : "-1"} type="button" @click=${(e) => { e.stopPropagation(); this.activateTab(name); }}>
        <span class="sp-rail-tab__glyph" aria-hidden="true">${glyph}</span>
        <span class="sp-rail-tab__label" data-l10n-id=${l10n}>${label}</span>
        ${count == null ? html`<span class="sp-rail-tab__count" hidden></span>` : html`<span class="sp-rail-tab__count">${count}</span>`}
        <span class="sp-rail-tab__shortcut" aria-hidden="true">${shortcut}</span>
      </button>
    `;
  }

  render() {
    const versionAttr = this.dataset.version || "";
    return html`
      <div class="sp-rail-section">
        <div class="sp-rail-section__label" data-l10n-id="nav-section-navigate">Navigate</div>
        ${this._tab("marketplace", "Marketplace", "nav-marketplace", "⌘1",
          html`<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2.5 21 7v10l-9 4.5L3 17V7l9-4.5z"/><path d="M3 7l9 4.5L21 7"/><path d="M12 11.5V21.5"/></svg>`,
          this.marketplaceCount)}
        ${this._tab("agents", "Agents", "nav-agents", "⌘2",
          html`<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/></svg>`,
          this.agentCount)}
        ${this._tab("status", "Status", "nav-status", "⌘3",
          html`<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="5"/><circle cx="12" cy="12" r="1.5" fill="currentColor"/></svg>`,
          null)}
        ${this._tab("settings", "Settings", "nav-settings", "⌘4",
          html`<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.04 1.56V21a2 2 0 0 1-4 0v-.09a1.7 1.7 0 0 0-1.04-1.56 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.56-1.04H3a2 2 0 0 1 0-4h.09A1.7 1.7 0 0 0 4.6 9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1.04-1.56V3a2 2 0 0 1 4 0v.09A1.7 1.7 0 0 0 15 4.6a1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9a1.7 1.7 0 0 0 1.56 1.04H21a2 2 0 0 1 0 4h-.09A1.7 1.7 0 0 0 19.4 15z"/></svg>`,
          null)}
      </div>
      <div class="sp-rail__spacer"></div>
      <div class="sp-rail__divider" aria-hidden="true"></div>
      <sp-rail-profile data-version=${versionAttr}></sp-rail-profile>
    `;
  }
}

customElements.define("sp-rail", SpRail);
