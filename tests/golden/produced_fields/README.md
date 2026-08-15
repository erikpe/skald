# Produced field golden tests

`fields.golden.toml` owns the first executable produced-object field-read
slice. It covers a direct primitive load and nested inline-class fields used
as a method receiver, explicit-copy source, and read-only alias argument. It
also proves that primitive and class optionals, inline and shared arrays,
optional shared arrays, shared and optional-shared object owners, optional
boxes, and arrays of class optionals remain valid after the produced root is
destroyed. A separate owner-order case secures a projected shared pointee
before a later argument replaces the original owner and exercises legal
self-overlap in shared-field assignment. Named and produced forms are compared
for the representative optional, class-optional, array, and shared-owner
observations.

The lifecycle-order case traces explicit field copy construction, later
argument effects, consumer execution, and reverse destruction of multiple
produced roots. It proves that an owning result is secured before its source
root is destroyed and that each root and nested field is destroyed exactly
once.

The rejection cases freeze diagnostics for direct and nested writes, class
field replacement, mutable methods and aliases, private fields, and invalid
member kinds on produced roots.

The conformance case covers construction, direct/static/instance/interface
producers, inherited and checked projections, nested fields, closed generics,
structural getters, copies and returns, later arguments, logical and loop
paths, register/stack pressure, and `Str.join`. The runtime-trace case proves a
panic reached through a produced-field alias consumer keeps the ordinary
source stacktrace and non-unwinding behavior. Together with the owning-field
and lifecycle cases, this is the complete source-to-native field surface.

Run the group with:

```text
scripts/golden.sh --determinism full --filter 'produced_fields/**'
```
