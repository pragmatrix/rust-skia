#!/usr/bin/env python3
"""One-shot: rewrite scanned_f flag arguments in type_bindings.rs from
scan-results.json verdicts."""
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
results = json.loads((HERE / "scan-results.json").read_text())
table = HERE.parent / "type_bindings.rs"
text = table.read_text()

pattern = re.compile(
    r'scanned_f\("([^"]+)", (FIELDS_FUNCTIONS|FIELDS|NONE|FUNCTIONS), ?(&?\[[^\]]*\])\)'
)
count = 0


def repl(m):
    global count
    name, old_flag, feats = m.groups()
    v = results.get(name)
    if v is None:
        return m.group(0)
    props = set(v.split("+"))
    if props == {"fields", "functions"}:
        flag = "FIELDS_FUNCTIONS"
    elif "functions" in props:
        flag = "FUNCTIONS"
    elif "fields" in props:
        flag = "FIELDS"
    else:
        flag = "NONE"
    if flag != old_flag:
        count += 1
    return f'scanned_f("{name}", {flag}, {feats})'


table.write_text(pattern.sub(repl, text))
print("flag changes:", count)
