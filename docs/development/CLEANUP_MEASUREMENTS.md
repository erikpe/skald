# Cleanup Measurement Baseline

Status: authoritative for the reproducible baseline used to evaluate cleanup
work that can affect compiler cost, generated code, or native execution. MIR
structure remains owned by the
[local final-MIR redundancy measurement](MIR_REDUNDANCY_MEASUREMENT.md), while
specialized acceptance procedures retain their workload-specific metrics.

## Procedure

From the repository root, run:

```text
make cleanup-baseline
```

The target builds the optimized, assertion-enabled `golden` compiler and the C
runtime, then measures every reviewed workload. Each invocation creates a
unique ignored directory below `build/measurements/cleanup-baseline/` and
writes two JSON files:

- `deterministic.json` contains compiler revision and dirty state, compiler
  profile, target, MIR profile and exclusions, the resolved pass schedule,
  repetition policy, source inventory and byte size, compiler arguments,
  runtime-trace policy, per-occurrence proof-snapshot analysis usage, artifact
  sizes and hashes, and repeated native-result digests;
- `report.json` contains that projection plus compiler wall time and peak RSS,
  executable-build observations, native wall time, and the unique run path.

Assembly compilation is repeated at least twice and must emit identical bytes.
One separate untimed trace compilation records analysis requests,
computations, same-snapshot repetitions, distinct callable-snapshot keys, and
result-table activity for each requesting pass occurrence. This trace run does
not contribute to compiler wall-time or peak-RSS samples.
Native workloads repeat the same exit status, standard output, and standard
error digests. Paired variants alternate order during warmups and measured
runs. Every child process has a configurable watchdog; a timeout terminates
the complete process group. Timing summaries report sample count, median,
median absolute deviation, minimum, and maximum.

The operational values depend on host load, CPU policy, kernel, compiler
toolchain, and linker. They are evidence for before/after comparisons on the
same controlled host and never a correctness threshold. The deterministic
projection deliberately excludes those observations, and the baseline does
not run as part of `make check` or `make check-long`.

For a quick harness check or a focused baseline after the prerequisites exist:

```text
make measurement-support-test
python3 scripts/measure_cleanup_baseline.py \
  --workload compile/small-source --compile-repeats 2
```

Use `--workload` more than once to select an exact subset. `--timeout`,
`--compile-repeats`, `--native-warmups`, and `--native-repeats` make the
execution policy explicit. Keep the compiler profile argument accurate when
supplying a different compiler executable.

## Workload coverage

The compile baseline covers these pressure shapes:

| Workload | Coverage |
| --- | --- |
| `compile/small-source` | Fixed process and compiler startup cost. |
| `compile/many-modules-large-cfg` | Filesystem module loading and large callable control-flow graphs. |
| `compile/many-generic-applications` | Generic interface selection and specialization volume. |
| `compile/nested-ownership` | Nested generic vector shapes and ownership lowering. |

The native portion includes generic vector growth, matched range/`while`
pairs for `u8`, `u64`, and `i64`, and the enabled/omitted panic runtime-trace
pairs. These reuse the maintained fixtures under `tests/benchmarks/`; the
[generic range](RANGE_LOOP_PERFORMANCE.md) and
[panic runtime trace](PANIC_RUNTIME_TRACE_PERFORMANCE.md) documents define
their specialized interpretation.

## Comparing cleanup changes

Capture a complete report before and after a candidate change with the same
host configuration and repetition policy. First compare `deterministic.json`:
unexpected source inventory, schedule, semantic digest, or assembly hash
changes need an explanation. Then compare the operational medians together
with median absolute deviation and the full observed range. Repeat noisy runs;
do not turn a single wall-time delta into a gate.
