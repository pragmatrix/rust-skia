# CONTEXT.md

## Binding Generation

### Type

A C++ type that appears in the transitive closure of the `C_*` wrapper
functions handed to bindgen.

### Opt-In

The requirement that every Type in the closure must be explicitly listed to
appear in the generated bindings. Absence from the list excludes the type.
The inverse of the historical opt-out (blocklist) model.

### Layout-Only Type

A Type that is included solely so that containing types can compute correct
size and alignment. Its members are never read or written by the Rust side.
Example reason to care: a Layout-Only Type may be generated as opaque.

### Member-Accessed Type

A Type whose members are read or written by the safe layer or by wrapper
code. A Member-Accessed Type cannot be masked as opaque or stubbed without
breaking the safe layer.

### Type Bindings Table

The table that records, per Type, whether it is Layout-Only or
Member-Accessed, and the derived generation action (include / opaque / stub /
raw-line). Kept alongside the ADR for the opt-in decision. Verified by
scanning the Rust code upfront.

### Closure Inventory

The printed inventory of everything bindgen actually generated, used as the
review artifact at milestone updates. Not an input to generation; the
Opt-In list filters, the inventory shows what passed.

### Drift

Any difference between the generated closure and the Type Bindings Table's
expectations (a listed entry went dead, or a new type appeared). Drift fails
the build hard.