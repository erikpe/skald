# VM Benchmark Port Discoveries

Status: resolved; preserved with the completed VM benchmark roadmap.

This record holds the compiler-robustness follow-up found while implementing
the VM benchmark. The benchmark port itself retained its reviewed test-only
scope.

## Owning inline-field read through a shared receiver

- **Original evidence:** While establishing the minimal VM, copying the inline
  `Str` field selected by `self.program->name` into a local caused `skac` to
  panic at `crates/skald-compiler/src/typeck/expression/place.rs` with
  `cast receiver was rejected above`. The type checker reached
  `check_object_source_place` for a dereference-relative field receiver and
  asserted because that receiver had no binding path.
- **Resolution:** Copy-source checking now preserves the containing shared
  pointee as the checked view and applies the inline field as its consumer
  projection. Stable bindings borrow their live owner directly; replaceable
  shared fields and produced owners retain the existing hidden-anchor
  behavior. The result uses ordinary copy construction without changing the
  shared-owner ABI or cast semantics.
- **Coverage:** Type-check and MIR-verification tests cover stable, anchored,
  and produced shared receivers. The shared-ownership golden suite executes
  local initialization, value-parameter construction, and an ordinary
  constructor argument from the copied inline field.
