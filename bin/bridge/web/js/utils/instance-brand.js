const THEME_LINK_ID = "instance-theme";

// The window title and remote theme follow the active instance; the compiled
// brand painted the first frame, so this only replaces what differs.
export function applyInstanceBrand(snap) {
  if (!snap) return;
  if (snap.window_title && document.title !== snap.window_title) {
    document.title = snap.window_title;
  }
  const existing = document.getElementById(THEME_LINK_ID);
  const href = snap.theme_css_url || "";
  if (!href) {
    if (existing) existing.remove();
    return;
  }
  if (existing && existing.getAttribute("href") === href) return;
  const link = existing || document.createElement("link");
  link.id = THEME_LINK_ID;
  link.rel = "stylesheet";
  link.href = href;
  if (!existing) document.head.append(link);
}
