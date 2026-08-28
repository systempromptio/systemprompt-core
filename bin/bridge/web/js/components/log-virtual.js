const ROW_HEIGHT = 18;
const VIEWPORT_OVERSCAN = 10;
export const DEFAULT_CAPACITY = 20000;

export function createLogVirtual(rootEl, { capacity = DEFAULT_CAPACITY, initial = [] } = {}) {
  const viewport = rootEl.querySelector(".sp-log-virtual__viewport");
  const spacerTop = rootEl.querySelector(".sp-log-virtual__spacer-top");
  const spacerBottom = rootEl.querySelector(".sp-log-virtual__spacer-bottom");
  if (!viewport || !spacerTop || !spacerBottom) {
    throw new Error("log-virtual: missing required child elements");
  }

  let source = initial.slice(-capacity);
  let buffer = source;
  let predicate = null;
  let stickyTail = true;
  let scheduled = false;

  function reproject() {
    buffer = predicate ? source.filter(predicate) : source;
  }

  function render() {
    scheduled = false;
    const containerHeight = rootEl.clientHeight || 1;
    const scrollTop = rootEl.scrollTop;
    const visibleCount = Math.ceil(containerHeight / ROW_HEIGHT) + VIEWPORT_OVERSCAN;
    const startIdx = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - VIEWPORT_OVERSCAN);
    const endIdx = Math.min(buffer.length, startIdx + visibleCount);

    spacerTop.style.height = `${startIdx * ROW_HEIGHT}px`;
    spacerBottom.style.height = `${Math.max(0, buffer.length - endIdx) * ROW_HEIGHT}px`;

    const slice = buffer.slice(startIdx, endIdx);
    const frag = document.createDocumentFragment();
    slice.forEach((entry, i) => {
      const li = document.createElement("li");
      li.className = `sp-log__line sp-log__line--${entry.level || "info"}`;
      li.textContent = entry.text;
      li.dataset.action = "expand-line";
      li.dataset.index = String(startIdx + i);
      li.tabIndex = 0;
      frag.append(li);
    });
    viewport.replaceChildren(frag);
  }

  function schedule() {
    if (scheduled) return;
    scheduled = true;
    requestAnimationFrame(render);
  }

  rootEl.addEventListener("scroll", () => {
    const atBottom = rootEl.scrollHeight - rootEl.scrollTop - rootEl.clientHeight < 4;
    stickyTail = atBottom;
    schedule();
  });

  function normalize(entry) {
    return typeof entry === "string"
      ? { text: entry, level: "info" }
      : { text: entry.text || entry.line || String(entry), level: entry.level || "info", meta: entry.meta };
  }

  // Replaces the whole buffer in one pass. Backfilling history and re-running a
  // search both go through here, so neither is a thousand separate appends.
  function setAll(entries) {
    source = entries.map(normalize).slice(-capacity);
    reproject();
    stickyTail = true;
    schedule();
    requestAnimationFrame(() => { rootEl.scrollTop = rootEl.scrollHeight; });
  }

  /** Re-window the existing buffer against a predicate; null clears the filter. */
  function setFilter(fn) {
    predicate = fn || null;
    reproject();
    schedule();
  }

  function append(entry) {
    const normalized = normalize(entry);
    source.push(normalized);
    if (source.length > capacity) {
      source.splice(0, source.length - capacity);
    }
    reproject();
    if (stickyTail) {
      schedule();
      requestAnimationFrame(() => {
        rootEl.scrollTop = rootEl.scrollHeight;
      });
    } else {
      schedule();
    }
  }

  function clear() {
    source = [];
    reproject();
    schedule();
  }

  function entries() {
    return buffer.slice();
  }

  schedule();
  return { append, setAll, setFilter, clear, entries, root: rootEl };
}
