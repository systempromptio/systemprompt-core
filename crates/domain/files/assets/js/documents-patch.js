/*
 * Patches rendered markup into a live subtree in place. Replacing innerHTML
 * on every event would discard focus, caret position, scroll offset and any
 * half-typed input, which is fatal in a co-editing surface that re-renders on
 * every remote update. Assumptions: nodes that can reorder carry `data-key`;
 * a custom element or a `[data-preserve]` node owns its children, so only its
 * attributes are synchronised; form values the user is editing are left alone
 * while the control has focus.
 */
const ownsChildren = (el) => el.localName.includes("-") || el.hasAttribute("data-preserve");

const syncAttributes = (live, next) => {
  for (const { name } of [...live.attributes]) {
    if (!next.hasAttribute(name)) { live.removeAttribute(name); }
  }
  for (const { name, value } of next.attributes) {
    if (live.getAttribute(name) !== value) { live.setAttribute(name, value); }
  }
  if ("value" in live && live !== document.activeElement && next.hasAttribute("value")) {
    live.value = next.getAttribute("value");
  }
};

const sameKind = (a, b) => a.nodeType === b.nodeType && a.nodeName === b.nodeName
  && (a.nodeType !== Node.ELEMENT_NODE || a.dataset.key === b.dataset.key);

const patchNode = (live, next) => {
  if (live.nodeType !== Node.ELEMENT_NODE) {
    if (live.nodeValue !== next.nodeValue) { live.nodeValue = next.nodeValue; }
    return;
  }
  syncAttributes(live, next);
  if (!ownsChildren(live)) { patchChildren(live, next); }
};

const keyed = (parent) => {
  const map = new Map();
  for (const child of parent.children) {
    if (child.dataset.key != null) { map.set(child.dataset.key, child); }
  }
  return map;
};

export const patchChildren = (live, next) => {
  const byKey = keyed(live);
  const incoming = [...next.childNodes];
  let cursor = live.firstChild;
  for (const node of incoming) {
    const key = node.nodeType === Node.ELEMENT_NODE ? node.dataset.key : undefined;
    const match = key != null ? byKey.get(key) : cursor;
    if (match && sameKind(match, node)) {
      if (match !== cursor) { live.insertBefore(match, cursor); }
      patchNode(match, node);
      cursor = match.nextSibling;
    } else {
      live.insertBefore(node, cursor);
    }
  }
  while (cursor) {
    const stale = cursor;
    cursor = cursor.nextSibling;
    stale.remove();
  }
};

export const patchHtml = (live, html) => {
  const template = document.createElement("template");
  template.innerHTML = html;
  patchChildren(live, template.content);
};
