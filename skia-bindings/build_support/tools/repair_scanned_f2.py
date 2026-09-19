#!/usr/bin/env python3
"""Repair v2: replace bool-arg fragments + placeholder flags in one pass."""
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
results = json.loads((HERE / "scan-results.json").read_text())
table = HERE.parent / "type_bindings.rs"
text = table.read_text()

SHAPER = "SHAPER_FEATURES"
FEATURES = {
    "GrGLenum": '&["ganesh", "gl"]',
    "GrGLFormat": '&["ganesh", "gl"]',
    "VkComponentMapping": '&["vulkan"]',
    "skresources::ResourceProvider": '&["svg"]',
    "skresources::ImageAsset": '&["svg"]',
    "skgpu::graphite::BackendSemaphore": '&[]',
    "skia::textlayout::TextAlign": '&["textlayout"]',
    "skia::textlayout::PositionWithAffinity": '&["textlayout"]',
    "skia::textlayout::TextBaseline": '&["textlayout"]',
    "skia::textlayout::TextDecorationStyle": '&["textlayout"]',
    "skia::textlayout::TextDecorationMode": '&["textlayout"]',
    "skia::textlayout::Decoration": '&["textlayout"]',
    "skia::textlayout::PlaceholderAlignment": '&["textlayout"]',
    "skia::textlayout::StyleMetrics": '&["textlayout"]',
    "skia::textlayout::TextBox": '&["textlayout"]',
    "skia::textlayout::TextDirection": '&["textlayout"]',
    "skia::textlayout::TextHeightBehavior": '&["textlayout"]',
    "skia::textlayout::TextRange": '&["textlayout"]',
    "SkShaper::RunHandler": SHAPER,
    "skottie::Animation": '&["skottie"]',
    "skottie::ResourceProvider": '&["skottie"]',
    "sksg::ImageFilter": '&["skottie"]',
    "sksg::Matrix": '&["skottie"]',
    "sksg::Node": '&["skottie"]',
    "sksg::Shader": '&["skottie"]',
}


def flag_of(name):
    v = results.get(name, "")
    props = v.split("+")
    if "fields" in props and "functions" in props:
        return "FIELDS_FUNCTIONS"
    if "fields" in props:
        return "FIELDS"
    return "NONE"


# bool-arg fragments concatenated in corrupted lines
frag = re.compile(
    r'scanned_f\("([^"]+)",\s*(?:true|false)(?:,\s*(?:true|false))?\s*,?\]'
)
count = 0


def repl(m):
    global count
    name = m.group(1)
    feats = FEATURES.get(name, "&[]")
    count += 1
    return f'scanned_f("{name}", {flag_of(name)}, {feats})'


text = frag.sub(repl, text)

# any remaining healthy calls missing a space: normalize
table.write_text(text)
print(f"rebuilt {count} calls")
