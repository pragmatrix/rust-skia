#!/usr/bin/env python3
"""Bisect: find the table entry whose Opaque generation panics bindgen.

Uses SKIA_TABLE_FILTER="!<name>" (see skia_bindgen.rs) to skip exactly one
table entry at a time; the run that stops panicking identifies the culprit.
"""
import os
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent.parent.parent
FEATURES = (
    "binary-cache,embed-icudtl,pdf,ganesh,gl,vulkan,metal,"
    "textlayout,svg,skottie,webp,graphite"
)


def opaque_entries():
    text = (HERE.parent / "type_bindings.rs").read_text()
    return sorted(set(re.findall(r'opaque_f\(\s*\n?\s*"([^"]+)"', text)))


def panics(skip):
    if skip:
        os.environ["SKIA_TABLE_FILTER"] = f"!{skip}"
    else:
        os.environ.pop("SKIA_TABLE_FILTER", None)
    env = dict(os.environ)
    env["SSL_CERT_FILE"] = "/opt/homebrew/etc/openssl@3/cert.pem"
    env["RUSTUP_TOOLCHAIN"] = "stable"
    proc = subprocess.run(
        ["cargo", "check", "-p", "skia-bindings", "--features", FEATURES],
        cwd=REPO,
        capture_output=True,
        text=True,
        timeout=3600,
    )
    out = proc.stdout + proc.stderr
    return ("codegen/mod.rs:2397" in out or "type-parameter" in out), out


def main():
    entries = opaque_entries()
    print(f"{len(entries)} opaque entries")
    baseline, _ = panics(None)
    print(f"baseline panics: {baseline}")
    if not baseline:
        print("no panic at baseline; nothing to bisect")
        return
    for i, entry in enumerate(entries):
        p, out = panics(entry)
        m = re.search(r"panicked at ([^\n]+)", out)
        detail = m.group(1) if m else ""
        print(f"[{i + 1}/{len(entries)}] skip {entry}: panic={p} {detail}")
        if not p:
            print(f"\nCULPRIT: {entry}")
            return
    print("no single entry responsible")


if __name__ == "__main__":
    main()