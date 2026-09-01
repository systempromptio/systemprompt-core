import { subscribe } from "/assets/js/bridge.js";
import { subscribeSnapshot } from "/assets/js/services/state-store.js";
import { isReady as i18nReady } from "/assets/js/i18n.js";
import { patchChildren } from "/assets/js/components/reconcile.js";

export class SpElement extends HTMLElement {
  constructor() {
    super();
    this._unsubs = [];
    this._scheduled = false;
    this._connected = false;
    this._handlers = Object.create(null);
  }

  bridgeSubscribe(channel, cb) {
    const unsub = subscribe(channel, cb);
    this._unsubs.push(unsub);
    return unsub;
  }

  /** The shared snapshot: `cb` runs now if loaded, then on every change. */
  useSnapshot(cb) {
    const unsub = subscribeSnapshot(cb);
    this._unsubs.push(unsub);
    return unsub;
  }

  connectedCallback() {
    this._connected = true;
    if (typeof this.onConnect === "function") {
      this.onConnect();
    }
    this._renderNow();
    this._bindActions();
    if (!i18nReady()) {
      const onReady = () => this.invalidate();
      document.addEventListener("sp-i18n-ready", onReady, { once: true });
      this._unsubs.push(() => document.removeEventListener("sp-i18n-ready", onReady));
    }
  }

  disconnectedCallback() {
    this._connected = false;
    for (const u of this._unsubs) {
      try { u(); } catch (e) { console.error("SpElement teardown", e); }
    }
    this._unsubs = [];
    if (typeof this.onDisconnect === "function") {
      this.onDisconnect();
    }
  }

  invalidate() {
    if (this._scheduled || !this._connected) { return; }
    this._scheduled = true;
    queueMicrotask(() => {
      this._scheduled = false;
      if (this._connected) { this._renderNow(); }
    });
  }

  _renderNow() {
    if (typeof this.render !== "function") { return; }
    const out = this.render();
    if (typeof out === "string") {
      if (this._everRendered) {
        // Re-renders patch in place. Blowing away innerHTML on every state
        // event is what made the window flicker, and it also discarded scroll
        // position, focus, and half-edited form state on each probe tick.
        patchChildren(this, out);
      } else {
        // First paint replaces whatever server-rendered markup was in the light
        // DOM outright — there is nothing yet worth preserving, and it avoids
        // reconciling against markup this component did not author.
        this.innerHTML = out;
        this._everRendered = true;
      }
    }
    if (typeof this.afterRender === "function") {
      this.afterRender();
    }
  }

  registerAction(name, fn) {
    this._handlers[name] = fn;
  }

  _dispatch(e, dataKey, prefix) {
    const trigger = e.target.closest(`[data-${dataKey}]`);
    if (!trigger || !this.contains(trigger)) { return null; }
    const fn = this._handlers[`${prefix}${trigger.dataset[dataKey]}`];
    if (fn) { fn.call(this, trigger, e); }
    return trigger;
  }

  _bindActions() {
    if (this._actionsBound) { return; }
    this._actionsBound = true;
    this.addEventListener("click", (e) => this._dispatch(e, "action", ""));
    // Only click was bound, so a `data-action` on anything but a real <button>
    // was mouse-only -- which is how the whole marketplace pane, both its
    // category rail and its item list, became unreachable by keyboard.
    this.addEventListener("keydown", (e) => {
      if (e.key !== "Enter" && e.key !== " ") { return; }
      const trigger = e.target.closest("[data-action]");
      if (!trigger || !this.contains(trigger)) { return; }
      const tag = trigger.tagName;
      if (tag === "BUTTON" || tag === "A" || tag === "SUMMARY" || tag === "INPUT") { return; }
      if (!this._handlers[trigger.dataset.action]) { return; }
      e.preventDefault();
      this._dispatch(e, "action", "");
    });
    this.addEventListener("input", (e) => this._dispatch(e, "input", "input:"));
    // `change` is what a checkbox and a committed text field report. `input`
    // fires on every keystroke, which is the wrong moment to write anything to
    // disk and the wrong moment to call a form dirty.
    this.addEventListener("change", (e) => this._dispatch(e, "change", "change:"));
  }
}

export function reactive(proto, names) {
  for (const name of names) {
    const key = `__${name}`;
    Object.defineProperty(proto, name, {
      get() { return this[key]; },
      set(v) {
        if (this[key] === v) { return; }
        this[key] = v;
        this.invalidate();
      },
      configurable: true,
      enumerable: true,
    });
  }
}
