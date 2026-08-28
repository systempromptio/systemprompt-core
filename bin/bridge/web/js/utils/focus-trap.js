// What `aria-modal="true"` promises, and what the drawer did not deliver.
//
// The agent drawer announced that the rest of the page was unavailable while
// leaving it fully reachable: Tab walked straight out of the dialog into the
// rail, the tabs and the activity log, all sitting behind a scrim. A false
// promise to assistive technology is worse than no promise, so the attribute
// and these two functions travel together.

const BACKGROUND = ["sp-topbar", "sp-rail", "main.sp-shell", "sp-activity-log"];

const FOCUSABLE = [
  "a[href]", "button:not([disabled])", "input:not([disabled])",
  "select:not([disabled])", "textarea:not([disabled])", "summary",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

function tabbable(root) {
  return Array.from(root.querySelectorAll(FOCUSABLE))
    .filter((el) => el.offsetParent !== null || el === document.activeElement);
}

/**
 * Cycles Tab and Shift+Tab inside `panel`. Returns true when it handled the
 * event, so a caller can leave every other key alone.
 */
export function trapTab(e, panel) {
  if (e.key !== "Tab" || !panel) { return false; }
  const items = tabbable(panel);
  if (items.length === 0) { return false; }
  const first = items[0];
  const last = items[items.length - 1];
  const active = document.activeElement;

  if (e.shiftKey && (active === first || !panel.contains(active))) {
    e.preventDefault();
    last.focus();
    return true;
  }
  if (!e.shiftKey && (active === last || !panel.contains(active))) {
    e.preventDefault();
    first.focus();
    return true;
  }
  return false;
}

/**
 * `inert` is what actually removes the background from the accessibility tree
 * and from the tab order; the scrim only stopped the mouse.
 */
export function setBackgroundInert(on, exclude) {
  for (const selector of BACKGROUND) {
    for (const el of document.querySelectorAll(selector)) {
      if (exclude && (el === exclude || el.contains(exclude))) { continue; }
      el.inert = on;
    }
  }
}
