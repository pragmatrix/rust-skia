---
name: cpp-to-rust-documentation
description: "Use when porting documentation from Skia C++ headers to Rust APIs, including Rustdoc comments, parameter descriptions, type links, or C++-to-Rust names."
---

# C++ to Rust Documentation

When porting documentation from C++ headers:

- Keep wording as close to the original as possible, including grammatical errors, except where Rust terminology or syntax requires a change.
- Document parameters using a list entry per parameter, backticking the Rust
  parameter name and following it directly with the description (for example,
  `` - `color` unpremultiplied RGBA ``). Do not add a colon between name and
  description, and do not reword the description into a sentence.
- Convert `@return` clauses into a prose sentence starting with `Returns ...`
  instead of a list entry (for example, `Returns unpremultiplied RGBA.`).
- Link types using brackets (for example, `[`Type`]`).
- Link functions and methods using the Rust name with call parentheses (for example, `SkPath::arcTo` becomes `[`Path::arc_to()`]`).
- Use Rust equivalents for C++ types and constants (for example, `SkPoint` becomes `[`Point`]` and `kMove_Verb` becomes `[`PathVerb::Move`]`).
- Do not rename functions when generating documentation.
- If the Rust function name differs from the C++ function name, use the Rust name in the documentation text.
- Ensure documentation parameter names match the Rust function parameter names.
- Refer to parameter values in prose as backticked code (`` `None` ``), and use
  `None` / `Some` instead of `nullptr` / optional wording when describing Rust
  `Option` parameters.
- Do not port `example:` fiddle links; rustdoc does not render Skia fiddles.
- Every reference to a Rust item in documentation must be a working intra-doc
  link, never plain backticked text. When a link fails to resolve, find the
  path that resolves instead of downgrading the reference to plain code text:

  - Enum variants link like any other item once the enum's path is correct
    (for example, `GrBackendApi::kOpenGL` becomes
    `[`crate::gpu::ganesh::BackendApi::OpenGL`]`). A failure usually means the
    path points at a different type with the same name — qualify further
    rather than dropping the link.
  - From inside the module that defines an item, link it with `[`self::item`]`
    — the module's own name is not in scope there.
  - Link the type that actually declares the member: references in prose to
    `dst.colorType()` style C++ chains become links to the Rust counterpart
    methods, e.g. `[`Pixmap::color_type()`]`; sibling methods of the documented
    type are `[`Self::method()`]`, or `[`Type::method()`]` when `Self` refers
    to a different type (for example inside a trait impl like `Iterator`).

## Module level documentation

- When a Rust module wraps a single C++ header or class, port the header's class
  overview documentation into the module's `//!` documentation at the top of the
  module file.
- If the C++ header provides no class or module level documentation, write a
  very concise module description instead (one or two lines, no prose padding).
