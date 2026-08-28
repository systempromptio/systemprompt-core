// Arrow-key navigation for a composite widget — a tablist, a listbox, a menu.
//
// Three surfaces needed this and only the rail had it, privately: the
// marketplace category rail gave every item `tabindex="0"` and no key handler,
// which is a five-stop keyboard trap you can enter and cannot use, and the
// marketplace item list and the profile menu had no keyboard story at all.

const HORIZONTAL = new Set(["ArrowRight", "ArrowLeft"]);
const VERTICAL = new Set(["ArrowDown", "ArrowUp"]);
const EDGES = new Set(["Home", "End"]);

/**
 * Moves the roving `tabindex` to `items[index]` and focuses it. Everything that
 * owns a composite widget calls this rather than setting tabindex by hand, so
 * exactly one item is ever in the page's tab order.
 */
export function focusItem(items, index) {
  const target = items[index];
  if (!target) { return null; }
  for (const el of items) { el.tabIndex = el === target ? 0 : -1; }
  target.focus();
  return target;
}

export function syncRoving(items, activeIndex) {
  const active = activeIndex >= 0 ? activeIndex : 0;
  for (let i = 0; i < items.length; i += 1) { items[i].tabIndex = i === active ? 0 : -1; }
}

/**
 * `orientation` decides which arrow pair moves the cursor; the other pair is
 * left to the browser so a vertical list does not swallow horizontal scrolling.
 * `onMove` receives the newly focused element — pass the selection handler for a
 * tablist (selection follows focus) and omit it for a listbox where Enter
 * commits.
 */
export function handleRovingKey(e, items, currentIndex, { orientation = "vertical", onMove } = {}) {
  const moves = orientation === "horizontal" ? HORIZONTAL : VERTICAL;
  if (!moves.has(e.key) && !EDGES.has(e.key)) { return false; }
  if (items.length === 0) { return false; }

  const cur = currentIndex < 0 ? -1 : currentIndex;
  let next;
  if (e.key === "Home") { next = 0; }
  else if (e.key === "End") { next = items.length - 1; }
  else if (e.key === "ArrowDown" || e.key === "ArrowRight") {
    next = cur < 0 ? 0 : (cur + 1) % items.length;
  } else {
    next = cur < 0 ? items.length - 1 : (cur - 1 + items.length) % items.length;
  }

  e.preventDefault();
  const target = focusItem(items, next);
  if (target && onMove) { onMove(target, next); }
  return true;
}
