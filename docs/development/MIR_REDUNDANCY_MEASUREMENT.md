# Local final-MIR redundancy measurement

Status: authoritative for the opt-in repository opportunity census and its
generated report. The frozen terminology, corpus, schema, and recommendation
rules remain defined by the
[measurement contract](../archive/LOCAL_MIR_REDUNDANCY_MEASUREMENT_CONTRACT.md).
The durable corpus-version-one result and recommendation are recorded in the
[measurement report](../archive/LOCAL_MIR_REDUNDANCY_MEASUREMENT_REPORT.md).

The `skald-mir-measure` repository tool measures scalar-spill constant
provenance, redundant primitive casts, exact same-block primitive common
subexpressions, and dead normalized path-activation protocols. It invokes the
real whole-world compiler driver with the current `default` final-MIR schedule
and omitted runtime traces. The trace occurrence stream records the complete
proof-rich and final-stage schedule. A borrowed pipeline inspector analyzes
verified products in memory; it does not parse MIR dumps, register a pass,
alter backend input, or add work to ordinary compilation.

The stage-aware inspector provides distinct proof-rich and normalized views.
The `input` report slot uses `proof-rich-input`; `pre-reachability` uses the
last final checkpoint before `whole-world-reachability`, currently
the checkpoint after post-proof basic-block merging; and `final` uses the
normalized product-final checkpoint after reachability. Proof-rich
observations call the proof-sealed analyzer entry points, while final
observations call the normalized-seal entry points. The tool never forges
either seal, and the pre-to-final delta therefore measures the final
reachability occurrence again.

The tool keeps corpus resolution, real-driver checkpoint collection, stable
report projection, aggregation, and rendering in separate internal owners.
Compiler-side candidate semantics remain in the read-only
`passes::redundancy` analyzers; the repository tool only projects their stable
observations and does not maintain a second optimization model. Each analyzer
retains at most eight proven and eight blocked examples per observation. These
owned examples identify the callable, block, instruction position, optional
result value, classification, and ordered blocker reasons without retaining a
MIR borrow or requiring a second traversal. Value-centered examples identify
a value and executable position. Storage-centered activation examples identify
the storage and include an executable position when the protocol has one;
declaration-only candidates never receive a fabricated instruction location.
Dense identities are audit aids within one compiler result and are not stable
across unrelated rewrites or compiler revisions.

The dead-path-activation category exists only at normalized checkpoints. Its
proof-rich `input` counts are therefore zero by construction. At normalized
checkpoints it inspects only source-free boolean
`NormalizedPathActivation` declarations. Complete candidates contain only
exact base loads with unused boolean results, exact unauthorized base stores,
and storage lifetime markers. Projected or alias places, authorized stores,
material load results, attachments, checked/proof roles, calls,
ownership/lifecycle operations, I/O, and other executable uses are explicit
barriers. The analysis retains exact owned declaration, instruction, and load-
result snapshots for later revalidation, but remains read-only and is not a
registered optimization pass.

The redundant-cast category uses the compiler's shared integer-cast-chain
analysis. It follows arbitrary-length `u8`/`i64`/`u64` chains within one basic
block even when instructions intervene or intermediate results have other
uses. Its details include the conservative eliminated-cast-step upper bound
and maximum observed chain depth; maximum-valued details retain their maximum
rather than being summed when workload reports are aggregated. Identity-result
chains remain subject to the ordinary-use and same-block forwarding boundary;
non-integer families and cross-block provenance retain explicit conservative
blockers.

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

Every schema-version-two report records the corpus identity, compiler revision
and dirty state, fixed target/runtime-trace/profile configuration, exact
resolved pass schedule, canonical compilation and native-run context,
assembly size, per-workload and per-checkpoint counts, callable breakdowns,
directed overlap counts, category breadth, bounded site examples, and
saturating totals. Native stdin is represented by origin, optional repository-
relative path, byte count, and SHA-256 rather than embedded content.
Saturation is sticky across workload aggregation: overflow in any structure,
candidate, or overlap count marks its checkpoint and the top-level totals even
when later workloads contribute zero to that count.

Schema version two extends the archived version-one contract with the
`dead_path_activations` family, a common
`removable_storages_upper_bound`, and storage-centered examples whose block and
instruction are absent for declaration-only candidates. Existing value-
centered examples retain their block, instruction, and optional value fields.

Pass `--operational` to include compile duration. Operational durations are
nondeterministic context and are excluded by default; they must never be used
for structural determinism or correctness assertions. The tool records native
run inputs for reproducibility but does not execute programs during this
structural census, so native timing and executable size remain absent unless a
later explicitly requested measurement produces them.

## Integer cast-chain activation observation

The integer cast-chain activation review reran the frozen corpus immediately
before and after adding the single default
`integer-cast-chain-canonicalization` occurrence. Both runs used
compiler revision `d3d6630e35df6fcde431d32f8c8b75a00ab68b93`; the before tree
was clean with 15 selected occurrences, while the after tree contained the
reviewed activation changes and 16 selected occurrences. The structural and
cast-census observations were identical:

| Checkpoint | Instructions | Values | Interesting/proven/blocked casts | Eliminated-step bound | Maximum chain depth |
| --- | ---: | ---: | ---: | ---: | ---: |
| input, before and after | 119147 | 46813 | 6 / 5 / 1 | 5 | 2 |
| pre-reachability, before and after | 118563 | 46363 | 0 / 0 / 0 | 0 | 0 |
| final, before and after | 20204 | 6818 | 0 / 0 / 0 | 0 | 0 |

The new default occurrence is therefore a no-op on corpus version one: the
preceding simplification occurrences already remove its initial
constant-shaped candidates. This preserves the archived study's conclusion
and is not evidence for broader primitive-chain work. The dedicated dynamic
source fixture supplies the activation evidence that this corpus cannot: its
default occurrence rewrites arbitrary integer chains, its pass-disabled form
retains a candidate, and its dead-pure-disabled form retains an orphan without
undoing endpoint canonicalization.

## Dead normalized path-activation baseline

The initial cleanup baseline was generated from corpus version one at revision
`32f5210390694e3766b1c5a6ef719f787bf6b228` with the dead-activation analysis
changes present in the working tree. All three manually reviewed final
candidates are complete four-instruction protocols: one load and its unused
boolean result, one store, one `StorageLive`, and one `StorageDead`. The store
source producers remain outside the removal bound.

| Checkpoint | Inspected | Proven | Blocked | Storage bound | Value bound | Instruction bound | Dominant blocker |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| input | 0 | 0 | 0 | 0 | 0 | 0 | none; activations are normalized-only |
| pre-reachability | 804 | 3 | 801 | 3 | 3 | 12 | material load result (801) |
| final | 57 | 3 | 54 | 3 | 3 | 12 | material load result (54) |

This is deliberately a pre-activation baseline. The current default cleanup
occurs before the `pre-reachability` checkpoint, so current reports have no
proven dead activation remaining at either normalized checkpoint.

The activation review reran corpus version one at revision
`ddbcd3d2f838288be262dc4bd33f1d0e92297258` after enabling the default cleanup.
The three former candidates disappear exactly: the protected count is
unchanged, while the inspected count falls by three because removed storage is
no longer present at either later checkpoint.

| Checkpoint | Before inspected / proven / blocked | Current inspected / proven / blocked | Removed storage / value / instruction bounds |
| --- | ---: | ---: | ---: |
| input | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 |
| pre-reachability | 804 / 3 / 801 | 801 / 0 / 801 | 3 / 3 / 12 |
| final | 57 / 3 / 54 | 54 / 0 / 54 | 3 / 3 / 12 |

This is deliberately an entity census rather than a runtime benchmark. It
demonstrates removal of three storage declarations, three unused load-result
values, and twelve protocol instructions without claiming the broader value of
general load, store, or producer elimination.

In the baseline, one final candidate was `proof_protected` in
`focused/local-simplification`, and two were in `main` in
`primitives/cast-matrix`. The same three candidates existed before and after
whole-world reachability; all other reviewed workloads had no proven final
candidate. This baseline records incidence before any cleanup pass exists and
does not revise the archived local-redundancy study.
