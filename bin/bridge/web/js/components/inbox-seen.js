import { bridge } from "/assets/js/bridge.js";

// Why: the gateway records a notification as seen only when a person has the
// inbox in front of them. A hidden tab panel has no box, so the observer stays
// silent until the reader opens the Inbox tab in a visible window; the bridge
// drops ids it has already reported, so a repeat report costs no request.
export function watchSeen(component) {
  let isShown = false;
  const report = async () => {
    if (!isShown || document.visibilityState !== "visible") { return; }
    const ids = (component.inbox?.items ?? []).map((item) => item.id);
    if (ids.length === 0) { return; }
    try {
      await bridge.invoke("inbox.seen", { ids });
    } catch (err) {
      console.error("inbox.seen failed", err);
    }
  };
  const observer = new IntersectionObserver((entries) => {
    isShown = entries.some((entry) => entry.isIntersecting);
    report();
  });
  observer.observe(component);
  document.addEventListener("visibilitychange", report);
  const stop = () => {
    observer.disconnect();
    document.removeEventListener("visibilitychange", report);
  };
  return { report, stop };
}
