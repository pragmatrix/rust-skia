#!/usr/bin/env python3
"""Insert the missing commas after concatenated repaired calls."""
import re
from pathlib import Path

table = Path(__file__).resolve().parent.parent / "type_bindings.rs"
text = table.read_text()

text = re.sub(
    r"(scanned_f\([^)]*?\))(?!\s*,)(\s*(?:scanned_f|opaque_f|excluded|included|stub|//))",
    r"\1,\2",
    text,
)
table.write_text(text)
print("commas fixed")
