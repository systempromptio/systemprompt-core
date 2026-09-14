import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { trapTab, setBackgroundInert } from "/assets/js/utils/focus-trap.js";

const COPY = {
  disconnect: {
    title: () => t("device-disconnect-title") || "Disconnect this computer?",
    body: () => t("device-disconnect-body") || "Removes managed integrations and scheduled synchronization. Your sign-in and settings are kept so you can reconnect later.",
    confirm: () => t("device-disconnect-confirm") || "Disconnect computer",
  },
  purge: {
    title: () => t("device-purge-title") || "Reset Bridge completely?",
    body: () => t("device-purge-body") || "Removes integrations, credentials, identity, configuration, managed files and saved state. The app returns to fresh setup.",
    confirm: () => t("device-purge-confirm") || "Reset everything",
  },
  "remove-application": {
    title: () => t("device-remove-title") || "Remove Bridge application?",
    body: () => t("device-remove-body") || "First removes all Bridge integrations and local data, then shows the installed application so you can remove it safely.",
    confirm: () => t("device-remove-confirm") || "Clean up and continue",
  },
};

export class SpDeviceAction extends SpElement {
  constructor() {
    super();
    this.action = null;
    this.busy = false;
    this.error = "";
    this.guidance = null;
    this._previousFocus = null;
    this.registerAction("cancel", () => this._dismiss());
    this.registerAction("confirm", () => this._confirm());
    this.registerAction("reveal", () => bridge.revealApplication().catch((e) => { this.error = String(e); }));
    this.registerAction("quit", () => bridge.quit());
    this._onKey = (e) => {
      if (!this.action) { return; }
      const panel = this.querySelector(".sp-device-action__panel");
      if (trapTab(e, panel)) { return; }
      if (e.key === "Escape" && !this.busy) { this._dismiss(); }
    };
  }

  onConnect() {
    this.useSnapshot((snapshot) => {
      const next = snapshot && snapshot.pending_device_action;
      if (!this.busy && !this.guidance) { this.action = next || null; }
    });
    document.addEventListener("keydown", this._onKey);
  }

  onDisconnect() {
    document.removeEventListener("keydown", this._onKey);
    this._setOpen(false);
  }

  _setOpen(open) {
    document.body.classList.toggle("is-device-action-open", open);
    setBackgroundInert(open, this);
  }

  async _dismiss() {
    if (this.busy) { return; }
    await bridge.deviceActionDismiss().catch(() => {});
    this.action = null;
    this.guidance = null;
    this.error = "";
    if (this._previousFocus && this._previousFocus.isConnected) { this._previousFocus.focus(); }
  }

  async _confirm() {
    if (this.busy || !this.action) { return; }
    this.busy = true;
    this.error = "";
    try {
      if (this.action === "disconnect") { await bridge.systemDisconnect(); }
      else if (this.action === "purge") { await bridge.systemPurge(); }
      else {
        const guidance = await bridge.removalGuidance();
        await bridge.systemPurge();
        this.guidance = guidance;
      }
      if (!this.guidance) { this.action = null; }
    } catch (e) {
      this.error = String(e && e.message ? e.message : e);
    } finally {
      this.busy = false;
    }
  }

  afterRender() {
    const open = !!this.action;
    if (open && !this._wasOpen) {
      this._previousFocus = document.activeElement;
      queueMicrotask(() => this.querySelector("button")?.focus());
    }
    this._setOpen(open);
    this._wasOpen = open;
  }

  render() {
    if (!this.action) { return ""; }
    const copy = COPY[this.action] || COPY.purge;
    const guide = this.guidance ? this._renderGuidance() : "";
    return `<section class="sp-device-action" role="presentation">
      <div class="sp-device-action__panel" role="dialog" aria-modal="true" aria-labelledby="device-action-title">
        <h2 id="device-action-title">${escapeHtml(this.guidance ? (t("device-remove-guide-title") || "Finish removing Bridge") : copy.title())}</h2>
        ${guide || `<p>${escapeHtml(copy.body())}</p>`}
        ${this.error ? `<p class="sp-device-action__error" role="alert">${escapeHtml(this.error)}</p>` : ""}
        <div class="sp-device-action__actions">${this.guidance
          ? `<button class="sp-btn-primary" data-action="reveal">${escapeHtml(t("device-remove-reveal") || "Show application")}</button><button class="sp-btn-danger" data-action="quit">${escapeHtml(t("device-remove-quit") || "Quit Bridge")}</button>`
          : `<button class="sp-btn-danger" data-action="confirm" ${this.busy ? "disabled" : ""}>${escapeHtml(this.busy ? (t("device-action-working") || "Working…") : copy.confirm())}</button>`}
          <button class="sp-btn-ghost" data-action="cancel" ${this.busy ? "disabled" : ""}>${escapeHtml(this.guidance ? (t("device-action-done") || "Done") : (t("device-action-cancel") || "Cancel"))}</button>
        </div>
      </div></section>`;
  }

  _renderGuidance() {
    const method = this.guidance.method;
    const message = method === "scoop"
      ? (t("device-remove-guide-scoop") || "Quit Bridge, then run “scoop uninstall bridge” in PowerShell.")
      : method === "macos"
        ? (t("device-remove-guide-macos") || "Show the application in Finder, quit Bridge, then move it to Trash.")
        : (t("device-remove-guide-standalone") || "Show the application, quit Bridge, then delete it using the same method you used to install it.");
    return `<p>${escapeHtml(message)}</p><p class="sp-device-action__path">${escapeHtml(this.guidance.path || "")}</p>`;
  }
}

reactive(SpDeviceAction.prototype, ["action", "busy", "error", "guidance"]);
customElements.define("sp-device-action", SpDeviceAction);
