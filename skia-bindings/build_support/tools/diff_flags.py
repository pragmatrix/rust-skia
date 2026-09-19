#!/usr/bin/env python3
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
results = json.loads((HERE / "scan-results.json").read_text())
text = (HERE.parent / "type_bindings.rs").read_text()
pattern = re.compile(
    r'scanned_f\("([^"]+)", (FIELDS_FUNCTIONS|FIELDS|NONE|FUNCTIONS), ?(&?\[[^\]]*\])\)'
)
diffs = 0
for m in pattern.finditer(text):
    name, flag, feats = m.groups()
    v = results.get(name)
    if v is None:
        continue
    props = set(v.split("+"))
    if props == {"fields", "functions"}:
        want = "FIELDS_FUNCTIONS"
    elif props == {"fields"}:
        want = "FIELDS"
    elif "functions" in props:
        want = "FUNCTIONS"
    else:
        want = "NONE"
    if want != flag:
        diffs += 1
        if diffs <= 6:
            print(name, "table:", flag, "scan:", v, "->", want)
print("total diffs:", diffs)
