from pathlib import Path

p = Path("codegen/mod.rs")
text = p.read_text()
old = """        if is_opaque {
            // Opaque item should not have generated methods, fields.
            debug_assert!(fields.is_empty());
            debug_assert!(methods.is_empty());
        }"""
new = '''        if is_opaque && !fields.is_empty() {
            eprintln!(
                "SKIA_DEBUG: opaque type '{}' has {} field(s): {:?}",
                item.canonical_name(ctx),
                fields.len(),
                fields.first().map(|f| f.getName()),
            );
            debug_assert!(
                fields.is_empty(),
                "SKIA_DEBUG: opaque {} has fields",
                item.canonical_name(ctx)
            );
        }'''
assert old in text, "target block not found"
text = text.replace(old, new, 1)
p.write_text(text)
print("instrumented")