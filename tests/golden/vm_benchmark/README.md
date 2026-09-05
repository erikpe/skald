# VM Benchmark Golden Fixture

This fixture is a Skald-native port of Niflheim's deterministic bytecode-VM
correctness workload. It deliberately combines modules, shared interface
owners, inheritance, virtual and interface dispatch, arrays, casts, calls,
strings, and exact numeric observations in one source graph.

The provider modules below `cases/modules/vm_benchmark/` are the authoritative
copy. They are golden-owned test data rather than a duplicated sample tree.
One logical entry exposes twelve individually checked guest cases and selector
`999` for the checked aggregate. The golden spec compiles that same source
graph under the default optimization profile, with MIR optimization disabled,
and with runtime traces omitted; all three variants reuse the same thirteen
exact output expectations.

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
build/vm-benchmark 999
```

Run the focused golden selection with:

```sh
make golden-filter GOLDEN_FILTER='vm_benchmark/**'
```

Run one compiler profile or the full deterministic audit with:

```sh
scripts/golden.sh --variant default --filter 'vm_benchmark/**'
scripts/golden.sh --variant optimization-none --filter 'vm_benchmark/**'
scripts/golden.sh --variant omit-runtime-trace --filter 'vm_benchmark/**'
scripts/golden.sh --determinism full --jobs 1 --filter 'vm_benchmark/**'
```

## Maintenance observations

These figures are non-normative observations from one warm debug-tool run on
the development host on 2026-09-05. They are useful for spotting order-of-
magnitude regressions, but are not thresholds and will vary with the host and
build profile. “Aggregate run” executes and checks all twelve guest programs;
“slowest leaf” is from a separate complete thirteen-leaf run.

| Variant | Compile | Link | Aggregate run | Assembly | Observed slowest leaf |
|---|---:|---:|---:|---:|---|
| `default` | 7.070 s | 0.232 s | 11.122 ms | 7,563,064 bytes | `aggregate_output`, 45.215 ms |
| `optimization-none` | 2.768 s | 0.243 s | 11.171 ms | 7,868,527 bytes | `slice_copy_output`, 29.693 ms |
| `omit-runtime-trace` | 7.014 s | 0.203 s | 5.871 ms | 5,354,324 bytes | `prime_sum_100_output`, 61.833 ms |

Every observed compiler, linker, and native process remained below the golden
runner's default 30-second per-process timeout. Timing is deliberately absent
from the checked expectations.
