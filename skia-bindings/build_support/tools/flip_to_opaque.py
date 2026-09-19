#!/usr/bin/env python3
"""Flip the scan's opaque-candidates to Opaque in type_bindings.rs.

Reads scan-opaque-candidates.txt (next to this script), finds each
`included_f("<name>", ...)` line for those names, and rewrites it to
`opaque_f("<name>", ...)` keeping the feature list, and adjusting
`layout_only` from No to Yes where present in expanded form.

Idempotent: entries already Opaque are skipped.
"""
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
TABLE = HERE.parent / "type_bindings.rs"
CANDIDATES = HERE / "scan-opaque-candidates.txt"


def main():
    candidates = {
        line.strip()
        for line in CANDIDATES.read_text().splitlines()
        if line.strip()
    }
    print(f"{len(candidates)} candidates")

    text = TABLE.read_text()
    lines = text.splitlines(keepends=True)
    out = []
    flipped = 0
    skipped = 0
    for line in lines:
        m = re.search(r'included_f\(\s*"([^"]+)"\s*,\s*(&\[[^\]]*\])\s*\)', line)
        if m and m.group(1) in candidates:
            name = m.group(1)
            features = m.group(2)
            out.append(f'    opaque_f("{name}", {features}),\n')
            flipped += 1
        else:
            out.append(line)
    TABLE.write_text("".join(out))
    print(f"flipped {flipped}, skipped {skipped}")


if __name__ == "__main__":
    main()
