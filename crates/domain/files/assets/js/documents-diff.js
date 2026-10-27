const leafLines = (nodes, out = new Map()) => {
  for (const node of nodes ?? []) {
    if (node.children?.length) {
      leafLines(node.children, out);
    } else {
      out.set(node.id, { id: node.id, hash: node.hash8, text: node.text ?? "" });
    }
  }
  return out;
};

export const diffProjections = (before, after) => {
  const left = leafLines(before.root);
  const right = leafLines(after.root);
  const lines = [];
  for (const [id, line] of right) {
    const old = left.get(id);
    if (!old) {
      lines.push({ ...line, change: "added" });
    } else if (old.hash !== line.hash) {
      lines.push({ ...line, change: "changed", was: old.text });
    }
  }
  for (const [id, line] of left) {
    if (!right.has(id)) { lines.push({ ...line, change: "removed" }); }
  }
  return lines;
};
