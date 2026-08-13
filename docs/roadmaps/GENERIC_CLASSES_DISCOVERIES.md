# Generic Classes Discoveries

Status: resolved by G10.

These findings were recorded while implementing the generic classes roadmap.
They do not change the frozen language design and remain outside the active
roadmap task that exposed them.

## Make failed-specialization rollback cover every dependent resolved product

**Priority:** Medium.

**Status:** Resolved in G10.

**Problem:** Final mechanical or nominal requirement validation atomically
removes generated class declarations, class definitions, and the generated
hierarchy after any closed specialization fails. Products computed before
that validation, including virtual-family metadata and ordinary resolved
bodies that mention generated identities, are not rebuilt. Compilation stops
on the diagnostics, so no lower phase consumes the inconsistent program, but
the diagnostic `ResolvedProgram` does not fully uphold its ordinary table
invariants.

**Evidence:**
`resolve/resolver/program/specialization/validation.rs` truncates `classes` and
`class_definitions` and restores `hierarchy`; virtual families and previously
resolved ordinary function/class bodies are produced earlier in
`resolve/resolver/program/resolver.rs` and remain present.

**Likely owner:** Generic roadmap G10 diagnostic/product hardening,
`resolve/resolver/program/resolver.rs`, and
`resolve/resolver/program/specialization/validation.rs`.

**Useful boundary:** Either validate all closed requirements before producing
products that depend on generated classes, or implement one cohesive rollback
that rebuilds every ordinary-only table and dispatch annotation. Preserve
current application/definition evidence, dense IDs, deterministic ordering,
and the rule that erroneous programs never reach type checking or lowering.

**Resolution:** G10 snapshots the ordinary class declarations before candidate
publication. On a failed bound or inferred requirement it marks every reserved
generated identity failed, restores that snapshot and the ordinary hierarchy,
and clears dependent class/function definitions and virtual families. Focused
tests assert that the diagnostic product contains no partially published
specialization bodies or dispatch tables.

## Qualify source-facing closed names across modules

**Priority:** Medium.

**Status:** Resolved in G10.

**Problem:** Closed specialization names now structurally render ordinary
class, interface, array, optional, shared, and nested-specialization arguments
instead of leaking numeric type identities. The renderer currently uses leaf
declaration names. Two argument declarations with the same leaf name in
different modules therefore remain identity-distinct but can look ambiguous in
diagnostics and MIR dumps.

**Evidence:**
`resolve/resolver/program/specialization/names.rs` resolves exact semantic
identities to declaration names but does not yet prepend their canonical module
paths. Backend symbols remain collision-proof through module ownership and the
closed `ClassId`, so this is an observability issue rather than a storage alias.

**Likely owner:** Generic roadmap G10 module, dump, diagnostic, and determinism
hardening.

**Useful boundary:** Render the shortest unambiguous canonical source path, or
always render the canonical qualified path, using `ProgramModuleTable` rather
than reparsing names. Keep semantic identity selection and backend mangling
independent from display formatting.

**Resolution:** Whole-program specialization names now use canonical
`ProgramModuleTable` paths for templates and every nominal argument, including
nested applications. Singleton compilation retains compact leaf names. Module
cycle, alias, selective-import, graph-permutation, and independent-process
tests freeze the resulting resolved/HIR/MIR/static-plan observations.
