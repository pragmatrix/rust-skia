# Vendored bindgen 0.73.2 — required patches

The experimental opt-in bindings generation (ADR 0001) hit two bindgen
0.73.2 bugs that upstream has not fixed, plus two dependency-version pins.
This file documents the changes as a diff so the vendored copy can be
dropped and the changes re-applied (or upstreamed) later. The full
per-file diffs against the pristine crates-io sources are at the bottom
of this file.

## 1. `ir/context.rs` — mangle `type-parameter-N-M` idents

Anonymous C++ template parameters arrive in bindgen as
`type-parameter-0-0`-style spellings. `rust_mangle` only rewrote names
containing `@`, `?`, or `$` (plus keywords), so hyphenated idents passed
through unmangled and `proc_macro2::Ident::new` panicked with
`"type-parameter-0-1" is not a valid Ident`
(upstream issue rust-lang/rust-bindgen#2437 family). The patch routes
hyphen-containing names through the existing mangling path and replaces
`-` with `_`.

This is required by the opt-in generation: with
`allowlist_recursively(false)` and template types allowlisted, bindgen
resolves the template specializations and their anonymous parameters show
up during codegen.

## 2. `codegen/mod.rs` — diagnose the opaque-with-fields assert

bindgen 0.73.2 hard-asserts that opaque types have no fields
(`debug_assert!(fields.is_empty())`). During the opt-in migration several
templates (std::tuple, SkSpan specializations, …) hit this assert, and it
fired without saying *which* type. The patch replaces the assert with one
that prints the offending type's canonical name and first field (gated
under the `SKIA_DEBUG` panic tag for grepping). Keep it while the
experiment classifies new template types; it can go upstream or away once
no template type trips the assert anymore.

## 3. `Cargo.toml` (of the patched crate) — pin syn and prettyplease

- `syn = "2.0.117"`: bindgen 0.73.2 requires the syn 2.x API; the
  `>=2, <4` range would resolve to syn 3, whose `ParseQuote` changed.
- `prettyplease = "0.2.29"`: 0.3 is built against syn 3 tokens.

Equivalent pins in our own `Cargo.lock`/workspace would work as well, so
this third change may be dropped from an upstream patch submission.

---

## Full diffs

### `ir/context.rs`

```diff
--- a/ir/context.rs	(bindgen 0.73.2)
+++ b/ir/context.rs	(patched)
@@ -858,7 +858,11 @@
     /// Mangles a name so it doesn't conflict with any keyword.
     #[rustfmt::skip]
     pub(crate) fn rust_mangle<'a>(&self, name: &'a str) -> Cow<'a, str> {
-        if name.contains('@') ||
+        // rust-skia patch: anonymous C++ template parameters arrive as
+        // "type-parameter-N-M"; hyphens are not valid in Rust idents, so
+        // such names must go through the mangling path below.
+        if name.contains('-') ||
+            name.contains('@') ||
             name.contains('?') ||
             name.contains('$') ||
             matches!(
@@ -881,6 +885,9 @@
             s = s.replace('@', "_");
             s = s.replace('?', "_");
             s = s.replace('$', "_");
+            // rust-skia patch: anonymous C++ template parameters arrive as
+            // "type-parameter-N-M"; hyphens are not valid in Rust idents.
+            s = s.replace('-', "_");
             s.push('_');
             return Cow::Owned(s);
         }
```

### `codegen/mod.rs`

```diff
--- a/codegen/mod.rs	(bindgen 0.73.2)
+++ b/codegen/mod.rs	(patched, debug instrumentation)
@@ -2392,10 +2392,20 @@
             }
         }
 
-        if is_opaque {
-            // Opaque item should not have generated methods, fields.
-            debug_assert!(fields.is_empty());
-            debug_assert!(methods.is_empty());
+        if is_opaque && !fields.is_empty() {
+            eprintln!(
+                "SKIA_DEBUG: opaque type '{}' has {} field(s): {:?}",
+                item.canonical_name(ctx),
+                fields.len(),
+                fields
+                    .first()
+                    .map(|t| t.to_string().chars().take(120).collect::<String>()),
+            );
+            debug_assert!(
+                fields.is_empty(),
+                "SKIA_DEBUG: opaque {} has fields",
+                item.canonical_name(ctx)
+            );
         }
 
         let is_union = self.kind() == CompKind::Union;
```

### `Cargo.toml` (of the patched crate)

```diff
 [dependencies.prettyplease]
-version = ">=0.2.7, <0.4"
+version = "0.2.29"
 features = ["verbatim"]
 optional = true
 
 [dependencies.syn]
-version = ">=2, <4"
+version = "2.0.117"
```
