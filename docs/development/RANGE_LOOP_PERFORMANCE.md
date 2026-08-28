# Tight Range-Loop Performance

Status: reproducible acceptance procedure with one recorded reference result.

Immediate exact `u8`, `u64`, and `i64` range loops are required to approach a
semantically matched handwritten `while`. Deterministic MIR and x86-64 tests
own the durable shape: one termination comparison, one same-typed induction
increment, no call, optional, owner, runtime operation, or range aggregate,
and the same non-jump instruction profile. Host wall time is supporting
evidence and remains outside `make check`.

## Workloads and command

[`tests/benchmarks/range_loop`](../../tests/benchmarks/range_loop/README.md)
contains one range and one `while` program for each integer type. Each pair has
the same current/end/item model, advance-before-body order, iteration count,
checksum work, and successful self-check. `u8` repeats a bounded inner loop;
`u64` and `i64` traverse 20 million values.

Build the runtime and compiler, emit trace-free assembly and executables, run
two warmups and nine interleaved measurements per form, and enforce at most
10% range overhead with:

```sh
make range-loop-benchmark
```

For machine-readable observations or a different exploratory repeat count:

```sh
python3 scripts/measure_range_loops.py \
  --compiler target/debug/skac --repeats 15 --warmups 3 --json
```

The script validates every exit status, reports median/minimum/maximum wall
time, compilation time, assembly and executable byte counts, and the source
`main` function's mnemonic profile. Generated artifacts remain under ignored
`build/measurements/range-loop/`. Inspect the emitted source functions without
freezing addresses or labels, for example:

```sh
diff -u build/measurements/range-loop/u64_while.s \
  build/measurements/range-loop/u64_range.s
objdump -d build/measurements/range-loop/u64_range
```

Close background work and use a stable performance policy for less noisy
numbers. The threshold concerns median execution only; compilation and whole-
artifact size include the canonical standard-library dependency and are
reported to expose fixed overhead rather than treated as hot-loop cost.

## Reference result

Recorded 2026-08-28 on Linux 5.15 WSL2, an AMD Ryzen 7 7800X3D, rustc 1.97.1,
and Ubuntu GCC 13.3.0. The command used the standard nine repeats and two
warmups with runtime traces omitted:

| Type | Range median | `while` median | Range overhead |
|---|---:|---:|---:|
| `u8` | 143.104 ms | 143.029 ms | 0.052% |
| `u64` | 97.943 ms | 97.866 ms | 0.079% |
| `i64` | 97.799 ms | 97.699 ms | 0.102% |

All three are within the 10% acceptance target. In each emitted source
function, the range and `while` mnemonic profiles were identical except for
one extra unconditional jump on the fused range's cold scalar-cleanup path.
The hot comparison, conditional edge, item handling, two additions (induction
and checksum), and memory traffic matched. Range executables were 80 bytes
larger in this run because their whole programs retain canonical range
dependency artifacts; this fixed size difference does not occur in the hot
loop and had no material median-time effect.
