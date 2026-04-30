export function syncRailIndicator() {
  const rail = document.querySelector(".sp-rail");
  if (rail) {
    const active = rail.querySelector('.sp-rail-tab[data-tab][aria-selected="true"]');
    if (active) {
      const railRect = rail.getBoundingClientRect();
      const tabRect = active.getBoundingClientRect();
      const y = (tabRect.top - railRect.top) + rail.scrollTop;
      rail.style.setProperty("--sp-rail-active-y", `${y}px`);
      rail.style.setProperty("--sp-rail-active-h", `${tabRect.height}px`);
      rail.dataset.activeReady = "true";
    } else {
      rail.dataset.activeReady = "false";
    }
  }
}

export function initRailIndicator() {
  window.addEventListener("resize", syncRailIndicator);
}
