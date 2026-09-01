// --- DOM reconciliation ------------------------------------------------------
//
// A deliberately small keyed patcher. It exists to stop `innerHTML = html` from
// destroying and rebuilding every subtree on every state event; it is not a
// general virtual DOM and makes three assumptions that hold across this app:
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

export function patchChildren(parent, html) {
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

  removeLeftovers(cursor, keyed);
}

// Anything still ahead of the cursor, plus unclaimed keyed nodes, is gone.
function removeLeftovers(cursor, keyed) {
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
  if (!ownsChildren(oldNode)) { reconcile(oldNode, newNode); }
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
