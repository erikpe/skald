# Panic Runtime Trace Performance

Status: authoritative for the reproducible Linux x86-64 enabled-versus-omitted
measurement procedure and the evidence used to accept default-on panic runtime
traces. Functional behavior remains owned by the
[language contract](../language/ERRORS.md#panic-runtime-traces), target details
by the [backend contract](../compiler/BACKEND.md#runtime-trace-target-boundary),
and test ownership by [Testing](TESTING.md#runtime-trace-coverage).

## Procedure

Run the complete fixed workload set from the repository root:

```text
make runtime-trace-benchmark
```

The Make target builds `skac` and the runtime, then invokes the
standard-library-only Python script with two warmups and nine timed samples per
policy. The script
compiles each source twice with the same compiler and toolchain: once using the
default enabled policy and once with `--omit-runtime-trace`. Timed enabled and
omitted executions alternate order to limit systematic thermal and scheduler
bias.

For focused investigation after building the prerequisites:

```text
python3 scripts/measure_panic_runtime_trace.py \
  --workload call_recursion --warmups 3 --repeats 21
python3 scripts/measure_panic_runtime_trace.py --json
```

Generated assembly and executables are written beneath the ignored
`build/measurements/panic-runtime-trace/` directory. The procedure is not a
pass/fail timing gate: host load, CPU policy, kernel, linker, and toolchain all
affect wall time and file size. It does fail if compilation or execution fails,
if omitted assembly retains any trace artifact, or if enabled assembly no
longer uses well-formed six-instruction pushes, two-instruction pops, and
two-instruction replacements.

## Workloads and metrics

The three dedicated fixtures live under
[`tests/benchmarks/panic_runtime_trace`](../../tests/benchmarks/panic_runtime_trace/README.md).
One existing whole-program golden source supplies broader compiler output:

| Workload | Intended isolation |
|---|---|
| `call_recursion` | One million depth-16 recursive traversals; emphasizes source activation and call-site replacement. |
| `tight_loop` | Twenty million arithmetic iterations with no call or panic-capable operation in the loop; isolates fixed entry/return cost. |
| `allocation` | Three million shared-object allocations, initializer calls, reads, releases, and deallocations. |
| `representative_golden` | The primitive operator profile plus its reachable standard-library closure; representative assembly/metadata size and short native smoke timing. |

Assembly instruction counts include only instructions in text sections, not
directives or static records. `push/pop/replace` reports trace event counts;
their instruction contribution is respectively six, two, and two. Assembly
bytes include deterministic metadata and strings. Executable bytes are the
linked file size, including the common runtime and linker alignment. Wall time
is the median successful process duration; the sub-millisecond representative
golden timing is dominated by process launch and must not be interpreted as a
precise application-speed result.

## Recorded acceptance observation

The following non-gating observation was recorded on 2026-08-09 from commit
`2b5c4218` plus the runtime-trace rollout changes, using Linux
5.15.167.4 under WSL2 on an AMD Ryzen 7 7800X3D, rustc 1.97.1, GCC 13.3.0,
and Python 3.12.3. Each policy received two warmups and nine alternating timed
samples.

| Workload | Policy | Assembly bytes | Instructions | Push/pop/replace | Trace instructions | Executable bytes | Median ms | Enabled time delta |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| call recursion | enabled | 7,934 | 148 | 2/4/2 | 24 | 17,064 | 117.789 | +6.9% |
| call recursion | omitted | 4,633 | 124 | 0/0/0 | 0 | 16,984 | 110.191 | — |
| tight loop | enabled | 7,681 | 171 | 1/2/1 | 12 | 17,064 | 188.469 | +0.2% |
| tight loop | omitted | 6,044 | 159 | 0/0/0 | 0 | 17,064 | 188.049 | — |
| allocation | enabled | 9,342 | 177 | 2/3/3 | 24 | 17,064 | 86.391 | +0.0% |
| allocation | omitted | 6,122 | 153 | 0/0/0 | 0 | 17,064 | 86.382 | — |
| representative golden | enabled | 2,609,941 | 60,667 | 113/218/727 | 2,568 | 382,296 | 0.551 | +0.6% |
| representative golden | omitted | 2,167,793 | 58,099 | 0/0/0 | 0 | 312,664 | 0.548 | — |

For every workload, the complete instruction-count delta equals exactly the
reported six/two/two trace instruction count. No C maintenance call, hidden
successful-path check, or additional instruction family appears. The pure
tight loop has no per-iteration trace work and measured no material delta.
Allocation-heavy execution measured +0.0%, and the representative golden smoke
run measured +0.6%; both are small enough to treat as noise-sensitive on this
host. Call-heavy recursion measured +6.9%, the expected concentrated cost of
maintaining a frame for every source activation and replacing the caller
location at every recursive call.

Static metadata is the dominant size cost. The representative whole-program
source increased generated assembly by 20.4%, instructions by 4.4%, and the
linked executable by 22.3%. Small programs gained only 0–80 linked bytes after
section and page alignment despite visibly larger textual assembly.

These results support the default-on policy and the documented zero-cost
omission escape hatch. They do not justify a target-private dataflow
optimization: the measured instruction deltas are entirely the reviewed
sequences, failure-only stores are already off successful paths, and removing
a location update without a proven incoming-location fact would weaken
correctness for modest uncertain gain. Future optimization work should begin
with a fresh profile and retain `--omit-runtime-trace` as the exact baseline.
