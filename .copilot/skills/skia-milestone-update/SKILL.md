---
name: skia-milestone-update
description: Perform a Skia milestone update for rust-skia (align rust-skia with a new Skia chrome/mXX branch). Use when the user asks to "continue the milestone update", "update to milestone mXX", or references the milestone update process. Covers README update, build org diff, header diff accounting, wrapper updates, version bump, API diff review, and final checks.
---

# Skia Milestone Update

This skill performs a Skia milestone update for rust-skia, aligning it with a new
Skia `chrome/mXX` branch. The authoritative checklist is the
[Template: Skia Milestone Update PR](https://github.com/rust-skia/rust-skia/wiki/Template:-Skia-Milestone-Update-PR)
wiki page; follow every item there. Project-specific conventions and the
`make diff-skia` caveat live in `AGENTS.md` — read it before starting.

## Inputs

- `OLD_TAG`: the current Skia submodule tag (e.g. `m150-0.98.1`)
- `NEW_TAG`: the target Skia submodule tag (e.g. `m151-0.99.0`)
- `OLD_MILESTONE` / `NEW_MILESTONE`: the numeric milestones (e.g. `150` / `151`)

Determine these from `skia-bindings/Cargo.toml` (`[package.metadata] skia = "..."`)
and `git -C skia-bindings/skia describe --tags` / `git -C skia-bindings/skia tag --list 'm1*'`.

## Notes that go beyond the wiki checklist

- **Versioning:** each milestone bump increments the minor version
  (e.g. `0.98.0` -> `0.99.0`). Add the version to any new `deprecated` attributes
  (`since = "0.X.Y"`).
- **Include diffs:** use direct `git -C skia-bindings/skia diff OLD_TAG..NEW_TAG -- ...`
  commands. Do not use `make diff-skia` for include/API diffs; that target only
  compares rust-skia-specific commits in the Skia submodule against master (it is the
  "Do the `rust-skia:` commits ... match with `master`" checklist item).
- **Account for every changed public header** before editing wrappers. Start from the
  full list of changed public headers (`git -C skia-bindings/skia diff --name-only
  OLD_TAG..NEW_TAG -- 'include/**/*.h' 'modules/*/include/**/*.h'`) and the complete
  diff of all of them, then walk through every file before making any edits. Include
  changes that look trivial (include moves, comments, friend declarations, build-file
  public header lists). Common include-path-only moves (e.g. `include/private/base/*`
  -> `include/private/*`) need no binding/wrapper update — record them as `no change`.

  Do not cherry-pick a subset of the changed headers (for example, only the ones with
  the largest diffs). Process every header deterministically:

  1. Cross-reference every header in the list against the existing binding/wrapper
     surface — check whether a `skia-bindings/src/*.cpp` wraps any function/type from
     it, and whether a corresponding `skia-safe/src/...` wrapper module exists.
  2. Classify each changed header into one of:
     - **no binding exists, no wrapper exists** — usually no action unless it is a newly
       exposed public API that should be wrapped;
     - **binding exists, wrapper exists** — diff the header and update the C wrapper in
       `skia-bindings/src/*.cpp` and the Rust wrapper in `skia-safe/src/...` to match;
     - **new public header** — decide whether to add a binding + wrapper, and record
       the decision.
  3. Keep a written accounting (in a scratch file like `/tmp/mXX_accounting.md`) of
     every changed header and the decision made for it (`no change`, `updated binding`,
     `updated wrapper`, `added new`, `skipped — internal only`, `skipped — no
     binding/wrapper`). **Do not start editing wrappers until every header has an entry
     in this accounting.**
  4. Batch the headers by classification and process each batch in one pass, editing the
     matching `*.cpp` and `skia-safe/src/...` files together. Re-run
     `cargo check -p skia-bindings` (touching `bindings.cpp` first to force bindgen
     regeneration) and `cargo check -p skia-safe` after each batch.
- **Wrapper updates:** preserve method/debug-field ordering aligned with the upstream
  C++ header. Add `todo!()` for anything that cannot be updated right now. Stay
  compatible with previous versions of skia-safe without trying too hard before 1.0;
  use `#[deprecated]` if needed. Look for `todo!()` macros that can now be resolved.
  Review `Send` & `Sync` and `Debug` implementations for new wrappers.

## Release notes

Release notes are authored separately (typically by the maintainer at release time)
following `.github/release-notes-guidelines.md`. They are **not** part of the milestone
update PR process itself — do not draft or update GitHub release notes during the
milestone update unless explicitly asked.

## Style & conventions

See `AGENTS.md` and `.github/copilot-instructions.md` for the full set. Highlights:
- Keep Rust method and debug-field ordering aligned with the upstream C++ header order.
- Keep top-level type declarations in the same sequence as the upstream C++ header.
- For nested C++ types, keep the parent Rust type first and define nested Rust types
  directly below the parent.
- Derive `Debug` for all public types unless there's a specific reason not to; place
  `Debug` first in the derive list.
- Do not pass C++ class types by value across `extern "C"`; use pointers and/or
  out-parameters. Use placement new for non-trivial out-parameters.
- Match the surrounding code style; keep functions small and deterministic.
- Do not refactor adjacent working code unless required for correctness.
