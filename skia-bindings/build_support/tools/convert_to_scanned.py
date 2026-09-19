#!/usr/bin/env python3
"""One-shot: convert scanned `included_f` entries in type_bindings.rs to
`scanned_f` using the verdicts in the SCAN block. Keeps verdicts with an
extra `functions` prop as Scan::FieldsFunctions, the rest Scan::Fields."""
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
TABLE = HERE.parent / "type_bindings.rs"

text = TABLE.read_text()

# verdicts from the SCAN block
verdicts = {}
for m in re.finditer(r"// SCAN: (.+?) = (.+)", text):
    verdicts[m.group(1)] = m.group(2)

def scan_rust(verdict):
    props = set(verdict.split("+"))
    if "functions" in props:
        return "Scan::FieldsFunctions"
    return "Scan::Fields"

count = 0
def repl(m):
    global count
    name, sp, feats = m.group(1), m.group(2), m.group(3)
    if name not in verdicts:
        return m.group(0)
    count += 1
    return f'scanned_f("{name}",{sp}{scan_rust(verdicts[name])},{sp}{feats})'

pattern = re.compile(
    r'included_f\(\s*"([^"]+)"\s*(\s*)((?:&?)\[[^\]]*\])\s*,?\s*\)',
    re.S,
)
new_text = pattern.sub(repl, text)

TABLE.write_text(new_text)
print(f"converted {count} entries")
