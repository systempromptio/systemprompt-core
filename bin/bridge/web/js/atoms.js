export function atom(initial) {
  let value = initial;
  const subs = new Set();
  return {
    get value() { return value; },
    set value(next) {
      if (value === next) return;
      value = next;
      for (const cb of Array.from(subs)) {
        try { cb(value); } catch (e) { console.error("atom subscriber threw", e); }
      }
    },
    subscribe(cb) {
      subs.add(cb);
      cb(value);
      return () => subs.delete(cb);
    },
  };
}

export const gatewayAtom         = atom({ state: "unknown" });
export const identityAtom        = atom(null);
export const signedInAtom        = atom(false);
export const agentsOnboardedAtom = atom(false);
