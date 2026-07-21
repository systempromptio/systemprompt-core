import { subscribe } from "/assets/js/bridge.js";
import { isReady as i18nReady } from "/assets/js/i18n.js";

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

// --- DOM reconciliation ------------------------------------------------------
//
// A deliberately small keyed patcher. It exists to stop `innerHTML = html` from
// destroying and rebuilding every subtree on every state event; it is not a
// general virtual DOM and makes two assumptions that hold across this app:
//
//  1. A custom element (any tag containing "-") owns its own children. We sync
//     its attributes and stop — recursing would wipe content the child rendered
//     for itself.
//  2. `data-preserve` marks an element whose content is written imperatively
//     rather than by render() (a virtual list, a live counter). Same treatment:
//     attributes only. Without it, the reconciler would overwrite that content
//     with the empty placeholder the template carries.
//  3. Nodes carrying `data-key` are matched by key across renders; everything
//     else is matched positionally.

/** Does this element own its own children, leaving us to patch attributes only? */
function ownsChildren(el) {
  return el.tagName.includes("-") || el.hasAttribute("data-preserve");
}

function keyOf(node) {
  return node.nodeType === Node.ELEMENT_NODE ? node.getAttribute("data-key") : null;
}

/** Can `oldNode` be patched into `newNode`, or must it be replaced outright? */
function isCompatible(oldNode, newNode) {
  if (!oldNode || oldNode.nodeType !== newNode.nodeType) { return false; }
  if (newNode.nodeType !== Node.ELEMENT_NODE) { return true; }
  return oldNode.tagName === newNode.tagName && keyOf(oldNode) === keyOf(newNode);
}

function patchChildren(parent, html) {
  // A fresh template per call: `<template>` content tolerates fragments that a
  // <div> would drop (bare <tr>, <td>), and a local one cannot be clobbered by
  // a nested render.
  const parsed = document.createElement("template");
  parsed.innerHTML = html;
  reconcile(parent, parsed.content);
}

function reconcile(parent, source) {
  const incoming = Array.from(source.childNodes);
  // Keyed survivors are looked up by key so a reorder moves the existing
  // element (and its state) instead of rebuilding it.
  const keyed = new Map();
  for (const child of parent.childNodes) {
    const key = keyOf(child);
    if (key !== null) { keyed.set(key, child); }
  }

  let cursor = parent.firstChild;
  for (const next of incoming) {
    const key = keyOf(next);
    const match = key !== null ? keyed.get(key) : cursor;

    if (match && isCompatible(match, next)) {
      if (match !== cursor) { parent.insertBefore(match, cursor); }
      else { cursor = cursor.nextSibling; }
      patchNode(match, next);
      if (key !== null) { keyed.delete(key); }
    } else {
      parent.insertBefore(next, cursor);
    }
  }

  // Anything still ahead of the cursor, plus unclaimed keyed nodes, is gone.
  while (cursor) {
    const doomed = cursor;
    cursor = cursor.nextSibling;
    doomed.remove();
  }
  for (const orphan of keyed.values()) { orphan.remove(); }
}

function patchNode(oldNode, newNode) {
  if (newNode.nodeType !== Node.ELEMENT_NODE) {
    if (oldNode.nodeValue !== newNode.nodeValue) { oldNode.nodeValue = newNode.nodeValue; }
    return;
  }
  patchAttributes(oldNode, newNode);
  // Assumption 1: custom elements render their own subtree.
  if (!isCustomElement(oldNode)) { reconcile(oldNode, newNode); }
}

function patchAttributes(oldEl, newEl) {
  for (const { name, value } of Array.from(newEl.attributes)) {
    if (oldEl.getAttribute(name) !== value) {
      oldEl.setAttribute(name, value);
      syncProperty(oldEl, name, value);
    }
  }
  for (const { name } of Array.from(oldEl.attributes)) {
    if (!newEl.hasAttribute(name)) {
      oldEl.removeAttribute(name);
      syncProperty(oldEl, name, null);
    }
  }
}

// Form controls diverge from their attributes as soon as the user touches them.
// We only reach here when the *rendered* attribute actually changed, so pushing
// it onto the property is an intentional state update, not a clobber of an
// in-progress edit.
function syncProperty(el, name, value) {
  if (name === "checked") { el.checked = value !== null; }
  else if (name === "disabled") { el.disabled = value !== null; }
  else if (name === "value" && value !== null && el.value !== value) { el.value = value; }
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
