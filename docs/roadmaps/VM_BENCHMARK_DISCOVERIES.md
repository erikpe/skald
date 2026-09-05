# VM Benchmark Port Discoveries

Status: one actionable compiler-robustness finding; it does not block the port.

This record holds follow-up work found while implementing the VM benchmark
roadmap but not required to broaden the active workload. The benchmark roadmap
keeps its reviewed test-only scope.

## Owning inline-field read through a shared receiver can panic the compiler

- **Priority:** high compiler robustness; non-blocking for the benchmark port.
- **Evidence:** While establishing the minimal VM, copying the inline `Str`
  field selected by `self.program->name` into a local caused `skac` to panic at
  `crates/skald-compiler/src/typeck/expression/place.rs:591` with
  `cast receiver was rejected above`. The type checker reached
  `check_object_source_place` for a field receiver without a binding path and
  asserted instead of accepting the documented owner-preserving field source
  or emitting a source diagnostic. Carrying the case name as a separate inline
  VM field avoids the unsupported shape, and the complete golden then compiles
  and runs.
- **Likely owner:** object-source place classification in
  `typeck/expression/place.rs`, with focused coverage beside the existing
  produced-field and shared-owner consumer tests.
- **Useful boundary:** Reduce the source shape to one shared owner containing
  an inline class field, then copy that field into an owning local and pass it
  to an ordinary constructor. Decide from the existing language contracts
  whether the source is valid; either lower the owner-preserving copy or emit a
  stable diagnostic. In every case, remove the internal assertion path and add
  a no-panic regression.
- **Exclusions:** Do not couple the fix to VM benchmark source, alter shared
  owner ABI, broaden cast semantics, or include it in a later benchmark task
  merely because this workload exposed it.
