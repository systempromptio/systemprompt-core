import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { runAction } from "/assets/js/utils/action.js";
import { t } from "/assets/js/i18n.js";

// The commands here live in the macOS menu bar. On Windows a muda menu bar
// renders as a system-coloured Win32 strip between the title bar and this
// near-black UI, so the strip is not installed there and this is where they go.
export class SpTopbarMenu extends SpElement {
  constructor() {
    super();
    this.menuOpen = false;
    this._onDocPointer = (e) => {
      if (this.menuOpen && !this.contains(e.target)) { this.menuOpen = false; }
    };
    this._onDocKey = (e) => {
      if (e.key === "Escape" && this.menuOpen) { this.menuOpen = false; }
    };
    this.registerAction("toggle-menu", () => { this.menuOpen = !this.menuOpen; });
    // Why: drive the rail's own tab button rather than a second navigation
    // path, so tab activation, focus handling and persistence keep working
    // through whatever the rail does next.
    this.registerAction("open-settings", () => {
      this.menuOpen = false;
      document.querySelector('.sp-rail-tab[data-tab="settings"]')?.click();
    });
    this.registerAction("open-log-folder", (trigger) => {
      this.menuOpen = false;
      return runAction(trigger, {
        run: () => bridge.openLogFolder(),
        success: t("toast-log-folder-opened") || "Opened the log folder.",
        context: t("menu-open-log-folder") || "Open log folder",
      });
    });
    this.registerAction("export-bundle", (trigger) => {
      this.menuOpen = false;
      return runAction(trigger, {
        run: () => bridge.diagnosticsExportBundle(),
        success: t("toast-bundle-exported") || "Exported the diagnostic bundle.",
        context: t("menu-export-bundle") || "Export diagnostic bundle",
      });
    });
    this.registerAction("open-config-folder", (trigger) => {
      this.menuOpen = false;
      return runAction(trigger, {
        run: () => bridge.openConfigFolder(),
        success: t("toast-folder-opened") || "Opened the configuration folder.",
        context: t("menu-open-config") || "Open config folder",
      });
    });
  }

  onConnect() {
    document.addEventListener("pointerdown", this._onDocPointer);
    document.addEventListener("keydown", this._onDocKey);
  }

  onDisconnect() {
    document.removeEventListener("pointerdown", this._onDocPointer);
    document.removeEventListener("keydown", this._onDocKey);
  }

  render() {
    const open = this.menuOpen;
    const item = (action, key, fallback) =>
      `<button class="sp-topbar-menu__item" type="button" role="menuitem" data-action="${action}">${escapeHtml(t(key) || fallback)}</button>`;
    const menu = open
      ? `<div class="sp-topbar-menu__list" role="menu">
           ${item("open-settings", "topbar-menu-settings", "Settings")}
           ${item("open-log-folder", "menu-open-log-folder", "Open log folder")}
           ${item("export-bundle", "menu-export-bundle", "Export diagnostic bundle")}
           ${item("open-config-folder", "menu-open-config", "Open config folder")}
         </div>`
      : "";
    return `
      <button class="sp-topbar-menu__trigger" type="button" data-action="toggle-menu"
              aria-haspopup="menu" aria-expanded="${open ? "true" : "false"}"
              aria-label="${escapeHtml(t("topbar-menu-label") || "More")}">
        <span aria-hidden="true">⋯</span>
      </button>
      ${menu}
    `;
  }
}

reactive(SpTopbarMenu.prototype, ["menuOpen"]);
customElements.define("sp-topbar-menu", SpTopbarMenu);
