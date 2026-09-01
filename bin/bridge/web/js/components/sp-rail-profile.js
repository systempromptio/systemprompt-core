import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { notifyOk, notifyErr } from "/assets/js/utils/notify.js";
import { t } from "/assets/js/i18n.js";
import { handleRovingKey, syncRoving } from "/assets/js/utils/roving.js";
import { logout } from "/assets/js/services/session-service.js";

const VERSION = (() => {
  const tag = document.querySelector('meta[name="sp-version"]');
  return (tag && tag.content) || "";
})();

/** Re-check interval. Updates are a background nicety, not a live feed. */
const RECHECK_MS = 6 * 60 * 60 * 1000;

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
      const cur = items.indexOf(document.activeElement);
      handleRovingKey(e, items, cur);
    };
    // Capture phase: the rail itself scrolls, and a scroll event on it does not
    // bubble to window.
    this._onReposition = () => {
      if (this.menuOpen) { this._positionMenu(); }
    };
    this.registerAction("toggle-menu", () => {
      this.menuOpen = !this.menuOpen;
      this._needsMenuFocus = this.menuOpen;
      if (!this.menuOpen) { this._returnFocusToTrigger(); }
    });
    this.registerAction("logout", () => this._onLogout());
    this.registerAction("update-install", () => this._onUpdateInstall());
    this.registerAction("update-restart", () => {
      notifyOk(t("toast-update-restarting") || "Restarting to finish the update…");
      bridge.updateRestart().catch((e) => notifyErr(e, t("rail-profile-restart-cta") || "Restart to finish updating"));
    });
    this.registerAction("open-external", (el, ev) => {
      const url = el && el.dataset && el.dataset.href;
      if (!url) { return; }
      if (ev && typeof ev.preventDefault === "function") { ev.preventDefault(); }
      bridge.openExternalUrl(url).catch((e) => notifyErr(e, url));
    });
  }

  /** The update phase carried on the state snapshot; see `UpdateUiState`. */
  get _update() {
    return (this.snapshot && this.snapshot.update) || { phase: "unknown" };
  }

  /** True while the update is mid-flight and the trigger must not act again. */
  get _updateBusy() {
    return ["downloading", "installing"].includes(this._update.phase);
  }

  onConnect() {
    this.classList.add("sp-rail-profile");
    if (!this._baseVersion) {
      this._baseVersion = this.dataset.version || VERSION || "";
    }
    this.useSnapshot((s) => { this.snapshot = s; this._maybeCheck(); });
    document.addEventListener("pointerdown", this._onDocPointer);
    document.addEventListener("keydown", this._onDocKey);
    this.addEventListener("keydown", this._onMenuKey);
    window.addEventListener("resize", this._onReposition);
    window.addEventListener("scroll", this._onReposition, true);
    this._recheckTimer = setInterval(() => { this._checkedAt = 0; this._maybeCheck(); }, RECHECK_MS);
  }

  onDisconnect() {
    document.removeEventListener("pointerdown", this._onDocPointer);
    document.removeEventListener("keydown", this._onDocKey);
    window.removeEventListener("resize", this._onReposition);
    window.removeEventListener("scroll", this._onReposition, true);
    if (this._recheckTimer) { clearInterval(this._recheckTimer); this._recheckTimer = null; }
  }

  /**
   * Checks once the gateway probe has settled and we are actually signed in —
   * the endpoint is authenticated, so checking earlier just logs a 401. Cheap
   * to call on every snapshot: the timestamp guard collapses the repeats.
   */
  _maybeCheck() {
    const snap = this.snapshot;
    if (!snap || !snap.signed_in) { return; }
    if (this._updateBusy || this._update.can_restart) { return; }
    const now = Date.now();
    if (this._checkedAt && now - this._checkedAt < RECHECK_MS) { return; }
    this._checkedAt = now;
    bridge.updateCheck().catch((e) => console.debug("update check failed", e));
  }

  _menuItems() {
    return Array.from(this.querySelectorAll(".sp-rail-profile__menu-item"));
  }

  _returnFocusToTrigger() {
    const trigger = this.querySelector('[data-action="toggle-menu"]');
    if (trigger && trigger.isConnected) { trigger.focus(); }
  }

  afterRender() {
    this._positionMenu();
    const items = this._menuItems();
    if (items.length === 0) { return; }
    syncRoving(items, 0);
    if (this._needsMenuFocus) { this._needsMenuFocus = false; items[0].focus(); }
  }

  // Why: the rail is a scroll container (`.sp-rail { overflow-y: auto }`), so
  // the absolutely-positioned menu opening upward gets clipped into the rail's
  // scroll overflow on short windows — leaving "Log out" rendered but
  // unreachable. Re-anchor the menu to the viewport (fixed) so no ancestor
  // overflow can ever swallow it; the stylesheet's absolute placement remains
  // only as a fallback for the frame this runs in. Called on resize and scroll
  // too, since a fixed menu does not travel with its trigger.
  _positionMenu() {
    const menu = this.querySelector(".sp-rail-profile__menu");
    const trigger = this.querySelector(".sp-rail-profile__trigger");
    if (!menu || !trigger) { return; }
    const r = trigger.getBoundingClientRect();
    menu.style.position = "fixed";
    menu.style.left = `${Math.max(4, r.left)}px`;
    menu.style.right = "auto";
    menu.style.bottom = `${Math.max(4, window.innerHeight - r.top + 4)}px`;
    menu.style.minWidth = `${Math.max(140, r.width)}px`;
  }

  async _onLogout() {
    this.logoutError = await logout();
    if (!this.logoutError) { this.menuOpen = false; }
  }

  _onUpdateInstall() {
    if (this._updateBusy) { return; }
    this.menuOpen = false;
    // Progress and failure both arrive on `state.changed`, so nothing to do
    // with the resolved value here.
    bridge.updateInstall().catch((e) => notifyErr(e, t("rail-profile-update-cta") || "Click here to update"));
  }

  /// The subtitle doubles as the update progress line; when nothing is
  /// happening it falls back to the usual `tenant · version`.
  _subtitle(tenant) {
    const u = this._update;
    // Why: only the phases that have a line carry a key; every other phase
    // falls through to the version line. The update payload is the arguments.
    const line = (u.in_progress || u.tone === "err") ? t(`update-phase-${u.phase}`, u) : "";
    if (line) { return line; }
    const base = this._baseVersion;
    return tenant ? `${tenant} · ${base}` : base;
  }

  render() {
    const id = (this.snapshot && this.snapshot.verified_identity) || null;
    const signedIn = !!(id && (id.email || id.user_id));
    const tenant = id && id.tenant_id;
    const idLabel = (id && (id.email || id.user_id)) || "bridge workspace";
    const logoutLabel = escapeHtml(t("rail-profile-logout") || "Log out");
    const u = this._update;
    const open = this.menuOpen && signedIn;

    // An available or installed update turns the whole control into the call to
    // action; the identity and Log out stay reachable through the menu, because
    // this is the only place either is offered.
    const cta = u.can_install
      ? { action: "update-install", label: t("rail-profile-update-cta") || "Click here to update", sub: `v${u.version}` }
      : u.can_restart
        ? { action: "update-restart", label: t("rail-profile-restart-cta") || "Restart to finish updating", sub: `v${u.version}` }
        : null;

    const menuItems = [];
    if (u.can_install) {
      menuItems.push(`<button class="sp-rail-profile__menu-item" type="button" role="menuitem" data-action="update-install">${escapeHtml((t("rail-profile-update-to") || "Update to") + ` v${u.version}`)}</button>`);
    }
    if (u.can_restart) {
      menuItems.push(`<button class="sp-rail-profile__menu-item" type="button" role="menuitem" data-action="update-restart">${escapeHtml(t("rail-profile-restart-cta") || "Restart to finish updating")}</button>`);
    }
    if (u.can_install && u.notes_url) {
      menuItems.push(`<a class="sp-rail-profile__menu-item" role="menuitem" href="${escapeHtml(u.notes_url)}" data-href="${escapeHtml(u.notes_url)}" data-action="open-external">${escapeHtml(t("rail-profile-release-notes") || "Release notes")}</a>`);
    }
    menuItems.push(`<button class="sp-rail-profile__menu-item" type="button" role="menuitem" data-action="logout" data-l10n-id="rail-profile-logout">${logoutLabel}</button>`);

    // Only a signed-in session has anything to offer here, so the trigger stays
    // inert (and unfocusable) otherwise rather than opening an empty menu.
    const menuMarkup = open
      ? `
        <div class="sp-rail-profile__menu" role="menu">
          ${menuItems.join("")}
          ${this.logoutError ? `<p class="sp-rail-profile__menu-error">${escapeHtml(this.logoutError)}</p>` : ""}
        </div>
      `
      : "";

    // The CTA is its own button rather than a re-labelled trigger so the menu
    // (and Log out with it) keeps a target of its own.
    const ctaMarkup = cta && signedIn
      ? `
        <button class="sp-rail-profile__cta" type="button" data-action="${cta.action}">
          <span class="sp-rail-profile__cta-label">${escapeHtml(cta.label)}</span>
          <span class="sp-rail-profile__cta-sub">${escapeHtml(cta.sub)}</span>
        </button>
      `
      : "";

    // Why: the visible identity lives in `.sp-rail-profile__meta`, which the
    // icon-only breakpoint sets to `display: none` -- that removes it from the
    // accessibility tree, not just from view, leaving the button unnamed. The
    // name has to be on the button itself.
    const triggerLabel = signedIn
      ? `${t("rail-profile-aria") || "Account and workspace"} \u2014 ${idLabel}`
      : (t("rail-profile-aria") || "Account and workspace");

    return `
      ${ctaMarkup}
      <button class="sp-rail-profile__trigger${this._updateBusy ? " is-busy" : ""}" type="button" data-action="toggle-menu"
              ${signedIn ? "" : "disabled"}
              aria-label="${escapeHtml(triggerLabel)}"
              aria-haspopup="menu" aria-expanded="${open ? "true" : "false"}">
        <span class="sp-avatar__mark" aria-hidden="true"><span>${escapeHtml(initials(id && (id.email || id.user_id)))}</span></span>
        <span class="sp-rail-profile__meta">
          <span class="sp-rail-profile__id">${escapeHtml(idLabel)}</span>
          <span class="sp-rail-profile__sub">${escapeHtml(this._subtitle(tenant))}</span>
        </span>
        ${signedIn ? `<span class="sp-rail-profile__caret" aria-hidden="true">⌃</span>` : ""}
      </button>
      ${menuMarkup}
    `;
  }
}

reactive(SpRailProfile.prototype, ["snapshot", "menuOpen", "logoutError"]);
customElements.define("sp-rail-profile", SpRailProfile);
