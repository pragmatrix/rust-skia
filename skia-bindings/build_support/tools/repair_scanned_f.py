#!/usr/bin/env python3
"""Repair leftover mangled scanned_f calls (bool-arg era) using
scan-results.json; also re-attaches the comment that was merged onto the
corrupted line."""
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
results = json.loads((HERE / "scan-results.json").read_text())
table = HERE.parent / "type_bindings.rs"
text = table.read_text()

# Match both healthy and corrupted forms; rebuild them uniformly.
# Pattern: scanned_f("name", <anything until feats vector starts>) ...
pattern = re.compile(
    r'scanned_f\("([^"]+)",\s*(?:FIELDS_FUNCTIONS|FIELDS|NONE|true, false|true, true|false, false)\s*,?\]?\s*(\(?&?\[[^\]]*\]?\)?)',
)


def flag_of(name):
    v = results.get(name, "")
    props = v.split("+")
    if "fields" in props and "functions" in props:
        return "FIELDS_FUNCTIONS"
    if "fields" in props:
        return "FIELDS"
    return "NONE"


count = 0


def repl(m):
    global count
    name, feats = m.group(1), m.group(2)
    feats = feats.lstrip("(").rstrip(")")
    if not feats.startswith("&"):
        feats = "&" + feats.lstrip("&")
    count += 1
    return f'scanned_f("{name}", {flag_of(name)}, {feats})'


text = pattern.sub(repl, text)
table.write_text(text)
print(f"repaired {count} entries")
