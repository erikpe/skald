# Low-Level Compiler Foundation Measurements

Status: authoritative for the foundation collection/comparison protocol and
the complete, durably retained pre-migration baseline at `9e3cebb1`.
The equivalent-build timing comparison is **inconclusive**; no architecture
performance result or adoption cost clearance is claimed. The
[completed preparation roadmap](../archive/LOW_LEVEL_PHASE_ARCHITECTURE_ROADMAP.md) records
baseline qualification and durable evidence; the
[frozen phase design](../archive/LOW_LEVEL_PHASE_ARCHITECTURE_DESIGN_PROPOSAL.md#foundation-measurement-and-adoption-policy)
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

## Reviewed pre-migration baseline

The [retained evidence](../../tests/measurements/low_level_compiler/pre_migration_9e3cebb1/README.md)
was captured on 2026-09-17 from committed compiler source
`9e3cebb172db1c5b0f7813ce7c34a87b019c9e9d`. This measurement revision includes
the boundary/behavior witnesses and generated-retain exhaustion-call ABI fix.
It is distinct from program/phase implementation baseline `495debd3`.
Both roles use one preserved, clean `golden` binary built with
`rustc 1.97.1 (8bab26f4f 2026-07-14)`, no extra flags, and the shared runtime
built by `make golden-tools runtime`. This equivalent-build comparison qualifies
collection/replay, not an architectural improvement.

| Artifact | SHA-256 |
| --- | --- |
| Compiler | `a1daa5eb6574963a53685c36bdb84f149db7da7306aa2e08f9324483efa18061` |
| Runtime archive | `4b321ca197cf0d672ea0ffaf60529619725346e1c46612b2c52d490a540969a8` |
| Frozen manifest | `b1e779bd317354f61450b65c874d78b8986727cf2e3fe3be32b1bb32c9634d57` |

The [evidence index](../../tests/measurements/low_level_compiler/pre_migration_9e3cebb1/index.json)
hashes the two gzip-compressed full reports, manifest, build record, comparison
and raw untimed diagnostics/native output. These six records total about
355 KB. The reports retain every raw sample/event, all 45 input hashes,
harness/host/tool identities, assembly/executable hashes, summaries, MIR
observations and per-callable metrics. Compression preserves the original report
bytes. Large repeated assemblies and native executables remain scratch artifacts;
they are not required to verify the retained records.

Both independent full captures used 15 compile and 31 native samples per
role/workload, one warmup each and reversed starting orders. They retain 1,260
measured compile events and 2,108 measured native events in total, plus 152
warmups. The first capture is `pair-azv57129` (16:44 UTC); the second is
`pair-uk5oz741` (16:49 UTC). Both collecting checkout identities were clean
at `9e3cebb1`; documentation/evidence work is separate from that build identity.

The host was an AMD Ryzen 7 7800X3D under Linux WSL2
`5.15.167.4-microsoft-standard-WSL2`, Python 3.12.3, Ubuntu GCC 13.3.0 and
GNU binutils 2.42. All timed processes and descendants were pinned to logical
CPU 14. No intentional builds/tests/other benchmarks ran concurrently.
Governor/turbo controls were unavailable in the guest; Windows background load
and scheduling were uncontrolled. Start/end Linux one-minute load averages were
0.59/1.00 and 1.00/1.02. Exact tools/hashes, notes and command arguments are
retained in the reports and
[build record](../../tests/measurements/low_level_compiler/pre_migration_9e3cebb1/build-record.json).

**Readiness:** complete, reproducible baseline inputs are available for later
comparison. All 21 configurations agree on assembly and static metrics across
both roles/captures; linked executable hashes also agree. Every native execution
matches independent expected status/stdout/stderr, every untimed trace emission
matches quiet assembly, and every required frame/text metric is supported.
There are 1,099 per-configuration callable observations, reproduced in all four
role/capture combinations. No metric, sample-count or semantic evidence is missing.

**Cost qualification:** the unchanged comparator reports **inconclusive** with
no compatibility/validity issues. All RSS/text gates and all 17 native timing
gates are within review limits. Ten compile timing gates are within limits;
eleven are inconclusive: small-source, range u8/u64/i64 while, range i64,
both call-recursion modes, both tight-loop modes, and both allocation modes.
These approximately 2–4 ms compile processes include startup/wrapper costs;
noise can hide a 10% crossing. The frozen policy remains unchanged, with no
cost exception granted. More samples alone do not remove within-sample MAD.
Improve host control or measurement support and explicitly requalify affected
evidence before clearing these future adoption gates. Preserve these noisy runs
when collecting replacements. Native durations of roughly 2–170 ms meet the
noise rule in both pairs, including the shortest workloads; no native-duration
repair was needed. This readiness permits downstream architecture design and
implementation, while cost clearance remains an adoption obligation.

The following compact summary uses the **first capture's baseline role only**;
it does not pool pairs or compiler roles. Full raw ranges/MADs and the second
capture are authoritative in the retained reports; comparison remains per
workload.

| Workload | Compile median ± MAD (ms) | Peak RSS median (KiB) | Native median ± MAD (ms) | Native text (bytes) |
| --- | ---: | ---: | ---: | ---: |
| `compile/small-source` | 1.983 ± 0.045 | 8792 | — | — |
| `compile/many-modules-large-cfg` | 1534.245 ± 14.558 | 74072 | — | — |
| `compile/many-generic-applications` | 5.188 ± 0.095 | 10272 | — | — |
| `compile/nested-ownership` | 845.289 ± 10.972 | 59504 | — | — |
| `native/generic-vector-growth` | 367.870 ± 4.084 | 28868 | 2.188 ± 0.037 | 10535 |
| `native/range-u8-range` | 4.074 ± 0.102 | 9608 | 143.583 ± 0.351 | 1623 |
| `native/range-u8-while` | 2.854 ± 0.145 | 9220 | 143.204 ± 0.360 | 1623 |
| `native/range-u64-range` | 3.589 ± 0.119 | 9500 | 98.299 ± 0.238 | 1383 |
| `native/range-u64-while` | 2.408 ± 0.072 | 9208 | 98.491 ± 0.196 | 1383 |
| `native/range-i64-range` | 4.034 ± 0.157 | 9568 | 98.514 ± 0.366 | 1383 |
| `native/range-i64-while` | 2.670 ± 0.111 | 9152 | 98.726 ± 0.217 | 1383 |
| `native/runtime-trace-call-recursion-enabled` | 2.885 ± 0.105 | 9424 | 112.361 ± 0.296 | 1687 |
| `native/runtime-trace-call-recursion-omitted` | 2.592 ± 0.036 | 9404 | 103.716 ± 0.459 | 1527 |
| `native/runtime-trace-tight-loop-enabled` | 3.490 ± 0.263 | 9280 | 170.210 ± 0.757 | 1911 |
| `native/runtime-trace-tight-loop-omitted` | 3.391 ± 0.115 | 9316 | 170.666 ± 0.944 | 1815 |
| `native/runtime-trace-allocation-enabled` | 2.840 ± 0.145 | 9460 | 82.357 ± 0.680 | 1799 |
| `native/runtime-trace-allocation-omitted` | 2.930 ± 0.053 | 9392 | 83.083 ± 0.451 | 1639 |
| `native/runtime-trace-representative-golden-enabled` | 734.067 ± 7.550 | 32936 | 2.363 ± 0.037 | 148456 |
| `native/runtime-trace-representative-golden-omitted` | 728.863 ± 3.587 | 32744 | 2.338 ± 0.023 | 140760 |
| `native/scalar-calls-enabled` | 450.608 ± 2.683 | 28680 | 33.429 ± 0.439 | 34584 |
| `native/scalar-calls-omitted` | 450.136 ± 3.881 | 28764 | 32.210 ± 0.239 | 30664 |

The largest fixed/peak explicit stack observation is 10,472 bytes in the
large-module configuration's `_EiselPowers` static initializer. All individual
frames, peak reservations and direct operand counts are retained under each
workload's `frames.functions`. Identical static observations across both pairs
are verified independently of noisy time; they do not measure dynamic traffic.

## Verify or rebuild retained evidence

Verify hashes, build/manifest identity, full counts/order, raw event/row agreement,
untimed observation/native output agreement and comparison replay without running
timed processes:

```text
python3 scripts/verify_low_level_baseline.py tests/measurements/low_level_compiler/pre_migration_9e3cebb1
make measurement-support-test
```

The verifier exits zero for valid complete records with supported metrics, even
when a timing comparison is inconclusive. It prints that comparison outcome;
zero is evidence integrity, not cost clearance. Missing/corrupt files, altered
comparison results, missing warmups/metrics and mismatched attestations fail.
The ordinary comparison CLI can replay decompressed reports:

```text
mkdir -p build/measurements/retained-replay
gzip -dc tests/measurements/low_level_compiler/pre_migration_9e3cebb1/first.json.gz > build/measurements/retained-replay/first.json
gzip -dc tests/measurements/low_level_compiler/pre_migration_9e3cebb1/second.json.gz > build/measurements/retained-replay/second.json
python3 scripts/measure_cleanup_baseline.py compare-foundation \
  build/measurements/retained-replay/first.json \
  build/measurements/retained-replay/second.json \
  --output build/measurements/retained-replay/comparison.json
```

That replay exits nonzero because the recorded overall result is inconclusive.
Absolute/relative capture-path metadata may differ; comparison content must agree.

To reconstruct the compiler/runtime, create an isolated checkout at
`9e3cebb172db1c5b0f7813ce7c34a87b019c9e9d`, select recorded Rust 1.97.1
and the recorded C tools/flags, and run `make golden-tools runtime` there.
For example, from the current repository:

```text
git worktree add --detach /tmp/skald-foundation-baseline 9e3cebb172db1c5b0f7813ce7c34a87b019c9e9d
cd /tmp/skald-foundation-baseline
RUSTUP_TOOLCHAIN=1.97.1 make golden-tools runtime
```

Preserve the rebuilt binary/build record and run both recorded collection commands
with that path, explicit revision and shared runtime/stdlib roots. Build path,
host/tool changes or collector changes can change hashes or compatibility;
such a rebuild produces explicitly requalified evidence, not a replacement
claim for the original bytes. Do not edit the original retained samples.
Closing review tightened unsupported stack recipe detection for second-operand
stack/base writes in `xchg`/`xadd` and `loop` control flow. Untimed re-extraction
matched all 4,396 retained role/capture callable observations; the historical
records and harness identity remain unchanged. This guard changes the current
harness fingerprint, so use the recapture rule below for future comparisons.

Future architecture comparisons need independent old/new pairs collected with
one compatible harness/input/host configuration. Recapture the preserved baseline
as well when those compatibility inputs change, and record the reason.
Retain this record through foundation acceptance; later archival must preserve
its evidence and links.
