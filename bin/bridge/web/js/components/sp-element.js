import { subscribe } from "/assets/js/bridge.js";

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

  connectedCallback() {
    this._connected = true;
    if (typeof this.onConnect === "function") {
      this.onConnect();
    }
    this._renderNow();
    this._bindActions();
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
      this.innerHTML = out;
    }
    if (typeof this.afterRender === "function") {
      this.afterRender();
    }
  }

  registerAction(name, fn) {
    this._handlers[name] = fn;
  }

  _bindActions() {
    if (this._actionsBound) { return; }
    this._actionsBound = true;
    this.addEventListener("click", (e) => {
      const trigger = e.target.closest("[data-action]");
      if (trigger && this.contains(trigger)) {
        const fn = this._handlers[trigger.dataset.action];
        if (fn) { fn.call(this, trigger, e); }
      }
    });
    this.addEventListener("input", (e) => {
      const trigger = e.target.closest("[data-input]");
      if (trigger && this.contains(trigger)) {
        const fn = this._handlers[`input:${trigger.dataset.input}`];
        if (fn) { fn.call(this, trigger, e); }
      }
    });
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

export function escapeHtml(s) {
  if (s == null) { return ""; }
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function attr(name, value) {
  if (value == null || value === false) { return ""; }
  if (value === true) { return name; }
  return `${name}="${escapeHtml(value)}"`;
}
