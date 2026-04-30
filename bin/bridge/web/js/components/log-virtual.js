const ROW_HEIGHT = 18;
const VIEWPORT_OVERSCAN = 10;
const DEFAULT_CAPACITY = 20000;

export function createLogVirtual(rootEl, capacity = DEFAULT_CAPACITY) {
  const viewport = rootEl.querySelector(".sp-log-virtual__viewport");
  const spacerTop = rootEl.querySelector(".sp-log-virtual__spacer-top");
  const spacerBottom = rootEl.querySelector(".sp-log-virtual__spacer-bottom");
  if (!viewport || !spacerTop || !spacerBottom) {
    throw new Error("log-virtual: missing required child elements");
  }

  const buffer = [];
  let stickyTail = true;
  let scheduled = false;

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
    for (const entry of slice) {
      const li = document.createElement("li");
      li.className = `sp-log__line sp-log__line--${entry.level || "info"}`;
      li.textContent = entry.text;
      frag.appendChild(li);
    }
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

  function append(entry) {
    const normalized = typeof entry === "string"
      ? { text: entry, level: "info" }
      : { text: entry.text || entry.line || String(entry), level: entry.level || "info" };
    buffer.push(normalized);
    if (buffer.length > capacity) {
      buffer.splice(0, buffer.length - capacity);
    }
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
    buffer.length = 0;
    schedule();
  }

  schedule();
  return { append, clear };
}
