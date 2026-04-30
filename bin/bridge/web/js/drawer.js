import { $ } from "./dom.js?t=__TOKEN__";
import { createLogVirtual } from "./components/log-virtual.js?t=__TOKEN__";

let toastTimer = null;
let lastShownMessage = "";
let logVirtual = null;

function ensureLog() {
  if (logVirtual) return logVirtual;
  const root = $("log");
  if (!root || !root.classList.contains("sp-log-virtual")) return null;
  try {
    logVirtual = createLogVirtual(root);
  } catch (e) {
    console.error("log-virtual init failed", e);
    return null;
  }
  return logVirtual;
}

function classifyLevel(line) {
  return /(fail|error|refused|denied|reject)/i.test(line)
    ? "error"
    : /(warn)/i.test(line)
      ? "warn"
      : "info";
}

export function append(line) {
  const v = ensureLog();
  if (!v) return;
  const ts = new Date().toLocaleTimeString();
  v.append({ text: `[${ts}] ${line}`, level: classifyLevel(line) });
}

export function showToast(message, kind = "info", durationMs = 6000) {
  const toast = $("sp-toast");
  const msgEl = $("sp-toast-msg");
  if (!toast || !msgEl || !message) {
    return;
  }
  msgEl.textContent = message;
  toast.dataset.kind = kind;
  toast.hidden = false;
  clearTimeout(toastTimer);
  if (durationMs > 0) {
    toastTimer = setTimeout(hideToast, durationMs);
  }
}

export function hideToast() {
  const toast = $("sp-toast");
  if (toast) {
    toast.hidden = true;
  }
  clearTimeout(toastTimer);
  toastTimer = null;
}

export function reportError(message) {
  append(message);
  showToast(message, "error", 8000);
  console.error(message);
}

export function syncToastFromState(snap) {
  const msg = snap && snap.last_action_message;
  if (!msg || msg === lastShownMessage) {
    return;
  }
  lastShownMessage = msg;
  const isFailure = /(fail|error|refused|unreachable|invalid|reject|denied)/i.test(msg);
  showToast(msg, isFailure ? "error" : "info", isFailure ? 8000 : 4000);
}

export function initToast() {
  const close = $("sp-toast-close");
  if (close) {
    close.addEventListener("click", hideToast);
  }
  ensureLog();
  if (logVirtual) {
    logVirtual.append({ text: "Ready.", level: "info" });
  }
}
