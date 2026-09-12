#!/usr/bin/env python3
"""Strip local-toolchain settings from the committed cargo configs before a CI build.

The committed test config selects the unstable codegen backend and forces the
local sccache wrapper; the stable CI toolchain has neither, and CI sets its own
RUSTC_WRAPPER from the job environment. Every job that compiles must apply the
same surgery, or their sccache objects diverge.
"""

import pathlib
import re

for path in (".cargo/config.toml", "crates/tests/.cargo/config.toml"):
    p = pathlib.Path(path)
    if not p.exists():
        continue
    s = p.read_text()
    s = re.sub(r"\n\[unstable\][^\[]*", "\n", s, flags=re.S)
    s = re.sub(r"\n\[profile\.dev\][^\[]*", "\n", s, flags=re.S)
    s = re.sub(r"\nRUSTC_WRAPPER\s*=.*", "", s)
    p.write_text(s)
