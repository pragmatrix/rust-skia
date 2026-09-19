#!/usr/bin/env python3
"""Map each E0425 'cannot find type' to its enclosing item in bindings.rs,
to find the *decision* (which table entry / generated struct or function)
that made the reference exist."""
import re
import sys
from pathlib import Path

BINDINGS = Path(sys.argv[1])
text = BINDINGS.read_text()
lines = text.splitlines()

owners = {}
for i, line in enumerate(lines):
    m = re.search(r"cannot find type `([A-Za-z0-9_:]+)`", line)
    # parse compiler-independent: instead scan for known missing names
for name in [
    "SkCodecs_ColorProfile", "SkSL_SampleUsage", "SkRecordCanvas",
    "sktext_GlyphRunBuilder", "SkSL_RP_Program", "SkSL_Program",
    "SkPDFArray", "SkFILEStream", "SkPDF_DateTime", "SkOpenTypeSVGDecoder",
    "std_allocator_traits", "std_streampos", "std_streamoff", "std_string",
    "C_SkFontMgr_Request",
]:
    sites = []
    lines_n = len(lines)
    pat = re.compile(rf"\b{name}\b")
    for i, line in enumerate(lines):
        if pat.search(line):
            sites.append(i)
    # find enclosing item for each site
    print(f"== {name} ({len(sites)} refs)")
    seen = set()
    for s in sites[:40]:
        # walk back for enclosing definition
        owner = "?"
        for j in range(s, -1, -1):
            d = re.match(r"\s*(?:pub )?(?:unsafe )?(?:fn|struct|impl|enum) ([A-Za-z0-9_]+)", lines[j])
            if d:
                owner = lines[j].strip()[:90]
                break
        key = owner
        if key not in seen:
            seen.add(key)
            print(f"   L{s}: {owner}")