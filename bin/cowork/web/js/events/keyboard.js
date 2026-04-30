import { $ } from "../dom.js?t=__TOKEN__";
import { activateTab } from "../tabs.js?t=__TOKEN__";

const TAB_KEYS = {
  "1": "marketplace",
  "2": "status",
  "3": "settings",
};

function isTextInput(target) {
  if (!target) {
    return false;
  } else {
    return target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;
  }
}

function handleKeydown(e) {
  const mod = e.metaKey || e.ctrlKey;
  if (!mod) {
    return;
  }
  if (e.key === "f") {
    const search = $("mkt-search");
    if (search) {
      e.preventDefault();
      search.focus();
      search.select();
    }
  } else if (TAB_KEYS[e.key]) {
    if (!isTextInput(e.target)) {
      e.preventDefault();
      activateTab(TAB_KEYS[e.key]);
    }
  } else {
    void 0;
  }
}

export function initKeyboard() {
  document.addEventListener("keydown", handleKeydown);
}
