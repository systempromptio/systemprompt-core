"""The unit table, scripts/units.txt: every package outside the test workspace
as `kernel`, `capability:<name>` or `tooling`.

Read by the affected-set selector (scope and `--check`) and the flow report
(kernel churn); the kernel path prefixes are derived from it through the cargo
graph, never listed a second time.
"""
from pathlib import Path
import re

UNITS = Path(__file__).resolve().parents[1] / "units.txt"
UNIT_PATTERN = re.compile(r"^(kernel|tooling|capability:[a-z][a-z0-9-]*)$")


class UnitsError(Exception):
    """A malformed scripts/units.txt row."""


def parse_units(text):
    units = {}
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        fields = line.split()
        if len(fields) != 2 or not UNIT_PATTERN.match(fields[1]):
            raise UnitsError(f"units line {number}: expected `<package> kernel|tooling|capability:<name>`")
        if fields[0] in units:
            raise UnitsError(f"units line {number}: {fields[0]} listed twice")
        units[fields[0]] = fields[1]
    return units


def load_units(path=UNITS):
    return parse_units(Path(path).read_text())


def unit_dirs(graph, units, unit):
    """Package directories of `graph` (`cargo_graph.repo_graph` shape) in `unit`, sorted."""
    return sorted(directory for directory, node in graph.items() if units.get(node["name"]) == unit)
