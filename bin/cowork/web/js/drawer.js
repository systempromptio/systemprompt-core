import { $ } from "./dom.js?t=__TOKEN__";

export function append(line) {
  const log = $("log");
  if (log) {
    const ts = new Date().toLocaleTimeString();
    log.textContent += `\n[${ts}] ${line}`;
    log.scrollTop = log.scrollHeight;
  }
}

export function reportError(message) {
  append(message);
  console.error(message);
}
