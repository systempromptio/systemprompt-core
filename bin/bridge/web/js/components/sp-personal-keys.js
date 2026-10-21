// Settings section for personal provider keys (personal subscriptions). The
// gateway verifies a key upstream before storing it; a stored or removed key
// re-syncs every host, so OpenCode's model list follows within seconds.

import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { notifyOk } from "/assets/js/utils/notify.js";
import { renderPersonalKeys } from "/assets/js/components/personal-keys-parts.js";

function failure(error) {
  const reason = (error && error.detail && error.detail.reason) || "gateway_error";
  return { reason, message: (error && error.message) || (t("personal-keys-error-generic") || "The gateway did not answer.") };
}

export class SpPersonalKeys extends SpElement {
  constructor() {
    super();
    this.rows = null;
    this.loadError = null;
    this.gatewayUrl = "";
    this._ui = Object.create(null);
    this.registerAction("input:pk-key", (el) => {
      const ui = this.ui(el.dataset.provider);
      ui.editing = true; ui.draft = el.value; ui.error = null; this.invalidate();
    });
    this.registerAction("pk-save", (el) => this._save(el.dataset.provider));
    this.registerAction("pk-retry", (el) => (el.dataset.provider ? this._save(el.dataset.provider) : this._load()));
    this.registerAction("pk-remove", (el) => this._remove(el.dataset.provider));
    this.registerAction("pk-reload", () => this._load());
    this.registerAction("pk-edit", (el) => this._edit(el.dataset.provider));
    this.registerAction("pk-sign-in", () => bridge.signIn(this.gatewayUrl, true).then(() => this._load(), (e) => { this.loadError = failure(e); }));
    this.addEventListener("focusin", (e) => {
      const provider = e.target && e.target.dataset ? e.target.dataset.provider : null;
      if (e.target.dataset.input === "pk-key" && provider && !this.ui(provider).editing) { this._edit(provider); }
    });
  }

  ui(provider) {
    if (!this._ui[provider]) { this._ui[provider] = { editing: false, draft: "", busy: false, error: null }; }
    return this._ui[provider];
  }

  onConnect() {
    this.useSnapshot((s) => { this.gatewayUrl = (s && s.gateway_url) || ""; });
    this._load();
  }

  async _load() {
    try {
      this.rows = await bridge.personalKeysList();
      this.loadError = null;
    } catch (e) {
      this.loadError = failure(e);
    }
    this.invalidate();
  }

  _edit(provider) {
    Object.assign(this.ui(provider), { editing: true, draft: "", error: null });
    this.invalidate();
    setTimeout(() => { const input = this.querySelector(`#pk-${CSS.escape(provider)}`); if (input) { input.focus(); } }, 0);
  }

  async _save(provider) {
    const ui = this.ui(provider);
    const key = (ui.draft || "").trim();
    if (!key) { this._edit(provider); return; }
    ui.busy = true; ui.error = null; this.invalidate();
    try {
      const row = await bridge.personalKeySet(provider, key, null);
      Object.assign(ui, { editing: false, draft: "" });
      this.rows = (this.rows || []).map((r) => (r.provider === provider ? row : r));
      notifyOk(t("toast-personal-key-saved") || "Key verified and saved. Your agents are being updated.");
    } catch (e) {
      ui.error = failure(e);
    } finally {
      ui.busy = false; this.invalidate();
    }
  }

  async _remove(provider) {
    const ui = this.ui(provider);
    ui.busy = true; ui.error = null; this.invalidate();
    try {
      await bridge.personalKeyRemove(provider);
      notifyOk(t("toast-personal-key-removed") || "Key removed.");
      await this._load();
    } catch (e) {
      ui.error = failure(e);
    } finally {
      ui.busy = false; this.invalidate();
    }
  }

  render() { return renderPersonalKeys(this); }
}

reactive(SpPersonalKeys.prototype, ["rows", "loadError"]);
customElements.define("sp-personal-keys", SpPersonalKeys);
