#!/usr/bin/env python3
"""Write side-by-side.html and summary.tsv from a parity ledger.

Reads <out>/ledger.json and <out>/traces (parity-ledger's output) and the
reference runner's index.json, and writes one self-contained page: the totals,
a row per scenario and protocol, then per run the reference agent's normalised
trace beside the harness's, the fields that differ marked with the class of
the difference and its reviewed reason. summary.tsv holds the same rows.

Exits 1 when a difference is unclassified, a defect is open or a run could not
be compared. Standard library only; side-by-side.sh calls it last.
"""

import argparse
import html
import json
import os
import re
import sys

VS_REFERENCE = "harness-vs-reference"
VS_EXPECTED = "harness-vs-expected"
CLASSES = ["same", "intended", "gap", "defect", "reference-defect", "unclassified"]
VERDICTS = CLASSES + ["error", "skipped"]
OPEN = ("unclassified", "defect", "error")

STYLE = """
:root { color-scheme: light dark; --bg: #ffffff; --fg: #1b1f24; --dim: #5c6670; --line: #d5dae0;
  --panel: #f5f7f9; --same: #1a7f37; --intended: #0969da; --gap: #9a6700; --defect: #cf222e;
  --reference-defect: #8250df; --unclassified: #bf3989; --error: #cf222e; --skipped: #5c6670; }
@media (prefers-color-scheme: dark) { :root { --bg: #0f1317; --fg: #e3e7eb; --dim: #97a1ab;
  --line: #2c343c; --panel: #171c22; --same: #3fb950; --intended: #58a6ff; --gap: #d29922;
  --defect: #ff7b72; --reference-defect: #bc8cff; --unclassified: #f778ba; --error: #ff7b72;
  --skipped: #97a1ab; } }
* { box-sizing: border-box; }
body { margin: 0 auto; padding: 24px; max-width: 1500px; background: var(--bg); color: var(--fg);
  font: 14px/1.45 system-ui, -apple-system, "Segoe UI", sans-serif; }
h1 { font-size: 22px; margin: 0 0 4px; } h2 { font-size: 17px; margin: 40px 0 6px; }
h3 { font-size: 14px; margin: 18px 0 6px; } p { margin: 6px 0; } .dim { color: var(--dim); }
code, pre { font: 12px/1.5 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
table { border-collapse: collapse; width: 100%; }
th, td { border: 1px solid var(--line); padding: 5px 8px; text-align: left; vertical-align: top; }
th { background: var(--panel); font-weight: 600; } td.n { text-align: right; font-variant-numeric: tabular-nums; }
.wrap { overflow-x: auto; } a { color: var(--intended); }
.tag { display: inline-block; padding: 0 7px; border-radius: 9px; border: 1px solid currentColor;
  font-size: 12px; font-weight: 600; white-space: nowrap; }
.totals { display: flex; flex-wrap: wrap; gap: 8px; margin: 12px 0 20px; }
.totals div { border: 1px solid var(--line); border-radius: 6px; padding: 6px 12px; background: var(--panel); }
.totals b { display: block; font-size: 20px; font-variant-numeric: tabular-nums; }
table.trace { table-layout: fixed; } table.trace th:first-child { width: 110px; }
table.trace pre { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
table.trace tr.differs td { background: var(--panel); }
mark { background: transparent; color: inherit; border-radius: 3px; padding: 0 2px;
  outline: 2px solid currentColor; outline-offset: -1px; font-weight: 600; }
td.value pre { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; max-height: 12em; overflow-y: auto; }
""" + "".join(".c-%s { color: var(--%s); }\n" % (name, name) for name in VERDICTS)


def esc(value):
    return html.escape(str(value), quote=True)


def tag(name):
    return '<span class="tag c-%s">%s</span>' % (esc(name), esc(name))


def load(path, default=None):
    try:
        with open(path, encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, ValueError):
        return default


def title_of(scenarios, scenario):
    try:
        with open(os.path.join(scenarios, scenario, "scenario.toml"), encoding="utf-8") as handle:
            found = re.search(r'^title\s*=\s*"(.*)"\s*$', handle.read(), re.M)
    except OSError:
        return ""
    return found.group(1) if found else ""


def scalar_list(value):
    return isinstance(value, list) and not any(isinstance(item, (dict, list)) for item in value)


class Absent:
    pass


ABSENT = Absent()


def render(mine, other, path, marks, depth=0):
    """One side of a node as marked-up text. A path the ledger names is marked whole."""
    pad = "  " * depth
    if path in marks:
        shown = "(absent)" if mine is ABSENT else json.dumps(mine, indent=2, sort_keys=True, ensure_ascii=False)
        shown = shown.replace("\n", "\n" + pad)
        return '<mark class="c-%s" title="%s">%s</mark>' % (esc(marks[path]), esc(path), esc(shown))
    if mine is ABSENT:
        return '<span class="dim">(absent)</span>'
    if isinstance(mine, dict):
        theirs = other if isinstance(other, dict) else {}
        keys = list(mine) + [key for key in theirs if key not in mine]
        if not keys:
            return "{}"
        lines = []
        for key in keys:
            child = "%s.%s" % (path, key) if path else key
            if key not in mine and child not in marks:
                continue
            value = render(mine.get(key, ABSENT), theirs.get(key, ABSENT), child, marks, depth + 1)
            lines.append("%s  %s: %s" % (pad, esc(json.dumps(key, ensure_ascii=False)), value))
        return "{\n%s\n%s}" % ("\n".join(lines), pad)
    if isinstance(mine, list) and not scalar_list(mine):
        theirs = other if isinstance(other, list) else []
        lines = []
        for at in range(max(len(mine), len(theirs))):
            child = "%s[%d]" % (path, at)
            if at >= len(mine) and child not in marks:
                continue
            value = render(
                mine[at] if at < len(mine) else ABSENT, theirs[at] if at < len(theirs) else ABSENT, child, marks, depth + 1
            )
            lines.append("%s  %s" % (pad, value))
        return "[\n%s\n%s]" % ("\n".join(lines), pad)
    return esc(json.dumps(mine, sort_keys=True, ensure_ascii=False))


def blocks(reference, harness):
    """The parts of two traces shown as one table row each: every step, then every other key."""
    listed = []
    steps = [side.get("steps") if isinstance(side.get("steps"), list) else [] for side in (reference, harness)]
    for at in range(max(len(steps[0]), len(steps[1]))):
        listed.append(
            ("steps[%d]" % at, steps[0][at] if at < len(steps[0]) else ABSENT, steps[1][at] if at < len(steps[1]) else ABSENT)
        )
    for key in list(reference) + [key for key in harness if key not in reference]:
        if key != "steps":
            listed.append((key, reference.get(key, ABSENT), harness.get(key, ABSENT)))
    return listed


def touches(path, block):
    return path == block or path.startswith(block + ".") or path.startswith(block + "[")


def kind(step):
    return step.get("kind", "") if isinstance(step, dict) else ""


def trace_table(reference, harness, marks):
    rows = []
    for path, theirs, ours in blocks(reference, harness):
        differs = any(touches(marked, path) for marked in marks)
        label = esc(path)
        kinds = sorted({kind(theirs), kind(ours)} - {""})
        if kinds:
            label += '<br><span class="dim">%s</span>' % esc(" / ".join(kinds))
        rows.append(
            '<tr%s><th>%s</th><td><pre>%s</pre></td><td><pre>%s</pre></td></tr>'
            % (' class="differs"' if differs else "", label, render(theirs, ours, path, marks), render(ours, theirs, path, marks))
        )
    return (
        '<div class="wrap"><table class="trace"><thead><tr><th></th><th>Reference agent</th><th>Harness</th></tr></thead>'
        "<tbody>%s</tbody></table></div>" % "".join(rows)
    )


def value_cell(value):
    return '<td class="value"><pre>%s</pre></td>' % esc(json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False))


def differences_table(entries, first, second, flip):
    """The ledger's entries; its "expected" is the harness's value in the reference comparison."""
    rows = []
    for entry in entries:
        one, other = (entry.get("actual"), entry.get("expected")) if flip else (entry.get("expected"), entry.get("actual"))
        rows.append(
            "<tr><td><code>%s</code></td><td>%s</td>%s%s<td>%s</td></tr>"
            % (
                esc(entry.get("path", "")),
                tag(entry.get("class", "unclassified")),
                value_cell(one),
                value_cell(other),
                esc(entry.get("reason", "")) or '<span class="dim">not reviewed</span>',
            )
        )
    return (
        '<div class="wrap"><table><thead><tr><th>Field</th><th>Class</th><th>%s</th><th>%s</th><th>Reason</th></tr></thead>'
        "<tbody>%s</tbody></table></div>" % (esc(first), esc(second), "".join(rows))
    )


def gather(out, reference_dir):
    ledger = load(os.path.join(out, "ledger.json"))
    if ledger is None:
        sys.exit("render: %s holds no ledger.json" % out)
    index = load(os.path.join(reference_dir, "index.json"), {}) or {}
    runs = {}

    def run(scenario, protocol):
        return runs.setdefault((scenario, protocol), {"scenario": scenario, "protocol": protocol})

    for row in ledger.get("rows", []):
        entry = run(row["scenario"], row["protocol"])
        entry["reference" if row["comparison"] == VS_REFERENCE else "expected"] = row
    for item in index.get("runs", []):
        key = (item.get("scenario"), item.get("protocol"))
        if key in runs:
            runs[key]["exported"] = item
    return ledger, index, [runs[key] for key in sorted(runs)]


def judge(run):
    """The run's verdict against the reference agent, and a note where there is no comparison."""
    exported = run.get("exported") or {}
    row = run.get("reference")
    if row is not None:
        return row.get("verdict", "error"), row.get("error") or ""
    if (run.get("expected") or {}).get("result") == "error":
        return "error", "the harness run failed: %s" % run["expected"].get("error", "")
    if exported.get("status") == "skipped":
        return "skipped", exported.get("reason", "")
    if exported:
        return "error", "the reference run was not exported: %s" % exported.get("reason", "")
    return "error", "the reference runner has no entry for this run"


def counts(row):
    found = dict.fromkeys(CLASSES[1:], 0)
    for entry in (row or {}).get("entries", []):
        found[entry.get("class", "unclassified")] = found.get(entry.get("class", "unclassified"), 0) + 1
    return found


def section(run, out, anchor, title):
    verdict, note = judge(run)
    parts = [
        '<h2 id="%s">%s <span class="dim">%s</span> %s</h2>'
        % (anchor, esc(run["scenario"]), esc(run["protocol"]), tag(verdict))
    ]
    if title:
        parts.append('<p class="dim">%s</p>' % esc(title))
    if note:
        parts.append("<p>%s</p>" % esc(note))
    findings = (run.get("exported") or {}).get("findings") or []
    if findings:
        parts.append("<p>Reference runner findings: %s</p>" % esc("; ".join(findings)))
    row = run.get("reference")
    traces = os.path.join(out, "traces", run["scenario"], run["protocol"])
    reference, harness = load(os.path.join(traces, "reference.json")), load(os.path.join(traces, "harness.json"))
    if row is not None and isinstance(reference, dict) and isinstance(harness, dict):
        entries = row.get("entries", [])
        marks = {entry["path"]: entry.get("class", "unclassified") for entry in entries}
        parts.append(trace_table(reference, harness, marks))
        if entries:
            parts.append("<h3>Differences from the reference agent</h3>")
            parts.append(differences_table(entries, "Reference agent", "Harness", True))
    expected = (run.get("expected") or {}).get("entries", [])
    if expected:
        parts.append("<h3>Differences of the harness from the expected trace</h3>")
        parts.append(differences_table(expected, "Expected", "Harness", False))
    return "\n".join(parts)


def total_boxes(caption, totals):
    runs, differences = totals.get("runs", {}), totals.get("differences", {})
    boxes = ["<div><b>%d</b>runs</div>" % sum(runs.values())]
    for name in VERDICTS:
        if name in runs or name in ("same", "unclassified", "defect"):
            boxes.append('<div class="c-%s"><b>%d</b>%s</div>' % (name, runs.get(name, 0), esc(name)))
    listed = ", ".join("%d %s" % (differences[name], name) for name in CLASSES if differences.get(name))
    return "<h3>%s</h3><div class=\"totals\">%s</div><p class=\"dim\">Differences by class: %s</p>" % (
        esc(caption),
        "".join(boxes),
        esc(listed or "none"),
    )


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--out", required=True, help="the directory parity-ledger wrote; the page is written beside the ledger")
    parser.add_argument("--reference", help="the reference runner's export (default: <out>/reference)")
    parser.add_argument("--scenarios", required=True, help="the corpus directory, for the scenario titles")
    options = parser.parse_args(argv)
    out = os.path.abspath(options.out)
    ledger, index, runs = gather(out, options.reference or os.path.join(out, "reference"))

    verdicts = {}
    summary, sections, lines = [], [], []
    header = ["scenario", "protocol", "reference_run", "verdict", "differences"] + CLASSES[1:] + ["expected_verdict", "note"]
    for run in runs:
        verdict, note = judge(run)
        verdicts[verdict] = verdicts.get(verdict, 0) + 1
        found = counts(run.get("reference"))
        expected = (run.get("expected") or {}).get("verdict", "")
        status = (run.get("exported") or {}).get("status", "missing")
        anchor = "run-%s-%s" % (re.sub(r"[^A-Za-z0-9-]", "-", run["scenario"]), run["protocol"])
        cells = "".join('<td class="n">%s</td>' % (found[name] or "") for name in CLASSES[1:])
        summary.append(
            '<tr><td><a href="#%s">%s</a></td><td>%s</td><td>%s</td><td>%s</td>%s<td>%s</td><td>%s</td></tr>'
            % (anchor, esc(run["scenario"]), esc(run["protocol"]), esc(status), tag(verdict), cells, tag(expected) if expected else "", esc(note))
        )
        sections.append(section(run, out, anchor, title_of(options.scenarios, run["scenario"])))
        row = [run["scenario"], run["protocol"], status, verdict, sum(found.values())]
        row += [found[name] for name in CLASSES[1:]] + [expected, note]
        lines.append("\t".join(re.sub(r"\s+", " ", str(cell)) for cell in row))

    totals = ledger.get("totals", {})
    against_reference = dict(totals.get(VS_REFERENCE, {}))
    against_reference["runs"] = verdicts
    agent = index.get("agent", {})
    page = [
        "<!doctype html>",
        '<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">',
        "<title>Parity: the harness beside the reference agent</title><style>%s</style></head><body>" % STYLE,
        "<h1>The harness beside the reference agent</h1>",
        '<p class="dim">Reference agent: %s, executable sha256 %s. Both products ran the golden scenarios against the '
        "scripted instance; each trace below is the normalised one the comparator compares.</p>"
        % (esc(agent.get("version", "unknown")), esc(agent.get("sha256", "unknown"))),
        total_boxes("Against the reference agent", against_reference),
        total_boxes("The harness against the expected traces", totals.get(VS_EXPECTED, {})),
        '<div class="wrap"><table><thead><tr><th>Scenario</th><th>Protocol</th><th>Reference run</th><th>Verdict</th>'
        + "".join("<th>%s</th>" % esc(name) for name in CLASSES[1:])
        + "<th>Against expected</th><th>Note</th></tr></thead><tbody>%s</tbody></table></div>" % "".join(summary),
        "\n".join(sections),
        "</body></html>",
    ]
    with open(os.path.join(out, "side-by-side.html"), "w", encoding="utf-8") as handle:
        handle.write("\n".join(page) + "\n")
    with open(os.path.join(out, "summary.tsv"), "w", encoding="utf-8") as handle:
        handle.write("\t".join(header) + "\n" + "".join(line + "\n" for line in lines))

    expected_runs = totals.get(VS_EXPECTED, {}).get("runs", {})
    print("against the reference agent: %s" % (", ".join("%d %s" % (verdicts[name], name) for name in VERDICTS if name in verdicts) or "no runs"))
    print("against the expected traces: %s" % (", ".join("%d %s" % (expected_runs[name], name) for name in VERDICTS if name in expected_runs) or "no runs"))
    print("written: %s" % os.path.join(out, "side-by-side.html"))
    if not runs:
        print("no run was compared", file=sys.stderr)
        return 1
    open_runs = sum(verdicts.get(name, 0) + expected_runs.get(name, 0) for name in OPEN)
    open_entries = sum(
        totals.get(comparison, {}).get("differences", {}).get(name, 0) for comparison in (VS_REFERENCE, VS_EXPECTED) for name in ("unclassified", "defect")
    )
    if open_runs or open_entries:
        print("open: an unclassified difference, a defect or a run that could not be compared remains", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
