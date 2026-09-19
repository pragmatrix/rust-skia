# Opt-in binding generation

Bindings generation previously combined a functional opt-in
(`allowlist_function("C_.*")` plus a small free-function allowlist) with a
growing set of opt-out masks (`BLOCKLISTED_TYPES`, `OPAQUE_TYPES`,
`raw_line` stubs) over the transitive closure of types reachable from the
`C_*` wrappers. Those masks decayed silently across milestone updates:
unhandled closure drift surfaced as link errors or wrong layouts far from
its cause.

We instead opt **in** to types: every type in the transitive closure must be
explicitly classified in an Type Bindings Table (a Rust const in
`build_support/`) as either Layout-Only (needed for size/alignment only,
generated as opaque) or Member-Accessed (members are used by the Rust side,
generated in full). The table parameterizes the bindgen builder directly.
Discovery of any type not present in the table is a hard error, enforced in
the `ParseCallbacks::new_item_found` callback during generation — bindgen
has no per-type parse veto, only this post-discovery hook. Entries also
carry the feature sets (`skshaper`, `skparagraph`, `vulkan`, …) under which
the type appears, so reduced-feature builds stay minimal; `std::` templates
and `Vk*` reexports live in the same table.

Migration starts fresh: no byte-diff equivalence with the old output.
Verification sequence is `cargo check -p skia-bindings` first, then the
feature-gated builds, then `skia-org`. At milestone updates, build errors
list the offending types so the table can be re-reviewed deliberately
instead of masks being patched reactively.

## Current state (experimental branch `experimental-opt-in-bindings`)

The table has been rebuilt to be **scan-driven**. Entries come in two
flavors (`Source`):

- `Manual(TypeAction)`: hand-written action (legacy opaque/stub/exclude
  entries, template stand-ins, types outside the touch-point scan).
- `Scanned(ScanFlags)`: two usage flags harvested from the Rust-side
  touch-point corpus by `build_support/tools/scan_type_usage.py`, written
  as or-combined unqualified consts (`FIELDS`, `FUNCTIONS`,
  `FIELDS_FUNCTIONS`, `NONE`). The **action is derived** by the builder:

  | fields needed | functions needed | derived action |
  |---------------|------------------|----------------|
  | yes | yes | Include |
  | yes | no | Include + blocklist all generated method wrappers (`{Name}_.+`; hand-written `C_*` wrappers are unaffected) |
  | no | yes | Opaque |
  | no | no | Exclude |

The scanner rewrites the flags wholesale
(`scan_type_usage.py --update-table`, `apply_scan_flags.py`) and keeps the
`// SCAN:` verdict block inside `type_bindings.rs` as evidence. Its
struct-literal matching requires the qualified spelling to avoid
false positives between same-named types (`skhdr::Metadata` vs
`skia_safe::pdf::Metadata`).

Template types are classified by their *actual* Rust use: sized
`StubGeneric` definitions (e.g. `std::unique_ptr`, `SkTDArray`,
`skia_private::AutoTMalloc`) are injected via `raw_line` where a
transmuted parent needs their layout; std:: types only ever referenced by
pointer get a single opaque stand-in (`std_string`); everything else in the
template family is excluded. Known-suspect classes remain:

- `new_item_found` unclassified reports list exactly the types needing a
  table review — currently the workflow's main feedback loop.
- The scan does not yet have a "referenced as a type name only" property
  (e.g. enum aliases like `SkJpegEncoder::Downsample`); the handful of such
  types carry manual overrides.
- The vendored bindgen copy has been removed from the repository; its
  changes are documented as a diff in `docs/bindgen-0.73.2-patches.md`
  (mangling fix, opaque-with-fields diagnostics, syn/prettyplease pins).
  Caveat: the syn pin cannot be reproduced through the workspace lockfile
  alone — this workspace's `skia-svg-macros` requires syn 3, so bindgen
  0.73.2 resolves to syn 3 and does not compile. Until bindgen 0.73.x+ is
  released against syn 2 (or upstream fixes #2437), building requires
  re-applying the documented patch (e.g. via `[patch.crates-io]`).
- Some remaining `cargo check -p skia-bindings --lib` errors (nested type
  aliases of generics like `sk_sp_element_type` referring to their parent's
  template parameters, and `FILE` in hand-written wrappers) are the next
  items; the vendored bindgen at `patches/bindgen-0.73.2` already carries
  the `rust_mangle` hyphen fix for `type-parameter-N-M` idents.

## Considered options

- Enumerating only roots and letting bindgen resolve transitively
  (rejected: reintroduces the implicit closure this design removes).
- A machine-readable manifest file outside build code (rejected: the table
  must parameterize bindgen directly, so a Rust const is simpler).
- Failing only on dead entries while auto-including new transitives
  (rejected: drift must be reviewed, not absorbed).
