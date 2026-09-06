# VM Benchmark Golden Fixture

This fixture is a Skald-native port of Niflheim's deterministic bytecode-VM
correctness workload. It deliberately combines modules, shared interface
owners, inheritance, virtual and interface dispatch, arrays, casts, calls,
strings, generic maps and vectors, floating-point formatting and parsing, and
exact numeric observations in one source graph.

The provider modules below `cases/modules/vm_benchmark/` are the authoritative
copy. They are golden-owned test data rather than a duplicated sample tree.
One logical entry exposes twelve individually checked guest cases and selector
`999` for the checked aggregate. The golden spec compiles that same source
graph under the default optimization profile, with MIR optimization disabled,
and with runtime traces omitted; all three variants reuse the same thirteen
exact output expectations.

## Optimization canaries

The entry module runs a small, output-free canary set before dispatching the
selected guest case. The canaries deliberately cover algebraic identity and
annihilator rewrites, checked quotient/remainder/shift folding, propagation
through checked and logical carriers, all four constant short-circuit choices,
and the unreachable cleanup exposed by their guard. Their results are checked
at runtime, including under the `optimization-none` variant, so they add MIR
coverage without changing any benchmark result or checksum.

The dedicated optimization golden fixtures remain the authoritative focused
tests for exact pass behavior. These canaries ensure that the broad workload
also exercises these productive optimization shapes at least once.

## Runtime bookkeeping

Normal VM execution records total and per-opcode instruction counts in a
`Map<Str, u64>`; the total recovered from that map is the instruction count in
each checked result. Function calls retain caller frames in a
`Vec<shared Frame>`, so recursive guest programs exercise vector growth,
push, pop, and ownership behavior. The exact-double builtin formats its
computed finite `f64` through `Str.from_f64`, parses it back through
`Str.to_f64`, and requires an exact round trip before using the formatted
length in its result.

## Ownership model

Niflheim's source model uses implicit garbage-collected references. The Skald
port makes those relationships explicit:

- instruction, builtin, erased constant, frame, and program identities use
  `shared` owners;
- instruction tables use optional shared-interface slots while builders fill
  fixed capacities, then checked access requires every executable slot to be
  populated; builtin tables contain shared interface owners;
- VM services receive the VM through a call-scoped `mut ref VmApi` view;
- each frame owns shared register-array backing, while the vector call stack
  retains shared frame owners, so saving a frame preserves register identity
  without copying its mutable contents; and
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
build profile. “Aggregate run” executes and checks all twelve guest programs.

| Variant | Compile | Link | Aggregate run | Assembly |
|---|---:|---:|---:|---:|
| `default` | 9.363 s | 0.284 s | 43.517 ms | 9,433,348 bytes |
| `optimization-none` | 3.162 s | 0.295 s | 44.299 ms | 9,537,769 bytes |
| `omit-runtime-trace` | 9.172 s | 0.249 s | 44.532 ms | 6,851,646 bytes |

Every observed compiler, linker, and native process remained below the golden
runner's default 60-second per-process timeout. Timing is deliberately absent
from the checked expectations.
