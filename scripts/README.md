# Scripts

Repository workflow scripts belong here when repeated build, golden-test, or
release tasks justify them. Core compiler behavior must remain available
through `skac`, and validation must remain available through the Makefile
rather than existing only in shell scripts. See the
[development workflow](../docs/development/README.md).

`golden.sh` builds `skac` and `skald-golden` with the repository's optimized
assertion-enabled `golden` profile, changes to the repository root, and
forwards every argument unchanged to the Rust golden runner. It is useful for
inspection and combinations of filters that do not need a dedicated Make
target:

```text
scripts/golden.sh --list --filter 'syntax/**'
scripts/golden.sh --exact 'primitive_strings/values::values::default::bytes_slices_and_concatenation' --show-output
scripts/golden.sh --filter 'declarations/**' --determinism compile
```

The Makefile remains authoritative for complete ordinary and full-determinism
validation through `make golden-test` and `make golden-determinism-test`.

`measure_panic_runtime_trace.py` performs the reproducible, non-gating
enabled-versus-omitted measurement documented in
[Panic Runtime Trace Performance](../docs/development/PANIC_RUNTIME_TRACE_PERFORMANCE.md).
The standard invocation builds its prerequisites and runs all four workload
profiles:

```text
make runtime-trace-benchmark
```

Call the script directly to change repeat counts, select workloads, or emit
JSON after building `skac` and the runtime. Generated measurement artifacts
remain in a unique run directory under the ignored
`build/measurements/panic-runtime-trace/` directory.

`measure_cleanup_baseline.py` is the cross-cutting measurement entry point for
cleanup work. It captures deterministic compiler and artifact facts separately
from compiler resource use and native timing across the reviewed compile and
native workload matrix:

```text
make cleanup-baseline
```

The report contract and comparison procedure are documented in
[Cleanup Measurement Baseline](../docs/development/CLEANUP_MEASUREMENTS.md).
`measurement_support.py` owns the small shared subprocess, watchdog, unique
directory, alternating-order, timing-summary, hashing, and repository-identity
helpers. Its focused tests run through `make measurement-support-test`.

`measure_generic_vec.py` compiles the representative generic-vector growth,
copy, pop, and clear workload under `tests/benchmarks/generic_vec/`, then
reports assembly/executable size, compile time, and median native run time:

```sh
make generic-vec-benchmark
python3 scripts/measure_generic_vec.py --compiler target/golden/skac --json
```

It is a reproducible measurement procedure, not a timing correctness gate.
Artifacts remain in unique run directories under the ignored
`build/measurements/generic-vec/` directory. Child processes have watchdogs,
and timing output includes median absolute deviation.

`measure_range_loops.py` compiles matched `u8`, `u64`, and `i64` fused-range
and handwritten-`while` workloads, validates their checksums, records assembly
profiles and artifact sizes, and reports interleaved repeated median timings:

```sh
make range-loop-benchmark
python3 scripts/measure_range_loops.py --compiler target/golden/skac --json
```

The Make target enforces the documented maximum 10% range overhead. Timing is
deliberately outside `make check`; deterministic MIR and assembly-shape tests
are the correctness gates. Artifacts remain in unique run directories under
the ignored `build/measurements/range-loop/` directory. Paired variants
alternate order, child processes have watchdogs, and timing output includes
median absolute deviation.
