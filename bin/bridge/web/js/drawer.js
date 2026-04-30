import { $ } from "./dom.js?t=__TOKEN__";

let toastTimer = null;
let lastShownMessage = "";

export function append(line) {
  const log = $("log");
  if (log) {
    const ts = new Date().toLocaleTimeString();
    log.textContent += `\n[${ts}] ${line}`;
    log.scrollTop = log.scrollHeight;
  }
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
}
