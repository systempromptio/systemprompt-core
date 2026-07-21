import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { logout } from "/assets/js/services/session-service.js";

const VERSION = (() => {
  const tag = document.querySelector('meta[name="sp-version"]');
  return (tag && tag.content) || "";
})();

function initials(idSrc) {
  const letters = (idSrc || "").replace(/[^a-zA-Z]/g, "").slice(0, 2).toUpperCase();
  return letters || "SP";
}

export class SpRailProfile extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.menuOpen = false;
    this.logoutError = "";
    this._baseVersion = "";
    this._onDocPointer = (e) => {
      if (this.menuOpen && !this.contains(e.target)) { this.menuOpen = false; }
    };
    this._onDocKey = (e) => {
      if (e.key === "Escape" && this.menuOpen) { this.menuOpen = false; }
    };
    this.registerAction("toggle-menu", () => { this.menuOpen = !this.menuOpen; });
    this.registerAction("logout", () => this._onLogout());
  }

  onConnect() {
    this.classList.add("sp-rail-profile");
    this.setAttribute("aria-label", "Profile and workspace");
    if (!this._baseVersion) {
      this._baseVersion = this.dataset.version || VERSION || "";
    }
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
    document.addEventListener("pointerdown", this._onDocPointer);
    document.addEventListener("keydown", this._onDocKey);
  }

  onDisconnect() {
    document.removeEventListener("pointerdown", this._onDocPointer);
    document.removeEventListener("keydown", this._onDocKey);
  }

  async _onLogout() {
    this.logoutError = await logout();
    if (!this.logoutError) { this.menuOpen = false; }
  }

  render() {
    const id = (this.snapshot && this.snapshot.verified_identity) || null;
    const signedIn = !!(id && (id.email || id.user_id));
    const tenant = id && id.tenant_id;
    const idLabel = (id && (id.email || id.user_id)) || "bridge workspace";
    const subBase = this._baseVersion;
    const sub = tenant ? `${tenant} · ${subBase}` : subBase;
    const logoutLabel = escapeHtml(t("rail-profile-logout") || "Log out");
    const open = this.menuOpen && signedIn;

    // Only a signed-in session has anything to offer here, so the trigger stays
    // inert (and unfocusable) otherwise rather than opening an empty menu.
    const menuMarkup = open
      ? `
        <div class="sp-rail-profile__menu" role="menu">
          <button class="sp-rail-profile__menu-item" type="button" role="menuitem" data-action="logout" data-l10n-id="rail-profile-logout">${logoutLabel}</button>
          ${this.logoutError ? `<p class="sp-rail-profile__menu-error">${escapeHtml(this.logoutError)}</p>` : ""}
        </div>
      `
      : "";

    return `
      <button class="sp-rail-profile__trigger" type="button" data-action="toggle-menu"
              ${signedIn ? "" : "disabled"}
              aria-haspopup="menu" aria-expanded="${open ? "true" : "false"}">
        <span class="sp-avatar__mark" aria-hidden="true"><span>${escapeHtml(initials(id && (id.email || id.user_id)))}</span></span>
        <span class="sp-rail-profile__meta">
          <span class="sp-rail-profile__id">${escapeHtml(idLabel)}</span>
          <span class="sp-rail-profile__sub">${escapeHtml(sub)}</span>
        </span>
        ${signedIn ? `<span class="sp-rail-profile__caret" aria-hidden="true">⌃</span>` : ""}
      </button>
      ${menuMarkup}
    `;
  }
}

reactive(SpRailProfile.prototype, ["snapshot", "menuOpen", "logoutError"]);
customElements.define("sp-rail-profile", SpRailProfile);
