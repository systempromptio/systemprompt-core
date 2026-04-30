import { $ } from "./dom.js?t=__TOKEN__";
import { TAB_LABELS } from "./tabs.js?t=__TOKEN__";

export function setCrumb(name) {
  const crumb = $("crumb-current");
  if (crumb) {
    const label = TAB_LABELS[name] || name || "";
    if (crumb.textContent !== label) {
      const nav = $("crumb");
      if (nav) {
        nav.dataset.changing = "true";
      }
      setTimeout(() => {
        crumb.textContent = label;
        if (nav) {
          nav.dataset.changing = "false";
        }
      }, 120);
    }
  }
}
