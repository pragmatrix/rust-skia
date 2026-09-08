---
name: cpp-to-rust-documentation
description: "Use when porting documentation from Skia C++ headers to Rust APIs, including Rustdoc comments, parameter descriptions, type links, or C++-to-Rust names."
---

# C++ to Rust Documentation

When porting documentation from C++ headers:

- Keep wording as close to the original as possible, including grammatical errors, except where Rust terminology or syntax requires a change.
- Document parameters using a list (for example, `- param: description`).
- Link types using brackets (for example, `[`Type`]`).
- Link functions and methods using the Rust name with call parentheses (for example, `SkPath::arcTo` becomes `[`Path::arc_to()`]`).
- Use Rust equivalents for C++ types and constants (for example, `SkPoint` becomes `[`Point`]` and `kMove_Verb` becomes `[`PathVerb::Move`]`).
- Do not rename functions when generating documentation.
- If the Rust function name differs from the C++ function name, use the Rust name in the documentation text.
- Ensure documentation parameter names match the Rust function parameter names.

## Module level documentation

- When a Rust module wraps a single C++ header or class, port the header's class
  overview documentation into the module's `//!` documentation at the top of the
  module file.
- If the C++ header provides no class or module level documentation, write a
  very concise module description instead (one or two lines, no prose padding).
