# VM Benchmark Golden Fixture

This fixture is a Skald-native port of Niflheim's deterministic bytecode-VM
correctness workload. It deliberately combines modules, shared interface
owners, inheritance, virtual and interface dispatch, arrays, casts, calls,
strings, and exact numeric observations in one source graph.

The provider modules below `cases/modules/vm_benchmark/` are the authoritative
copy. They are golden-owned test data rather than a duplicated sample tree.
Each implementation stage adds named runs to the same compiled logical entry.

## Ownership model

Niflheim's source model uses implicit garbage-collected references. The Skald
port makes those relationships explicit:

- instruction, builtin, erased constant, frame, and program identities use
  `shared` owners;
- instruction tables use optional shared-interface slots while builders fill
  fixed capacities, then checked access requires every executable slot to be
  populated; builtin tables contain shared interface owners;
- VM services receive the VM through a call-scoped `mut ref VmApi` view;
- each frame owns shared register-array backing, while the fixed-capacity frame
  stack uses optional owner slots until calls populate them, so saving a frame
  preserves register identity without copying its mutable contents; and
- benchmark cases and results remain ordinary inline values.

The object graph is acyclic. It requires no tracing-GC compatibility surface.

## Running manually

From the repository root, build and run the logical entry with:

```sh
cargo run --locked -p skac -- \
  --entry app \
  --module-root tests/golden/vm_benchmark/cases/modules \
  -o build/vm-benchmark
build/vm-benchmark 1
```

Run the focused golden selection with:

```sh
make golden-filter GOLDEN_FILTER='vm_benchmark/**'
```
