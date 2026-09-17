# Low-Level Compiler Foundation Measurements

Status: authoritative for the foundation collection/comparison protocol. No
qualified pre-migration baseline or architecture performance result has been
captured yet. The
[preparation roadmap](../roadmaps/LOW_LEVEL_PHASE_ARCHITECTURE_ROADMAP.md) owns
baseline qualification and durable evidence; the
[frozen phase design](../roadmaps/LOW_LEVEL_PHASE_ARCHITECTURE_DESIGN_PROPOSAL.md#foundation-measurement-and-adoption-policy)
owns adoption policy. The [generic cleanup harness](CLEANUP_MEASUREMENTS.md)
remains available for exploratory measurements.

## Frozen inputs

The [foundation manifest](../../tests/benchmarks/low_level_compiler/manifest.json)
has format/schema 1 and workload version 1. Its exact bytes are hashed and
copied into each capture. It defines 21 workloads, compiler/native arguments,
input roots, trace modes and independent expected native observations.

| Workloads | Purpose / fixed inputs |
| --- | --- |
| Four `compile/*` families | Small source, many modules/large CFG, many generic applications, nested ownership; unchanged from the cleanup corpus |
| `native/generic-vector-growth` | Existing 4096-element vector growth/copy/pop/clear kernel |
| Six `native/range-*` variants | Existing matched range/while loops for u8, u64 and i64, with tracing omitted |
| Eight `native/runtime-trace-*` variants | Existing recursion, tight-loop, allocation and representative golden, each with tracing enabled/omitted |
| Two `native/scalar-calls-*` variants | Runtime arguments `17 1000000`; scalar arithmetic, call, checked signed division/remainder and join; enabled/omitted tracing |

The scalar kernel prints `-993156\n`, independently computed with integer
floor division and `remainder = dividend - quotient * divisor`. The representative
operator golden uses its checked-in expected stdout file. All other native
workloads exit successfully with empty stdout; every native workload has empty
stderr. Every warmup and measured execution checks exact status/stdout/stderr
hashes against the manifest, so two compilers agreeing on a wrong result cannot
qualify evidence.

Both compiler builds use the optimized, assertion-enabled `golden` profile,
explicit `x86_64-sysv` target and default MIR optimization with no exclusions.
The manifest selects the trace mode for each workload. Minimum optimization and
broader ABI/correctness parity remain regression-suite obligations, documented
in the [migration coverage record](../roadmaps/LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md).
This measurement manifest does not replace those suites.

## Prepare builds and host

Build both compilers and the shared runtime outside timing. Preserve each
binary and its build record: source commit, dirty state, Cargo profile, exact
rustc version, build command and binary hash. Normally use committed source and
the same Rust toolchain for both builds. For example, `make golden-tools runtime`
builds the ordinary golden tools/runtime before the current compiler is copied
to a preserved path. Use an isolated checkout to build a historical compiler;
do not overwrite a binary while a capture is running.

Compiler revisions are explicit caller attestations: the harness resolves the
provided Git commit and hashes the executable, but cannot recover its source
revision or build toolchain from binary bytes. `--build-toolchain` attests the
exact rustc version used for **both** builds, not the currently installed version
for an old binary. Mark dirty builds with `--compiler-dirty`/`--candidate-dirty`.
`--build-flags` records any shared extra Cargo/Rust flags; its empty default
attests that neither build used extra flags. Keep build commands/flags identical
apart from the implementation revision.
The collecting checkout revision/dirty state is recorded separately and is
never substituted for a preserved binary's revision. Retain the build records
with reviewed evidence.

Use the same controlled Linux host and CPU affinity for both paired runs.
Record governor/turbo policy, competing load and relevant conditions in
`--host-notes`. Pin an allowed CPU with `taskset -c CPU` if appropriate. The
harness records CPU model/affinity, governors, OS/kernel, Python version,
resolved C driver/assembler/linker, GNU size/time versions and binary hashes;
start/end load averages remain operational observations. Notes do not enforce
turbo or load control; the operator must keep those conditions stable.

Set `CC`, `SKALD_RUNTIME_ARCHIVE` and `SKALD_STDLIB_ROOT` explicitly if using
nondefault tools or input roots. The collector forces their resolved shared
values into both child environments, avoiding a historical binary's embedded
standard-library/runtime path. Runtime archive/configuration identity and all
selected source roots, expected-output files and standard-library `.ska` bytes
are recorded. Inputs, runtime, manifest, compilers and host/tool identities are
checked again before publishing a successful report.
Measurement-script hashes are also compatibility inputs; changing the collector
requires collecting both pairs with the same updated implementation.

## Collect and compare

From the repository root, replace the capitalized build/provenance values:

```text
python3 scripts/measure_cleanup_baseline.py foundation \
  --compiler build/preserved/skac-baseline --compiler-revision BASELINE_COMMIT \
  --candidate-compiler build/preserved/skac-candidate --candidate-revision CANDIDATE_COMMIT \
  --build-toolchain 'EXACT_RECORDED_RUSTC_VERSION' \
  --host-notes 'RECORDED_CPU_POLICY_AND_LOAD_CONTROL' --order-start 0
```

Run that command again with the same binary/input/host settings and
`--order-start 1`. Each invocation creates a distinct `pair-*` directory under
`build/measurements/low-level-compiler/`. For each workload, baseline/candidate
order alternates in both compile and native rounds. Default counts are one
compile/native warmup, five measured compile samples and nine measured native
samples **per compiler/workload**. Counts may increase, never decrease in a
qualifying capture. The 30-second process-group watchdog is configurable with
`--timeout`; keep the same value for both captures.

The existing Make target also accepts protocol arguments. For a current-build
smoke after substituting its provenance, for example:

```text
make cleanup-baseline CLEANUP_BASELINE_ARGS="foundation --compiler-revision BUILD_COMMIT --build-toolchain 'EXACT_RECORDED_RUSTC_VERSION' --host-notes 'SMOKE_UNQUALIFIED' --smoke --compile-repeats 2 --native-repeats 1 --workload compile/small-source"
```

Use the same variable for full paired options; select a preserved baseline with
`GOLDEN_COMPILER=PATH`. Make prerequisites finish before collection begins.
Leaving `CLEANUP_BASELINE_ARGS` empty preserves the exploratory cleanup mode.

Assembly emission is timed without reporting. All warmup/measured assemblies
must agree within a compiler configuration. Separate untimed trace compilations
must emit the same assembly, and retain pass/analysis observations and raw trace
stderr. Native linking is untimed and uses the compiler's maintained driver.
Build/link time is excluded from the compile-emission/native timing samples.
Old/new assembly is allowed to differ.

Compare the two independent paired reports:

```text
python3 scripts/measure_cleanup_baseline.py compare-foundation \
  build/measurements/low-level-compiler/pair-FIRST/report.json \
  build/measurements/low-level-compiler/pair-SECOND/report.json \
  --output build/measurements/low-level-compiler/comparison.json
```

The comparison recomputes median, MAD and range from raw samples; summary fields
are conveniences, not comparison authority. Compiler revisions/hashes must
match their corresponding role across captures; baseline and candidate may
differ. Manifest/input/runtime/host/toolchain/mode/repetition settings must
match. Quiet/trace artifact equivalence and native observations are mandatory.
Reusing the same capture twice is rejected.

| Outcome | Required action |
| --- | --- |
| `incompatible` | Correct input/build/host/provenance settings and collect compatible independent pairs |
| `invalid` | Repair semantic, reporting or determinism divergence before collecting acceptance evidence |
| `inconclusive` | Supply missing metrics, increase duration/repetitions or improve host control; rerun both pairs |
| `review-required` | Correct the repeatable cost regression or record an explicit architectural tradeoff with workload, evidence, cause and owner before adoption |
| `within-review-limits` | Proceed to architectural review and unconditional correctness/ABI/trace gates; this is not automatic adoption approval |

An increase **above** 10% in compiler/native median time or 15% in compiler
peak-RSS/native text size triggers review only when reproduced in both paired
captures. Timing median differences must also exceed twice the larger MAD in
each capture. A one-pair regression or uncertainty that could hide a threshold
crossing remains inconclusive. Workloads are classified individually; no
aggregate average can erase a regression. Frame growth is recorded separately
without an invented automatic budget. New frame-limit failures require review.
The comparison exits zero only for `within-review-limits`; other outcomes still
write the comparison JSON and exit nonzero.

## Metric definitions and retention

Compiler wall time is assembly-emission process time, including process startup
and GNU time wrapper overhead; peak RSS is GNU time's maximum resident set in
KiB. Native wall time includes process startup. Summaries retain sample count,
median, MAD and min/max; ordered raw events include warmups, compiler role,
iteration and measured values. They distinguish static code observations from
dynamic runtime traffic.

Assembly byte size/hash describes the emitted text file. Native text bytes
sum ELF `.text`/`.text.*` sections using GNU `size -A`, including linked runtime
code but excluding ELF headers, writable data and unrelated sections; total
executable size is not used as a substitute.

Per-callable x86 frame bytes are the fixed local reservation plus the saved
eight-byte frame pointer, excluding return addresses and transient outgoing-call
reservations. Lifecycle helpers may compute addresses before their one fixed
frame, or conditionally skip setup. Helpers without a fixed frame can still
reserve nested `rsp` scratch areas. A separate per-callable `max_stack_bytes`
records peak explicit reservations, including outgoing-call areas, by checking
constant depth through physical branches/joins/loops. Return addresses are
excluded. Unrecognized writes, inconsistent joins, accumulating loops and
unbalanced returns are unsupported. Stack-neutral leaves have a proven zero;
these measurements do not independently verify ABI call alignment.
Static frame accesses count explicit memory operands based directly
on `rbp`/`rsp`, including outgoing stack argument accesses; `lea`, implicit
push/pop traffic and accesses through computed pointer aliases are excluded.
These counts are static direct-operand counts, not execution counts or a full
alias analysis. New save/frame/address recipes need extraction fixtures before
they qualify; unknown shapes/missing sections produce **unsupported** metrics,
and comparison stays inconclusive rather than guessing zero. The target-specific
metric owner is separate from compiler semantic/artifact-retention logic.

Each directory retains manifest, full report with raw samples/events, repeated
assemblies, untimed observation stderr, executable hashes and native output.
Failures after collection starts preserve `failure.json` and available artifacts and cannot
be compared as successful captures. Failed process groups are terminated by the
existing watchdog. Outputs may use an external `--output-root`.

Ignored build directories are scratch storage. Before accepting the baseline,
retain the manifests, build records, two reports and comparison durably through
foundation adoption: small checked-in machine-readable evidence or an explicitly
documented durable store with hashes/access/rebuild instructions. Keep large
executables out of Git. The baseline-qualification step owns that retention.

A quick smoke uses `--smoke --compile-repeats 2 --native-repeats 1 --workload
compile/small-source --workload native/scalar-calls-omitted`, with the same
required provenance flags. Exact subsets without `--smoke` also remain
nonqualifying for full adoption. Neither smoke nor subset captures claim
performance acceptance. Deterministic support tests run with `make
measurement-support-test`; timing runs are outside `make check`.
