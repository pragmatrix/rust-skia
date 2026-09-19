"""Convert all stub_generic entries to included_f, removing definitions.

The stub_generic approach failed: its hard-coded struct sizes are
system/libc++-dependent. With the vendored bindgen (patches/bindgen-0.73.2,
wired via [patch.crates-io]) the anonymous-template-parameter panic is fixed
at the root, so the std:: templates can be plain Include — bindgen resolves
and sizes them itself, per target.
"""
import re
from pathlib import Path

p = Path(__file__).resolve().parent.parent / "type_bindings.rs"
text = p.read_text()
lines = text.splitlines(keepends=True)

out = []
i = 0
converted = 0
while i < len(lines):
    line = lines[i]
    if re.match(r"\s*stub_generic\(\s*$", line):
        # gather lines until the closing "),"
        j = i
        while j < len(lines) and not re.search(r"\)\s*,\s*$", lines[j]):
            j += 1
        block = "".join(lines[i : j + 1])
        m = re.search(r'"([^"]+)"', block)
        if m:
            name = m.group(1)
            # find the existing entry's features if a twin exists
            fm = re.search(r'included_f\("' + re.escape(name) + r'",\s*(\[[^\]]*\])\)', text)
            features = fm.group(1) if fm else "&[]"
            out.append(f'    included_f("{name}", {features}),\n')
            converted += 1
        i = j + 1
        continue
    out.append(line)
    i += 1

new_text = "".join(out)
p.write_text(new_text)
print(f"converted {converted} stub_generic entries to included_f")