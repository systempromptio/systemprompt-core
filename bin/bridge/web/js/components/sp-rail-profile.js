import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { bridge } from "/assets/js/bridge.js";
import { notifyErr } from "/assets/js/utils/notify.js";
import { handleRovingKey, syncRoving } from "/assets/js/utils/roving.js";
import { logout } from "/assets/js/services/session-service.js";
import { profileInitials, railProfileSubtitle } from "/assets/js/utils/profile-format.js";
import {
  UPDATE_RECHECK_MS, updateStateOf, isUpdateBusy, maybeCheckForUpdate, installUpdate, restartForUpdate,
} from "/assets/js/utils/update-check.js";
import {
  positionRailProfileMenu, renderRailProfileMenu, renderRailProfileCta, railProfileTriggerLabel,
} from "/assets/js/utils/rail-profile-menu.js";

const VERSION = (() => {
  const tag = document.querySelector('meta[name="sp-version"]');
  return (tag && tag.content) || "";
})();

export class SpRailProfile extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.menuOpen = false;
    this.logoutError = "";
    this._baseVersion = "";
    this._checkedAt = 0;
    this._recheckTimer = null;
    this._onDocPointer = (e) => {
      if (this.menuOpen && !this.contains(e.target)) { this.menuOpen = false; }
    };
    this._onDocKey = (e) => {
      if (e.key === "Escape" && this.menuOpen) { this.menuOpen = false; }
    };
    // role="menu" is a claim that arrow keys work. They did not, and nothing
    // moved focus into the menu or back to the trigger.
    this._onMenuKey = (e) => {
      if (!this.menuOpen) { return; }
      const items = this._menuItems();
      handleRovingKey(e, items, items.indexOf(document.activeElement));
    };
    // Capture phase: the rail itself scrolls, and a scroll event on it does not
    // bubble to window.
    this._onReposition = () => {
      if (this.menuOpen) { positionRailProfileMenu(this); }
    };
    this._registerActions();
  }

  _registerActions() {
    this.registerAction("toggle-menu", () => {
      this.menuOpen = !this.menuOpen;
      this._needsMenuFocus = this.menuOpen;
      if (!this.menuOpen) { this._returnFocusToTrigger(); }
    });
    this.registerAction("logout", () => this._onLogout());
    this.registerAction("update-install", () => installUpdate(this));
    this.registerAction("update-restart", () => restartForUpdate());
    this.registerAction("open-external", (el, ev) => {
      const url = el && el.dataset && el.dataset.href;
      if (!url) { return; }
      if (ev && typeof ev.preventDefault === "function") { ev.preventDefault(); }
      bridge.openExternalUrl(url).catch((e) => notifyErr(e, url));
    });
  }

  onConnect() {
    this.classList.add("sp-rail-profile");
    if (!this._baseVersion) {
      this._baseVersion = this.dataset.version || VERSION || "";
    }
    this.useSnapshot((s) => { this.snapshot = s; maybeCheckForUpdate(this); });
    document.addEventListener("pointerdown", this._onDocPointer);
    document.addEventListener("keydown", this._onDocKey);
    this.addEventListener("keydown", this._onMenuKey);
    window.addEventListener("resize", this._onReposition);
    window.addEventListener("scroll", this._onReposition, true);
    this._recheckTimer = setInterval(() => { this._checkedAt = 0; maybeCheckForUpdate(this); }, UPDATE_RECHECK_MS);
  }

  onDisconnect() {
    document.removeEventListener("pointerdown", this._onDocPointer);
    document.removeEventListener("keydown", this._onDocKey);
    window.removeEventListener("resize", this._onReposition);
    window.removeEventListener("scroll", this._onReposition, true);
    if (this._recheckTimer) { clearInterval(this._recheckTimer); this._recheckTimer = null; }
  }

  _menuItems() {
    return Array.from(this.querySelectorAll(".sp-rail-profile__menu-item"));
  }

  _returnFocusToTrigger() {
    const trigger = this.querySelector('[data-action="toggle-menu"]');
    if (trigger && trigger.isConnected) { trigger.focus(); }
  }

  afterRender() {
    positionRailProfileMenu(this);
    const items = this._menuItems();
    if (items.length === 0) { return; }
    syncRoving(items, 0);
    if (this._needsMenuFocus) { this._needsMenuFocus = false; items[0].focus(); }
  }

  async _onLogout() {
    this.logoutError = await logout();
    if (!this.logoutError) { this.menuOpen = false; }
  }

  render() {
    const id = (this.snapshot && this.snapshot.verified_identity) || null;
    const signedIn = !!(id && (id.email || id.user_id));
    const idLabel = (id && (id.email || id.user_id)) || "bridge workspace";
    const update = updateStateOf(this.snapshot);
    const open = this.menuOpen && signedIn;
    return `
      ${renderRailProfileCta(update, signedIn)}
      <button class="sp-rail-profile__trigger${isUpdateBusy(update) ? " is-busy" : ""}" type="button" data-action="toggle-menu"
              ${signedIn ? "" : "disabled"}
              aria-label="${escapeHtml(railProfileTriggerLabel(signedIn, idLabel))}"
              aria-haspopup="menu" aria-expanded="${open ? "true" : "false"}">
        <span class="sp-avatar__mark" aria-hidden="true"><span>${escapeHtml(profileInitials(id && (id.email || id.user_id)))}</span></span>
        <span class="sp-rail-profile__meta">
          <span class="sp-rail-profile__id">${escapeHtml(idLabel)}</span>
          <span class="sp-rail-profile__sub">${escapeHtml(railProfileSubtitle(update, this._baseVersion, id && id.tenant_id))}</span>
        </span>
        ${signedIn ? `<span class="sp-rail-profile__caret" aria-hidden="true">⌃</span>` : ""}
      </button>
      ${renderRailProfileMenu(update, open, this.logoutError)}
    `;
  }
}

reactive(SpRailProfile.prototype, ["snapshot", "menuOpen", "logoutError"]);
customElements.define("sp-rail-profile", SpRailProfile);
