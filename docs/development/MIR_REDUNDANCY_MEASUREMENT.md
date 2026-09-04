# Local final-MIR redundancy measurement

Status: authoritative for the opt-in repository opportunity census and its
generated report. The frozen terminology, corpus, schema, and recommendation
rules remain defined by the
[measurement contract](../roadmaps/LOCAL_MIR_REDUNDANCY_MEASUREMENT_CONTRACT.md).
The durable corpus-version-one result and recommendation are recorded in the
[measurement report](../roadmaps/LOCAL_MIR_REDUNDANCY_MEASUREMENT_REPORT.md).

The `skald-mir-measure` repository tool measures scalar-spill constant
provenance, redundant primitive casts, and exact same-block primitive common
subexpressions. It invokes the real whole-world compiler driver with the
current `default` final-MIR schedule and omitted runtime traces. A borrowed
pipeline inspector analyzes verified `input`, `pre-reachability`, and `final`
products in memory; it does not parse MIR dumps, register a pass, alter backend
input, or add work to ordinary compilation.

Run the complete reviewed corpus with:

```text
make mir-redundancy-measure
```

The command writes
`build/measurements/local-mir-redundancy.json`. `build/` is ignored, so reports
can be regenerated without checking generated artifacts into source control.
The authoritative version-one manifest is
`tests/measurements/local_mir_redundancy.toml`. Golden-backed entries reference
the complete validated golden plan; benchmark-only entries use contained
repository-relative source paths. Duplicate IDs, duplicate canonical
compilation identities, unknown golden identities, non-default golden builds,
and lexical or canonical path escapes fail before compilation.

For focused iteration, invoke the tool directly:

```text
cargo run --locked -p skald-mir-measure -- \
  --workload benchmark/range-i64

cargo run --locked -p skald-mir-measure -- \
  --format json \
  --workload focused/checked-protocols \
  --output build/measurements/checked-protocols.json
```

Repeat `--workload` to select a partial corpus. Without `--output`, the report
is written to standard output. Explicit output paths must remain below
`build/measurements/`. Human and JSON output are projections of the same typed
report; JSON object fields and identity-bearing arrays have canonical order.

Every report records the corpus identity, compiler revision and dirty state,
fixed target/runtime-trace/profile configuration, exact resolved pass schedule,
canonical compilation and native-run context, assembly size, per-workload and
per-checkpoint counts, callable breakdowns, directed overlap counts, category
breadth, and saturating totals. Native stdin is represented by origin, optional
repository-relative path, byte count, and SHA-256 rather than embedded content.

Pass `--operational` to include compile duration. Operational durations are
nondeterministic context and are excluded by default; they must never be used
for structural determinism or correctness assertions. The tool records native
run inputs for reproducibility but does not execute programs during this
structural census, so native timing and executable size remain absent unless a
later explicitly requested measurement produces them.
